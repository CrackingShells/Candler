//! The refusal codes of the errors of process supervision.
//!
//! One `refusal_table!` call per error type: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other.

use std::error::Error;

use crate::supervise::SuperviseError;
use crate::supervise::hook::HookError;
use candler_schemas::refusal::{CatalogRow, Catalogued, Refusal, Side};
use candler_schemas::refusal_table;

/// The path of an error that names no place in a document.
fn no_path<E>(_error: &E) -> Option<String> {
    None
}

refusal_table! {
    SuperviseError, path = no_path;
    MissingField { .. } => row(
        "candler.internal",
        Side::Candler,
        "A spawn request was asked to build before a required field was set.",
        "-",
    ),
    InvalidRequest { .. } => row(
        "candler.internal",
        Side::Candler,
        "A spawn request cannot be carried out.",
        "-",
    ),
    Spawn { .. } => row(
        "supervise.spawn_failed",
        Side::Environment,
        "The operating system refused to start the program, such as a program that is not installed.",
        "-",
    ),
    Io { .. } => row(
        "supervise.io_failed",
        Side::Environment,
        "A file or process operation of the supervisor failed.",
        "-",
    ),
    ProcStat { .. } => row(
        "candler.internal",
        Side::Candler,
        "A process stat line does not hold the start time.",
        "-",
    ),
    Event { .. } => row(
        "candler.internal",
        Side::Candler,
        "An event or a record that the schemas refuse.",
        "-",
    ),
    Survived { .. } => row(
        "supervise.child_survived_kill",
        Side::Environment,
        "A child process is still running after it was sent `SIGKILL`.",
        "-",
    ),
    Clock { .. } => row(
        "candler.internal",
        Side::Candler,
        "A time cannot be read or written.",
        "-",
    ),
}

refusal_table! {
    HookError, path = no_path;
    Failed { .. } => row(
        "candler.internal",
        Side::Candler,
        "The stop hook could not do its work.",
        "-",
    ),
    Refused { .. } => row(
        "candler.internal",
        Side::Candler,
        "The stop hook declined to act.",
        "-",
    ),
}

/// The rows of every coded error of this module, in the order of the types.
#[must_use]
pub(crate) fn catalog() -> Vec<CatalogRow> {
    let tables: [&'static [CatalogRow]; 2] = [
        <SuperviseError as Catalogued>::rows(),
        <HookError as Catalogued>::rows(),
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

/// The coded error of this module that `error` is, or `None` when it is none of them.
#[must_use]
pub(crate) fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    error
        .downcast_ref::<SuperviseError>()
        .map(as_refusal)
        .or_else(|| error.downcast_ref::<HookError>().map(as_refusal))
}
