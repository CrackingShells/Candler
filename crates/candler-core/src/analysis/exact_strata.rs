//! The exact conditional test of a 2×2×K table: the convolution of the blocks' hypergeometrics.

use super::blocking::Stratum;
use super::error::StatsError;

/// The two-sided mid-p of the exact conditional test over the blocks.
///
/// # Errors
/// Returns [`StatsError::NotComputed`] until the test is computed.
pub fn exact_conditional(_strata: &[Stratum]) -> Result<f64, StatsError> {
    Err(StatsError::NotComputed)
}
