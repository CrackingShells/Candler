//! The sweep of containers by label, for a run, for the current job and for a lost job.

use futures::future::BoxFuture;

use crate::isolation::model::{ContainerSweep, SweepScope};

/// Why a sweep could not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SweepError {
    /// The sweeper is not implemented.
    #[error("sweeper is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
    /// Docker failed while listing.
    #[error("the sweep could not list containers: {detail}")]
    List {
        /// What docker reported.
        detail: String,
    },
}

/// Finds and destroys the containers in a scope.
pub trait ContainerSweeper: Send + Sync {
    /// Destroys what the guard allows in `scope` and says what it did.
    ///
    /// # Errors
    /// [`SweepError::List`] when the containers cannot be listed.
    fn sweep<'a>(
        &'a self,
        scope: &'a SweepScope,
    ) -> BoxFuture<'a, Result<ContainerSweep, SweepError>>;
}

/// The sweeper that finds containers by their labels.
#[derive(Debug, Default)]
pub struct LabelSweeper;
