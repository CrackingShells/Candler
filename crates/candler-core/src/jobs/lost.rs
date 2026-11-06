//! Recovery of a job whose supervisor was lost: its processes and containers are ended and the
//! job is sealed.

use futures::future::BoxFuture;

use super::dir::JobDir;
use crate::isolation::container::sweeper::ContainerSweeper;
use crate::supervise::Supervisor;

/// What a recovery did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecoverReport {
    /// How many processes it ended.
    pub processes: u32,
    /// How many containers it destroyed.
    pub containers: u32,
}

/// Why a recovery failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RecoverError {
    /// Recovery is not implemented.
    #[error("recovery is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
}

/// Ends what the lost job left behind.
pub fn recover<'a>(
    sup: &'a Supervisor,
    sweep: &'a dyn ContainerSweeper,
    dir: &'a JobDir,
) -> BoxFuture<'a, Result<RecoverReport, RecoverError>> {
    let _ = (sup, sweep, dir);
    Box::pin(async {
        Err(RecoverError::Unavailable {
            detail: "recover".to_owned(),
        })
    })
}
