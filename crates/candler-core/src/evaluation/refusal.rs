//! The refusal codes of the errors of evaluating stored runs: the plug-in registry, the protocol
//! client, the run context, the request plan, the engine and the equivalence file.
//!
//! One `refusal_table!` call per error type: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other.
//!
//! A variant that only wraps another error is a `delegate` row: it has no code and returns the
//! wrapped error's code and path. Its side in the table is the best single answer for what it
//! wraps, and [`coded`] never gives it away: it hands the walk of causes the wrapped error, so a
//! record carries the wrapped error's own side.

use std::error::Error;

#[cfg(unix)]
use crate::evaluation::client::ClientError;
use crate::evaluation::context::ContextError;
#[cfg(unix)]
use crate::evaluation::engine::{EvaluateError, PlanError};
#[cfg(unix)]
use crate::evaluation::registry::{NeedError, RegistryError};
use crate::evaluation::selection::diff::EquivalenceError;
use candler_schemas::refusal::{CatalogRow, Catalogued, Refusal, Side};
use candler_schemas::refusal_table;

/// The path of an error that names no place in a document.
fn no_path<E>(_error: &E) -> Option<String> {
    None
}

/// The pointer of the place in a document where a [`ContextError`] sits, if it names one.
fn context_error_path(error: &ContextError) -> Option<String> {
    match error {
        ContextError::Store(inner) => Refusal::path(inner),
        ContextError::NotSealed { .. }
        | ContextError::Glob { .. }
        | ContextError::Unsupported { .. }
        | ContextError::InputRefused { .. }
        | ContextError::MissingMetadata { .. }
        | ContextError::Workspace { .. }
        | ContextError::Io { .. }
        | ContextError::CorruptBlob { .. }
        | ContextError::Build(..) => None,
    }
}

/// The pointer of the place in a document where an [`EvaluateError`] sits, if it names one.
#[cfg(unix)]
fn evaluate_error_path(error: &EvaluateError) -> Option<String> {
    match error {
        EvaluateError::Registry(inner) => Refusal::path(inner),
        EvaluateError::Suite { source, .. } => Refusal::path(source),
        EvaluateError::Store(inner) => Refusal::path(inner),
        EvaluateError::Plan(inner) => Refusal::path(inner),
        EvaluateError::Describe { source, .. } => Refusal::path(source),
        EvaluateError::Slots(inner) => Refusal::path(inner),
        EvaluateError::EnvironmentNotReady { .. }
        | EvaluateError::Io { .. }
        | EvaluateError::Glob { .. }
        | EvaluateError::Task(..) => None,
    }
}

/// The pointer of the place in a document where a [`RegistryError`] sits, if it names one.
///
/// A params failure carries the pointer of the refused value inside the entry's `params`, and
/// that pointer is what it returns; a manifest that is not valid returns the schemas' own.
#[cfg(unix)]
fn registry_error_path(error: &RegistryError) -> Option<String> {
    match error {
        RegistryError::Manifest { source, .. } => Refusal::path(source),
        RegistryError::Need { source, .. } => Refusal::path(source),
        RegistryError::Params { pointer, .. } if !pointer.is_empty() => Some(pointer.clone()), // BORROW: the path owns its text
        RegistryError::Params { .. }
        | RegistryError::Read { .. }
        | RegistryError::NoManifest { .. }
        | RegistryError::ManyManifests { .. }
        | RegistryError::DuplicatePlugin { .. }
        | RegistryError::DescribeMismatch { .. }
        | RegistryError::UnknownEvaluator { .. }
        | RegistryError::NotDescribed { .. }
        | RegistryError::VersionOutsideRange { .. }
        | RegistryError::UnknownKind { .. }
        | RegistryError::BadParamsSchema { .. }
        | RegistryError::NoStateHome
        | RegistryError::EnvironmentFile { .. } => None,
    }
}

#[cfg(unix)]
refusal_table! {
    ClientError, path = no_path;
    Spawn (..) => row(
        "evaluation.evaluator_not_started",
        Side::Plugin,
        "The evaluator's command, as the plug-in manifest gives it, cannot be started.",
        "-",
    ),
    Pipes { .. } => row(
        "candler.internal",
        Side::Candler,
        "The evaluator was started without a pipe for stdin, stdout or stderr.",
        "-",
    ),
    Encode (..) => row(
        "candler.internal",
        Side::Candler,
        "A request to the evaluator cannot be written as JSON.",
        "-",
    ),
    Cancelled { .. } => row(
        "evaluation.cancelled",
        Side::Environment,
        "The caller cancelled the request, for example by stopping the job; nothing is committed for it.",
        "-",
    ),
    Describe { .. } => row(
        "plugin.describe_failed",
        Side::Plugin,
        "The evaluator did not answer `describe` with a description, or answered with something that is not one.",
        "-",
    ),
}

refusal_table! {
    ContextError, path = context_error_path;
    Store (inner) => delegate(inner, Side::Environment),
    NotSealed { .. } => row(
        "evaluation.run_not_sealed",
        Side::Environment,
        "A run selected for evaluation has no `run.done` seal, so it can still change.",
        "-",
    ),
    Glob { .. } => row(
        "evaluation.artifact_glob_invalid",
        Side::Plugin,
        "An artifact glob in the needs of a kind is not a glob.",
        "-",
    ),
    Unsupported { .. } => row(
        "evaluation.need_unsupported",
        Side::Plugin,
        "The needs of a kind ask for something this version of Candler does not supply, such as snapshots.",
        "-",
    ),
    InputRefused { .. } => row(
        "evaluation.input_unavailable",
        Side::Plugin,
        "A kind needs an input that the run does not hold, that is reserved for the image, or whose list is not in the store.",
        "-",
    ),
    MissingMetadata { .. } => row(
        "evaluation.metadata_missing",
        Side::Plugin,
        "A kind needs a metadata path that the run manifest does not hold, or holds without a value.",
        "-",
    ),
    Workspace { .. } => row(
        "evaluation.workspace_unusable",
        Side::Config,
        "The workspace directory is relative, not valid UTF-8, inside the store or above it.",
        "-",
    ),
    Io { .. } => row(
        "evaluation.context_io_failed",
        Side::Environment,
        "A file system call failed while the run's files were materialised for an evaluator.",
        "-",
    ),
    CorruptBlob { .. } => row(
        "evaluation.blob_corrupt",
        Side::Environment,
        "A blob of the store does not have the digest the store records for it.",
        "-",
    ),
    Build (..) => row(
        "candler.internal",
        Side::Candler,
        "The context sent to an evaluator cannot be formed from what was materialised.",
        "-",
    ),
}

#[cfg(unix)]
refusal_table! {
    EvaluateError, path = evaluate_error_path;
    EnvironmentNotReady { .. } => row(
        "plugin.environment_not_ready",
        Side::Plugin,
        "The environment of a plug-in the suite names has not been built: its `ready` marker is missing.",
        "-",
    ),
    Registry (inner) => delegate(inner, Side::Plugin),
    Suite { source, .. } => delegate(source, Side::Suite),
    Io { .. } => row(
        "evaluation.io_failed",
        Side::Environment,
        "A file system call of the engine failed, such as reading the suite file.",
        "-",
    ),
    Store (inner) => delegate(inner, Side::Environment),
    Glob { .. } => row(
        "evaluation.run_selection_invalid",
        Side::Config,
        "The selection of runs is not a glob.",
        "-",
    ),
    Plan (inner) => delegate(inner, Side::Suite),
    Describe { source, .. } => delegate(source, Side::Plugin),
    Slots (inner) => delegate(inner, Side::Environment),
    Task (..) => row(
        "candler.internal",
        Side::Candler,
        "A blocking task of the engine panicked or was lost.",
        "-",
    ),
}

#[cfg(unix)]
refusal_table! {
    PlanError, path = no_path;
    Cycle { .. } => row(
        "evaluation.dependency_cycle",
        Side::Suite,
        "Entries of the suite depend on each other in a loop, so no order of requests serves them.",
        "-",
    ),
    UnknownDependency { .. } => row(
        "evaluation.dependency_unknown",
        Side::Suite,
        "An entry of the suite depends on an id that no entry holds.",
        "-",
    ),
    Interleaved { .. } => row(
        "evaluation.evaluators_interleaved",
        Side::Suite,
        "Entries of one evaluator depend on entries of another that depend on them in turn, so no order of requests serves them.",
        "-",
    ),
}

#[cfg(unix)]
refusal_table! {
    RegistryError, path = registry_error_path;
    Read { .. } => row(
        "plugin.manifest_unreadable",
        Side::Plugin,
        "A plug-in manifest file does not exist or cannot be read.",
        "-",
    ),
    Manifest { source, .. } => delegate(source, Side::Plugin),
    NoManifest { .. } => row(
        "config.plugin_dir_no_manifest",
        Side::Config,
        "A directory of `plugins.dirs` does not exist or holds neither `plugin.toml` nor `candler-plugin.toml`.",
        "-",
    ),
    ManyManifests { .. } => row(
        "plugin.manifest_ambiguous",
        Side::Plugin,
        "A plug-in directory holds both `plugin.toml` and `candler-plugin.toml`.",
        "-",
    ),
    DuplicatePlugin { .. } => row(
        "plugin.name_repeated",
        Side::Plugin,
        "Two plug-in manifests have the same name.",
        "-",
    ),
    DescribeMismatch { .. } => row(
        "plugin.describe_mismatch",
        Side::Plugin,
        "A plug-in describes itself with a name or a version that its manifest does not give.",
        "-",
    ),
    UnknownEvaluator { .. } => row(
        "suite.evaluator_unknown",
        Side::Suite,
        "An entry of the suite names an evaluator for which no plug-in manifest was loaded.",
        "-",
    ),
    NotDescribed { .. } => row(
        "candler.internal",
        Side::Candler,
        "An evaluator the suite names has no answer to `describe` when the suite is checked.",
        "-",
    ),
    VersionOutsideRange { .. } => row(
        "suite.evaluator_version_mismatch",
        Side::Suite,
        "A plug-in describes a version outside the range the suite entry asks for.",
        "-",
    ),
    UnknownKind { .. } => row(
        "suite.kind_unknown",
        Side::Suite,
        "An entry of the suite names a kind that its evaluator does not declare.",
        "-",
    ),
    BadParamsSchema { .. } => row(
        "plugin.params_schema_invalid",
        Side::Plugin,
        "The params schema a plug-in declares for a kind does not compile.",
        "-",
    ),
    Params { .. } => row(
        "suite.params_invalid",
        Side::Suite,
        "The params of a suite entry do not satisfy the params schema of its kind.",
        "The JSON pointer, inside the `params` of the entry, of the value the schema refuses; none when the whole of `params` is refused.",
    ),
    NoStateHome { .. } => row(
        "evaluation.state_home_unset",
        Side::Environment,
        "Neither `XDG_STATE_HOME` nor `HOME` is set, so the environments of plug-ins have no home.",
        "-",
    ),
    EnvironmentFile { .. } => row(
        "plugin.environment_file_unreadable",
        Side::Plugin,
        "A lock or project file named by the environment section of a plug-in manifest cannot be read.",
        "-",
    ),
    Need { source, .. } => delegate(source, Side::Plugin),
}

#[cfg(unix)]
refusal_table! {
    NeedError, path = no_path;
    Unsupported { .. } => row(
        "plugin.need_unsupported",
        Side::Plugin,
        "A kind declares a need that this version of Candler does not supply.",
        "-",
    ),
    ReservedInput { .. } => row(
        "plugin.need_input_reserved",
        Side::Plugin,
        "A kind needs the input kind that is reserved for the image.",
        "-",
    ),
    OutsideOutputs { .. } => row(
        "plugin.need_glob_outside_outputs",
        Side::Plugin,
        "An artifact glob in the needs of a kind is not under `outputs/`.",
        "-",
    ),
    BadGlob { .. } => row(
        "plugin.need_glob_invalid",
        Side::Plugin,
        "An artifact glob in the needs of a kind is not a valid glob.",
        "-",
    ),
}

refusal_table! {
    EquivalenceError, path = no_path;
    Io { .. } => row(
        "suite.equivalence_unreadable",
        Side::Suite,
        "The equivalence file does not exist or cannot be read.",
        "-",
    ),
    Parse { .. } => row(
        "suite.equivalence_invalid",
        Side::Suite,
        "The equivalence file is not a list of rows of a run and an expectation.",
        "-",
    ),
    Run { .. } => row(
        "suite.equivalence_run_invalid",
        Side::Suite,
        "A row of the equivalence file names a run that is not a run key.",
        "-",
    ),
    DuplicateRow { .. } => row(
        "suite.equivalence_row_repeated",
        Side::Suite,
        "The equivalence file lists one row of a run and an expectation twice.",
        "-",
    ),
}

/// The rows of every coded error of this module, in the order of the types.
#[must_use]
pub(crate) fn catalog() -> Vec<CatalogRow> {
    let mut tables: Vec<&'static [CatalogRow]> = Vec::new();
    #[cfg(unix)]
    tables.push(<ClientError as Catalogued>::rows());
    tables.push(<ContextError as Catalogued>::rows());
    #[cfg(unix)]
    {
        tables.push(<EvaluateError as Catalogued>::rows());
        tables.push(<PlanError as Catalogued>::rows());
        tables.push(<RegistryError as Catalogued>::rows());
        tables.push(<NeedError as Catalogued>::rows());
    }
    tables.push(<EquivalenceError as Catalogued>::rows());
    tables
        .iter()
        .flat_map(|rows| rows.iter().copied())
        .collect()
}

/// Views a coded error as its [`Refusal`].
fn as_refusal<'e, E: Refusal>(found: &'e E) -> &'e dyn Refusal {
    found
}

/// The refusal that answers for a variant of `wrapper` that only wraps `inner`: what the walk of
/// causes finds from `inner`, so that its side is the wrapped error's own, or `wrapper` when the
/// walk finds nothing.
fn through<'e, E: Error + 'static>(inner: &'e E, wrapper: &'e dyn Refusal) -> &'e dyn Refusal {
    crate::refusal::find(inner).unwrap_or(wrapper)
}

/// The refusal that answers for a [`ContextError`].
fn context_answer(error: &ContextError) -> &dyn Refusal {
    match error {
        ContextError::Store(inner) => through(inner, error),
        ContextError::NotSealed { .. }
        | ContextError::Glob { .. }
        | ContextError::Unsupported { .. }
        | ContextError::InputRefused { .. }
        | ContextError::MissingMetadata { .. }
        | ContextError::Workspace { .. }
        | ContextError::Io { .. }
        | ContextError::CorruptBlob { .. }
        | ContextError::Build(..) => error,
    }
}

/// The refusal that answers for an [`EvaluateError`].
#[cfg(unix)]
fn evaluate_answer(error: &EvaluateError) -> &dyn Refusal {
    match error {
        EvaluateError::Registry(inner) => through(inner, error),
        EvaluateError::Suite { source, .. } => through(source, error),
        EvaluateError::Store(inner) => through(inner, error),
        EvaluateError::Plan(inner) => through(inner, error),
        EvaluateError::Describe { source, .. } => through(source, error),
        EvaluateError::Slots(inner) => through(inner, error),
        EvaluateError::EnvironmentNotReady { .. }
        | EvaluateError::Io { .. }
        | EvaluateError::Glob { .. }
        | EvaluateError::Task(..) => error,
    }
}

/// The refusal that answers for a [`RegistryError`].
#[cfg(unix)]
fn registry_answer(error: &RegistryError) -> &dyn Refusal {
    match error {
        RegistryError::Manifest { source, .. } => through(source, error),
        RegistryError::Need { source, .. } => through(source, error),
        RegistryError::Read { .. }
        | RegistryError::NoManifest { .. }
        | RegistryError::ManyManifests { .. }
        | RegistryError::DuplicatePlugin { .. }
        | RegistryError::DescribeMismatch { .. }
        | RegistryError::UnknownEvaluator { .. }
        | RegistryError::NotDescribed { .. }
        | RegistryError::VersionOutsideRange { .. }
        | RegistryError::UnknownKind { .. }
        | RegistryError::BadParamsSchema { .. }
        | RegistryError::Params { .. }
        | RegistryError::NoStateHome
        | RegistryError::EnvironmentFile { .. } => error,
    }
}

/// The coded error of this module that `error` is, or `None` when it is none of them.
///
/// A variant that only wraps another coded error answers with the wrapped error's refusal, so
/// the side of a record is the one of the error that arose and not of the wrapper.
#[must_use]
pub(crate) fn coded<'e>(error: &'e (dyn Error + 'static)) -> Option<&'e dyn Refusal> {
    #[cfg(unix)]
    {
        if let Some(found) = error.downcast_ref::<EvaluateError>() {
            return Some(evaluate_answer(found));
        }
        if let Some(found) = error.downcast_ref::<RegistryError>() {
            return Some(registry_answer(found));
        }
        if let Some(found) = error.downcast_ref::<ClientError>() {
            return Some(as_refusal(found));
        }
        if let Some(found) = error.downcast_ref::<PlanError>() {
            return Some(as_refusal(found));
        }
        if let Some(found) = error.downcast_ref::<NeedError>() {
            return Some(as_refusal(found));
        }
    }
    if let Some(found) = error.downcast_ref::<ContextError>() {
        return Some(context_answer(found));
    }
    error.downcast_ref::<EquivalenceError>().map(as_refusal)
}
