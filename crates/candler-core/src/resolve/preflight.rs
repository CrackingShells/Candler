//! Preflight steps, their reports and the resolver that runs them.

use std::path::PathBuf;

use candler_schemas::primitives::{FieldPath, FormatError, NameToken};
use candler_schemas::values::{JsonValue, ValuesError};

/// One check a step ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    /// The check's name.
    pub name: NameToken,
    /// Whether the check passed.
    pub passed: bool,
    /// What was checked and what was found.
    pub explanation: String,
}

/// What one preflight step reports.
#[derive(Debug, Clone, PartialEq)]
pub struct StepReport {
    /// The step that produced the report.
    pub step: NameToken,
    /// The values the step derived, by path.
    pub derived: Vec<(FieldPath, JsonValue)>,
    /// The checks the step ran.
    pub checks: Vec<CheckResult>,
}

/// Why a resolution could not complete.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ResolveError {
    /// A path that has to be a JSON string is not UTF-8.
    #[error("the path {} is not UTF-8", path.display())]
    NonUtf8Path {
        /// The path.
        path: PathBuf,
    },
    /// A resolved value is not a hashable JSON value.
    #[error(transparent)]
    Value(#[from] ValuesError),
    /// A field path built by the resolver is malformed.
    #[error(transparent)]
    Format(#[from] FormatError),
}
