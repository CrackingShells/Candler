//! The stop hook the container run attaches to its spawn request.
//!
//! Before the ladder signals the docker client, the hook asks the run's agent container to stop
//! through the guard. After the process sweep it takes the container's last inspect, then sweeps
//! the run's containers through the sweeper, which destroys only through the guard. Neither step
//! reaches a container by name or by anything but the run's own labels, and a failure in either
//! is returned for the ladder to log; it never stops the ladder.

use std::sync::{Arc, Mutex, PoisonError};

use candler_schemas::primitives::{JobId, RunKey};
use futures::future::BoxFuture;

use super::guard::{ContainerGuard, GuardError};
use super::sweeper::ContainerSweeper;
use crate::isolation::EndFacts;
use crate::isolation::layout::{LABEL_JOB, LABEL_ROLE, LABEL_RUN};
use crate::isolation::model::{
    ContainerOutcome, ContainerRef, ContainerSweep, LabelSel, SweepScope,
};
use crate::supervise::hook::{HookError, StopHook, StopHookCtx};

/// The [`StopHook`] of a container run: it asks the container to stop before the ladder signals,
/// and destroys it through the guard after. It is built for one request.
pub struct ContainerStopHook {
    guard: Arc<ContainerGuard>,
    // TRAIT_OBJECT: the sweeper is the label sweeper, or a fake in tests
    sweeper: Arc<dyn ContainerSweeper>,
    job: JobId,
    outcome: Mutex<Option<ContainerOutcome>>,
}

impl ContainerStopHook {
    /// A hook of the job `job` that stops and destroys through `guard` and sweeps through
    /// `sweeper`.
    #[must_use]
    pub fn new(guard: Arc<ContainerGuard>, sweeper: Arc<dyn ContainerSweeper>, job: JobId) -> Self {
        // TRAIT_OBJECT: the sweeper is chosen by the runtime and by tests
        Self {
            guard,
            sweeper,
            job,
            outcome: Mutex::new(None),
        }
    }

    /// The guard the hook destroys through.
    #[must_use]
    pub fn guard(&self) -> Arc<ContainerGuard> {
        Arc::clone(&self.guard)
    }

    /// The containers of `key` in this job that docker lists, agents only when `agents` is set.
    async fn of_run(&self, key: &RunKey, agents: bool) -> Result<Vec<ContainerRef>, HookError> {
        let selector = LabelSel {
            key: LABEL_JOB.to_owned(), // BORROW: .to_owned() gives the selector its own text
            value: self.job.to_string(),
        };
        let listed = self
            .guard
            .reader()
            .ps_by_label(&selector)
            .await
            .map_err(|error| HookError::Failed {
                detail: error.to_string(),
            })?;
        let run = key.to_string();
        Ok(listed
            .into_iter()
            .filter(|container| container.labels.get(LABEL_RUN) == Some(run.as_str())) // BORROW: .as_str() compares the label
            .filter(|container| !agents || container.labels.get(LABEL_ROLE) == Some("agent"))
            .collect())
    }
}

/// How a guard error reads to the ladder.
fn hook_error(error: &GuardError) -> HookError {
    match error {
        GuardError::Refused(why) => HookError::Refused {
            detail: why.code().to_owned(), // BORROW: .to_owned() gives the error its own text
        },
        GuardError::Docker { .. } | GuardError::View { .. } => HookError::Failed {
            detail: error.to_string(),
        },
    }
}

impl StopHook for ContainerStopHook {
    fn pre_stop<'a>(&'a self, ctx: &'a StopHookCtx) -> BoxFuture<'a, Result<(), HookError>> {
        Box::pin(async move {
            let Some(key) = &ctx.run else {
                return Ok(());
            };
            let scope = SweepScope::Run(key.clone()); // BORROW: .clone() gives the scope its own key
            let mut first = None;
            for container in self.of_run(key, true).await? {
                if let Err(error) = self.guard.stop(&container, &scope).await {
                    first.get_or_insert(hook_error(&error));
                }
            }
            first.map_or(Ok(()), Err)
        })
    }

    fn destroy<'a>(
        &'a self,
        ctx: &'a StopHookCtx,
    ) -> BoxFuture<'a, Result<ContainerSweep, HookError>> {
        Box::pin(async move {
            let Some(key) = &ctx.run else {
                return Ok(ContainerSweep::default());
            };
            let scope = SweepScope::Run(key.clone()); // BORROW: .clone() gives the scope its own key
            let mut last = None;
            if let Ok(agents) = self.of_run(key, true).await
                && let Some(agent) = agents.first()
            {
                // BORROW: .as_str() hands the id to docker
                last = self.guard.reader().inspect(agent.cid.as_str()).await.ok();
            }
            let sweep = self.sweeper.sweep(&scope).await.unwrap_or(ContainerSweep {
                unknown: true,
                ..ContainerSweep::default()
            });
            *self.outcome.lock().unwrap_or_else(PoisonError::into_inner) = Some(ContainerOutcome {
                inspect: last,
                sweep: sweep.clone(), // BORROW: .clone() keeps the sweep for the ladder
            });
            Ok(sweep)
        })
    }
}

impl EndFacts for ContainerStopHook {
    fn final_inspect(&self) -> Option<ContainerOutcome> {
        self.outcome
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone() // BORROW: .clone() copies the outcome out of the lock
    }
}
