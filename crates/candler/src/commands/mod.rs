//! The verbs of the `candler` binary, one file each.

pub mod kill;
pub mod ps;
pub mod run;
pub mod supervise;
pub mod watch;

/// Why a verb failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CliError {
    /// The verb is not implemented.
    #[error("`{verb}` is not available yet")]
    Unavailable {
        /// The verb.
        verb: &'static str,
    },
    /// The async runtime could not start.
    #[error("the runtime could not start: {detail}")]
    Runtime {
        /// What went wrong.
        detail: String,
    },
}
