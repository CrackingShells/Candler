//! The refusal codes of the errors of the run store.
//!
//! One `refusal_table!` call per error type: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other.

use std::error::Error;

use crate::store::StoreError;
use candler_schemas::refusal::{CatalogRow, Catalogued, Refusal, Side};
use candler_schemas::refusal_table;

/// The pointer of the place in a document where a [`StoreError`] sits, if it names one.
fn store_error_path(error: &StoreError) -> Option<String> {
    match error {
        StoreError::Format(inner) => Refusal::path(inner),
        StoreError::Io { .. }
        | StoreError::NotAStore { .. }
        | StoreError::DigestMismatch { .. }
        | StoreError::RunExists { .. }
        | StoreError::RunSealed { .. }
        | StoreError::ManifestMismatch { .. }
        | StoreError::StreamExists { .. }
        | StoreError::OutcomeExists { .. }
        | StoreError::RecordMismatch { .. }
        | StoreError::IndexCorrupt { .. }
        | StoreError::Parse { .. } => None,
    }
}

refusal_table! {
    StoreError, path = store_error_path;
    Io { .. } => row(
        "store.io_failed",
        Side::Environment,
        "A file system call of the store failed, such as a read, a write or a rename.",
        "-",
    ),
    NotAStore { .. } => row(
        "store.not_a_store",
        Side::Environment,
        "The directory named as the store has no valid `store.json` marker.",
        "-",
    ),
    DigestMismatch { .. } => row(
        "store.digest_mismatch",
        Side::Environment,
        "Bytes read from or written to the store do not have the digest that was expected of them.",
        "-",
    ),
    RunExists { .. } => row(
        "store.run_exists",
        Side::Environment,
        "A run directory already exists and the caller did not ask to replace it, or a finished run cannot be replaced.",
        "-",
    ),
    RunSealed { .. } => row(
        "store.run_sealed",
        Side::Environment,
        "The run is sealed by `run.done` and cannot change.",
        "-",
    ),
    ManifestMismatch { .. } => row(
        "candler.internal",
        Side::Candler,
        "A manifest does not belong to the run it was written for.",
        "-",
    ),
    StreamExists { .. } => row(
        "candler.internal",
        Side::Candler,
        "A raw stream is already stored in the run.",
        "-",
    ),
    OutcomeExists { .. } => row(
        "candler.internal",
        Side::Candler,
        "The final outcome file of a request exists and is never replaced.",
        "-",
    ),
    RecordMismatch { .. } => row(
        "candler.internal",
        Side::Candler,
        "A record does not belong to the request its outcome writer was opened for.",
        "-",
    ),
    IndexCorrupt { .. } => row(
        "candler.internal",
        Side::Candler,
        "A line of the store index other than the last does not parse; `rebuild_index` repairs it.",
        "-",
    ),
    Parse { .. } => row(
        "candler.internal",
        Side::Candler,
        "A stored file is not a valid record.",
        "-",
    ),
    Format (inner) => delegate(inner, Side::Environment),
}

/// The rows of every coded error of this module, in the order of the types.
#[must_use]
pub(crate) fn catalog() -> Vec<CatalogRow> {
    let tables: [&'static [CatalogRow]; 1] = [<StoreError as Catalogued>::rows()];
    tables
        .iter()
        .flat_map(|rows| rows.iter().copied())
        .collect()
}

/// The refusal that answers for a [`StoreError`]: a format error it wraps answers with its own
/// code, side and path, and every other variant with its own.
fn store_answer(error: &StoreError) -> &dyn Refusal {
    match error {
        StoreError::Format(inner) => crate::refusal::through(inner, error),
        StoreError::Io { .. }
        | StoreError::NotAStore { .. }
        | StoreError::DigestMismatch { .. }
        | StoreError::RunExists { .. }
        | StoreError::RunSealed { .. }
        | StoreError::ManifestMismatch { .. }
        | StoreError::StreamExists { .. }
        | StoreError::OutcomeExists { .. }
        | StoreError::RecordMismatch { .. }
        | StoreError::IndexCorrupt { .. }
        | StoreError::Parse { .. } => error,
    }
}

/// The coded error of this module that `error` is, or `None` when it is none of them.
#[must_use]
pub(crate) fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    error.downcast_ref::<StoreError>().map(store_answer)
}
