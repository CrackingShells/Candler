//! The records the listing and comparison verbs print on stdout: the job list of `ps`, the
//! experiment listing of `ls`, the report of `diff` and the per-cell rates of `show --rates`.
//!
//! Each is a standalone document whose `schema` field holds its version marker. The CLI builds
//! one from what the core computed and prints it as JSON, or renders its text form from it.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::generate::GeneratedSchema;
use crate::primitives::schema_tag;

/// The version marker of the job list.
pub const JOB_LIST_SCHEMA: &str = "job-list/1";

/// The version marker of the experiment listing.
pub const EXPERIMENT_LIST_SCHEMA: &str = "experiment-list/1";

/// The version marker of the diff report.
pub const DIFF_REPORT_SCHEMA: &str = "diff-report/1";

/// The version marker of the rates.
pub const RATES_SCHEMA: &str = "rates/1";

/// The version marker of the `show` counts.
pub const SHOW_COUNTS_SCHEMA: &str = "show-counts/1";

/// The version marker of the ingest report.
pub const INGEST_REPORT_SCHEMA: &str = "ingest-report/1";

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    vec![
        GeneratedSchema::of::<JobList>("job-list"),
        GeneratedSchema::of::<ExperimentList>("experiment-list"),
        GeneratedSchema::of::<DiffReport>("diff-report"),
        GeneratedSchema::of::<Rates>("rates"),
        GeneratedSchema::of::<ShowCounts>("show-counts"),
        GeneratedSchema::of::<IngestReport>("ingest-report"),
    ]
}

schema_tag! {
    /// The constant `schema` field of the job list.
    pub JobListTag = JOB_LIST_SCHEMA
}

schema_tag! {
    /// The constant `schema` field of the experiment listing.
    pub ExperimentListTag = EXPERIMENT_LIST_SCHEMA
}

schema_tag! {
    /// The constant `schema` field of the diff report.
    pub DiffReportTag = DIFF_REPORT_SCHEMA
}

schema_tag! {
    /// The constant `schema` field of the `show` counts.
    pub ShowCountsTag = SHOW_COUNTS_SCHEMA
}

schema_tag! {
    /// The constant `schema` field of the ingest report.
    pub IngestReportTag = INGEST_REPORT_SCHEMA
}

schema_tag! {
    /// The constant `schema` field of the rates.
    pub RatesTag = RATES_SCHEMA
}

/// The jobs of a store and the state of each: what `candler ps` prints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JobList {
    /// The version marker of the document: always `job-list/1`.
    pub schema: JobListTag,
    /// The root of the store the jobs were read from.
    pub store: String,
    /// One row per job directory, in the order of their names.
    pub jobs: Vec<JobRow>,
}

impl JobList {
    /// A list of `jobs` read from the store at `store`.
    #[must_use]
    pub const fn new(store: String, jobs: Vec<JobRow>) -> Self {
        Self {
            schema: JobListTag,
            store,
            jobs,
        }
    }
}

/// One job of a [`JobList`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JobRow {
    /// The job id, or the directory name when the directory cannot be read.
    pub id: String,
    /// The digest of the job's spec, or `-` when it cannot be read.
    pub spec: String,
    /// `running`, `starting`, `ended (<reason>)`, `unreadable <path>: <detail>` or a `lost: ...`
    /// line.
    pub state: String,
    /// `<runs ended>/<runs started>`, `?` when the event log cannot be read.
    pub progress: String,
    /// What the job runs: `experiment:<12 hex of the experiment digest>`, `suite:<file name>`, or
    /// `-` when the job has no readable spec file.
    pub what: String,
    /// Whether the row reports a failure.
    pub failed: bool,
}

/// The experiment files a project holds and the stored runs each matches: what `candler ls`
/// prints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExperimentList {
    /// The version marker of the document: always `experiment-list/1`.
    pub schema: ExperimentListTag,
    /// The root of the store the runs were counted in; `null` when the root cannot be resolved.
    pub store: Option<String>,
    /// One entry per file found, in the order of their paths.
    pub experiments: Vec<ListedExperiment>,
}

impl ExperimentList {
    /// A listing of `experiments` whose runs were counted in the store at `store`.
    #[must_use]
    pub const fn new(store: Option<String>, experiments: Vec<ListedExperiment>) -> Self {
        Self {
            schema: ExperimentListTag,
            store,
            experiments,
        }
    }
}

/// One file of an [`ExperimentList`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum ListedExperiment {
    /// A file that reads as an experiment.
    Experiment {
        /// The file.
        path: String,
        /// How many cells it declares.
        cells: u64,
        /// How many sealed stored runs match it.
        runs: u64,
    },
    /// A file that is no experiment, or could not be read.
    Skipped {
        /// The file.
        path: String,
        /// Why it was skipped.
        reason: String,
    },
}

/// The comparison of two evaluator versions over the stored outcomes: what `candler diff` prints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiffReport {
    /// The version marker of the document: always `diff-report/1`.
    pub schema: DiffReportTag,
    /// How many expectations were compared.
    pub compared: u64,
    /// How many of them are unchanged.
    pub unchanged: u64,
    /// How many changes the equivalence file excuses.
    pub excused: u64,
    /// How many changes are not excused.
    pub changed: u64,
    /// Each change that is not excused.
    pub changes: Vec<DiffChange>,
    /// The expectations of one side that the other lacks, with how many rows each.
    pub uncompared: Vec<DiffUncompared>,
    /// The equivalence file the comparison was made with, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equivalence: Option<DiffEquivalence>,
}

/// One change of a [`DiffReport`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiffChange {
    /// The class of the change, as the core names it.
    pub class: String,
    /// The run the change is in.
    pub run: String,
    /// The expectation that changed.
    pub expectation: String,
    /// The four fields of the base side, `null` when that side has no row.
    pub from: Value,
    /// The four fields of the compared side, `null` when that side has no row.
    pub to: Value,
}

/// An expectation of a [`DiffReport`] that only one side holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiffUncompared {
    /// The expectation.
    pub expectation: String,
    /// How many rows it has.
    pub count: u64,
}

/// The equivalence file a [`DiffReport`] was made with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiffEquivalence {
    /// The digest of the file.
    pub sha256: String,
    /// How many rows it names.
    pub rows: u64,
}

/// The per-cell rates of one evaluator: what `candler show --rates` prints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rates {
    /// The version marker of the document: always `rates/1`.
    pub schema: RatesTag,
    /// The gate and quality rates of each cell, by the cell's digest.
    pub cell: BTreeMap<String, CellRate>,
}

impl Rates {
    /// The rates of the cells in `cell`.
    #[must_use]
    pub const fn new(cell: BTreeMap<String, CellRate>) -> Self {
        Self {
            schema: RatesTag,
            cell,
        }
    }
}

/// The rates of one cell in [`Rates`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CellRate {
    /// The rate of the gate expectations, as the core computed it.
    pub gate: Value,
    /// The rate of the quality expectations, as the core computed it.
    pub quality: Value,
}

/// How many current outcomes each evaluator has with each status: what `candler show` prints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShowCounts {
    /// The version marker of the document: always `show-counts/1`.
    pub schema: ShowCountsTag,
    /// The counts by evaluator, then by status.
    pub counts: BTreeMap<String, BTreeMap<String, u64>>,
}

/// What one ingested path added to the store: what `candler ingest` prints, one document per
/// results folder, or one for all the raw trace directories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IngestReport {
    /// The version marker of the document: always `ingest-report/1`.
    pub schema: IngestReportTag,
    /// How many runs the ingest holds.
    pub runs: u64,
    /// How many outcome records a results folder held; set for a results folder only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub records: Option<u64>,
    /// How many runs the ingest created; set for raw trace directories only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<u64>,
    /// How many runs the store already held; set for raw trace directories only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub existing: Option<u64>,
    /// The entries left out and why; set for raw trace directories only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<Vec<IngestSkipped>>,
}

/// An entry an ingest left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IngestSkipped {
    /// The entry.
    pub path: String,
    /// Why it was left out.
    pub reason: String,
}

/// What a plan holds: the cells, the runs still to start and the runs already sealed, with the
/// caps that bound them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanCounts {
    /// The cells of the experiment.
    pub cells: u64,
    /// The runs the plan starts.
    pub new_runs: u64,
    /// The runs the store already holds sealed.
    pub reused_runs: u64,
    /// The new runs times the per-run wall cap in seconds, when the product fits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_s_bound: Option<u64>,
    /// The job caps the experiment file sets, by path.
    pub job_caps: BTreeMap<String, Value>,
}
