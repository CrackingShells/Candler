//! The stop hook the container run attaches to its spawn request.
//!
//! Before the ladder signals the docker client, the hook asks the run's agent container to stop
//! through the guard. After the process sweep it takes the container's last inspect, then sweeps
//! the run's containers through the sweeper, which destroys only through the guard. Neither step
//! reaches a container by name or by anything but the run's own labels, and a failure in either
//! is returned for the ladder to log; it never stops the ladder.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use candler_harness::spec::{Action, CommandSpec};
use candler_schemas::primitives::{JobId, RunKey};
use futures::future::BoxFuture;

use super::exec::exec_args;
use super::guard::{ContainerGuard, GuardError};
use super::sweeper::ContainerSweeper;
use crate::isolation::layout::{LABEL_JOB, LABEL_ROLE, LABEL_RUN};
use crate::isolation::model::{
    ContainerOutcome, ContainerRef, ContainerSweep, LabelSel, SweepScope,
};
use crate::isolation::{EndFacts, ObservedView};
use crate::supervise::hook::{HookError, StopHook, StopHookCtx};

/// The [`StopHook`] of a container run: it asks the container to stop before the ladder signals,
/// and destroys it through the guard after. It is built for one request.
pub struct ContainerStopHook {
    guard: Arc<ContainerGuard>,
    // TRAIT_OBJECT: the sweeper is the label sweeper, or a fake in tests
    sweeper: Arc<dyn ContainerSweeper>,
    job: JobId,
    observer: Option<ObservedView>,
    outcome: Mutex<Option<ContainerOutcome>>,
    // the one stop of the agent: its verdict is what every later asker is told
    stopped: tokio::sync::OnceCell<Option<String>>,
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
            observer: None,
            outcome: Mutex::new(None),
            stopped: tokio::sync::OnceCell::new(),
        }
    }

    /// The same hook that reads the session ids a stop action needs from `view`, which shows
    /// what the run's live reader has seen.
    #[must_use]
    pub fn with_observer(mut self, view: ObservedView) -> Self {
        self.observer = Some(view);
        self
    }

    /// The guard the hook destroys through.
    #[must_use]
    pub fn guard(&self) -> Arc<ContainerGuard> {
        Arc::clone(&self.guard)
    }

    /// The containers of `key` in this job that docker lists, agents only when `agents` is set.
    async fn of_run(&self, key: &RunKey, agents: bool) -> Result<Vec<ContainerRef>, HookError> {
        let selector = LabelSel {
            key: LABEL_RUN.to_owned(), // BORROW: .to_owned() gives the selector its own text
            value: key.to_string(),
        };
        let listed = self
            .guard
            .reader()
            .ps_by_label(&selector)
            .await
            .map_err(|error| HookError::Failed {
                detail: error.to_string(),
            })?;
        let job = self.job.to_string();
        Ok(listed
            .into_iter()
            .filter(|container| container.labels.get(LABEL_JOB) == Some(job.as_str())) // BORROW: .as_str() compares the label
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

impl ContainerStopHook {
    /// Stops the agent container of `key` through the guard: one `docker stop` with the grace,
    /// which returns once the container has exited. The attached client is not touched.
    ///
    /// # Errors
    /// [`HookError`] when docker cannot list or stop the container, or the guard refuses it.
    pub async fn stop_agent(&self, key: &RunKey) -> Result<(), HookError> {
        // A cancel and the deadline's ladder both ask: the first one stops, the other is told its answer.
        let verdict = self
            .stopped
            .get_or_init(|| async {
                match self.stop_once(key).await {
                    Ok(()) => None,
                    Err(HookError::Failed { detail }) => Some(detail),
                    Err(other) => Some(format!("{other:?}")),
                }
            })
            .await;
        verdict.as_ref().map_or(Ok(()), |detail| {
            Err(HookError::Failed {
                detail: detail.clone(), // BORROW: .clone() gives the error its own text
            })
        })
    }

    async fn stop_once(&self, key: &RunKey) -> Result<(), HookError> {
        let scope = SweepScope::Run(key.clone()); // BORROW: .clone() gives the scope its own key
        let mut first = None;
        for container in self.of_run(key, true).await? {
            if let Err(error) = self.guard.stop(&container, &scope).await {
                first.get_or_insert(hook_error(&error));
            }
        }
        first.map_or(Ok(()), Err)
    }
}

impl StopHook for ContainerStopHook {
    fn pre_stop<'a>(&'a self, ctx: &'a StopHookCtx) -> BoxFuture<'a, Result<(), HookError>> {
        Box::pin(async move {
            match &ctx.run {
                Some(key) => self.stop_agent(key).await,
                None => Ok(()),
            }
        })
    }

    fn exec<'a>(
        &'a self,
        ctx: &'a StopHookCtx,
        spec: &'a CommandSpec,
        grace: Duration,
    ) -> BoxFuture<'a, Result<String, HookError>> {
        Box::pin(async move {
            let Some(key) = &ctx.run else {
                return Err(HookError::Failed {
                    detail: String::from("the stop names no run"),
                });
            };
            // The session id comes only from what the run's live reader observed.
            let observed = self.observer.as_ref().map(|view| view());
            let id = observed.as_ref().and_then(|seen| seen.root_session_id()); // BORROW: lends the id out of the view
            let Some(Action::Exec(command)) = Action::Exec(spec.clone()).resolve(id) else {
                // BORROW: .clone() gives resolve its own spec
                // EXPLICIT: with no id to fill the placeholder the step is skipped, the ladder goes on
                return Ok(String::from("skipped: no session id observed yet"));
            };
            let scope = SweepScope::Run(key.clone()); // BORROW: .clone() gives the scope its own key
            let agents = self.of_run(key, true).await?;
            let Some(agent) = agents.first() else {
                return Err(HookError::Failed {
                    detail: String::from("the run has no agent container"),
                });
            };
            match self.guard.destroyable(agent, &scope).await {
                // EXPLICIT: the container is live, the exec goes on
                Ok(Some(_)) => {}
                Ok(None) => return Ok(String::from("skipped: the container is gone")),
                Err(error) => return Err(hook_error(&error)),
            }
            let args = exec_args(&agent.cid, &command).map_err(|error| HookError::Failed {
                detail: error.to_string(),
            })?;
            match self.guard.reader().capture(args, grace).await {
                Ok(_) => Ok(String::from("exec done")),
                Err(error) => Err(HookError::Failed {
                    detail: error.to_string(),
                }),
            }
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
            // one fresh list of the run: its inspects give the last inspect and the sweep's targets
            let selector = LabelSel {
                key: LABEL_RUN.to_owned(), // BORROW: .to_owned() gives the selector its own text
                value: key.to_string(),
            };
            let job = self.job.to_string();
            let mut last = None;
            let swept = match self.guard.reader().ps_inspect_by_label(&selector).await {
                Ok(inspects) => {
                    let mine: Vec<_> = inspects
                        .into_iter()
                        // BORROW: .as_str() compares the label
                        .filter(|inspect| inspect.labels.get(LABEL_JOB) == Some(job.as_str()))
                        .collect();
                    last = mine
                        .iter()
                        .find(|inspect| inspect.labels.get(LABEL_ROLE) == Some("agent"))
                        .cloned(); // BORROW: .clone() copies the inspect out of the list
                    let listed: Vec<ContainerRef> = mine
                        .into_iter()
                        .map(|inspect| {
                            ContainerRef::new(inspect.id, inspect.labels, inspect.created)
                        })
                        .collect();
                    self.sweeper.sweep_listed(&scope, listed).await
                }
                Err(_) => self.sweeper.sweep(&scope).await,
            };
            let sweep = swept.unwrap_or(ContainerSweep {
                unknown: true,
                ..ContainerSweep::default()
            });
            self.outcome
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get_or_insert(ContainerOutcome {
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
