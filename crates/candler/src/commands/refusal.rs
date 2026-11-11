//! The refusals the binary raises itself, and the one place a verb's failure is written for
//! stderr.
//!
//! [`CliRefusal`] holds the refusals that no library crate raises: they name a command line or
//! an environment, not a file. [`render`] writes a failure as it always was (`candler:
//! <message>`) or, under `--error-format json`, as one `refusal/1` record whose code, side and
//! path come from the refusal the error carries or holds in its chain of causes.

use std::error::Error;

use candler_core::experiment::paths;
use candler_schemas::refusal::{Refusal, RefusalCode, RefusalRecord, Side, pointer_of};
use candler_schemas::refusal_table;

use super::{Carried, CliError};
use crate::ErrorFormat;

/// A refusal of the command line or the environment of the binary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CliRefusal {
    /// No store root is named by the command line, the environment or the configuration.
    #[error("no store: pass --store, set CANDLER_STORE or configure store.root")]
    NoStore,
    /// `--set` names a setting that an experiment file's field replaced.
    #[error("`{path}` is no longer a setting: set `[treatment] source` in the experiment file")]
    RetiredSetting {
        /// The path of the retired setting.
        path: String,
    },
    /// `--set` names a key under `request` that no request setting has.
    #[error(
        "`{path}` is not a request setting: the known ones are {}",
        known_request_keys()
    )]
    UnknownRequestKey {
        /// The path `--set` named.
        path: String,
    },
    /// A dotted path that no path class of the surface knows, written under `[fixed]`, in a
    /// factor or in `--set`.
    #[error("`{path}` is not a path of the experiment surface; it is written in {location}")]
    UnknownFieldPath {
        /// The dotted path.
        path: String,
        /// Where it was written.
        location: FieldLocation,
    },
}

/// Where an experiment's dotted path was written.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldLocation {
    /// A key of `[fixed]`.
    Fixed,
    /// A factor of the experiment file: its own `path`, or a leaf of one of its levels, given as
    /// the index of the level and the keys down to the leaf.
    Factor {
        /// The index of the factor in `factors`.
        index: usize,
        /// The level and the keys of the leaf, when the path comes from a table level.
        leaf: Option<(usize, Vec<String>)>,
    },
    /// A `--set` argument.
    Set,
}

impl std::fmt::Display for FieldLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fixed => f.write_str("`[fixed]`"),
            Self::Factor { index, .. } => write!(f, "the factor at `/factors/{index}`"),
            Self::Set => f.write_str("`--set`"),
        }
    }
}

/// The pointer into the experiment file of the place an unknown path was written, or `None` for
/// `--set`, which is no document.
fn unknown_path_pointer(path: &str, location: &FieldLocation) -> Option<String> {
    match location {
        FieldLocation::Fixed => Some(pointer_of(["fixed", path])),
        FieldLocation::Factor { index, leaf: None } => {
            Some(pointer_of(["factors", index.to_string().as_str(), "path"]))
        }
        FieldLocation::Factor {
            index,
            leaf: Some((level, keys)),
        } => {
            let (index, level) = (index.to_string(), level.to_string());
            let head = ["factors", index.as_str(), "levels", level.as_str()];
            Some(pointer_of(
                head.into_iter().chain(keys.iter().map(String::as_str)),
            ))
        }
        FieldLocation::Set => None,
    }
}

/// The request settings, each in backticks, as the refusal of an unknown one lists them.
fn known_request_keys() -> String {
    REQUEST_KEYS
        .iter()
        .map(|key| format!("`{key}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The request settings `--set` may name, as the experiment's path table defines them.
pub const REQUEST_KEYS: [&str; 3] = [
    paths::REQUEST_HARNESS,
    paths::REQUEST_TREATMENT,
    paths::REQUEST_INPUTS,
];

/// The pointer of the place in the experiment file where a [`CliRefusal`] sits, if it names one.
fn cli_refusal_path(error: &CliRefusal) -> Option<String> {
    match error {
        CliRefusal::UnknownFieldPath { path, location } => {
            unknown_path_pointer(path.as_str(), location)
        }
        CliRefusal::NoStore
        | CliRefusal::RetiredSetting { .. }
        | CliRefusal::UnknownRequestKey { .. } => None,
    }
}

refusal_table! {
    CliRefusal, path = cli_refusal_path;
    NoStore { .. } => row(
        "config.no_store",
        Side::Config,
        "No store root is given: no `--store`, no `CANDLER_STORE` and no `store.root` in a configuration file.",
        "-",
    ),
    RetiredSetting { .. } => row(
        "config.retired_setting",
        Side::Config,
        "`--set` names `request.treatment_source`, which `[treatment] source` in the experiment file replaced.",
        "-",
    ),
    UnknownRequestKey { .. } => row(
        "experiment.unknown_request_key",
        Side::Experiment,
        "`--set` names a key under `request` that is none of the request settings, such as a misspelt `request.harnes`.",
        "-",
    ),
    UnknownFieldPath { .. } => row(
        "experiment.unknown_path",
        Side::Experiment,
        "A dotted path under `[fixed]`, in a factor or in `--set` is none that the experiment surface has, such as a misspelt `model.idd`.",
        "The key under `/fixed`, the `path` of the factor, or the key of the level's table that holds the path; none for `--set`.",
    ),
}

/// The path of an error that names no place in a document.
fn cli_error_path(error: &CliError) -> Option<String> {
    match error {
        CliError::Coded {
            refusal, pointer, ..
        } => pointer.clone().or_else(|| Refusal::path(refusal)), // BORROW: the path is owned text
        CliError::Refused { .. } | CliError::Failed { .. } | CliError::Runtime { .. } => None,
    }
}

refusal_table! {
    CliError, path = cli_error_path;
    Refused { .. } => row(
        "candler.internal",
        Side::Candler,
        "A refusal that the verb that raises it has not given a code yet.",
        "-",
    ),
    Failed { .. } => row(
        "candler.internal",
        Side::Candler,
        "A verb failed; the record carries the code of the refusal found in the chain of causes, and this one when the chain holds none.",
        "-",
    ),
    Runtime { .. } => row(
        "candler.internal",
        Side::Candler,
        "The process could not run the verb at all.",
        "-",
    ),
    Coded { refusal, .. } => delegate(refusal, Side::Config),
}

impl Refusal for Box<dyn Carried> {
    fn code(&self) -> RefusalCode {
        (**self).code()
    }

    fn side(&self) -> Side {
        (**self).side()
    }

    fn path(&self) -> Option<String> {
        (**self).path()
    }
}

/// The rows of the binary's own coded errors.
#[cfg(test)]
#[must_use]
pub fn catalog() -> Vec<candler_schemas::refusal::CatalogRow> {
    use candler_schemas::refusal::{CatalogRow, Catalogued};

    let tables: [&'static [CatalogRow]; 2] = [
        <CliRefusal as Catalogued>::rows(),
        <CliError as Catalogued>::rows(),
    ];
    tables
        .iter()
        .flat_map(|rows| rows.iter().copied())
        .collect()
}

/// The messages of the causes of `error`, outermost first, without the links that only place
/// the error.
fn causes_of(error: &CliError) -> Vec<String> {
    let mut causes = Vec::new();
    let mut source = match error {
        CliError::Coded { refusal, .. } => refusal.source(),
        CliError::Refused { .. } | CliError::Failed { .. } | CliError::Runtime { .. } => {
            error.source()
        }
    };
    while let Some(cause) = source {
        causes.push(cause.to_string());
        source = cause.source();
    }
    causes
}

/// The refusal an error carries or holds in its chain of causes, if any.
fn refusal_of(error: &CliError) -> Option<&dyn Refusal> {
    match error {
        // The carried refusal may wrap another error: the walk answers with the wrapped error's
        // own side, where the carried error's own `side` is the one its table was written with.
        CliError::Coded { refusal, .. } => {
            let carried: &(dyn Error + 'static) = &**refusal;
            Some(candler_core::refusal::find(carried).unwrap_or(&**refusal))
        }
        CliError::Failed { source, .. } => candler_core::refusal::find(source.as_ref()),
        CliError::Refused { .. } | CliError::Runtime { .. } => None,
    }
}

/// The `refusal/1` record of `error`.
fn record_of(error: &CliError) -> RefusalRecord {
    let message = error.to_string();
    let record = match refusal_of(error) {
        Some(found) => RefusalRecord::new(found.code(), found.side(), message),
        None => RefusalRecord::new(
            RefusalCode::from_checked(INTERNAL_CODE),
            Side::Candler,
            message,
        ),
    };
    let record = record.with_causes(causes_of(error));
    let pointer = match error {
        CliError::Coded { pointer, .. } => pointer
            .clone()
            .or_else(|| refusal_of(error).and_then(Refusal::path)),
        CliError::Failed { .. } => refusal_of(error).and_then(Refusal::path),
        CliError::Refused { .. } | CliError::Runtime { .. } => None,
    };
    if let Some(pointer) = pointer {
        return record.clone().at_path(pointer).unwrap_or(record);
    }
    if let CliError::Coded {
        file: Some(file), ..
    } = error
    {
        return record
            .clone()
            .in_file(file.display().to_string())
            .unwrap_or(record);
    }
    record
}

/// The fallback code of an error that is not an author's.
const INTERNAL_CODE: &str = "candler.internal";

/// What `candler` prints on stderr for `error`: `candler: <message>` as `text`, or one
/// `refusal/1` record on one line as `json`.
#[must_use]
pub fn render(error: &CliError, format: ErrorFormat) -> String {
    match format {
        ErrorFormat::Text => format!("candler: {error}"),
        ErrorFormat::Json => {
            serde_json::to_string(&record_of(error)).unwrap_or_else(|_| format!("candler: {error}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use candler_schemas::refusal::Side;

    use super::catalog;

    /// Guards: R1 — every row of the binary's own refusals equals its row in `codes.toml`: a coded
    /// variant has its row in `[cli]` with the same code, side, enum and variant, a variant with the
    /// fallback code has its `[internal]` row, and every `[cli]` row is raised by a variant.
    ///
    /// Oracle: `docs/contract/codes.toml`, read and never generated from the code.
    ///
    /// Bite: change the side of a variant of `CliRefusal`; add a row to `[cli]` that no variant
    /// raises.
    #[test]
    fn r1_the_binary_rows_equal_codes_toml() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/contract/codes.toml");
        let table = std::fs::read_to_string(path)
            .unwrap()
            .parse::<toml::Table>()
            .unwrap();
        let cli = table["cli"].as_table().unwrap();
        let internal = table["internal"].as_table().unwrap();
        let mut raised = Vec::new();
        for row in catalog() {
            let who = format!("{}::{}", row.enum_name, row.variant);
            if row.delegates {
                assert!(row.code.is_empty(), "{who}: a delegating row owns no code");
                continue;
            }
            if row.code == "candler.internal" {
                assert_eq!(row.side, Side::Candler, "{who}");
                assert!(
                    internal
                        .values()
                        .any(|entry| entry["enum"].as_str() == Some(row.enum_name)
                            && entry
                                .get("variant")
                                .is_none_or(|v| v.as_str() == Some(row.variant))),
                    "{who} has the fallback code and no [internal] row"
                );
                continue;
            }
            let registered = cli
                .get(row.code)
                .unwrap_or_else(|| panic!("{who}: code {} is not in [cli]", row.code));
            assert_eq!(
                registered["side"].as_str(),
                Some(row.side.as_str()),
                "{who}"
            );
            assert_eq!(registered["enum"].as_str(), Some(row.enum_name), "{who}");
            assert_eq!(registered["variant"].as_str(), Some(row.variant), "{who}");
            raised.push(row.code);
        }
        for code in cli.keys() {
            assert!(
                raised.contains(&code.as_str()),
                "[cli] row {code} is raised by no variant of the binary"
            );
        }
    }
}
