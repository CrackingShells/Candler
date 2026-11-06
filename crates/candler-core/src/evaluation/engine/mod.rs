//! The evaluation engine: plans the requests for a selection of runs and records their outcomes.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use candler_schemas::primitives::RequestId;
use tokio::sync::watch;

use crate::supervise::Supervisor;

/// What one `evaluate` call grades: which runs of which store, by which plug-ins and suite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluateRequest {
    /// The store root.
    pub store: PathBuf,
    /// The plug-in manifest paths.
    pub plugins: Vec<PathBuf>,
    /// The suite file.
    pub suite: PathBuf,
    /// A glob over run keys; `None` selects every run.
    pub runs: Option<String>,
    /// How many requests run at once.
    pub concurrency: NonZeroUsize,
    /// The root under which the plug-in environments are built, outside the store.
    pub env_root: PathBuf,
    /// A flat deadline for every request; `None` sets each request's deadline to the sum of its
    /// items' deadlines plus a margin.
    pub request_deadline: Option<Duration>,
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
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EvaluateError {
    /// The engine is not built yet.
    #[error("evaluation engine is not built")]
    NotBuilt,
}

/// Grades the runs `request` selects and records their outcomes. Every evaluator process is
/// spawned and stopped through `supervisor`; when `cancel` turns `true`, the requests in flight are
/// cancelled through the client and commit no outcome file.
///
/// # Errors
///
/// Returns [`EvaluateError::NotBuilt`] until the engine is built.
#[allow(
    clippy::unused_async,
    reason = "the seam is async; the engine leaf fills it"
)]
pub async fn evaluate(
    request: EvaluateRequest,
    supervisor: Arc<Supervisor>,
    cancel: watch::Receiver<bool>,
) -> Result<EvaluateReport, EvaluateError> {
    drop((request, supervisor, cancel));
    Err(EvaluateError::NotBuilt)
}
