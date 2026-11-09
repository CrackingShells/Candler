//! The caps of a job: when one is crossed, and how runs in flight are told to stop.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use candler_schemas::execution::Budget;
use candler_schemas::primitives::RunKey;
use tokio::sync::watch;

use crate::jobs::model::JobCause;

/// The sender through which one run in flight is told to stop.
pub type RunStop = watch::Sender<Option<JobCause>>;

/// The runs in flight, each with the sender that stops it. A run is in the table from the moment
/// it starts until its `exec_run` returns; a run that ended is never in it. The table remembers
/// the last cause it was told to stop with, so a run that enters after a stop starts already told.
#[derive(Debug, Default)]
pub struct LiveRuns {
    table: Mutex<Table>,
}

/// The runs in flight and the cause they were last told to stop with.
#[derive(Debug, Default)]
struct Table {
    runs: BTreeMap<RunKey, RunStop>,
    stopped: Option<JobCause>,
}

impl LiveRuns {
    /// A table with no run in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Puts `key` in flight and returns the receiver its run listens on. The receiver already
    /// holds the cause the table was last told to stop with, when it was told one.
    pub fn enter(&self, key: &RunKey) -> watch::Receiver<Option<JobCause>> {
        let mut table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        let (stop, listen) = watch::channel(table.stopped);
        table.runs.insert(key.clone(), stop); // BORROW: the table owns the key
        listen
    }

    /// Takes `key` out of flight, because its run ended and was sealed.
    pub fn leave(&self, key: &RunKey) {
        self.table
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .runs
            .remove(key);
    }

    /// The runs in flight.
    #[must_use]
    pub fn keys(&self) -> Vec<RunKey> {
        self.table
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .runs
            .keys()
            .cloned() // BORROW: the caller owns the keys
            .collect()
    }
}

/// Tells every run in `live` to stop with `cause`, and every run that enters `live` later. A run
/// that has ended is not in `live` and does not hear it.
pub fn stop_live(live: &LiveRuns, cause: JobCause) {
    let mut table = live.table.lock().unwrap_or_else(PoisonError::into_inner);
    table.stopped = Some(cause);
    for stop in table.runs.values() {
        stop.send_replace(Some(cause));
    }
}

/// Which cap of a job was crossed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CapKind {
    /// The runs the job has started reached `budget.max_runs`.
    Runs,
    /// The tokens the job spent reached `budget.max_tokens`.
    Tokens,
    /// The cost of the job reached `budget.max_cost_usd`.
    Cost,
    /// The job's wall clock reached `budget.wall_s`.
    Wall,
}

impl CapKind {
    /// Whether crossing this cap stops runs in flight. Reaching `max_runs` only refuses a new
    /// start; a run in flight is within the cap.
    #[must_use]
    pub const fn stops_live(self) -> bool {
        match self {
            Self::Runs => false,
            Self::Tokens | Self::Cost | Self::Wall => true,
        }
    }
}

/// What a job has used so far.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Totals {
    /// The runs the job has started.
    pub runs_started: u32,
    /// The tokens the job's runs spent.
    pub tokens: u64,
    /// The cost of the job's runs in US dollars.
    pub cost_usd: f64,
    /// The time since the job began running its plans.
    pub elapsed: Duration,
}

impl Totals {
    /// The totals for a meter that holds `micro_usd` millionths of a dollar.
    #[must_use]
    #[expect(
        clippy::as_conversions,
        clippy::cast_precision_loss,
        reason = "a cost in millionths of a dollar is far below 2^53"
    )]
    pub fn of(runs_started: u32, tokens: u64, micro_usd: u64, elapsed: Duration) -> Self {
        Self {
            runs_started,
            tokens,
            cost_usd: micro_usd as f64 / 1_000_000.0, // CAST: u64 → f64, micro-dollars to dollars for the report
            elapsed,
        }
    }
}

/// The first cap of `budget` that `totals` has reached, in the order runs, tokens, cost, wall
/// clock. It reads the job's caps only; the per-run caps are values of a cell and end one run.
#[must_use]
pub fn caps_crossed(budget: &Budget, totals: &Totals) -> Option<CapKind> {
    if budget
        .max_runs
        .is_some_and(|cap| totals.runs_started >= cap)
    {
        return Some(CapKind::Runs);
    }
    if budget.max_tokens.is_some_and(|cap| totals.tokens >= cap) {
        return Some(CapKind::Tokens);
    }
    if budget
        .max_cost_usd
        .is_some_and(|cap| totals.cost_usd >= cap)
    {
        return Some(CapKind::Cost);
    }
    if budget
        .wall_s
        .is_some_and(|cap| totals.elapsed >= Duration::from_secs(u64::from(cap)))
    {
        return Some(CapKind::Wall);
    }
    None
}
