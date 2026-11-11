//! The refusals of `analyze`: the requests it cannot answer, each with a stable code.

use candler_schemas::refusal::Side;
use candler_schemas::refusal_table;

/// Why a report is not built: the caller asked for something the store cannot answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ReportError {
    /// The store holds the outcomes of several evaluator versions and none was named.
    #[error("the store holds outcomes of several evaluators ({}): name one", .found.join(", "))]
    MixedEvaluatorVersions {
        /// The evaluators the outcomes name, each as `name@version`.
        found: Vec<String>,
    },
    /// The evaluator asked for has no outcome in the store.
    #[error("no outcome of `{requested}` in the store; it holds: {}", .found.join(", "))]
    UnknownEvaluator {
        /// What was asked for.
        requested: String,
        /// The evaluators the outcomes name, each as `name@version`.
        found: Vec<String>,
    },
    /// A bare version is held by the outcomes of several evaluators.
    #[error("version {version} is held by several evaluators ({}): name one as `name@version`", .names.join(", "))]
    AmbiguousEvaluatorName {
        /// The version asked for.
        version: String,
        /// The evaluators that hold it, each as `name@version`.
        names: Vec<String>,
    },
    /// The experiment file named cannot be read as an experiment.
    #[error("experiment file `{file}` is unreadable: {detail}")]
    ExperimentUnreadable {
        /// The file.
        file: String,
        /// What is wrong with it.
        detail: String,
    },
    /// The store holds no outcome to analyse.
    #[error("the store holds no outcome to analyse")]
    NoOutcomes,
}

/// The place of a refusal in a parsed document: none of these sits in one.
const fn report_path(_error: &ReportError) -> Option<String> {
    None
}

refusal_table! {
    ReportError, path = report_path;
    MixedEvaluatorVersions { .. } => row(
        "analysis.mixed_evaluator_versions",
        Side::Config,
        "The store holds outcomes of several evaluator versions and the request names none; name one with `--evaluator`.",
        "-",
    ),
    UnknownEvaluator { .. } => row(
        "analysis.unknown_evaluator",
        Side::Config,
        "The evaluator, or the version, named has no outcome in the store.",
        "-",
    ),
    AmbiguousEvaluatorName { .. } => row(
        "analysis.ambiguous_evaluator_name",
        Side::Config,
        "A bare version is held by several evaluators; write `name@version`.",
        "-",
    ),
    ExperimentUnreadable { .. } => row(
        "analysis.experiment_unreadable",
        Side::Experiment,
        "The file given with `--experiment` cannot be read as an experiment file.",
        "-",
    ),
    NoOutcomes { .. } => row(
        "analysis.no_outcomes",
        Side::Environment,
        "The store holds no outcome record, so there is nothing to analyse; run `evaluate` first.",
        "-",
    ),
}
