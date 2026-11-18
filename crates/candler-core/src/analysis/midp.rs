//! Fisher's two-sided mid-p for one 2×2 table.

use super::blocking::Stratum;
use super::error::StatsError;

/// The two-sided mid-p of the 2×2 table of one block.
///
/// # Errors
/// Returns [`StatsError::NotComputed`] until the test is computed.
pub fn midp_two_sided(_stratum: &Stratum) -> Result<f64, StatsError> {
    Err(StatsError::NotComputed)
}
