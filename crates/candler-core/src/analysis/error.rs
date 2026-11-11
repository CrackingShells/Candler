//! The error of the statistics.

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
    /// The distribution library refused its parameters.
    #[error("distribution error: {0}")]
    Distribution(String),
}
