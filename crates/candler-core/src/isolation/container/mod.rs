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
use candler_schemas::primitives::{Marker, RunKey};
use futures::future::BoxFuture;

use self::guard::ContainerGuard;
use super::driver::DockerDriver;
use super::model::{ContainerOutcome, ContainerPlan, GatewayHandle};
use super::{IsolatedRequest, Isolation, IsolationError};
use crate::supervise::stop::Exit;

/// Isolation through docker containers. It is built once per job, after the gateway starts, and
/// holds no plan and no hook: both belong to one run.
pub struct DockerIsolation {
    gateway: GatewayHandle,
    // TRAIT_OBJECT: the driver is docker, or a recording fake in tests
    docker: Arc<dyn DockerDriver>,
    guard: Arc<ContainerGuard>,
}

impl DockerIsolation {
    /// An isolation that reaches the provider through `gateway`.
    #[must_use]
    pub fn new(
        gateway: GatewayHandle,
        docker: Arc<dyn DockerDriver>,
        guard: Arc<ContainerGuard>,
    ) -> Self {
        // TRAIT_OBJECT: the driver is docker, or a recording fake in tests
        Self {
            gateway,
            docker,
            guard,
        }
    }

    /// The gateway the agent's containers reach the provider through.
    #[must_use]
    pub const fn gateway<'a>(&'a self) -> &'a GatewayHandle {
        &self.gateway
    }

    /// The read driver the isolation inspects through.
    #[must_use]
    pub fn docker(&self) -> Arc<dyn DockerDriver> {
        Arc::clone(&self.docker)
    }

    /// The guard that destroys the run's containers.
    #[must_use]
    pub fn guard(&self) -> Arc<ContainerGuard> {
        Arc::clone(&self.guard)
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
        marker: Marker,
        key: RunKey,
    ) -> Result<IsolatedRequest, IsolationError> {
        let _ = (plan, command, marker, key);
        Err(IsolationError::Unavailable {
            detail: "DockerIsolation::run_request".to_owned(),
        })
    }

    fn collect_and_destroy<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        workspace: &'a Workspace<'a>,
        exit: &'a Exit,
    ) -> BoxFuture<'a, Result<ContainerOutcome, IsolationError>> {
        let _ = (plan, workspace, exit);
        Box::pin(async {
            Err(IsolationError::Unavailable {
                detail: "DockerIsolation::collect_and_destroy".to_owned(),
            })
        })
    }
}
