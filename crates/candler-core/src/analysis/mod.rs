//! Candler's default statistics: Wilson intervals, Fisher's exact test, Newcombe's difference
//! interval and Holm's adjustment.

use candler_schemas::values::ValueSource;
use statrs::distribution::{ContinuousCDF as _, Normal};

pub mod error;
pub mod fisher;
pub mod holm;
pub mod newcombe;
pub mod tally;
pub mod wilson;

use error::StatsError;

/// The significance level of every default test and the miss rate of every default interval.
pub const ALPHA: f64 = 0.05;

/// Where [`ALPHA`] comes from: the default the core ships.
#[must_use]
pub const fn alpha_source() -> ValueSource {
    ValueSource::CoreDefault
}

/// The two-sided normal quantile for [`ALPHA`].
///
/// # Errors
/// Returns [`StatsError::Distribution`] if the normal distribution library refuses its
/// parameters, which it does not for the standard normal.
pub fn z_two_sided() -> Result<f64, StatsError> {
    let normal = Normal::new(0.0, 1.0).map_err(|e| StatsError::Distribution(e.to_string()))?;
    Ok(normal.inverse_cdf(1.0 - ALPHA / 2.0))
}
