//! What `exec_run` is given about a cell, where a treatment's skill tree comes from, and why a
//! run cannot be executed.

use std::path::{Path, PathBuf};

use candler_harness::driver::DriverError;
use candler_harness::spec::{CommandSpec, PreparedFile};
use candler_schemas::execution::{
    Comparable, DigestRef, ExecutionError, SecretPresence, TreatmentRef,
};
use candler_schemas::primitives::{Digest, FieldPath};
use candler_schemas::values::SourceEntry;

use crate::isolation::IsolationError;
use crate::isolation::image::PackageReadings;
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
    /// The checkout the treatment's skill tree is archived from; none when the treatment names
    /// no source.
    pub treatment_source: Option<PathBuf>,
    /// The input files the cell declares, by workspace path, with their digests.
    pub inputs: Inputs,
    /// The import packages the experiment names, as the cell's env image holds them; empty when
    /// the experiment names none.
    pub packages: PackageReadings,
    /// The evaluator-only inputs the cell declares, as `{kind, digest}`: the manifest lists them,
    /// and no run is given their content.
    pub evaluation_inputs: Vec<DigestRef>,
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
        treatment_source: Option<PathBuf>,
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
            packages: PackageReadings::new(),
            evaluation_inputs: Vec::new(),
        }
    }

    /// The record with the evaluator-only inputs of its experiment.
    #[must_use]
    pub fn with_evaluation_inputs(mut self, evaluation_inputs: Vec<DigestRef>) -> Self {
        self.evaluation_inputs = evaluation_inputs;
        self
    }

    /// The record with the readings of the named packages in its env image.
    #[must_use]
    pub fn with_packages(mut self, packages: PackageReadings) -> Self {
        self.packages = packages;
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
