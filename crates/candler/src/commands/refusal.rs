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
    /// `request.harness` names a harness that this binary does not ship.
    #[error("`{name}` is not a harness this binary ships: the registered ones are {registered}")]
    UnknownHarness {
        /// The name the request gave.
        name: String,
        /// The registered names, joined.
        registered: String,
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
    /// An import rules file that is not the rules format: not TOML, a field missing, or a field
    /// the format does not have.
    #[error("{detail}")]
    RulesInvalid {
        /// What the TOML parser reported.
        detail: String,
    },
    /// `evaluate` finds no plug-in directory under `plugins.dirs`: there is no `candler.toml`, or
    /// its list is empty.
    #[error(
        "no plug-in directory: list one under `plugins.dirs` in a `candler.toml` at or above the working directory"
    )]
    NoPluginDirs,
    /// `slots.evaluators` is 0, so no evaluator could ever run.
    #[error("no evaluator slot to run in: slots.evaluators is 0")]
    NoEvaluatorSlots,
    /// The experiment file `run` or `resolve` was given cannot be read.
    #[error("cannot read {path}: {detail}")]
    ExperimentUnreadable {
        /// The path as the caller gave it.
        path: String,
        /// What the operating system reported.
        detail: String,
    },
    /// `kill` asked for a confirmation and was not given one.
    #[error("pass -y to confirm")]
    KillNotConfirmed,
    /// `inspect` was given a target that is none of its three forms.
    #[error("`{target}` is none of: {}", super::inspect::FORMS)]
    InspectTargetInvalid {
        /// The target as the caller gave it.
        target: String,
    },
    /// No run of the store starts with the prefix `inspect` was given.
    #[error("no run of the store starts with `{target}`")]
    NoSuchRun {
        /// The prefix as the caller gave it.
        target: String,
    },
    /// Several runs of the store start with the prefix `inspect` was given.
    #[error("`{target}` starts {count} runs: {listed}{tail}; give more of the key")]
    AmbiguousRun {
        /// The prefix as the caller gave it.
        target: String,
        /// How many runs start with it.
        count: usize,
        /// The first of their keys, joined.
        listed: String,
        /// How many more there are, as words, or nothing.
        tail: String,
    },
    /// The directory `inspect` was given is not a run directory of a store.
    #[error("`{path}`: {detail}")]
    RunDirectoryInvalid {
        /// The path as the caller gave it.
        path: String,
        /// What is wrong with it.
        detail: String,
    },
    /// An evaluator option holds text that is not `name@version`.
    #[error("{flag} `{text}`: {detail}")]
    EvaluatorInvalid {
        /// The option.
        flag: String,
        /// The text as the caller gave it.
        text: String,
        /// Why it does not parse.
        detail: String,
    },
    /// `diff` found no expectation graded by both evaluators.
    #[error("nothing compared between {from} and {to}")]
    NothingCompared {
        /// The base evaluator.
        from: String,
        /// The compared evaluator.
        to: String,
    },
    /// The command line is not one the parser accepts.
    #[error("{message}")]
    UsageInvalid {
        /// The parser's own text.
        message: String,
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
        | CliRefusal::UnknownRequestKey { .. }
        | CliRefusal::UnknownHarness { .. }
        | CliRefusal::RulesInvalid { .. }
        | CliRefusal::NoPluginDirs
        | CliRefusal::NoEvaluatorSlots
        | CliRefusal::ExperimentUnreadable { .. }
        | CliRefusal::KillNotConfirmed
        | CliRefusal::InspectTargetInvalid { .. }
        | CliRefusal::NoSuchRun { .. }
        | CliRefusal::AmbiguousRun { .. }
        | CliRefusal::RunDirectoryInvalid { .. }
        | CliRefusal::EvaluatorInvalid { .. }
        | CliRefusal::NothingCompared { .. }
        | CliRefusal::UsageInvalid { .. } => None,
    }
}

refusal_table! {
    CliRefusal, path = cli_refusal_path;
    NoStore { .. } => row(
        "config.no_store",
        Side::Config,
        "Not raised today: a verb that is given no store root takes the default store, and a store that is missing prints `store.not_a_store`.",
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
    UnknownHarness { .. } => row(
        "experiment.unknown_harness",
        Side::Experiment,
        "`request.harness`, written as `name` under `[harness]` or given to `--set`, names a harness that is not registered in this binary; the refusal lists the registered names.",
        "-",
    ),
    UnknownFieldPath { .. } => row(
        "experiment.unknown_path",
        Side::Experiment,
        "A dotted path under `[fixed]`, in a factor or in `--set` is none that the experiment surface has, such as a misspelt `model.idd`.",
        "The key under `/fixed`, the `path` of the factor, or the key of the level's table that holds the path; none for `--set`.",
    ),
    RulesInvalid { .. } => row(
        "config.import_rules_invalid",
        Side::Config,
        "The import rules file given to `ingest --rules` is not the rules format: it is not TOML, a field is missing, or a field is not one the format has.",
        "-",
    ),
    NoPluginDirs { .. } => row(
        "config.no_plugin_dirs",
        Side::Config,
        "`evaluate` is run where no `candler.toml` at or above the working directory lists a plug-in directory under `plugins.dirs`.",
        "-",
    ),
    NoEvaluatorSlots { .. } => row(
        "config.no_evaluator_slots",
        Side::Config,
        "`slots.evaluators` is 0 in the user configuration, so the pool that evaluators draw from has no slot.",
        "-",
    ),
    ExperimentUnreadable { .. } => row(
        "config.experiment_unreadable",
        Side::Config,
        "The experiment file given to `run` or `resolve` does not exist or cannot be read.",
        "-",
    ),
    KillNotConfirmed { .. } => row(
        "config.kill_not_confirmed",
        Side::Config,
        "`kill` asked for a confirmation and was not answered `y` or `yes`; `-y` skips the question.",
        "-",
    ),
    InspectTargetInvalid { .. } => row(
        "config.inspect_target_invalid",
        Side::Config,
        "The target given to `inspect` is none of a run directory, a prefix of a run key of at least 6 characters, or a job id.",
        "-",
    ),
    NoSuchRun { .. } => row(
        "config.no_such_run",
        Side::Config,
        "No run of the store has a key that starts with the prefix given to `inspect`.",
        "-",
    ),
    AmbiguousRun { .. } => row(
        "config.ambiguous_run",
        Side::Config,
        "More than one run of the store has a key that starts with the prefix given to `inspect`.",
        "-",
    ),
    RunDirectoryInvalid { .. } => row(
        "config.run_directory_invalid",
        Side::Config,
        "The directory given to `inspect` holds a `manifest.json` but is not named by a run key, or is not below the `runs` directory of a store.",
        "-",
    ),
    EvaluatorInvalid { .. } => row(
        "config.evaluator_invalid",
        Side::Config,
        "`--evaluator`, `--from` or `--to` holds text that is not `name@version` with a SemVer 2.0 version.",
        "-",
    ),
    NothingCompared { .. } => row(
        "config.nothing_compared",
        Side::Config,
        "`diff` found no expectation that both evaluators graded in the same run, such as an evaluator version the store lacks.",
        "-",
    ),
    UsageInvalid { .. } => row(
        "cli.usage_invalid",
        Side::Config,
        "The command line is not one the parser accepts: an unknown option, a missing argument, or a value such as a `--set` without `=`.",
        "-",
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
    // A failed evaluate job's own record crossed the process boundary as a file: it is printed as
    // written, with the message and the causes of the place that raised it.
    if let CliError::Failed { source, .. } = error
        && let Some(job) = source.downcast_ref::<super::evaluate::JobRefusal>()
    {
        return job.record().clone(); // BORROW: the caller owns the record it prints
    }
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

/// The `refusal/1` record, on one line, of a command line the parser refused, with the parser's own
/// text as the message.
#[must_use]
pub fn usage_record(message: &str) -> String {
    let refusal = CliRefusal::UsageInvalid {
        message: message.to_owned(), // BORROW: the refusal owns the text
    };
    let record = RefusalRecord::new(
        Refusal::code(&refusal),
        Refusal::side(&refusal),
        message.to_owned(), // BORROW: the record owns the text
    );
    serde_json::to_string(&record).unwrap_or_else(|_| format!("candler: {message}"))
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
        let table = include_str!("../../../../docs/contract/codes.toml")
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
