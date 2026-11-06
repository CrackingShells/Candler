//! The evaluation engine: plans the requests for a selection of runs and records their outcomes.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Duration;

use candler_schemas::primitives::RequestId;

/// What one `evaluate` call grades: which runs of which store, by which plug-ins and suite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluateRequest {
    /// The store root.
    pub store: PathBuf,
    /// The plug-in manifest paths.
    pub plugins: Vec<PathBuf>,
    /// The suite, by id.
    pub suite: String,
    /// A glob over run keys; `None` selects every run.
    pub runs: Option<String>,
    /// How many requests run at once.
    pub concurrency: NonZeroUsize,
    /// The deadline of one request.
    pub request_deadline: Duration,
}

/// What one `evaluate` call did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EvaluateReport {
    /// The id of every request sent.
    pub request_ids: Vec<RequestId>,
    /// The number of outcome records per status.
    pub counts: BTreeMap<String, u64>,
    /// The outcome files written.
    pub outcome_files: Vec<PathBuf>,
    /// The requests that failed, each with why.
    pub failed_requests: Vec<(RequestId, String)>,
}

/// Why an `evaluate` call could not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EvaluateError {
    /// The engine is not built yet.
    #[error("evaluation engine is not built")]
    NotBuilt,
}

/// Grades the runs `request` selects and records their outcomes.
///
/// # Errors
///
/// Returns [`EvaluateError::NotBuilt`] until the engine is built.
#[allow(
    clippy::unused_async,
    reason = "the seam is async; the engine leaf fills it"
)]
pub async fn evaluate(request: EvaluateRequest) -> Result<EvaluateReport, EvaluateError> {
    drop(request);
    Err(EvaluateError::NotBuilt)
}
