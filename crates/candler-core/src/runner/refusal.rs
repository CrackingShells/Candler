//! The refusal codes of the errors of planning and running a cell, and of the evidence a run collects.
//!
//! One `refusal_table!` call per error type: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other.

use std::error::Error;

use crate::refusal::wrapper_table;
use crate::runner::evidence::catalog::CatalogError;
use crate::runner::evidence::census::CensusError;
use crate::runner::evidence::tree::EvidenceError;
use crate::runner::exec::cell::ExecError;
use crate::runner::plan::PlanError;
use candler_schemas::refusal::{CatalogRow, Catalogued, Refusal, Side, pointer_of};
use candler_schemas::refusal_table;

/// The path of an error that names no place in a document.
fn no_path<E>(_error: &E) -> Option<String> {
    None
}

/// The pointer of the place in a document where a [`PlanError`] sits, if it names one.
fn plan_error_path(error: &PlanError) -> Option<String> {
    match error {
        PlanError::Cell { source } => Refusal::path(source),
        PlanError::Session(inner) => Refusal::path(inner),
        PlanError::Driver(inner) => Refusal::path(inner),
        PlanError::SettingAsFactor { .. } => Some(pointer_of(["factors"])),
        PlanError::NameCollision { .. }
        | PlanError::ExperimentLocked
        | PlanError::Changed(..)
        | PlanError::StoredCellUnknown { .. }
        | PlanError::StoredCellAmbiguous { .. }
        | PlanError::AttemptOverflow { .. }
        | PlanError::Store { .. }
        | PlanError::Lock { .. } => None,
    }
}

/// The pointer of the place in a document where a [`ExecError`] sits, if it names one.
fn exec_error_path(error: &ExecError) -> Option<String> {
    match error {
        ExecError::Isolation(inner) => Refusal::path(inner),
        ExecError::Store(inner) => Refusal::path(inner),
        ExecError::Driver(inner) => Refusal::path(inner),
        ExecError::Manifest(inner) => Refusal::path(inner),
        ExecError::Supervise(inner) => Refusal::path(inner),
    }
}

wrapper_table! {
    PlanError, path = plan_error_path;
    NameCollision { .. } => row(
        "experiment.name_collision",
        Side::Experiment,
        "Two names under one root map to the same deviation path, so a deviation there would be ambiguous.",
        "-",
    ),
    ExperimentLocked { .. } => row(
        "runner.experiment_locked",
        Side::Environment,
        "Another supervisor holds the experiment, or a job that owns an unfinished attempt is alive.",
        "-",
    ),
    Changed (..) => row(
        "runner.fact_changed",
        Side::Environment,
        "A fact re-checked now differs from the stored cell and the plan was not forced; `--force` accepts the difference.",
        "-",
    ),
    StoredCellUnknown { .. } => row(
        "runner.stored_cell_unknown",
        Side::Experiment,
        "A past experiment has stored cells that no cell of this experiment answers, so it cannot be replayed.",
        "-",
    ),
    StoredCellAmbiguous { .. } => row(
        "candler.internal",
        Side::Candler,
        "Two stored cells answer one cell of the experiment, or two past experiments match it.",
        "-",
    ),
    AttemptOverflow { .. } => row(
        "candler.internal",
        Side::Candler,
        "The highest attempt number of a replicate is the largest value, so no next attempt exists.",
        "-",
    ),
    Session (inner) => delegate(inner, Side::Experiment),
    Driver (inner) => delegate(inner, Side::Experiment),
    SettingAsFactor { .. } => row(
        "experiment.setting_as_factor",
        Side::Experiment,
        "A factor varies a setting, which no cell can vary; the setting belongs in `[fixed]`.",
        "The `factors` array of the experiment file; a verb that has read the file narrows it to the `path` of the factor.",
    ),
    Cell { source } => delegate(source, Side::Candler),
    Store { .. } => row(
        "candler.internal",
        Side::Candler,
        "The store could not be read or sealed while planning.",
        "-",
    ),
    Lock { .. } => row(
        "candler.internal",
        Side::Candler,
        "The experiment lock could not be tried.",
        "-",
    ),
}

wrapper_table! {
    ExecError, path = exec_error_path;
    Isolation (inner) => delegate(inner, Side::Environment),
    Store (inner) => delegate(inner, Side::Environment),
    Driver (inner) => delegate(inner, Side::Experiment),
    Manifest (inner) => delegate(inner, Side::Candler),
    Supervise (inner) => delegate(inner, Side::Environment),
}

refusal_table! {
    CatalogError, path = no_path;
    Unavailable { .. } => row(
        "candler.internal",
        Side::Candler,
        "Catalog capture is not wired in this build.",
        "-",
    ),
    Command { .. } => row(
        "runner.catalog_command_failed",
        Side::Environment,
        "The command that prints the tool catalog exited non-zero or was ended by a signal.",
        "-",
    ),
    Timeout { .. } => row(
        "runner.catalog_timeout",
        Side::Environment,
        "The command that prints the tool catalog overran its time.",
        "-",
    ),
    Parse { .. } => row(
        "runner.catalog_unreadable",
        Side::Environment,
        "The output of the catalog command is not a tool catalog.",
        "-",
    ),
    AbsolutePath { .. } => row(
        "candler.internal",
        Side::Candler,
        "The catalog holds a path from the host or the image's file system.",
        "-",
    ),
    Digest { .. } => row(
        "candler.internal",
        Side::Candler,
        "The catalog has no canonical form.",
        "-",
    ),
}

refusal_table! {
    CensusError, path = no_path;
    NoInit { .. } => row(
        "runner.census_no_init",
        Side::Environment,
        "The harness reported no tool inventory, so it did not run usably.",
        "-",
    ),
    Malformed { .. } => row(
        "runner.census_malformed",
        Side::Environment,
        "The tool inventory lacks a list the census needs, or the list holds an entry of the wrong kind.",
        "-",
    ),
    Value { .. } => row(
        "candler.internal",
        Side::Candler,
        "A census value could not be built.",
        "-",
    ),
}

refusal_table! {
    EvidenceError, path = no_path;
    Path { .. } => row(
        "candler.internal",
        Side::Candler,
        "An output path of a run is not a relative path.",
        "-",
    ),
    Tree { .. } => row(
        "candler.internal",
        Side::Candler,
        "The outputs of a run do not form a tree.",
        "-",
    ),
}

/// The rows of every coded error of this module, in the order of the types.
#[must_use]
pub(crate) fn catalog() -> Vec<CatalogRow> {
    let tables: [&'static [CatalogRow]; 5] = [
        <PlanError as Catalogued>::rows(),
        <ExecError as Catalogued>::rows(),
        <CatalogError as Catalogued>::rows(),
        <CensusError as Catalogued>::rows(),
        <EvidenceError as Catalogued>::rows(),
    ];
    tables
        .iter()
        .flat_map(|rows| rows.iter().copied())
        .collect()
}

/// Views a coded error as its [`Refusal`].
fn as_refusal<'e, E: Refusal>(found: &'e E) -> &'e dyn Refusal {
    found
}

/// The refusal that answers for an [`ExecError`]: each variant only wraps an error, so the
/// answer is the wrapped error's own code, side and path.
fn exec_answer(error: &ExecError) -> &dyn Refusal {
    match error {
        ExecError::Isolation(inner) => crate::refusal::through(inner, error),
        ExecError::Store(inner) => crate::refusal::through(inner, error),
        ExecError::Driver(inner) => crate::refusal::through(inner, error),
        ExecError::Manifest(inner) => crate::refusal::through(inner, error),
        ExecError::Supervise(inner) => crate::refusal::through(inner, error),
    }
}

/// The refusal that answers for a [`PlanError`]: the error a cell builder gave answers with its own
/// code, side and path, and every other variant with its own.
fn plan_answer(error: &PlanError) -> &dyn Refusal {
    match error {
        PlanError::Cell { source } => crate::refusal::through(source, error),
        PlanError::Driver(inner) => crate::refusal::through(inner, error),
        PlanError::NameCollision { .. }
        | PlanError::ExperimentLocked
        | PlanError::Changed(..)
        | PlanError::StoredCellUnknown { .. }
        | PlanError::StoredCellAmbiguous { .. }
        | PlanError::AttemptOverflow { .. }
        | PlanError::SettingAsFactor { .. }
        | PlanError::Session(..)
        | PlanError::Store { .. }
        | PlanError::Lock { .. } => error,
    }
}

/// The coded error of this module that `error` is, or `None` when it is none of them.
#[must_use]
pub(crate) fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    error
        .downcast_ref::<PlanError>()
        .map(plan_answer)
        .or_else(|| {
            error
                .downcast_ref::<ExecError>()
                .map(exec_answer)
                .or_else(|| {
                    error
                        .downcast_ref::<CatalogError>()
                        .map(as_refusal)
                        .or_else(|| {
                            error
                                .downcast_ref::<CensusError>()
                                .map(as_refusal)
                                .or_else(|| error.downcast_ref::<EvidenceError>().map(as_refusal))
                        })
                })
        })
}
