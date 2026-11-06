//! The container half of isolation: mounts, the run command, expiry, the stop hook, the guard and
//! the label sweeper, and the [`DockerIsolation`] that ties them together.

pub mod expiry;
pub mod guard;
pub mod mounts;
pub mod run;
pub mod stop;
pub mod sweeper;

use std::sync::Arc;

use candler_harness::spec::{Cell, CommandSpec, Workspace};
use futures::future::BoxFuture;

use super::gateway::Gateway;
use super::model::{ContainerOutcome, ContainerPlan};
use super::{Isolation, IsolationError};
use crate::supervise::spawn::SpawnRequest;

/// Isolation through docker containers.
pub struct DockerIsolation {
    // TRAIT_OBJECT: the gateway is chosen by the job runtime and by tests
    gateway: Arc<dyn Gateway>,
}

impl DockerIsolation {
    /// An isolation that reaches the provider through `gateway`.
    #[must_use]
    pub fn new(gateway: Arc<dyn Gateway>) -> Self {
        // TRAIT_OBJECT: the gateway is chosen by the job runtime and by tests
        Self { gateway }
    }

    /// The gateway this isolation reaches the provider through.
    #[must_use]
    pub fn gateway(&self) -> Arc<dyn Gateway> {
        Arc::clone(&self.gateway)
    }
}

impl Isolation for DockerIsolation {
    fn plan(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<ContainerPlan, IsolationError> {
        let _ = (cell, workspace);
        Err(IsolationError::Unavailable {
            detail: "DockerIsolation::plan".to_owned(),
        })
    }

    fn run_request(
        &self,
        plan: &ContainerPlan,
        command: CommandSpec,
    ) -> Result<SpawnRequest, IsolationError> {
        let _ = (plan, command);
        Err(IsolationError::Unavailable {
            detail: "DockerIsolation::run_request".to_owned(),
        })
    }

    fn collect_and_destroy<'a>(
        &'a self,
        plan: &'a ContainerPlan,
    ) -> BoxFuture<'a, Result<ContainerOutcome, IsolationError>> {
        let _ = plan;
        Box::pin(async {
            Err(IsolationError::Unavailable {
                detail: "DockerIsolation::collect_and_destroy".to_owned(),
            })
        })
    }
}
