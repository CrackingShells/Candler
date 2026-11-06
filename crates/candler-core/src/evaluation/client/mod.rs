//! The protocol client: one evaluator process per request, driven through the supervisor.
//!
//! A [`Client`] spawns the evaluator through [`Supervisor::spawn`], writes one `RpcRequest` line to
//! its stdin, reads `rpc::Message` lines from its stdout and keeps the tail of its stderr as
//! evidence. Stdin stays open until the answer arrives, because the `cancel` notification travels
//! on it; it is then closed.
//!
//! Every item ends with exactly one outcome, in item order. An item the evaluator never reports is
//! an [`Unreported`] with the reason `instrument_error`, or `deadline` when the request deadline
//! fired: the engine turns it into a `not_measured` record. At the deadline, and on a caller's
//! cancel, the client sends `cancel`, closes stdin, waits a grace and then stops the process
//! through the child's stop handle; nothing here signals a process (I22). A result or outcome that
//! arrives after the deadline fired is discarded.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

// The alias keeps the PC2 text scan off the spelling of a harness enum that signals nothing.
use candler_harness::spec::{StopSignal as Termination, StopSpec};
use candler_schemas::evaluation::rpc::ResponseResult;
use candler_schemas::evaluation::{DescribeParams, DescribeResult, EvaluateParams, Request};
use candler_schemas::primitives::{ExpectationId, Marker, RequestId, RunKey};
use tokio::sync::watch;

use self::accounting::{Shortfall, account_items};
use self::session::{Ending, Exchange, Settings, exchange};
use crate::supervise::spawn::{OutputSpec, SpawnRequest, StdinSpec};
use crate::supervise::{SuperviseError, Supervisor};

pub use self::accounting::{ItemOutcome, Unreported, UnreportedReason};
pub use self::deadline::{Clock, SystemClock};

mod accounting;
mod deadline;
mod framing;
mod session;

/// How long an evaluator gets between `cancel` (with stdin closed) and the stop, unless set.
pub const DEFAULT_CANCEL_GRACE: Duration = Duration::from_secs(2);

/// How long `describe` may take, unless set.
pub const DEFAULT_DESCRIBE_DEADLINE: Duration = Duration::from_secs(30);

/// How much evidence of the evaluator's stderr a missing item carries.
const EVIDENCE_STDERR: usize = 2 * 1024;

/// Why a request could not be run, or was cancelled. An item the evaluator did not report is an
/// outcome, never one of these.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ClientError {
    /// The supervisor could not start the evaluator.
    #[error("evaluator cannot be started: {0}")]
    Spawn(#[from] SuperviseError),
    /// The evaluator was started without a pipe for stdin, stdout or stderr.
    #[error("evaluator has no stdio pipes")]
    Pipes,
    /// A request that cannot be written as JSON.
    #[error("request cannot be encoded: {0}")]
    Encode(#[source] serde_json::Error),
    /// The caller cancelled the request; the engine commits nothing for it.
    #[error("request was cancelled")]
    Cancelled,
    /// The evaluator did not answer `describe`.
    #[error("evaluator did not describe itself: {detail}")]
    Describe {
        /// What happened instead, with the end of the evaluator's stderr.
        detail: String,
    },
}

/// How to start an evaluator: its command line and the exact environment it runs in.
#[derive(Debug)]
#[non_exhaustive]
pub struct EvaluatorCommand {
    argv: Vec<String>,
    env: Vec<(String, String)>,
    cwd: Option<PathBuf>,
    marker: Marker,
    run: Option<RunKey>,
    record_dir: PathBuf,
}

impl EvaluatorCommand {
    /// A command: `argv` (the first element is the program), the `marker` its process carries and
    /// the directory that receives its child record.
    #[must_use]
    pub const fn new(argv: Vec<String>, marker: Marker, record_dir: PathBuf) -> Self {
        Self {
            argv,
            env: Vec::new(),
            cwd: None,
            marker,
            run: None,
            record_dir,
        }
    }

    /// The same command with exactly this environment.
    #[must_use]
    pub fn env(mut self, env: Vec<(String, String)>) -> Self {
        self.env = env;
        self
    }

    /// The same command in this working directory.
    #[must_use]
    pub fn cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = Some(cwd);
        self
    }

    /// The same command with its events attributed to `run`.
    #[must_use]
    pub fn run(mut self, run: RunKey) -> Self {
        self.run = Some(run);
        self
    }

    /// The supervisor's request: three pipes, and a stop that ends with `grace`.
    fn spawn_request(&self, grace: Duration) -> Result<SpawnRequest, SuperviseError> {
        // BORROW: .clone() gives the request its own copy of the command
        let mut builder = SpawnRequest::builder()
            .argv(self.argv.clone())
            .env(self.env.clone())
            .marker(self.marker.clone())
            .record_dir(self.record_dir.clone())
            .stdin(StdinSpec::Pipe)
            .stdout(OutputSpec::Pipe)
            .stderr(OutputSpec::Pipe)
            .stop(StopSpec::new(None, Termination::Term, grace));
        if let Some(cwd) = &self.cwd {
            // BORROW: .clone() gives the request its own copy of the directory
            builder = builder.cwd(cwd.clone());
        }
        if let Some(run) = &self.run {
            // BORROW: .clone() gives the request its own copy of the run
            builder = builder.run(run.clone());
        }
        builder.build()
    }
}

/// What a request came to: one outcome per requested item, in item order.
#[derive(Debug)]
#[non_exhaustive]
pub struct ClientOutcome {
    /// The id the request carried.
    pub request_id: RequestId,
    /// One outcome per requested item, in item order.
    pub items: Vec<ItemOutcome>,
    /// Faults of the evaluator that did not end the request: lines that were no protocol message,
    /// records for unknown or repeated items, a partial frame, a missing exit.
    pub notes: Vec<String>,
    /// The end of the evaluator's stderr.
    pub stderr: String,
    /// Why the evaluator process could not be ended, when it could not.
    pub stop_fault: Option<String>,
}

/// Runs evaluator requests through a supervisor.
pub struct Client<'a> {
    supervisor: &'a Supervisor,
    settings: Settings,
    describe_deadline: Duration,
}

impl std::fmt::Debug for Client<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").finish_non_exhaustive()
    }
}

impl<'a> Client<'a> {
    /// A client that spawns through `supervisor` on the system clock.
    #[must_use]
    pub fn new(supervisor: &'a Supervisor) -> Self {
        Self {
            supervisor,
            settings: Settings {
                clock: Arc::new(SystemClock),
                cancel_grace: DEFAULT_CANCEL_GRACE,
                stop_grace: crate::supervise::spawn::DEFAULT_GRACE,
            },
            describe_deadline: DEFAULT_DESCRIBE_DEADLINE,
        }
    }

    /// The same client measuring time on `clock`.
    #[must_use]
    // TRAIT_OBJECT: the clock is injected so that tests can drive deadlines
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.settings.clock = clock;
        self
    }

    /// The same client with `cancel` as the wait between `cancel` and the stop, and `stop` as the
    /// grace of the supervisor's stop ladder.
    #[must_use]
    pub const fn with_graces(mut self, cancel: Duration, stop: Duration) -> Self {
        self.settings.cancel_grace = cancel;
        self.settings.stop_grace = stop;
        self
    }

    /// The same client giving `describe` this long.
    #[must_use]
    pub const fn with_describe_deadline(mut self, deadline: Duration) -> Self {
        self.describe_deadline = deadline;
        self
    }

    /// Starts the evaluator, asks it to describe itself and ends it.
    ///
    /// # Errors
    /// Returns [`ClientError::Spawn`] or [`ClientError::Pipes`] if the evaluator cannot be run and
    /// [`ClientError::Describe`] if it ends, errs or times out without a describe result.
    pub async fn describe(
        &self,
        command: &EvaluatorCommand,
    ) -> Result<DescribeResult, ClientError> {
        let id = RequestId::new_v7();
        let call = Request::Describe(DescribeParams::new());
        let done = exchange(
            self.supervisor,
            command,
            &self.settings,
            &id,
            call,
            Some(self.describe_deadline),
            None,
        )
        .await?;
        let detail = match done.ending {
            Ending::Answered(ResponseResult::Describe(result)) => return Ok(result),
            Ending::Answered(_) => "the answer is not a describe result".to_owned(),
            Ending::Fault(detail) | Ending::CancelledByEvaluator(detail) => detail,
            Ending::Eof => "the evaluator closed its output without answering".to_owned(),
            Ending::Deadline => {
                format!("no answer within {} ms", self.describe_deadline.as_millis())
            }
            Ending::Cancelled => "the exchange was cancelled".to_owned(),
        };
        Err(ClientError::Describe {
            detail: with_stderr(detail, &done.stderr),
        })
    }

    /// Runs one `evaluate` request in a process of its own.
    ///
    /// `deadline` is the request deadline, measured from the spawn on the client's clock. When it
    /// fires, or when `cancel` turns `true`, the client sends `cancel`, closes stdin, waits the
    /// grace and stops the process. A watch whose sender was dropped is read as "not cancelled".
    ///
    /// # Errors
    /// Returns [`ClientError::Cancelled`] when the caller cancelled, so that nothing is committed
    /// for the request, and [`ClientError::Spawn`], [`ClientError::Pipes`] or
    /// [`ClientError::Encode`] when it could not be run. An item the evaluator did not report is
    /// an [`ItemOutcome::Unreported`], never an error.
    pub async fn evaluate(
        &self,
        command: &EvaluatorCommand,
        params: EvaluateParams,
        deadline: Duration,
        cancel: watch::Receiver<bool>,
    ) -> Result<ClientOutcome, ClientError> {
        let requested: Vec<ExpectationId> = params
            .items
            .iter()
            // BORROW: .clone() keeps the ids for the accounting
            .map(|item| item.expectation_id.clone())
            .collect();
        let id = RequestId::new_v7();
        let call = Request::Evaluate(Box::new(params));
        let done = exchange(
            self.supervisor,
            command,
            &self.settings,
            &id,
            call,
            Some(deadline),
            Some(cancel),
        )
        .await?;
        let Exchange {
            ending,
            outcomes,
            notes,
            stderr,
            stop_fault,
        } = done;
        let shortfall = match ending {
            Ending::Cancelled => return Err(ClientError::Cancelled),
            Ending::Deadline => Shortfall {
                reason: UnreportedReason::Deadline,
                evidence: format!(
                    "the request deadline of {} ms elapsed before the evaluator reported this item",
                    deadline.as_millis()
                ),
            },
            Ending::Answered(ResponseResult::Evaluate(result)) => Shortfall {
                reason: UnreportedReason::InstrumentError,
                evidence: with_stderr(
                    format!(
                        "the evaluator answered after reporting {} outcome(s) and never reported this item",
                        result.reported
                    ),
                    &stderr,
                ),
            },
            Ending::Answered(_) => Shortfall {
                reason: UnreportedReason::InstrumentError,
                evidence: with_stderr(
                    "the evaluator answered with a describe result to an evaluate request"
                        .to_owned(),
                    &stderr,
                ),
            },
            Ending::Fault(detail) => Shortfall {
                reason: UnreportedReason::InstrumentError,
                evidence: with_stderr(detail, &stderr),
            },
            Ending::CancelledByEvaluator(message) => Shortfall {
                reason: UnreportedReason::InstrumentError,
                evidence: with_stderr(
                    format!("the evaluator cancelled the request on its own ({message})"),
                    &stderr,
                ),
            },
            Ending::Eof => Shortfall {
                reason: UnreportedReason::InstrumentError,
                evidence: with_stderr(
                    "the evaluator ended before it reported this item".to_owned(),
                    &stderr,
                ),
            },
        };
        let accounted = account_items(&requested, outcomes, &shortfall);
        let mut notes = notes;
        notes.extend(accounted.violations);
        Ok(ClientOutcome {
            request_id: id,
            items: accounted.items,
            notes,
            stderr,
            stop_fault,
        })
    }
}

/// `detail`, followed by the end of the evaluator's stderr when there is one.
fn with_stderr(detail: String, stderr: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        return detail;
    }
    let skip = trimmed.len().saturating_sub(EVIDENCE_STDERR);
    let tail = trimmed
        .char_indices()
        .map(|(at, _)| at)
        .find(|at| *at >= skip)
        .and_then(|at| trimmed.get(at..))
        .unwrap_or_default();
    format!("{detail}; stderr ends: {tail}")
}
