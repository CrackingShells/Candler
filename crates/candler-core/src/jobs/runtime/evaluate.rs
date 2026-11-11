//! The body of an evaluate job: an evaluation run as a supervised job of its own, with no Docker.
//!
//! It takes the same place as a run job in the protocol: the signal handlers, the session, the
//! subreaper role, the job's lock, the event log and the sealed end. The body in the middle is the
//! evaluation engine. Its evaluator processes draw from the machine-wide pool of `slots.evaluators`,
//! its child records go to the job's own `children/` directory so that the recovery of a lost job
//! finds them, and a sweep of the orphans closes the body before `job.ended`.

use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;
use std::time::Duration;

use candler_schemas::execution::{EventPayload, JobEnd, JobState};
use candler_schemas::primitives::JobId;
use candler_schemas::refusal::{RefusalCode, RefusalRecord, Side};
use futures::FutureExt as _;
use futures::future::BoxFuture;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::RuntimeError;
use super::session::SPEC_FILE;
use super::session::{ZombieTick, become_subreaper, detach_session, digest_of, note};
use super::signals::SignalWatch;
use super::sweep::{ProcMarkerSweeper, sweep_orphans};
use crate::evaluation::engine::{
    EvaluateReport, EvaluateRequest, EvaluatorSlots, evaluate_reporting,
};
use crate::evaluation::registry::discover_manifests;
use crate::jobs::dir::{JobDir, JobError, write_atomic};
use crate::jobs::events::JobEvents;
use crate::jobs::lock::JobLock;
use crate::jobs::model::{EvaluateSpec, JobCause, JobControl};
use crate::jobs::slots::{default_evaluator_slots, evaluator_slots_dir};
use crate::resolve::config::{ConfigEnv, UserConfig};
use crate::store::Store;
use crate::supervise::{EventSink, Supervisor};

/// The file of the job directory that holds the report of the evaluation, as the document
/// `EvaluateReport::to_document` makes. It is written before `job.ended`.
pub const REPORT_FILE: &str = "report.json";

/// The file of the job directory that holds the `refusal/1` record of the error a failed job ended
/// with, as the binary would print it. It is written, atomically, before `job.ended`, so that a
/// command that waits for the job can answer with the same code, side and path.
pub const REFUSAL_FILE: &str = "refusal.json";

/// Everything the supervisor of one evaluate job starts from. The spec's digest was checked against
/// `job.json` before this is built.
pub struct EvaluateEntry {
    /// The store the job's directory is in, and whose runs the job grades.
    pub store: Store,
    /// The job.
    pub job: JobId,
    /// The job's spec, read from the supervisor's stdin.
    pub spec: EvaluateSpec,
}

/// How an evaluate job ended and what its evaluation did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluateJobOutcome {
    /// How the job ended.
    pub end: JobEnd,
    /// What the evaluation did; empty when the engine could not run.
    pub report: EvaluateReport,
}

/// Writes the record of `error` to [`REFUSAL_FILE`]: the code, side and path the walk of causes
/// finds in it, or the fallback code when it finds none. A write that fails is noted and the job
/// goes on: the command that waits then falls back to the text of the log.
fn write_refusal(dir: &JobDir, error: &(dyn std::error::Error + 'static)) {
    let message = error.to_string();
    let record = match crate::refusal::find(error) {
        Some(found) => {
            let record = RefusalRecord::new(found.code(), found.side(), message);
            match found.path() {
                Some(pointer) => record.clone().at_path(pointer).unwrap_or(record),
                None => record,
            }
        }
        None => RefusalRecord::new(
            RefusalCode::from_checked("candler.internal"),
            Side::Candler,
            message,
        ),
    };
    let mut causes = Vec::new();
    let mut source = error.source();
    while let Some(cause) = source {
        causes.push(cause.to_string());
        source = cause.source();
    }
    let record = record.with_causes(causes);
    let written = serde_json::to_vec(&record)
        .map_err(|e| e.to_string())
        .and_then(|bytes| {
            write_atomic(&dir.path().join(REFUSAL_FILE), &bytes).map_err(|e| e.to_string())
        });
    if let Err(reason) = written {
        note(&format!("the refusal cannot be written: {reason}"));
    }
}

fn session_error(detail: &impl ToString) -> RuntimeError {
    RuntimeError::Session {
        detail: detail.to_string(),
    }
}

fn job_error(error: &JobError) -> RuntimeError {
    RuntimeError::Job {
        detail: error.to_string(),
    }
}

fn setup_error(detail: &impl ToString) -> RuntimeError {
    RuntimeError::Setup {
        detail: detail.to_string(),
    }
}

/// The task that carries the job's cancel into the engine's cancel watch; dropping it ends the
/// task.
#[derive(Debug)]
pub struct CancelBridge(JoinHandle<()>);

impl Drop for CancelBridge {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Turns the job's cancel (a cause set by a signal or a cap) into the `watch::Receiver<bool>` the
/// engine reads: the receiver turns `true` once the job is told to stop, and a cause already set
/// when this is called turns it at once. The bridge ends with the value.
#[must_use]
pub fn cancel_bridge(control: &JobControl) -> (watch::Receiver<bool>, CancelBridge) {
    let (sender, receiver) = watch::channel(false);
    let mut cause = control.cancel.subscribe();
    let task = tokio::spawn(async move {
        loop {
            if cause.borrow_and_update().is_some() {
                sender.send_replace(true);
                return;
            }
            if cause.changed().await.is_err() {
                return;
            }
        }
    });
    (receiver, CancelBridge(task))
}

/// Runs the evaluation of the job `entry` names as its supervisor, in the order that keeps a
/// signal from being lost and a job from ending without its event: the handlers, the session, the
/// subreaper, the lock, `job.started`, the evaluation, the orphan sweep, `job.ended`, the state, and
/// the lock last. A job that was told to stop ends `killed`, whatever the evaluation returns. A body
/// that panicked ends without `job.ended` and the ended state, so that the job reads as lost.
///
/// # Errors
/// [`RuntimeError::Session`] when the handlers, the session or the subreaper role cannot be had,
/// [`RuntimeError::Job`] when the job's directory, log or state fails,
/// [`RuntimeError::AlreadySupervised`] when another supervisor holds the lock, and
/// [`RuntimeError::Setup`] when the job cannot be set up: the configuration, no evaluator slot, a
/// suite that is not the one the spec records, a plug-in directory without one manifest. The job
/// still ends `failed` in its log in each of these cases.
pub fn supervise_evaluate<'a>(
    entry: EvaluateEntry,
) -> BoxFuture<'a, Result<EvaluateJobOutcome, RuntimeError>> {
    Box::pin(run(entry))
}

/// The order of [`supervise_evaluate`]. It mirrors `session::run` and does not call it, because
/// that function takes the run job's entry and services.
async fn run(entry: EvaluateEntry) -> Result<EvaluateJobOutcome, RuntimeError> {
    let (cancel, _kept) = watch::channel(None);
    let control = JobControl {
        cancel: Arc::new(cancel),
    };
    let _watch = SignalWatch::install(control.clone()).map_err(|e| session_error(&e))?;
    detach_session()?;
    become_subreaper()?;
    let dir = JobDir::open(entry.store.layout(), &entry.job).map_err(|e| job_error(&e))?;
    let kept = serde_json::to_vec(&entry.spec).map_err(|e| setup_error(&e))?;
    write_atomic(&dir.path().join(SPEC_FILE), &kept).map_err(|e| job_error(&e))?;
    let lock = match JobLock::acquire(&dir) {
        Ok(lock) => lock,
        Err(JobError::Held) => return Err(RuntimeError::AlreadySupervised),
        Err(error) => return Err(job_error(&error)),
    };
    let events = Arc::new(JobEvents::open(&dir).map_err(|e| job_error(&e))?);
    // TRAIT_OBJECT: the supervisor and the engine take their sink as a trait object
    let sink: Arc<dyn EventSink> = events;
    sink.emit(None, EventPayload::job_started());
    let seen = control.clone();
    let started = dir.set_state(JobState::Running).map_err(|e| job_error(&e));
    let result = match started {
        Ok(()) => std::panic::AssertUnwindSafe(serve(entry, &dir, &sink, &control))
            .catch_unwind()
            .await
            .unwrap_or_else(|_| {
                Err(RuntimeError::Body {
                    // BORROW: the error owns its detail
                    detail: "the supervisor panicked".to_owned(),
                })
            }),
        Err(error) => Err(error),
    };
    if let Err(error @ RuntimeError::Body { .. }) = &result {
        // A body that panicked may have skipped its teardown: the job ends without `job.ended`
        // and without the ended state, so that it reads as lost and is recovered from `children/`.
        note(&error.to_string());
        drop(lock);
        return result;
    }
    // The cause the handler set decides how a stopped job ends, whatever the body returns.
    let told_to_die = matches!(*seen.cancel.borrow(), Some(JobCause::Killed));
    if let Err(error) = &result {
        note(&error.to_string());
        write_refusal(&dir, error);
    }
    let result = result.map(|outcome| {
        if told_to_die {
            EvaluateJobOutcome {
                end: JobEnd::Killed,
                ..outcome
            }
        } else {
            outcome
        }
    });
    let end = match &result {
        _ if told_to_die => JobEnd::Killed,
        Ok(outcome) => outcome.end,
        Err(_) => JobEnd::Failed,
    };
    sink.emit(None, EventPayload::job_ended(end));
    let sealed = dir.set_state(JobState::Ended).map_err(|e| job_error(&e));
    drop(lock);
    match (result, sealed) {
        (Ok(outcome), Ok(())) => Ok(outcome),
        (Ok(_), Err(error)) | (Err(error), _) => Err(error),
    }
}

/// Everything between `job.started` and `job.ended`: set up the pool, the supervisor and the
/// request, run the evaluation, write its report and sweep the orphans it left.
async fn serve(
    entry: EvaluateEntry,
    dir: &JobDir,
    // TRAIT_OBJECT: the sink is the job's event log
    sink: &Arc<dyn EventSink>,
    control: &JobControl,
) -> Result<EvaluateJobOutcome, RuntimeError> {
    let EvaluateEntry { store, job, spec } = entry;
    let env = ConfigEnv::from_process();
    let user = UserConfig::load(&env).map_err(|e| setup_error(&e))?;
    let state = env
        .state_home()
        .ok_or_else(|| setup_error(&"no state directory for the evaluator slots"))?;
    let cpus = std::thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let count = user
        .slots_evaluators
        .unwrap_or_else(|| default_evaluator_slots(cpus));
    let count = NonZeroU32::new(count)
        .ok_or_else(|| setup_error(&"no evaluator slot to run in: slots.evaluators is 0"))?;

    let suite_bytes = tokio::fs::read(&spec.suite).await.map_err(|e| {
        setup_error(&format!(
            "cannot read the suite {}: {e}",
            spec.suite.display()
        ))
    })?;
    let digest = digest_of(&suite_bytes)?;
    if digest != spec.suite_digest {
        return Err(RuntimeError::SuiteChanged {
            suite: spec.suite.display().to_string(),
            found: digest.to_string(),
            recorded: spec.suite_digest.to_string(),
        });
    }
    let plugins = discover_manifests(&spec.plugin_dirs).map_err(|e| setup_error(&e))?;

    let first = Supervisor::new(Arc::clone(sink));
    let sweeper = ProcMarkerSweeper::new(first.guard()).with_events(Arc::clone(sink));
    let sup = Arc::new(first.with_sweeper(Arc::new(sweeper)));
    let _tick = ZombieTick::start(&sup);

    let request = EvaluateRequest {
        store: store.layout().root().to_path_buf(),
        plugins,
        suite: spec.suite,
        selection: spec.selection,
        slots: EvaluatorSlots {
            dir: evaluator_slots_dir(&state),
            count,
            job: job.to_string(),
        },
        env_root: spec.env_root,
        request_deadline: spec.request_deadline_secs.map(Duration::from_secs),
        record_dir: Some(dir.children_dir()),
    };
    let (cancel, _bridge) = cancel_bridge(control);
    let ran = evaluate_reporting(request, Arc::clone(&sup), cancel, Arc::clone(sink)).await;

    // The net under the marker sweep that every request's stop already ran: whatever was
    // reparented to this subreaper and no group of the supervisor holds.
    let swept = {
        let (sup, sink) = (Arc::clone(&sup), Arc::clone(sink));
        tokio::task::spawn_blocking(move || sweep_orphans(&sup, sink.as_ref())).await
    };
    if let Err(error) = swept {
        note(&format!("the orphan sweep failed: {error}"));
    }

    match ran {
        Ok(report) => {
            let mut bytes =
                serde_json::to_vec(&report.to_document()).map_err(|e| RuntimeError::Job {
                    detail: format!("the report cannot be written: {e}"),
                })?;
            bytes.push(b'\n');
            write_atomic(&dir.path().join(REPORT_FILE), &bytes).map_err(|e| job_error(&e))?;
            Ok(EvaluateJobOutcome {
                end: JobEnd::Completed,
                report,
            })
        }
        Err(error) => {
            note(&format!("the evaluation failed: {error}"));
            write_refusal(dir, &error);
            Ok(EvaluateJobOutcome {
                end: JobEnd::Failed,
                report: EvaluateReport::default(),
            })
        }
    }
}
