//! The error of the statistics.

use candler_schemas::primitives::RunKey;

/// Why a statistic is not computed.
///
/// None of these is an author's to fix: the inputs are counts the store holds, so the enum has no
/// refusal code.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum StatsError {
    /// There are no runs (`n = 0`), so no proportion exists.
    #[error("no runs: a proportion needs at least one run")]
    NoRuns,
    /// More passes than runs.
    #[error("{k} passes exceed {n} runs")]
    CountAboveRuns {
        /// The passes.
        k: u64,
        /// The runs.
        n: u64,
    },
    /// A count above 2^53, where it stops being exact as a float.
    #[error("{n} runs exceed 2^53, where a count stops being exact as a float")]
    CountTooLarge {
        /// The runs.
        n: u64,
    },
    /// A p-value that is `NaN` or outside `[0, 1]`.
    #[error("the p-value at index {index} is not a probability")]
    NotAProbability {
        /// The position of the value in the input.
        index: usize,
    },
    /// The statistic is declared and not yet computed.
    #[error("the statistic is not computed")]
    NotComputed,
    /// A current record belongs to a run whose validity the caller did not list, so nothing is
    /// guessed.
    #[error("run {run} has current records and no listed validity")]
    RunNotListed {
        /// The run.
        run: RunKey,
    },
    /// A stored value has a shape this version cannot read.
    #[error("unreadable stored value: {0}")]
    Unreadable(String),
    /// The distribution library refused its parameters.
    #[error("distribution error: {0}")]
    Distribution(String),
}
