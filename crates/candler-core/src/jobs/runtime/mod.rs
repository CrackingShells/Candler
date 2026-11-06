//! The detached supervisor of a job: it reads the job's spec, takes the job's lock and runs the
//! body under a supervisor of its own.

pub mod detach;
pub mod reap;
pub mod session;
pub mod signals;
pub mod sweep;

use futures::future::BoxFuture;

use super::model::{JobBody, JobOutcome, JobSpec};

/// Why the supervisor of a job failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RuntimeError {
    /// The supervisor is not implemented.
    #[error("job runtime is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
}

/// Runs `body` as the supervisor of the job `spec` describes.
pub fn supervise_entry<'a>(
    spec: JobSpec,
    body: JobBody,
) -> BoxFuture<'a, Result<JobOutcome, RuntimeError>> {
    let _ = (spec, body);
    Box::pin(async {
        Err(RuntimeError::Unavailable {
            detail: "supervise_entry".to_owned(),
        })
    })
}
