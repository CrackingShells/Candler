//! The budget of an experiment, read from its resolved values.
//!
//! The caps of one run (`budget.per_run.*`) are cell values: they reach the agent as flags and as
//! the container's expiry. Every other `budget.*` path is a cap of the job and never enters a cell.

use std::time::Duration;

use candler_schemas::execution::Budget;

use crate::isolation::layout::EXPIRY_MARGIN;

/// The caps of one run.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct RunCaps {
    /// The wall-clock cap in seconds; the supervisor's deadline and the container's expiry.
    pub wall_s: Option<u32>,
    /// The cap on the turns.
    pub turns: Option<u32>,
    /// The cap on the cost in US dollars.
    pub max_cost_usd: Option<f64>,
}

/// The caps of one run, from the budget's `per_run` table.
#[must_use]
pub const fn to_caps(budget: &Budget) -> RunCaps {
    RunCaps {
        wall_s: Some(budget.per_run.wall_s),
        turns: budget.per_run.turns,
        max_cost_usd: budget.per_run.max_cost_usd,
    }
}

impl RunCaps {
    /// The agent's flags for the caps that are set: `--max-turns <n>` and `--max-budget-usd <x>`.
    /// The wall cap has no flag: the supervisor's deadline and the container's expiry enforce it.
    #[must_use]
    pub fn flags(&self) -> Vec<String> {
        let mut flags = Vec::new();
        if let Some(turns) = self.turns {
            flags.push("--max-turns".to_owned()); // BORROW: owned flag for the argv
            flags.push(turns.to_string());
        }
        if let Some(cost) = self.max_cost_usd {
            flags.push("--max-budget-usd".to_owned()); // BORROW: owned flag for the argv
            flags.push(cost.to_string());
        }
        flags
    }

    /// How long the container lives: the wall cap plus the margin, so the supervisor's deadline,
    /// which is the wall cap, fires first and the container's own timeout matters only for an
    /// orphan.
    #[must_use]
    pub fn expiry(&self) -> Option<Duration> {
        self.wall_s
            .map(|wall| Duration::from_secs(u64::from(wall)).saturating_add(EXPIRY_MARGIN))
    }
}
