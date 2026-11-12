//! What `exec_run` is given about a cell, and why a run cannot be executed.

use std::path::PathBuf;

use candler_harness::driver::DriverError;
use candler_schemas::execution::{Comparable, DigestRef, ExecutionError, SecretPresence};
use candler_schemas::primitives::{Digest, FieldPath};
use candler_schemas::values::SourceEntry;

use crate::isolation::IsolationError;
use crate::isolation::image::PackageReadings;
use crate::runner::evidence::Inputs;
use crate::runner::evidence::session::ControlRef;
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
    /// The record of the session controls the cell's runs rely on; none for a cell that is not a
    /// session.
    pub control: Option<ControlRef>,
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
            control: None,
        }
    }

    /// The record with the session controls its runs rely on.
    #[must_use]
    pub fn with_control(mut self, control: ControlRef) -> Self {
        self.control = Some(control);
        self
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
