//! The stop hook the container run attaches to its spawn request.

use std::sync::Arc;

use futures::future::BoxFuture;

use super::guard::ContainerGuard;
use crate::isolation::model::ContainerSweep;
use crate::supervise::hook::{HookError, StopHook, StopHookCtx};

/// The [`StopHook`] of a container run: it asks the container to stop before the ladder signals,
/// and destroys it through the guard after.
pub struct ContainerStopHook {
    guard: Arc<ContainerGuard>,
}

impl ContainerStopHook {
    /// A hook that destroys through `guard`.
    #[must_use]
    pub const fn new(guard: Arc<ContainerGuard>) -> Self {
        Self { guard }
    }

    /// The guard the hook destroys through.
    #[must_use]
    pub fn guard(&self) -> Arc<ContainerGuard> {
        Arc::clone(&self.guard)
    }
}

impl StopHook for ContainerStopHook {
    fn pre_stop<'a>(&'a self, ctx: &'a StopHookCtx) -> BoxFuture<'a, Result<(), HookError>> {
        let _ = ctx;
        Box::pin(async {
            Err(HookError::Refused {
                detail: "ContainerStopHook::pre_stop".to_owned(),
            })
        })
    }

    fn destroy<'a>(
        &'a self,
        ctx: &'a StopHookCtx,
    ) -> BoxFuture<'a, Result<ContainerSweep, HookError>> {
        let _ = ctx;
        Box::pin(async {
            Err(HookError::Refused {
                detail: "ContainerStopHook::destroy".to_owned(),
            })
        })
    }
}
