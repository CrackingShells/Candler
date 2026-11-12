//! The seam through which ending a child reaches beyond its process group.
//!
//! A hook is attached to one spawn request with `SpawnRequestBuilder::stop_hook`. The ladder
//! calls [`StopHook::pre_stop`] before it signals the child, and [`StopHook::destroy`] after the
//! process sweep, but only when the ladder ended the child (`Ended::Deadline` or
//! `Ended::Stopped`): on `Ended::Exited` the caller does that work itself, in order. A hook that
//! fails does not stop the ladder; the error is the outcome the step is logged with.
//!
//! A supervisor never holds a hook of its own, so a control call spawned through
//! `Supervisor::run_captured` carries none and nothing recurses.

use std::time::Duration;

use candler_harness::spec::CommandSpec;
use futures::future::BoxFuture;

use candler_schemas::primitives::{Marker, RunKey};

use super::stop::Ended;
use crate::isolation::model::ContainerSweep;

/// What a hook is told about the child being ended.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct StopHookCtx {
    /// The run the child belongs to, if the request named one.
    pub run: Option<RunKey>,
    /// The marker the child carries in `CANDLER_RUN`.
    pub marker: Marker,
    /// How the ladder came to end the child: never `Ended::Exited` for `destroy`.
    pub ended: Ended,
}

impl StopHookCtx {
    /// The context of one child's stop.
    #[must_use]
    pub const fn new(run: Option<RunKey>, marker: Marker, ended: Ended) -> Self {
        Self { run, marker, ended }
    }
}

/// Why a hook failed. The ladder logs it as the step's outcome and goes on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum HookError {
    /// The hook could not do its work.
    #[error("stop hook failed: {detail}")]
    Failed {
        /// What went wrong; it never holds a secret.
        detail: String,
    },
    /// The hook declined to act.
    #[error("stop hook refused: {detail}")]
    Refused {
        /// Which rule refused.
        detail: String,
    },
}

/// What a caller adds to ending a child. Both methods default to doing nothing and succeeding.
pub trait StopHook: Send + Sync {
    /// Runs before the ladder signals the child, for work that must happen while the child's
    /// world still exists, such as asking a container to stop.
    ///
    /// # Errors
    /// [`HookError`] when the work failed; the ladder continues.
    fn pre_stop<'a>(&'a self, ctx: &'a StopHookCtx) -> BoxFuture<'a, Result<(), HookError>> {
        // EXPLICIT: the default hook has nothing to do before the stop
        let _ = ctx;
        Box::pin(async { Ok(()) })
    }

    /// Runs a harness's `Action::Exec` pre-stop `spec` inside the child's isolation, for at most
    /// `grace`, and says what came of it in a short phrase. The default has no isolation to run
    /// it in and refuses.
    ///
    /// # Errors
    /// [`HookError`] when the command could not be run; the ladder continues.
    fn exec<'a>(
        &'a self,
        ctx: &'a StopHookCtx,
        spec: &'a CommandSpec,
        grace: Duration,
    ) -> BoxFuture<'a, Result<String, HookError>> {
        let _ = (ctx, spec, grace);
        Box::pin(async {
            Err(HookError::Refused {
                detail: String::from("no isolation to run the command in"),
            })
        })
    }

    /// Runs after the process sweep, when the ladder ended the child, and says what it destroyed,
    /// what it refused and whether it could not tell that it found everything.
    ///
    /// # Errors
    /// [`HookError`] when the work failed; the ladder continues.
    fn destroy<'a>(
        &'a self,
        ctx: &'a StopHookCtx,
    ) -> BoxFuture<'a, Result<ContainerSweep, HookError>> {
        // EXPLICIT: the default hook destroys nothing
        let _ = ctx;
        Box::pin(async { Ok(ContainerSweep::default()) })
    }
}
