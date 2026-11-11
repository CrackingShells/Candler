//! The refusal codes of the errors of what an experiment file declares: its inputs and its surface.
//!
//! One `refusal_table!` call per error type: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other.

use std::error::Error;

use crate::experiment::inputs::InputError;
use crate::experiment::surface::SurfaceError;
use candler_schemas::refusal::{CatalogRow, Catalogued, Refusal, Side, pointer_of};
use candler_schemas::refusal_table;

/// The pointer of the place in a document where a [`InputError`] sits, if it names one.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the table takes one signature for every error"
)]
fn input_error_path(error: &InputError) -> Option<String> {
    match error {
        InputError::Refused { .. } | InputError::Store { .. } => Some(pointer_of(["inputs"])),
    }
}

/// The pointer of the place in a document where a [`SurfaceError`] sits, if it names one.
fn surface_error_path(error: &SurfaceError) -> Option<String> {
    match error {
        SurfaceError::Format(inner) => Refusal::path(inner),
        SurfaceError::Missing(..)
        | SurfaceError::Type { .. }
        | SurfaceError::ReservedRoot { .. }
        | SurfaceError::AbsolutePath { .. } => None,
    }
}

refusal_table! {
    InputError, path = input_error_path;
    Refused { .. } => row(
        "experiment.input_unusable",
        Side::Experiment,
        "A declared input cannot be used as written, for example a missing file or a directory where a file is needed.",
        "The `inputs` array of the experiment file.",
    ),
    Store { .. } => row(
        "experiment.input_not_stored",
        Side::Experiment,
        "A declared input file or list could not be copied into the store.",
        "The `inputs` array of the experiment file.",
    ),
}

refusal_table! {
    SurfaceError, path = surface_error_path;
    Missing (..) => row(
        "experiment.setting_missing",
        Side::Experiment,
        "A setting the harness needs has no value in the cell.",
        "-",
    ),
    Type { .. } => row(
        "experiment.setting_wrong_type",
        Side::Experiment,
        "A setting holds a JSON value of a type its path does not take.",
        "-",
    ),
    ReservedRoot { .. } => row(
        "experiment.setting_under_reserved_root",
        Side::Experiment,
        "A setting is written below a reserved root, which takes one value.",
        "-",
    ),
    AbsolutePath { .. } => row(
        "experiment.setting_absolute_path",
        Side::Experiment,
        "A setting holds an absolute path where a path relative to the project is required.",
        "-",
    ),
    Format (inner) => delegate(inner, Side::Experiment),
}

/// The rows of every coded error of this module, in the order of the types.
#[must_use]
pub(crate) fn catalog() -> Vec<CatalogRow> {
    let tables: [&'static [CatalogRow]; 2] = [
        <InputError as Catalogued>::rows(),
        <SurfaceError as Catalogued>::rows(),
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

/// The refusal that answers for a [`SurfaceError`]: a format error it wraps answers with its own
/// code, side and path, and every other variant with its own.
fn surface_answer(error: &SurfaceError) -> &dyn Refusal {
    match error {
        SurfaceError::Format(inner) => crate::refusal::through(inner, error),
        SurfaceError::Missing(..)
        | SurfaceError::Type { .. }
        | SurfaceError::ReservedRoot { .. }
        | SurfaceError::AbsolutePath { .. } => error,
    }
}

/// The coded error of this module that `error` is, or `None` when it is none of them.
#[must_use]
pub(crate) fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    error
        .downcast_ref::<InputError>()
        .map(as_refusal)
        .or_else(|| error.downcast_ref::<SurfaceError>().map(surface_answer))
}
