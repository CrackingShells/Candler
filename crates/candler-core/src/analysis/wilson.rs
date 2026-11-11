//! The Wilson score interval for a proportion.

use super::error::StatsError;
use super::z_two_sided;

/// The largest run count that is exact as an `f64`.
const EXACT_LIMIT: u64 = 1 << 53;

/// Passes out of graded runs: `k` of `n`, with `k` at most `n` and `n` at most 2^53.
///
/// `n = 0` is a valid count (nothing was graded); the methods that need a proportion refuse it
/// with [`StatsError::NoRuns`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Counts {
    k: u64,
    n: u64,
}

impl Counts {
    /// Counts `k` passes in `n` runs.
    ///
    /// # Errors
    /// Returns [`StatsError::CountAboveRuns`] if `k` exceeds `n`, and
    /// [`StatsError::CountTooLarge`] if `n` exceeds 2^53, where a count stops being exact as a
    /// float.
    pub const fn new(k: u64, n: u64) -> Result<Self, StatsError> {
        if n > EXACT_LIMIT {
            return Err(StatsError::CountTooLarge { n });
        }
        if k > n {
            return Err(StatsError::CountAboveRuns { k, n });
        }
        Ok(Self { k, n })
    }

    /// The passes.
    #[must_use]
    pub const fn k(&self) -> u64 {
        self.k
    }

    /// The graded runs.
    #[must_use]
    pub const fn n(&self) -> u64 {
        self.n
    }

    /// The runs as a float, or [`StatsError::NoRuns`] when there are none.
    pub(super) fn runs(&self) -> Result<f64, StatsError> {
        if self.n == 0 {
            return Err(StatsError::NoRuns);
        }
        Ok(to_f64(self.n))
    }

    /// The proportion of passes, or [`StatsError::NoRuns`] when there are no runs.
    pub(super) fn proportion(&self) -> Result<f64, StatsError> {
        Ok(to_f64(self.k) / self.runs()?)
    }
}

/// A count as a float: exact, because [`Counts::new`] refused every count above 2^53.
#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    reason = "CAST: u64 to f64 is exact below 2^53, which Counts::new enforces"
)]
const fn to_f64(count: u64) -> f64 {
    count as f64 // CAST: u64 → f64, exact because the count is at most 2^53
}

/// A confidence interval for a proportion, both limits in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Interval {
    lo: f64,
    hi: f64,
}

impl Interval {
    /// The lower limit.
    #[must_use]
    pub const fn lo(&self) -> f64 {
        self.lo
    }

    /// The upper limit.
    #[must_use]
    pub const fn hi(&self) -> f64 {
        self.hi
    }
}

/// The Wilson score interval of `k` passes in `n` runs at the level [`super::ALPHA`], with no
/// continuity correction and the limits clamped to `[0, 1]`.
///
/// The limits are the roots of `(p̂ − p)² = z² p (1 − p) / n`:
/// `(p̂ + z²/2n ± z √(p̂ (1 − p̂)/n + z²/4n²)) / (1 + z²/n)`.
///
/// # Errors
/// Returns [`StatsError::NoRuns`] if `n` is 0, and [`StatsError::Distribution`] if the normal
/// quantile cannot be computed.
pub fn wilson(counts: Counts) -> Result<Interval, StatsError> {
    let n = counts.runs()?;
    let p = counts.proportion()?;
    let z = z_two_sided()?;
    let z2 = z * z;
    let denominator = 1.0 + z2 / n;
    let centre = (p + z2 / (2.0 * n)) / denominator;
    let half = z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / denominator;
    Ok(Interval {
        lo: (centre - half).clamp(0.0, 1.0),
        hi: (centre + half).clamp(0.0, 1.0),
    })
}
