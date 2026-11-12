//! The refusal codes of the errors of raw frame files.
//!
//! One `refusal_table!` call per error type: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other.

use std::error::Error;

use crate::frames::FramesError;
use candler_schemas::refusal::{CatalogRow, Catalogued, Refusal, Side};
use candler_schemas::refusal_table;

/// The path of an error that names no place in a document.
fn no_path<E>(_error: &E) -> Option<String> {
    None
}

refusal_table! {
    FramesError, path = no_path;
    Io { .. } => row(
        "candler.internal",
        Side::Candler,
        "A raw frame file cannot be opened or read.",
        "-",
    ),
    Record { .. } => row(
        "candler.internal",
        Side::Candler,
        "A line of a raw frame file is not a raw frame record.",
        "-",
    ),
    MissingHeader { .. } => row(
        "candler.internal",
        Side::Candler,
        "A raw frame file's first line is not a header.",
        "-",
    ),
    SecondHeader { .. } => row(
        "candler.internal",
        Side::Candler,
        "A raw frame file holds a second header after the first line.",
        "-",
    ),
}

/// The rows of every coded error of this module, in the order of the types.
#[must_use]
pub(crate) fn catalog() -> Vec<CatalogRow> {
    let tables: [&'static [CatalogRow]; 1] = [<FramesError as Catalogued>::rows()];
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
    error.downcast_ref::<FramesError>().map(as_refusal)
}
