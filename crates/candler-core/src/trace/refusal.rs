//! The refusal codes of the errors of normalising raw frame files into a trace file.
//!
//! One `refusal_table!` call per error type: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other.

use std::error::Error;

use crate::trace::NormaliseError;
use candler_schemas::refusal::{CatalogRow, Catalogued, Refusal, Side};
use candler_schemas::refusal_table;

/// The path of an error that names no place in a document.
fn no_path<E>(_error: &E) -> Option<String> {
    None
}

refusal_table! {
    NormaliseError, path = no_path;
    Frames (..) => row(
        "candler.internal",
        Side::Candler,
        "A raw frame file that Candler's capture wrote cannot be read back.",
        "-",
    ),
    Undetected { .. } => row(
        "candler.internal",
        Side::Candler,
        "The reader does not recognise the format of a stream that was recognised when it was captured.",
        "-",
    ),
    Frame { .. } => row(
        "candler.internal",
        Side::Candler,
        "The reader refuses a frame that was accepted when it was captured.",
        "-",
    ),
    Time { .. } => row(
        "candler.internal",
        Side::Candler,
        "A frame has no valid wall time although the capture stamped it.",
        "-",
    ),
    Finish { .. } => row(
        "candler.internal",
        Side::Candler,
        "The reader cannot end the stream it has read.",
        "-",
    ),
    Trace { .. } => row(
        "candler.internal",
        Side::Candler,
        "The trace read from a stream is not a valid trace file.",
        "-",
    ),
}

/// The rows of every coded error of this module, in the order of the types.
#[must_use]
pub(crate) fn catalog() -> Vec<CatalogRow> {
    <NormaliseError as Catalogued>::rows().to_vec()
}

/// Views a coded error as its [`Refusal`].
fn as_refusal<'e, E: Refusal>(found: &'e E) -> &'e dyn Refusal {
    found
}

/// The coded error of this module that `error` is, or `None` when it is none of them.
#[must_use]
pub(crate) fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    error.downcast_ref::<NormaliseError>().map(as_refusal)
}
