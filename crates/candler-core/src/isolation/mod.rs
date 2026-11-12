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
pub(crate) mod refusal;
#[cfg(feature = "test-support")]
pub mod testing;

use std::sync::Arc;

use candler_harness::spec::{Cell, CommandSpec, Observed, Workspace};
use candler_schemas::primitives::{ContainerId, Marker, RunKey};
use futures::future::BoxFuture;

use crate::supervise::spawn::{OutputSpec, SpawnRequest};
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

/// What the run's live reader has seen so far, read when a stop needs it.
// TRAIT_OBJECT: the view is a closure over the run's live reader
pub type ObservedView = Arc<dyn Fn() -> Observed + Send + Sync>;

/// Where the run's standard output and standard error go, and what the run's live reader sees.
#[non_exhaustive]
pub struct RunOutputs {
    /// The standard output of the run's client.
    pub stdout: OutputSpec,
    /// The standard error of the run's client.
    pub stderr: OutputSpec,
    /// What the live reader has seen, for a stop action that needs the observed session id.
    pub observer: Option<ObservedView>,
}

impl std::fmt::Debug for RunOutputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunOutputs")
            .field("stdout", &self.stdout)
            .field("stderr", &self.stderr)
            .field("observer", &self.observer.is_some())
            .finish()
    }
}

impl RunOutputs {
    /// The two outputs of a run, with no live view.
    #[must_use]
    pub const fn new(stdout: OutputSpec, stderr: OutputSpec) -> Self {
        Self {
            stdout,
            stderr,
            observer: None,
        }
    }

    /// The same outputs with the view of what the live reader has seen.
    #[must_use]
    pub fn with_observer(mut self, observer: ObservedView) -> Self {
        self.observer = Some(observer);
        self
    }
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
    /// request carries a fresh stop hook that is the run's own, and writes its output where
    /// `outputs` says.
    ///
    /// # Errors
    /// [`IsolationError::Plan`] when the command cannot be wrapped.
    fn run_request(
        &self,
        plan: &ContainerPlan,
        command: CommandSpec,
        marker: Marker,
        key: RunKey,
        outputs: RunOutputs,
    ) -> Result<IsolatedRequest, IsolationError>;

    /// Inspects the run's container after the child's exit, then destroys it through the guard.
    /// The destroy is scoped to the run `key`, never to the run the container's own label names,
    /// and a container whose job label differs from the plan's is refused and reported.
    ///
    /// # Errors
    /// [`IsolationError::Docker`] when docker fails.
    fn collect_and_destroy<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        workspace: &'a Workspace<'a>,
        exit: &'a Exit,
        key: &'a RunKey,
    ) -> BoxFuture<'a, Result<ContainerOutcome, IsolationError>>;

    /// Runs `command` in a throwaway container over the run's mounts, environment and user and
    /// returns what it printed on standard output. The agent's container may have exited: what
    /// the harness keeps lives in the run's mounts. The container has no network and carries
    /// neither the gateway's address nor its token. An isolation without containers refuses.
    ///
    /// # Errors
    /// [`IsolationError::Plan`] when the command cannot be wrapped over the plan,
    /// [`IsolationError::Docker`] when docker fails, and [`IsolationError::Unavailable`] for an
    /// isolation that runs none.
    fn collect_exec<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        command: &'a CommandSpec,
    ) -> BoxFuture<'a, Result<Vec<u8>, IsolationError>> {
        let _ = (plan, command);
        Box::pin(async {
            Err(IsolationError::Unavailable {
                detail: String::from("collect_exec"),
            })
        })
    }

    /// Records the run's container once its id is known, to the job's `children/`, so that every
    /// destroy path finds the container recorded. It may be called many times; the container is
    /// recorded once, and each call returns the recorded id. An isolation without containers
    /// records nothing.
    ///
    /// It returns `Ok(None)` while the container is not yet known or not yet inspectable.
    ///
    /// # Errors
    /// [`IsolationError::Plan`] when the container's labels or image id are not those of the
    /// plan's job and the run `key`, a missing or malformed label included,
    /// [`IsolationError::Unavailable`] when no recorder is attached, or the isolation cannot
    /// write the record or read the clock.
    fn record_container<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        key: &'a RunKey,
    ) -> BoxFuture<'a, Result<Option<ContainerId>, IsolationError>> {
        let _ = (plan, key);
        Box::pin(async { Ok(None) })
    }
}
