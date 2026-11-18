//! The Cochran–Mantel–Haenszel test, the Mantel–Haenszel odds ratio and the Mantel–Fleiss criterion.
//! The CMH test and the Mantel–Haenszel odds ratio against statsmodels' `StratifiedTable`.

use super::blocking::Stratum;
use super::error::StatsError;

/// The CMH test of one comparison over its blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct CmhResult {
    /// The χ² statistic on 1 degree of freedom, without continuity correction.
    pub statistic: Option<f64>,
    /// The p-value of [`Self::statistic`].
    pub p: Option<f64>,
    /// The Mantel–Haenszel odds ratio of passing in arm B over arm A.
    pub or_mh: Option<f64>,
    /// The Robins–Breslow–Greenland interval of [`Self::or_mh`].
    pub or_ci: Option<(f64, f64)>,
    /// The Mantel–Fleiss quantity.
    pub mantel_fleiss: f64,
    /// Whether [`Self::mantel_fleiss`] reaches 5.
    pub mantel_fleiss_holds: bool,
    /// The p-value of the Breslow–Day homogeneity test.
    pub homogeneity_p: Option<f64>,
}

/// Stub.
///
/// # Errors
/// Always.
pub fn cmh(_strata: &[Stratum], _level: f64) -> Result<CmhResult, StatsError> {
    Err(StatsError::NotComputed)
}
