//! The refusal codes of the errors of what an experiment file declares: its inputs.
//!
//! One `refusal_table!` call per error type: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other.

use std::error::Error;

use crate::experiment::inputs::InputError;
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

/// The rows of every coded error of this module, in the order of the types.
#[must_use]
pub(crate) fn catalog() -> Vec<CatalogRow> {
    let tables: [&'static [CatalogRow]; 1] = [<InputError as Catalogued>::rows()];
    tables
        .iter()
        .flat_map(|rows| rows.iter().copied())
        .collect()
}

/// Views a coded error as its [`Refusal`].
fn as_refusal<'e, E: Refusal>(found: &'e E) -> &'e dyn Refusal {
    found
}

/// The coded error of this module that `error` is, or `None` when it is none of them.
#[must_use]
pub(crate) fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    error.downcast_ref::<InputError>().map(as_refusal)
}
