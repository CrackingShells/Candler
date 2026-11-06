//! Container isolation: the docker driver seams, the image and gateway services, the guard that
//! decides what may be destroyed, and the [`Isolation`] a run is planned and torn down through.
//!
//! The traits here are held as trait objects by the runner and the job modules; a test supplies
//! its own through the `testing` module behind the `test-support` feature.

pub mod container;
pub mod docker;
pub mod driver;
pub mod gateway;
pub mod image;
pub mod layout;
pub mod model;
#[cfg(feature = "test-support")]
pub mod testing;

use candler_harness::spec::{Cell, CommandSpec, Workspace};
use futures::future::BoxFuture;

use crate::supervise::spawn::SpawnRequest;

pub use self::model::{ContainerOutcome, ContainerPlan};

/// Why an isolation step failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum IsolationError {
    /// The step is not implemented by this isolation.
    #[error("isolation step is not available: {detail}")]
    Unavailable {
        /// Which step.
        detail: String,
    },
    /// A cell value the plan needs is missing or unusable.
    #[error("the cell cannot be planned: {detail}")]
    Plan {
        /// What is wrong.
        detail: String,
    },
    /// Docker failed.
    #[error("docker failed: {detail}")]
    Docker {
        /// What docker reported; it never holds a secret.
        detail: String,
    },
}

/// How a run's agent is placed in a container and taken out of it.
pub trait Isolation: Send + Sync {
    /// Decides the container of one cell.
    ///
    /// # Errors
    /// [`IsolationError::Plan`] when the cell lacks a value the plan needs.
    fn plan(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<ContainerPlan, IsolationError>;

    /// Wraps the harness's command into the request that starts the container and runs it.
    ///
    /// # Errors
    /// [`IsolationError::Plan`] when the command cannot be wrapped.
    fn run_request(
        &self,
        plan: &ContainerPlan,
        command: CommandSpec,
    ) -> Result<SpawnRequest, IsolationError>;

    /// Inspects the run's container, then destroys it through the guard.
    ///
    /// # Errors
    /// [`IsolationError::Docker`] when docker fails.
    fn collect_and_destroy<'a>(
        &'a self,
        plan: &'a ContainerPlan,
    ) -> BoxFuture<'a, Result<ContainerOutcome, IsolationError>>;
}
