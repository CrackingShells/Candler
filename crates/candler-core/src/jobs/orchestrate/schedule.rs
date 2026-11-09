//! Running a job's plans: in order, at most `max_parallel` at once, each in a slot of its own.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use candler_schemas::execution::{Budget, EventPayload, JobEnd};
use candler_schemas::primitives::{ReasonCode, RunKey};
use tokio::sync::watch;
use tokio::task::JoinSet;

use super::caps::{LiveRuns, Totals, caps_crossed, stop_live};
use crate::jobs::model::JobCause;
use crate::jobs::runtime::session::note;
use crate::jobs::slots::{Holder, Slot};
use crate::runner::exec::ExecError;
use crate::runner::exec::ends::Retry;
use crate::runner::exec::exec_run;
use crate::runner::plan::RunPlan;
use crate::runner::{JobMeter, RunControl, RunOutcome, Runtime};

/// How often the job's totals are compared with its caps.
const CAP_POLL: Duration = Duration::from_millis(25);

/// The time since the job began running its plans.
// TRAIT_OBJECT: the clock is the wall clock, or a test's own
pub type Clock = Arc<dyn Fn() -> Duration + Send + Sync>;

/// The clock that counts real time from now.
#[must_use]
pub fn wall_clock() -> Clock {
    let begun = Instant::now();
    Arc::new(move || begun.elapsed())
}

/// What running the plans produced.
#[derive(Debug)]
pub struct Schedule {
    /// The outcome of every run that ended, in the order they ended.
    pub runs: Vec<RunOutcome>,
    /// How the job ended.
    pub end: JobEnd,
}

/// What one task of the scheduler returns: the runs it ended and whether it failed.
struct TaskEnd {
    outcomes: Vec<RunOutcome>,
    failed: bool,
}

/// What the tasks of one scheduler have returned so far.
#[derive(Default)]
struct Tally {
    runs: Vec<RunOutcome>,
    failed: bool,
}

impl Tally {
    /// Takes the end of a task; a task that panicked is a failure.
    fn note(&mut self, ended: Result<TaskEnd, tokio::task::JoinError>) {
        match ended {
            Ok(end) => {
                self.failed |= end.failed;
                self.runs.extend(end.outcomes);
            }
            Err(_) => self.failed = true,
        }
    }
}

/// Runs the plans of one job.
pub struct Scheduler {
    rt: Arc<Runtime>,
    budget: Budget,
    meter: Arc<JobMeter>,
    live: Arc<LiveRuns>,
    halt: watch::Sender<Option<JobCause>>,
    started: AtomicU32,
    clock: Clock,
}

/// The code a run's failure is evented under.
const fn reason_of(error: &ExecError) -> &'static str {
    match error {
        ExecError::Isolation(_) => "isolation_failed",
        ExecError::Store(_) => "store_failed",
        ExecError::Driver(_) => "driver_failed",
        ExecError::Manifest(_) => "manifest_failed",
        ExecError::Supervise(_) => "supervise_failed",
    }
}

impl Scheduler {
    /// A scheduler over `rt` that counts the tokens and the cost of its runs in `meter`, and its
    /// time on the wall clock.
    #[must_use]
    pub fn new(rt: Arc<Runtime>, budget: Budget, meter: Arc<JobMeter>) -> Arc<Self> {
        Self::with_clock(rt, budget, meter, wall_clock())
    }

    /// A scheduler as [`Scheduler::new`] makes one, whose elapsed time is read from `clock`.
    #[must_use]
    pub fn with_clock(
        rt: Arc<Runtime>,
        budget: Budget,
        meter: Arc<JobMeter>,
        clock: Clock,
    ) -> Arc<Self> {
        let (halt, _) = watch::channel(None);
        Arc::new(Self {
            rt,
            budget,
            meter,
            live: Arc::new(LiveRuns::new()),
            halt,
            started: AtomicU32::new(0),
            clock,
        })
    }

    /// The runs in flight.
    #[must_use]
    pub fn live_runs(&self) -> Arc<LiveRuns> {
        Arc::clone(&self.live)
    }

    /// What the job has used so far.
    fn totals(&self) -> Totals {
        Totals::of(
            self.started.load(Ordering::Relaxed),
            self.meter.tokens(),
            self.meter.micro_usd(),
            (self.clock)(),
        )
    }

    /// The cause that halted the job, when one did.
    fn halted(&self) -> Option<JobCause> {
        *self.halt.borrow()
    }

    /// Records `cause` as the job's halt, and stops the runs in flight when `stop` is set. A kill
    /// replaces a budget stop; nothing else replaces a cause.
    fn stop(&self, cause: JobCause, stop: bool) {
        self.halt.send_if_modified(|held| match (*held, cause) {
            (None, _) | (Some(JobCause::BudgetStop), JobCause::Killed) => {
                *held = Some(cause);
                true
            }
            (Some(JobCause::Killed | JobCause::BudgetStop), _) => false,
        });
        if stop {
            stop_live(&self.live, cause);
        }
    }

    /// Whether a cap bars a new run; a crossed cap halts the job.
    fn barred(&self) -> bool {
        match caps_crossed(&self.budget, &self.totals()) {
            Some(kind) => {
                self.stop(JobCause::BudgetStop, kind.stops_live());
                true
            }
            None => false,
        }
    }

    /// Compares the totals with the caps until a cap that stops runs in flight is crossed.
    async fn watch_caps(self: Arc<Self>) {
        loop {
            tokio::time::sleep(CAP_POLL).await;
            if let Some(kind) = caps_crossed(&self.budget, &self.totals())
                && kind.stops_live()
            {
                self.stop(JobCause::BudgetStop, true);
                return;
            }
        }
    }

    /// Passes a cause sent on the job's own channel to the runs.
    async fn forward(self: Arc<Self>, mut cancel: watch::Receiver<Option<JobCause>>) {
        loop {
            let cause = *cancel.borrow_and_update();
            if let Some(cause) = cause {
                self.stop(cause, true);
            }
            if cancel.changed().await.is_err() {
                return;
            }
        }
    }

    /// Takes a slot for `key`.
    async fn slot_for(&self, key: &RunKey) -> Option<Slot> {
        let holder = Holder {
            job: self.rt.job.to_string(),
            run: Some(key.to_string()),
        };
        self.rt.slots.acquire(&holder).await.ok()
    }

    /// Runs one plan, and once more as the next attempt when the run asks for it.
    async fn run_plan(self: Arc<Self>, first: RunPlan, slot: Slot) -> TaskEnd {
        let mut outcomes = Vec::new();
        let mut plan = first;
        let mut slot = slot;
        let mut retried = false;
        loop {
            let ctl = RunControl {
                cancel: self.live.enter(&plan.key),
                meter: Arc::clone(&self.meter),
            };
            let result = exec_run(&self.rt, &plan, &slot, &ctl).await;
            let outcome = match result {
                Ok(outcome) => outcome,
                Err(error) => {
                    // the key stays in flight: the run is unsealed until the next plan seals it
                    note(&format!("run {} failed: {error}", plan.key));
                    if let Ok(reason) = reason_of(&error).parse::<ReasonCode>() {
                        self.rt
                            .sink
                            .emit(Some(&plan.key), EventPayload::run_failed(reason));
                    }
                    return TaskEnd {
                        outcomes,
                        failed: true,
                    };
                }
            };
            self.live.leave(&plan.key);
            let again = outcome.retry == Retry::Once && !retried;
            outcomes.push(outcome);
            if !again {
                return TaskEnd {
                    outcomes,
                    failed: false,
                };
            }
            retried = true;
            drop(slot);
            let Some(next) = self.next_plan(&plan) else {
                return TaskEnd {
                    outcomes,
                    failed: true,
                };
            };
            if self.halted().is_some() || self.barred() {
                return TaskEnd {
                    outcomes,
                    failed: false,
                };
            }
            let Some(held) = self.slot_for(&next.key).await else {
                return TaskEnd {
                    outcomes,
                    failed: true,
                };
            };
            if self.halted().is_some() {
                return TaskEnd {
                    outcomes,
                    failed: false,
                };
            }
            self.started.fetch_add(1, Ordering::Relaxed);
            slot = held;
            plan = next;
        }
    }

    /// The plan of the next attempt of `plan`'s replicate: the same cell, stored record and
    /// deviations, with the attempt the store hands out next.
    fn next_plan(&self, plan: &RunPlan) -> Option<RunPlan> {
        let replicate = plan.key.replicate();
        let attempt = self.rt.store.next_attempt(&plan.cell, replicate).ok()?;
        // BORROW: the new key owns its cell
        let key = RunKey::new(plan.cell.clone(), replicate, attempt).ok()?;
        Some(RunPlan {
            key,
            cell: plan.cell.clone(),     // BORROW: the new plan owns its cell
            stored: plan.stored.clone(), // BORROW: the new plan owns the stored cell
            attempt,
            deviations: plan.deviations.clone(), // BORROW: the new plan owns the deviations
        })
    }

    /// Runs `plans` in order and says how the job ended. `cancel` is the job's own channel, on
    /// which a kill arrives.
    pub async fn run(
        self: Arc<Self>,
        plans: Vec<RunPlan>,
        cancel: watch::Receiver<Option<JobCause>>,
    ) -> Schedule {
        let forward = tokio::spawn(Arc::clone(&self).forward(cancel));
        let watcher = tokio::spawn(Arc::clone(&self).watch_caps());
        let parallel = usize::try_from(self.budget.max_parallel)
            .unwrap_or(usize::MAX)
            .max(1);
        let mut halt = self.halt.subscribe();
        let mut set: JoinSet<TaskEnd> = JoinSet::new();
        let mut pending: VecDeque<RunPlan> = plans.into();
        let mut tally = Tally::default();
        // BORROW: the key outlives the pop of its plan
        while let Some(key) = pending.front().map(|plan| plan.key.clone()) {
            while set.len() >= parallel {
                if let Some(ended) = set.join_next().await {
                    tally.note(ended);
                }
            }
            if self.halted().is_some() || self.barred() {
                break;
            }
            let slot = tokio::select! {
                slot = self.slot_for(&key) => slot,
                _ = halt.changed() => None,
            };
            let Some(slot) = slot else {
                if self.halted().is_none() {
                    tally.failed = true;
                }
                break;
            };
            if self.halted().is_some() {
                break;
            }
            let Some(plan) = pending.pop_front() else {
                break;
            };
            self.started.fetch_add(1, Ordering::Relaxed);
            set.spawn(Arc::clone(&self).run_plan(plan, slot));
        }
        while let Some(ended) = set.join_next().await {
            tally.note(ended);
        }
        watcher.abort();
        forward.abort();
        let end = match self.halted() {
            Some(JobCause::Killed) => JobEnd::Killed,
            Some(JobCause::BudgetStop) => JobEnd::BudgetStop,
            None if tally.failed => JobEnd::Failed,
            None => JobEnd::Completed,
        };
        Schedule {
            runs: tally.runs,
            end,
        }
    }
}
