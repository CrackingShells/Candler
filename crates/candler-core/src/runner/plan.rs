//! Planning the runs of a job: which cells run, with which attempt, and which are already sealed.

use std::path::PathBuf;

use candler_schemas::execution::{Comparable, Deviation, ExperimentFile};
use candler_schemas::primitives::{Digest, RunKey};

use crate::resolve::layers::Resolution;
use crate::store::Store;

/// The digest of a cell.
pub type CellId = Digest;

/// What planning reads.
#[derive(Debug, Clone)]
pub struct PlanCtx {
    /// The store.
    pub store: Store,
    /// The path of the experiment's lock file.
    pub experiment_lock: PathBuf,
}

/// Whether a forced plan may accept a differing re-check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Force {
    /// A difference refuses the plan.
    No,
    /// A difference is accepted and recorded as a deviation.
    Yes,
}

/// One run to start.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RunPlan {
    /// The run's key: the cell, the replicate and the attempt.
    pub key: RunKey,
    /// The cell.
    pub cell: CellId,
    /// The comparable record a sealed run of the cell stored, when there is one.
    pub stored: Option<Comparable>,
    /// The attempt number.
    pub attempt: u32,
    /// The deviations a forced plan accepted.
    pub deviations: Vec<Deviation>,
}

/// What planning decided.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanOutcome {
    /// The runs to start.
    pub plans: Vec<RunPlan>,
    /// The runs that were already sealed and are skipped.
    pub sealed: Vec<RunKey>,
}

/// Why planning failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PlanError {
    /// Planning is not implemented.
    #[error("planning is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
    /// A stored run does not match the experiment.
    #[error("a stored run differs from the experiment at `{path}`")]
    Differs {
        /// The path of the first difference.
        path: String,
    },
}

/// Decides the runs of `file` under `resolution`.
///
/// # Errors
/// [`PlanError::Differs`] when a stored run differs and `force` is [`Force::No`].
pub fn plan(
    ctx: &PlanCtx,
    file: &ExperimentFile,
    resolution: &Resolution,
    force: Force,
) -> Result<PlanOutcome, PlanError> {
    let _ = (ctx, file, resolution, force);
    Err(PlanError::Unavailable {
        detail: "plan".to_owned(),
    })
}
