//! The refusal codes of the errors of resolution: the configuration files, the preflight and the resolver.
//!
//! One `refusal_table!` call per error type: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other.

use std::error::Error;

use crate::resolve::config::ConfigError;
use crate::resolve::preflight::{PreflightError, ResolveError};
use candler_schemas::refusal::{CatalogRow, Catalogued, Refusal, Side, pointer_of};
use candler_schemas::refusal_table;

/// The path of an error that names no place in a document.
fn no_path<E>(_error: &E) -> Option<String> {
    None
}

/// The pointer of the place in a document where a [`ConfigError`] sits, if it names one.
fn config_error_path(error: &ConfigError) -> Option<String> {
    match error {
        ConfigError::UnknownKey { key, .. }
        | ConfigError::MachineWideKey { key, .. }
        | ConfigError::WrongType { key, .. } => Some(pointer_of(key.split('.'))),
        ConfigError::Read { .. } | ConfigError::Parse { .. } | ConfigError::RelativeCwd { .. } => {
            None
        }
    }
}

/// The pointer of the place in a document where a [`ResolveError`] sits, if it names one.
fn resolve_error_path(error: &ResolveError) -> Option<String> {
    match error {
        ResolveError::Value(inner) => Refusal::path(inner),
        ResolveError::NonUtf8Path { .. }
        | ResolveError::DuplicateStep { .. }
        | ResolveError::DerivedTwice { .. }
        | ResolveError::Preflight { .. }
        | ResolveError::Format(..) => None,
    }
}

refusal_table! {
    ConfigError, path = config_error_path;
    Read { .. } => row(
        "config.file_unreadable",
        Side::Config,
        "A configuration file exists and cannot be read.",
        "-",
    ),
    Parse { .. } => row(
        "config.toml_invalid",
        Side::Config,
        "A configuration file is not valid TOML.",
        "-",
    ),
    UnknownKey { .. } => row(
        "config.unknown_key",
        Side::Config,
        "A configuration file holds a key that no table defines, such as a misspelt setting.",
        "The unknown key in the configuration file named in the message.",
    ),
    MachineWideKey { .. } => row(
        "config.machine_wide_key",
        Side::Config,
        "A project configuration sets a machine-wide key, which only the user configuration may set.",
        "The key in the project configuration file named in the message.",
    ),
    WrongType { .. } => row(
        "config.wrong_type",
        Side::Config,
        "A configuration key holds a value of the wrong type.",
        "The key in the configuration file named in the message.",
    ),
    RelativeCwd { .. } => row(
        "config.cwd_not_absolute",
        Side::Config,
        "The working directory is not an absolute path, so no ancestor can be searched for a configuration.",
        "-",
    ),
}

refusal_table! {
    PreflightError, path = no_path;
    Failed { .. } => row(
        "resolve.preflight_step_failed",
        Side::Environment,
        "A preflight step failed before it could report, for example a missing directory or program.",
        "-",
    ),
}

refusal_table! {
    ResolveError, path = resolve_error_path;
    NonUtf8Path { .. } => row(
        "resolve.path_not_unicode",
        Side::Config,
        "A path that has to be written as a JSON string is not valid UTF-8.",
        "-",
    ),
    Value (inner) => delegate(inner, Side::Config),
    Format (..) => row(
        "resolve.value_invalid",
        Side::Config,
        "A value of the resolution has the wrong form for what it names, such as a name with a space.",
        "-",
    ),
    DuplicateStep { .. } => row(
        "candler.internal",
        Side::Candler,
        "Two preflight steps were registered under one name.",
        "-",
    ),
    DerivedTwice { .. } => row(
        "candler.internal",
        Side::Candler,
        "Two preflight steps derive the same path.",
        "-",
    ),
    Preflight { .. } => row(
        "resolve.preflight_failed",
        Side::Environment,
        "At least one preflight step failed; every step ran and the message lists the failures.",
        "-",
    ),
}

/// The rows of every coded error of this module, in the order of the types.
#[must_use]
pub(crate) fn catalog() -> Vec<CatalogRow> {
    let tables: [&'static [CatalogRow]; 3] = [
        <ConfigError as Catalogued>::rows(),
        <PreflightError as Catalogued>::rows(),
        <ResolveError as Catalogued>::rows(),
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

/// The refusal that answers for a [`ResolveError`]: a value or format error it wraps answers with
/// its own code, side and path, and every other variant with its own.
fn resolve_answer(error: &ResolveError) -> &dyn Refusal {
    match error {
        ResolveError::Value(inner) => crate::refusal::through(inner, error),
        ResolveError::NonUtf8Path { .. }
        | ResolveError::DuplicateStep { .. }
        | ResolveError::DerivedTwice { .. }
        | ResolveError::Preflight { .. }
        | ResolveError::Format(..) => error,
    }
}

/// The coded error of this module that `error` is, or `None` when it is none of them.
#[must_use]
pub(crate) fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    error
        .downcast_ref::<ConfigError>()
        .map(as_refusal)
        .or_else(|| {
            error
                .downcast_ref::<PreflightError>()
                .map(as_refusal)
                .or_else(|| error.downcast_ref::<ResolveError>().map(resolve_answer))
        })
}
