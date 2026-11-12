//! The refusal codes of the errors of the harness interface: the driver, the trace reader and the command builder.
//!
//! One `refusal_table!` call per error type: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other.

use std::error::Error;

use crate::driver::DriverError;
use crate::reader::ReaderError;
use crate::spec::{CommandSpecError, InitReportError};
use candler_schemas::refusal::{CatalogRow, Catalogued, Refusal, Side};
use candler_schemas::refusal_table;

/// The path of an error that names no place in a document.
fn no_path<E>(_error: &E) -> Option<String> {
    None
}

/// The pointer of the place in a document where a [`DriverError`] sits, if it names one.
fn driver_error_path(error: &DriverError) -> Option<String> {
    match error {
        DriverError::Command(inner) => Refusal::path(inner),
        DriverError::MissingValue { .. }
        | DriverError::InvalidValue { .. }
        | DriverError::VersionOutput { .. }
        | DriverError::HarnessName { .. }
        | DriverError::Record { .. } => None,
    }
}

refusal_table! {
    DriverError, path = driver_error_path;
    MissingValue { .. } => row(
        "experiment.cell_value_missing",
        Side::Experiment,
        "A cell lacks a value that the harness driver needs.",
        "-",
    ),
    InvalidValue { .. } => row(
        "experiment.cell_value_invalid",
        Side::Experiment,
        "A cell value cannot be used by the harness driver.",
        "-",
    ),
    VersionOutput { .. } => row(
        "harness.version_unreadable",
        Side::Environment,
        "The output of the harness version probe names no version.",
        "-",
    ),
    HarnessName { .. } => row(
        "candler.internal",
        Side::Candler,
        "A harness name is not a valid name.",
        "-",
    ),
    Command (inner) => delegate(inner, Side::Candler),
    Record { .. } => row(
        "harness.session_record_unreadable",
        Side::Environment,
        "A session record that the harness wrote, such as a hook payload or a stream event, cannot be read by the driver's tracker.",
        "-",
    ),
}

refusal_table! {
    ReaderError, path = no_path;
    Frame { .. } => row(
        "candler.internal",
        Side::Candler,
        "A frame of a stream cannot be read.",
        "-",
    ),
    Incomplete { .. } => row(
        "candler.internal",
        Side::Candler,
        "A stream ended before the reader could name its sessions.",
        "-",
    ),
    Trace (..) => row(
        "candler.internal",
        Side::Candler,
        "Sessions do not form a valid trace.",
        "-",
    ),
}

refusal_table! {
    CommandSpecError, path = no_path;
    MissingField { .. } => row(
        "candler.internal",
        Side::Candler,
        "A command builder was asked to build before the argument vector was set.",
        "-",
    ),
    EmptyArgv { .. } => row(
        "candler.internal",
        Side::Candler,
        "A command has an empty argument vector, so there is no program to run.",
        "-",
    ),
    DuplicateEnv { .. } => row(
        "candler.internal",
        Side::Candler,
        "A command sets one environment variable twice.",
        "-",
    ),
    UnsafeSessionId { .. } => row(
        "candler.internal",
        Side::Candler,
        "A session id holds a character that cannot be written into a command, so the pre-stop is skipped.",
        "-",
    ),
}

refusal_table! {
    InitReportError, path = no_path;
    ToolsNotNames { .. } => row(
        "harness.inventory_malformed",
        Side::Environment,
        "The tool inventory a harness or the gateway observed is not a list of names.",
        "-",
    ),
    EmptyToolName { .. } => row(
        "harness.inventory_empty_name",
        Side::Environment,
        "The tool inventory a harness or the gateway observed holds an empty tool name.",
        "-",
    ),
    ServersNotList { .. } => row(
        "harness.server_list_malformed",
        Side::Environment,
        "The MCP server list a harness observed is not a list.",
        "-",
    ),
    NoTools { .. } => row(
        "harness.inventory_unrecorded",
        Side::Environment,
        "A gateway or collected census source saw no tool, so the inventory is unrecorded rather than empty.",
        "-",
    ),
}

/// The rows of every coded error of this module, in the order of the types.
#[must_use]
pub fn catalog() -> Vec<CatalogRow> {
    let tables: [&'static [CatalogRow]; 4] = [
        <DriverError as Catalogued>::rows(),
        <ReaderError as Catalogued>::rows(),
        <CommandSpecError as Catalogued>::rows(),
        <InitReportError as Catalogued>::rows(),
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
pub fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    error
        .downcast_ref::<DriverError>()
        .map(as_refusal)
        .or_else(|| {
            error
                .downcast_ref::<ReaderError>()
                .map(as_refusal)
                .or_else(|| {
                    error
                        .downcast_ref::<CommandSpecError>()
                        .map(as_refusal)
                        .or_else(|| error.downcast_ref::<InitReportError>().map(as_refusal))
                })
        })
}
