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

use std::sync::Arc;

use candler_harness::spec::{Cell, CommandSpec, Workspace};
use candler_schemas::primitives::{Marker, RunKey};
use futures::future::BoxFuture;

use crate::supervise::spawn::SpawnRequest;
use crate::supervise::stop::Exit;

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

/// What the isolation knows about a run's end once the child has been waited for.
pub trait EndFacts: Send + Sync {
    /// The final inspect and the destroy result of the run's container, when the stop hook
    /// took them; `None` for a backend that has no container.
    fn final_inspect(&self) -> Option<ContainerOutcome>;
}

/// A spawn request and the handle that reports on its end.
pub struct IsolatedRequest {
    /// The request, with the run's stop hook attached.
    pub request: SpawnRequest,
    // TRAIT_OBJECT: the facts come from the hook the request carries; the caller needs no docker type
    /// What the run's end left behind, read after the wait.
    pub end_facts: Arc<dyn EndFacts>,
}

impl IsolatedRequest {
    /// A request with its end facts.
    #[must_use]
    pub fn new(request: SpawnRequest, end_facts: Arc<dyn EndFacts>) -> Self {
        // TRAIT_OBJECT: the facts come from the hook the request carries
        Self { request, end_facts }
    }
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

    /// Wraps the harness's command into the request that starts the container and runs it. The
    /// request carries a fresh stop hook that is the run's own.
    ///
    /// # Errors
    /// [`IsolationError::Plan`] when the command cannot be wrapped.
    fn run_request(
        &self,
        plan: &ContainerPlan,
        command: CommandSpec,
        marker: Marker,
        key: RunKey,
    ) -> Result<IsolatedRequest, IsolationError>;

    /// Inspects the run's container after the child's exit, then destroys it through the guard.
    ///
    /// # Errors
    /// [`IsolationError::Docker`] when docker fails.
    fn collect_and_destroy<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        workspace: &'a Workspace<'a>,
        exit: &'a Exit,
    ) -> BoxFuture<'a, Result<ContainerOutcome, IsolationError>>;
}
