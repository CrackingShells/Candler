//! Newcombe's hybrid score interval for a difference of two proportions.

use super::error::StatsError;
use super::wilson::{Counts, wilson};

/// A difference of two proportions with its interval, the limits in `[-1, 1]`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Difference {
    diff: f64,
    lo: f64,
    hi: f64,
}

impl Difference {
    /// The first arm's proportion minus the second's.
    #[must_use]
    pub const fn diff(&self) -> f64 {
        self.diff
    }

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

/// The hybrid score interval of Newcombe (1998), method 10, for `a`'s proportion minus `b`'s, at
/// the level [`super::ALPHA`], with no continuity correction and the limits clamped to `[-1, 1]`.
///
/// With `d = p₁ − p₂` and the Wilson limits `(l₁, u₁)` and `(l₂, u₂)` of the two arms, the limits
/// are `d − √((p₁ − l₁)² + (u₂ − p₂)²)` and `d + √((u₁ − p₁)² + (p₂ − l₂)²)`.
///
/// # Errors
/// Returns [`StatsError::NoRuns`] if either arm has no runs, and [`StatsError::Distribution`] if
/// the normal quantile cannot be computed.
pub fn newcombe_difference(a: Counts, b: Counts) -> Result<Difference, StatsError> {
    let (p1, p2) = (a.proportion()?, b.proportion()?);
    let first = wilson(a)?;
    let second = wilson(b)?;
    let diff = p1 - p2;
    let below = (p1 - first.lo()).hypot(second.hi() - p2);
    let above = (first.hi() - p1).hypot(p2 - second.lo());
    Ok(Difference {
        diff,
        lo: (diff - below).clamp(-1.0, 1.0),
        hi: (diff + above).clamp(-1.0, 1.0),
    })
}
