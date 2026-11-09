//! What `exec_run` is given about a cell, where a treatment's skill tree comes from, and why a
//! run cannot be executed.

use std::path::{Path, PathBuf};

use candler_harness::driver::DriverError;
use candler_harness::spec::{CommandSpec, PreparedFile};
use candler_schemas::execution::{Comparable, ExecutionError, SecretPresence, TreatmentRef};
use candler_schemas::primitives::{Digest, FieldPath};
use candler_schemas::values::SourceEntry;

use crate::isolation::IsolationError;
use crate::isolation::image::PlannerAxis;
use crate::runner::evidence::Inputs;
use crate::store::StoreError;
use crate::supervise::SuperviseError;

/// Everything about a cell that the manifest of its runs repeats, resolved once for the job.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CellRun {
    /// The cell's comparable fields.
    pub comparable: Comparable,
    /// The digest of the experiment the cell belongs to.
    pub experiment: Digest,
    /// The field paths that vary across the experiment's cells.
    pub factors: Vec<FieldPath>,
    /// Where each value of the cell came from.
    pub sources: Vec<SourceEntry>,
    /// The secrets the cell needs, and whether each was present.
    pub secrets: Vec<SecretPresence>,
    /// The checkout the treatment's skill tree is archived from.
    pub treatment_source: PathBuf,
    /// The input files the cell declares, by workspace path, with their digests.
    pub inputs: Inputs,
    /// The planner library of the cell's env image, when the job probed it.
    pub planner: Option<PlannerAxis>,
}

impl CellRun {
    /// A cell's record, from what the job resolved.
    #[must_use]
    pub const fn new(
        comparable: Comparable,
        experiment: Digest,
        factors: Vec<FieldPath>,
        sources: Vec<SourceEntry>,
        secrets: Vec<SecretPresence>,
        treatment_source: PathBuf,
        inputs: Inputs,
    ) -> Self {
        Self {
            comparable,
            experiment,
            factors,
            sources,
            secrets,
            treatment_source,
            inputs,
            planner: None,
        }
    }

    /// The record with the planner library of its env image.
    #[must_use]
    pub fn with_planner(mut self, planner: Option<PlannerAxis>) -> Self {
        self.planner = planner;
        self
    }
}

/// Where a treatment's skill tree comes from: the command that archives it, and the files the
/// archive holds. The harness driver's crate supplies the real one.
pub trait SkillTree: Send + Sync {
    /// The command that archives the tree `treatment` pins from `source`.
    ///
    /// # Errors
    /// [`DriverError`] when the treatment pins no tree or the command cannot be built.
    fn archive_command(
        &self,
        treatment: &TreatmentRef,
        source: &Path,
    ) -> Result<CommandSpec, DriverError>;

    /// The files of `archive`, the output of the archive command for `treatment`.
    ///
    /// # Errors
    /// [`DriverError`] when the archive cannot be read or holds an entry outside the treatment's
    /// skill tree.
    fn files(
        &self,
        treatment: &TreatmentRef,
        archive: &[u8],
    ) -> Result<Vec<PreparedFile>, DriverError>;
}

/// Why a run could not be executed. It never means the run was deleted: either no run directory
/// was made, or the run is left unsealed for `Store::seal_interrupted`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ExecError {
    /// The run cannot be placed in a container, or the runtime has no isolation.
    #[error(transparent)]
    Isolation(#[from] IsolationError),
    /// The store refused a write.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The harness driver could not answer.
    #[error(transparent)]
    Driver(#[from] DriverError),
    /// The manifest of the run cannot be built.
    #[error("the manifest cannot be built: {0}")]
    Manifest(#[from] ExecutionError),
    /// The supervisor could not start or wait for a child.
    #[error(transparent)]
    Supervise(#[from] SuperviseError),
}
