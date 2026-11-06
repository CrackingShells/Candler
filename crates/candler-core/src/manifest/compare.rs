//! Whether two runs may be compared: their comparable records and the verdict per stratum.

use candler_schemas::execution::Comparable;

/// Why a verdict could not be reached.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum VerdictError {
    /// The comparison is not implemented.
    #[error("comparison is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
}

/// Whether two arms of a stratum are comparable.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Verdict {
    /// The arms differ only in what the experiment varies.
    Comparable,
    /// The arms differ elsewhere, at these paths.
    Differs(Vec<String>),
}

/// What the comparison reads of one arm.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ArmView {
    /// The comparable records of the arm's runs.
    pub runs: Vec<Comparable>,
}

/// The paths the comparison ignores: nuisance values that differ between any two runs.
pub const NUISANCE_PATHS: &[&str] = &[];

/// Whether two arms are comparable, run by run.
///
/// # Errors
/// [`VerdictError`] when a record cannot be read.
pub fn stratum_verdict(left: &ArmView, right: &ArmView) -> Result<Verdict, VerdictError> {
    let _ = (left, right);
    Err(VerdictError::Unavailable {
        detail: "stratum_verdict".to_owned(),
    })
}
