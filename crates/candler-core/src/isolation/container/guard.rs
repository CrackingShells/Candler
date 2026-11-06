//! The container guard: the one place that destroys a container. A container is destroyed only
//! when the job directory records it, or when its labels name this job and the scope allows it.

use std::sync::Arc;

use futures::future::BoxFuture;

use crate::isolation::driver::{DestroyDriver, DockerDriver};
use crate::isolation::model::{ContainerRef, NetworkName, SweepScope};
use crate::jobs::view::JobView;

/// Why the guard refused to destroy a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GuardRefusal {
    /// Nothing records the container and its labels do not name this job.
    Unrecorded,
    /// The container belongs to another job.
    OtherJob,
    /// The container carries no candler labels.
    Unlabelled,
    /// The recorded labels and the container's own labels differ.
    Mismatch,
}

/// Why the guard could not decide or act.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GuardError {
    /// The guard refused.
    #[error("the guard refused: {0:?}")]
    Refused(GuardRefusal),
    /// The guard is not implemented.
    #[error("guard is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
}

/// What a destroy did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destroyed {
    /// Whether a record named the container, as opposed to its labels.
    pub recorded: bool,
}

/// Destroys containers and networks, and only those it may.
pub struct ContainerGuard {
    // TRAIT_OBJECT: the destroy driver is docker, or a recording fake in tests
    destroy: Arc<dyn DestroyDriver>,
    // TRAIT_OBJECT: the read driver is docker, or a recording fake in tests
    docker: Arc<dyn DockerDriver>,
    // TRAIT_OBJECT: the view is the job directory, or a fake in tests
    view: Arc<dyn JobView>,
}

impl ContainerGuard {
    /// A guard over a destroy driver, a read driver and the view of the job's records.
    #[must_use]
    pub fn new(
        destroy: Arc<dyn DestroyDriver>,
        docker: Arc<dyn DockerDriver>,
        view: Arc<dyn JobView>,
    ) -> Self {
        // TRAIT_OBJECT: the drivers and the view are chosen by the job runtime and by tests
        Self {
            destroy,
            docker,
            view,
        }
    }

    /// Destroys `container` when the guard allows it within `scope`.
    pub fn destroy<'a>(
        &'a self,
        container: &'a ContainerRef,
        scope: &'a SweepScope,
    ) -> BoxFuture<'a, Result<Destroyed, GuardError>> {
        let _ = (&self.destroy, &self.docker, &self.view, container, scope);
        Box::pin(async {
            Err(GuardError::Unavailable {
                detail: "ContainerGuard::destroy".to_owned(),
            })
        })
    }

    /// Removes the network `name` when it is this job's.
    pub fn destroy_network<'a>(
        &'a self,
        name: &'a NetworkName,
    ) -> BoxFuture<'a, Result<(), GuardError>> {
        let _ = name;
        Box::pin(async {
            Err(GuardError::Unavailable {
                detail: "ContainerGuard::destroy_network".to_owned(),
            })
        })
    }
}
