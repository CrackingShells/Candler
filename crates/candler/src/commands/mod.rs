//! The verbs of the `candler` binary, one file each.

pub mod kill;
pub mod ps;
pub mod run;
pub mod supervise;
pub mod watch;

/// How a verb that did not fail ended; `main` maps it to the exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "Changed is returned by verbs that report differences, such as diff"
)]
pub enum Exit {
    /// The verb did its work: exit 0.
    Success,
    /// The verb found differences: exit 1.
    Changed {
        /// How many differences it found.
        count: usize,
    },
}

/// Why a verb failed.
#[derive(Debug, thiserror::Error)]
#[allow(
    dead_code,
    reason = "the verbs construct Refused and Failed once they are filled in"
)]
#[non_exhaustive]
pub enum CliError {
    /// The verb is not implemented.
    #[error("`{verb}` is not available yet")]
    Unavailable {
        /// The verb.
        verb: &'static str,
    },
    /// The request was refused before any work began.
    #[error("refused: {reason}")]
    Refused {
        /// Why it was refused.
        reason: String,
    },
    /// The verb started and failed.
    #[error("`{verb}` failed: {source}")]
    Failed {
        /// The verb.
        verb: &'static str,
        /// The cause.
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// The async runtime could not start.
    #[error("the runtime could not start: {detail}")]
    Runtime {
        /// What went wrong.
        detail: String,
    },
}
