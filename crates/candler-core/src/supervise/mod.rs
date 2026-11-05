//! Process supervision: spawning children in their own group with a child record, stopping them by escalation, and reaping a group from its record.
//!
//! A [`Supervisor`] spawns each child in a process group of its own, with exactly the environment
//! the request names and the marker in `CANDLER_RUN`, and writes the child's record to disk before
//! [`Supervisor::spawn`] returns. The supervisor reports what happens to a child as events to an
//! [`EventSink`], which owns the envelope's position, job and time.
//!
//! There is no `unsafe` code here, so a child does not die with its supervisor: no parent-death
//! signal is armed. A lost supervisor leaves the children to be found from their records.

use std::io;
use std::sync::Arc;

use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{RunKey, Timestamp};

#[cfg(feature = "test-support")]
use self::signal::Signaller;
use self::signal::{Guard, OsSignaller};
use self::stop::StraySweeper;

pub mod http;
pub mod output;
pub mod platform;
pub mod reap;
pub mod record;
pub mod spawn;
pub mod stop;

/// Signal delivery and the guard that decides what may be signalled. It is public only with the
/// `test-support` feature, which tests turn on.
#[cfg(feature = "test-support")]
pub mod signal;
#[cfg(not(feature = "test-support"))]
pub(crate) mod signal;

/// Why a spawn, a wait or a stop failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SuperviseError {
    /// A spawn request that was asked to build before a required field was set.
    #[error("spawn request field is missing (`{field}` was not set)")]
    MissingField {
        /// The name of the field.
        field: &'static str,
    },
    /// A spawn request that cannot be carried out; the detail never holds an environment value.
    #[error("spawn request is invalid: {detail}")]
    InvalidRequest {
        /// What is wrong with it.
        detail: String,
    },
    /// The operating system refused to start the program.
    #[error("cannot start {program}: {source}")]
    Spawn {
        /// The program, as the request names it.
        program: String,
        /// The operating-system error.
        #[source]
        source: io::Error,
    },
    /// A file or process operation failed.
    #[error("{context}: {source}")]
    Io {
        /// What was being done.
        context: &'static str,
        /// The operating-system error.
        #[source]
        source: io::Error,
    },
    /// A `/proc/<pid>/stat` line that does not hold the start time.
    #[error("process stat line is malformed: {detail}")]
    ProcStat {
        /// What is wrong with it.
        detail: String,
    },
    /// An event or a record that the schemas refuse.
    #[error("event is invalid: {detail}")]
    Event {
        /// What the schema reported.
        detail: String,
    },
    /// A child that is still running after `SIGKILL`.
    #[error("process {pid} did not exit after SIGKILL")]
    Survived {
        /// The process id of the child.
        pid: u32,
    },
    /// A time that cannot be read or written.
    #[error("clock is unreadable: {detail}")]
    Clock {
        /// What went wrong.
        detail: String,
    },
}

/// Where a supervisor sends its events. The sink owns the envelope's `seq`, `job_id` and `at`.
pub trait EventSink: Send + Sync {
    /// Takes an event about the run `run`, or about no run.
    fn emit(&self, run: Option<&RunKey>, payload: EventPayload);
}

/// Spawns children, stops them and reaps them, reporting to an [`EventSink`].
pub struct Supervisor {
    // TRAIT_OBJECT: the sink is chosen by the job runner and by tests
    pub(crate) sink: Arc<dyn EventSink>,
    // TRAIT_OBJECT: the sweeper is injected by the execution milestone
    pub(crate) sweeper: Option<Arc<dyn StraySweeper>>,
    /// The table of children and the only path to a signal.
    pub(crate) guard: Arc<Guard>,
}

impl Supervisor {
    /// A supervisor that reports to `sink`.
    #[must_use]
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        // TRAIT_OBJECT: the sink is chosen by the job runner and by tests
        Self {
            sink,
            sweeper: None,
            guard: Arc::new(Guard::new(Arc::new(OsSignaller))),
        }
    }

    /// Kills every group this supervisor still holds, each through the guard. It is for tests
    /// that must leave no process behind when an assertion fails.
    #[cfg(feature = "test-support")]
    pub fn kill_all(&self) {
        self.guard.signal_all(rustix::process::Signal::KILL);
    }

    /// Whether this supervisor's table still holds the group led by `pgid`.
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn holds_group(&self, pgid: u32) -> bool {
        self.guard.holds(pgid)
    }

    /// The same supervisor signalling through `signaller` instead of the operating system, with
    /// an empty table of children: call it before the first spawn. Tests use it to record the
    /// signals the guard lets through.
    #[cfg(feature = "test-support")]
    #[must_use]
    // TRAIT_OBJECT: tests replace the signaller with a recording fake
    pub fn with_signaller(mut self, signaller: Arc<dyn Signaller>) -> Self {
        self.guard = Arc::new(Guard::new(signaller));
        self
    }

    /// The same supervisor with `sweeper` called after every group kill to find and kill the
    /// processes of the run that left the group.
    #[must_use]
    // TRAIT_OBJECT: the sweeper is injected by the execution milestone
    pub fn with_sweeper(mut self, sweeper: Arc<dyn StraySweeper>) -> Self {
        self.sweeper = Some(sweeper);
        self
    }
}

impl std::fmt::Debug for Supervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Supervisor").finish_non_exhaustive()
    }
}

/// The current time as a wire timestamp.
pub(crate) fn now() -> Result<Timestamp, SuperviseError> {
    jiff::Timestamp::now()
        .to_string()
        .parse::<Timestamp>()
        .map_err(|e| SuperviseError::Clock {
            detail: e.to_string(),
        })
}
