//! The records `candler inspect` prints on stdout: one run with its outcomes, and one job with
//! the runs that name it.
//!
//! Each is a standalone document whose `schema` field holds its version marker. Values the
//! manifest records in a shape of their own, such as the integrity verdict, the host and the
//! import, are carried as they are stored.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::generate::GeneratedSchema;
use crate::primitives::schema_tag;

/// The version marker of the run record.
pub const INSPECT_RUN_SCHEMA: &str = "inspect-run/1";

/// The version marker of the job record.
pub const INSPECT_JOB_SCHEMA: &str = "inspect-job/1";

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    vec![
        GeneratedSchema::of::<InspectRun>("inspect-run"),
        GeneratedSchema::of::<InspectJob>("inspect-job"),
    ]
}

schema_tag! {
    /// The constant `schema` field of the run record.
    pub InspectRunTag = INSPECT_RUN_SCHEMA
}

schema_tag! {
    /// The constant `schema` field of the job record.
    pub InspectJobTag = INSPECT_JOB_SCHEMA
}

/// One run: whether it is sealed, what it was read from, the provenance and the other values it
/// measured, and its outcomes with their evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectRun {
    /// The version marker of the document: always `inspect-run/1`.
    pub schema: InspectRunTag,
    /// The run key.
    pub run: String,
    /// Whether the run is sealed.
    pub sealed: bool,
    /// The integrity verdict of the manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<Value>,
    /// The digest of the experiment the run belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experiment: Option<Value>,
    /// The job that made the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<Value>,
    /// When the run started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<Value>,
    /// When the run ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<Value>,
    /// The host the run was made on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<Value>,
    /// What the run was imported from, for an imported run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import: Option<Value>,
    /// The tool catalog the run ran with, as the run stored it: the canonical form of the
    /// snapshot without `captured_at`. Absent when the run has no stored snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog: Option<Value>,
    /// The measured values under `provenance.`, by path.
    pub provenance: BTreeMap<String, Value>,
    /// The other measured values, by path.
    pub measured: BTreeMap<String, Value>,
    /// The outcomes of the run, ordered by evaluator, then expectation, then newest request.
    pub outcomes: Vec<InspectOutcome>,
}

/// One outcome of an [`InspectRun`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectOutcome {
    /// The evaluator that graded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluator: Option<Value>,
    /// The expectation graded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expectation: Option<Value>,
    /// The role of the expectation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Value>,
    /// The status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Value>,
    /// The measured value, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// The weight of the outcome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<Value>,
    /// The cause of a status that is not a verdict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<Value>,
    /// The evidence the grading kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Value>,
    /// The request that graded it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<Value>,
}

/// One job: its state, the runs whose manifest names it and what it spent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectJob {
    /// The version marker of the document: always `inspect-job/1`.
    pub schema: InspectJobTag,
    /// The job id.
    pub id: String,
    /// The state of the job, worded as `candler ps` words it.
    pub state: String,
    /// The runs whose manifest names the job.
    pub runs: Vec<InspectJobRun>,
    /// What the sealed runs of the job cost.
    pub spent: Spent,
}

/// One run of an [`InspectJob`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectJobRun {
    /// The run key.
    pub key: String,
    /// The integrity verdict of the manifest.
    pub integrity: Value,
    /// How many current outcomes the run has with each status.
    pub outcomes: BTreeMap<String, u64>,
}

/// What a set of runs cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Spent {
    /// The cost in US dollars of the runs that report one.
    pub cost_usd: f64,
    /// How many runs report a cost.
    pub runs_counted: u64,
    /// How many runs report no total.
    pub runs_without_total: u64,
}
