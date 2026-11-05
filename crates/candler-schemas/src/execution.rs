//! The experiment file, the run manifest and its cell digest, the job record and the event envelope.
//!
//! An [`ExperimentFile`] is the TOML document that declares fixed values, factors with their
//! levels, replicates and a budget; its cells are a pure function of the file. An optional field
//! is omitted when absent and never written as `null`; the reader refuses an explicit `null`.
//! Unknown fields are ignored on input, and every struct and every tagged enum is read from an
//! object only, never from a list.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::ser::SerializeMap as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Number, Value};

use crate::canonical::{self, CanonicalError};
use crate::generate::GeneratedSchema;
use crate::map_only::MapOnly;
use crate::outcome::OutcomeRecord;
use crate::primitives::{
    Digest, FieldPath, FormatError, JobId, Name, NameToken, NameVersion, ReasonCode, RelPath,
    RequestId, RunKey, SEMVER_PATTERN, SecretName, Timestamp, object_only, present, schema_tag,
    text_type,
};
use crate::process::{ChildRecord, ExitStatus, ProcessStart};
use crate::trace::EndedBy;
use crate::values::{JsonValue, MeasuredValue, ResolvedValue, SourceEntry, ValuesError};

/// The version marker of an experiment file, written in its `schema` field.
pub const EXPERIMENT_SCHEMA: &str = "experiment/1";

/// The version marker of a run manifest, written in its `schema` field. A change to the digest an
/// existing manifest yields requires a new marker.
pub const RUN_MANIFEST_SCHEMA: &str = "run-manifest/1";

/// The version marker of a run tree, written in its `schema` field.
pub const RUN_TREE_SCHEMA: &str = "run-tree/1";

/// The version marker of a job record, written in its `schema` field.
pub const JOB_RECORD_SCHEMA: &str = "job-record/1";

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    Vec::new()
}

/// Why an execution record could not be built or read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ExecutionError {
    /// A [`GitCommit`] that is not 40 or 64 lowercase hex digits.
    #[error("git commit {problem} (got {value:?})")]
    GitCommit {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// A string format of the shared primitives that does not hold.
    #[error(transparent)]
    Format(#[from] FormatError),
    /// A value that cannot be hashed.
    #[error(transparent)]
    Values(#[from] ValuesError),
    /// A value that has no canonical text.
    #[error(transparent)]
    Canonical(#[from] CanonicalError),
    /// A document that is not TOML.
    #[error("document is not TOML ({detail})")]
    Toml {
        /// The reader's explanation.
        detail: String,
    },
    /// A TOML value that JSON cannot hold: a date or time, or a number that is not finite.
    #[error("document holds a value JSON cannot state at `{path}` ({problem})")]
    Unrepresentable {
        /// The dotted path of the value in the document.
        path: String,
        /// What is wrong with the value.
        problem: &'static str,
    },
    /// A document that is TOML but not an experiment file.
    #[error("document is not an experiment file ({detail})")]
    Document {
        /// The reader's explanation.
        detail: String,
    },
    /// A builder that was asked to build before a required field was set.
    #[error("execution record field is missing (`{field}` was not set)")]
    MissingField {
        /// The name of the field.
        field: &'static str,
    },
    /// A count, duration or process id that must be at least 1 and is 0.
    #[error("{field} is 0 (it counts from 1)")]
    Zero {
        /// The name of the field.
        field: &'static str,
    },
    /// A cost that is not a finite number above 0.
    #[error("{field} is not a finite number above 0")]
    NotPositive {
        /// The name of the field.
        field: &'static str,
    },
    /// A factor with no level, which would leave the experiment with no cell.
    #[error("factor `{factor}` has no level")]
    NoLevels {
        /// The path of the factor.
        factor: FieldPath,
    },
    /// A table that holds no value, where a level or a fixed table fills paths from its leaves.
    #[error("table at `{path}` holds no value")]
    EmptyTable {
        /// The path the table would fill.
        path: String,
    },
    /// Two paths of one experiment where one is the other or lies below it.
    #[error("paths `{first}` and `{second}` overlap: one is the other or lies below it")]
    PathOverlap {
        /// The first path.
        first: FieldPath,
        /// The second path.
        second: FieldPath,
    },
    /// A list that is not in the order its owner requires.
    #[error("{list} is not sorted by {key}")]
    Unsorted {
        /// The name of the list.
        list: &'static str,
        /// What the list is sorted by.
        key: &'static str,
    },
    /// A list that holds the same entry twice.
    #[error("{list} repeats an entry (got {value:?})")]
    Duplicate {
        /// The name of the list.
        list: &'static str,
        /// The repeated entry, by its key.
        value: String,
    },
    /// A resolved value at a reserved root that is not the root alone, or has no value.
    #[error("reserved root `{path}` {problem}")]
    ReservedRoot {
        /// The path of the value.
        path: String,
        /// What is wrong with it.
        problem: &'static str,
    },
    /// A value at a reserved root that is not the shape the root takes.
    #[error("value of reserved root `{root}` is malformed ({detail})")]
    ReservedValue {
        /// The reserved root.
        root: &'static str,
        /// The reader's explanation.
        detail: String,
    },
    /// A value that holds a string that is an absolute path, at any depth: a file enters a cell as an input digest.
    #[error("value at `{path}` holds an absolute path (a file enters a cell as an input digest)")]
    AbsolutePath {
        /// The path of the value.
        path: String,
    },
    /// A `sources` list that does not cover the values and the typed roots, or names a path that is neither.
    #[error("source of `{path}` {problem}")]
    SourcesMismatch {
        /// The path concerned.
        path: String,
        /// What is wrong with it.
        problem: &'static str,
    },
    /// An event of a kind this version knows, built as an unknown one: it would not read back as such.
    #[error("event kind `{kind}` is a kind this version knows")]
    KnownKind {
        /// The kind.
        kind: String,
    },
    /// A tree entry below a path that holds a file.
    #[error("tree entry `{path}` lies below the file `{file}`")]
    PathUnderFile {
        /// The entry that cannot exist.
        path: String,
        /// The file that is in its way.
        file: String,
    },
    /// A `dir` entry with an entry below it: only a directory that holds nothing is listed.
    #[error("tree directory `{dir}` is listed as empty but holds `{path}`")]
    DirNotEmpty {
        /// The directory listed as empty.
        dir: String,
        /// The entry below it.
        path: String,
    },
}

const GIT_COMMIT_PATTERN: &str = r"^(?:[0-9a-f]{40}|[0-9a-f]{64})(?![\s\S])";

text_type!(GitCommit {
    description: "A git commit: 40 or 64 lowercase hex digits.",
    pattern: GIT_COMMIT_PATTERN,
});

impl FromStr for GitCommit {
    type Err = ExecutionError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let hex = matches!(text.len(), 40 | 64)
            && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if hex {
            // BORROW: .to_owned() makes the owned text the value keeps
            Ok(Self {
                text: text.to_owned(),
            })
        } else {
            // BORROW: .to_owned() makes the owned text the error keeps
            Err(ExecutionError::GitCommit {
                problem: "is not 40 or 64 lowercase hex digits",
                value: text.to_owned(),
            })
        }
    }
}

/// A `SemVer` version written as text, for fields that hold a [`semver::Version`].
mod version_text {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(
        version: &semver::Version,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(version)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<semver::Version, D::Error> {
        let text = String::deserialize(deserializer)?;
        // BORROW: explicit .as_str() instead of Deref coercion
        semver::Version::parse(text.as_str()).map_err(serde::de::Error::custom)
    }
}

/// The schema of a [`semver::Version`] field: `SemVer` 2.0 text.
struct VersionText;

impl JsonSchema for VersionText {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "VersionText".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "A SemVer 2.0 version.",
            "pattern": SEMVER_PATTERN,
        })
    }
}

/// A `SemVer` version requirement written as text, for optional fields that hold a
/// [`semver::VersionReq`]; an explicit `null` is refused.
mod version_req_text {
    use serde::{Deserialize, Deserializer, Serializer};

    #[allow(clippy::ref_option)] // serde's `with` passes the field by reference
    pub(super) fn serialize<S: Serializer>(
        requirement: &Option<semver::VersionReq>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match requirement {
            Some(requirement) => serializer.collect_str(requirement),
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<semver::VersionReq>, D::Error> {
        match Option::<String>::deserialize(deserializer)? {
            // BORROW: explicit .as_str() instead of Deref coercion
            Some(text) => semver::VersionReq::parse(text.as_str())
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Err(serde::de::Error::custom(
                "optional field is null (an absent field is omitted)",
            )),
        }
    }
}

/// The schema of a [`semver::VersionReq`] field: a comparator list in Cargo's syntax.
struct VersionReqText;

impl JsonSchema for VersionReqText {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "VersionReqText".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "A version requirement: comparators such as `>=2.1, <3`, as Cargo writes them.",
            "minLength": 1,
            "$comment": "The grammar is that of the Rust `semver` crate's `VersionReq`; the reader parses it.",
        })
    }
}

schema_tag! {
    /// The constant `schema` field of an experiment file.
    pub ExperimentTag = EXPERIMENT_SCHEMA
}

/// The caps on one agent run: a treatment condition, the same for every arm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct PerRunCaps {
    /// The wall-clock cap of one run in seconds, from 1.
    #[schemars(extend("$comment" = "Fits in 32 bits; the reader enforces it."))]
    pub wall_s: u32,
    /// The cap on the turns of one run, from 1.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    #[schemars(extend("$comment" = "Fits in 32 bits; the reader enforces it."))]
    pub turns: Option<u32>,
    /// The cap on the cost of one run in US dollars, above 0.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    #[schemars(extend("$comment" = "Above 0 and finite; the reader enforces it."))]
    pub max_cost_usd: Option<f64>,
}

object_only!(PerRunCaps);

impl PerRunCaps {
    /// Caps a run at `wall_s` seconds, with no cap on turns or cost.
    #[must_use]
    pub const fn new(wall_s: u32) -> Self {
        Self {
            wall_s,
            turns: None,
            max_cost_usd: None,
        }
    }
}

/// The caps of an experiment: those of one run, and those of the job that runs its cells, with the number of runs that may be in flight at once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct Budget {
    /// The caps on one agent run.
    pub per_run: PerRunCaps,
    /// The cap on the runs the job starts, from 1.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    #[schemars(extend("$comment" = "Fits in 32 bits; the reader enforces it."))]
    pub max_runs: Option<u32>,
    /// The cap on the tokens the job spends, from 1.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    #[schemars(extend("$comment" = "Fits in 64 bits; a reader that holds numbers as doubles is exact only below 2^53."))]
    pub max_tokens: Option<u64>,
    /// The cap on the cost of the job in US dollars, above 0.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    #[schemars(extend("$comment" = "Above 0 and finite; the reader enforces it."))]
    pub max_cost_usd: Option<f64>,
    /// The wall-clock cap of the job in seconds, from 1.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    #[schemars(extend("$comment" = "Fits in 32 bits; the reader enforces it."))]
    pub wall_s: Option<u32>,
    /// The number of agent runs or evaluator processes in flight at once, from 1; 1 when absent.
    #[serde(default = "one")]
    #[schemars(extend("$comment" = "Fits in 32 bits; the reader enforces it."))]
    pub max_parallel: u32,
}

const fn one() -> u32 {
    1
}

object_only!(Budget);

impl Budget {
    /// A budget with the caps of one run and no cap on the job, running one process at a time.
    #[must_use]
    pub const fn new(per_run: PerRunCaps) -> Self {
        Self {
            per_run,
            max_runs: None,
            max_tokens: None,
            max_cost_usd: None,
            wall_s: None,
            max_parallel: 1,
        }
    }

    fn check(&self) -> Result<(), ExecutionError> {
        let count = |field, value: u64| {
            if value == 0 {
                Err(ExecutionError::Zero { field })
            } else {
                Ok(())
            }
        };
        let cost = |field, value: Option<f64>| match value {
            Some(cost) if !(cost.is_finite() && cost > 0.0) => {
                Err(ExecutionError::NotPositive { field })
            }
            Some(_) | None => Ok(()),
        };
        count("budget.per_run.wall_s", u64::from(self.per_run.wall_s))?;
        if let Some(turns) = self.per_run.turns {
            count("budget.per_run.turns", u64::from(turns))?;
        }
        cost("budget.per_run.max_cost_usd", self.per_run.max_cost_usd)?;
        if let Some(runs) = self.max_runs {
            count("budget.max_runs", u64::from(runs))?;
        }
        if let Some(tokens) = self.max_tokens {
            count("budget.max_tokens", tokens)?;
        }
        cost("budget.max_cost_usd", self.max_cost_usd)?;
        if let Some(wall) = self.wall_s {
            count("budget.wall_s", u64::from(wall))?;
        }
        count("budget.max_parallel", u64::from(self.max_parallel))
    }
}

/// The harness an experiment asks for: its name and, when it limits them, the versions that will do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct HarnessRequest {
    /// The name of the harness.
    pub name: Name,
    /// The versions of the harness that will do, as comparators such as `>=2.1, <3`; any version when absent.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "version_req_text"
    )]
    #[schemars(with = "Option<VersionReqText>")]
    pub version: Option<semver::VersionReq>,
}

object_only!(HarnessRequest);

impl HarnessRequest {
    /// Asks for the harness `name`, in any version.
    #[must_use]
    pub const fn new(name: Name) -> Self {
        Self {
            name,
            version: None,
        }
    }
}

/// The treatment an experiment applies: a name, and the tag and commit it is pinned to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct TreatmentRef {
    /// The name of the treatment.
    pub name: Name,
    /// The tag the treatment is pinned to, as its source names it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub tag: Option<String>,
    /// The commit the treatment is pinned to.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub commit: Option<GitCommit>,
}

object_only!(TreatmentRef);

impl TreatmentRef {
    /// Names a treatment, pinned to no tag and no commit.
    #[must_use]
    pub const fn new(name: Name) -> Self {
        Self {
            name,
            tag: None,
            commit: None,
        }
    }
}

/// One value of a factor: any JSON value except `null`, or a table whose leaves fill the paths under the factor's path.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Level {
    /// A value, held at the factor's own path.
    Value(JsonValue),
    /// A table; each leaf fills the factor's path followed by the leaf's keys. A leaf is any value that is not a table, so an array is one leaf.
    Fill(Map<String, Value>),
}

impl Level {
    /// A level that is a value.
    #[must_use]
    pub const fn value(value: JsonValue) -> Self {
        Self::Value(value)
    }

    /// A level that is a table whose leaves fill paths.
    ///
    /// # Errors
    /// Returns [`ExecutionError::EmptyTable`] if the table, or a table inside it, holds no value;
    /// [`ExecutionError::Format`] if a key is not a field path segment; and
    /// [`ExecutionError::Values`] if a leaf is `null` or cannot be hashed.
    pub fn fill(table: Map<String, Value>) -> Result<Self, ExecutionError> {
        let probe = FieldPath::from_str("level")?;
        fill_leaves(&table, &probe, &mut Vec::new())?;
        Ok(Self::Fill(table))
    }

    /// The values this level gives the paths at and under `base`.
    fn fills(&self, base: &FieldPath) -> Result<Vec<ResolvedValue>, ExecutionError> {
        match self {
            Self::Value(value) => Ok(vec![ResolvedValue::recorded(
                // BORROW: .clone() gives the cell its own path
                base.clone(),
                // BORROW: .clone() gives the cell its own value
                value.clone(),
            )]),
            Self::Fill(table) => {
                let mut out = Vec::new();
                fill_leaves(table, base, &mut out)?;
                Ok(out)
            }
        }
    }
}

/// Collects the leaves of `table` as values at `base` followed by their keys.
fn fill_leaves(
    table: &Map<String, Value>,
    base: &FieldPath,
    out: &mut Vec<ResolvedValue>,
) -> Result<(), ExecutionError> {
    if table.is_empty() {
        return Err(ExecutionError::EmptyTable {
            path: base.to_string(),
        });
    }
    for (key, value) in table {
        // BORROW: explicit .as_str() instead of Deref coercion
        let path = base.join(key.as_str())?;
        match value {
            Value::Object(inner) => fill_leaves(inner, &path, out)?,
            Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Array(_) => {
                // BORROW: .clone() gives the cell its own value
                out.push(ResolvedValue::recorded(
                    path,
                    JsonValue::new(value.clone())?,
                ));
            }
        }
    }
    Ok(())
}

impl Serialize for Level {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => value.serialize(serializer),
            Self::Fill(table) => table.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Object(table) => Self::fill(table).map_err(serde::de::Error::custom),
            other @ (Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Array(_)) => JsonValue::new(other)
                .map(Self::Value)
                .map_err(serde::de::Error::custom),
        }
    }
}

impl JsonSchema for Level {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Level".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": ["string", "number", "boolean", "array", "object"],
            "description": "One value of a factor: any JSON value except null, or an object whose leaves fill the paths under the factor's path.",
            "$comment": "A leaf of an object level is any value that is not an object; the keys are field path segments, an object holds at least one leaf, and a leaf is not null. Integers lie within ±(2^53 - 1). JSON Schema cannot state this, and the Rust reader enforces it.",
        })
    }
}

/// A field that varies across the cells of an experiment: its path and its levels, in the order they are written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct Factor {
    /// The dotted path of the field.
    pub path: FieldPath,
    /// The values the field takes, one cell per value; at least one.
    #[schemars(length(min = 1))]
    pub levels: Vec<Level>,
}

object_only!(Factor);

impl Factor {
    /// A factor with its levels.
    #[must_use]
    pub const fn new(path: FieldPath, levels: Vec<Level>) -> Self {
        Self { path, levels }
    }
}

/// The values an experiment file fixes in advance, precomputed so that its cells are infallible.
#[derive(Debug, Clone, PartialEq)]
struct Plan {
    /// The `fixed` values, in path order.
    fixed: Vec<ResolvedValue>,
    /// For each factor and each of its levels, the values the level gives.
    levels: Vec<Vec<Vec<ResolvedValue>>>,
    /// The per-run caps as values.
    caps: Vec<ResolvedValue>,
}

/// An experiment file: the values fixed in every cell, the factors whose levels make the cells, the replicates, the budget, the harness, the treatment and the names of the secrets the runs need.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "ExperimentFile")]
struct RawExperimentFile {
    /// The version marker of the document: always `experiment/1`.
    schema: ExperimentTag,
    /// The values every cell holds, by dotted field path.
    #[serde(default)]
    fixed: BTreeMap<FieldPath, JsonValue>,
    /// The factors, in the order the cells are made from them: the first factor varies slowest.
    #[serde(default)]
    #[schemars(extend(
        "$comment" = "No path of a factor, nor a path a level fills, equals or lies below or above another factor's path or a key of `fixed`; the Rust reader refuses an overlap."
    ))]
    factors: Vec<Factor>,
    /// The runs of each cell, from 1.
    #[schemars(extend("$comment" = "Fits in 32 bits; the reader enforces it."))]
    replicates: u32,
    /// The caps on a run and on the job.
    budget: Budget,
    /// The harness the runs use.
    harness: HarnessRequest,
    /// The treatment the runs apply.
    treatment: TreatmentRef,
    /// The names of the environment variables the runs need; their values are never written.
    #[serde(default)]
    secrets: Vec<SecretName>,
}

impl TryFrom<MapOnly<RawExperimentFile>> for ExperimentFile {
    type Error = ExecutionError;

    fn try_from(raw: MapOnly<RawExperimentFile>) -> Result<Self, Self::Error> {
        let RawExperimentFile {
            schema: ExperimentTag,
            fixed,
            factors,
            replicates,
            budget,
            harness,
            treatment,
            secrets,
        } = raw.0;
        Self::new(
            fixed, factors, replicates, budget, harness, treatment, secrets,
        )
    }
}

/// An experiment file: the values fixed in every cell, the factors whose levels make the cells, the replicates, the budget, the harness, the treatment and the names of the secrets the runs need.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawExperimentFile>")]
#[non_exhaustive]
pub struct ExperimentFile {
    schema: ExperimentTag,
    fixed: BTreeMap<FieldPath, JsonValue>,
    factors: Vec<Factor>,
    replicates: u32,
    budget: Budget,
    harness: HarnessRequest,
    treatment: TreatmentRef,
    secrets: Vec<SecretName>,
    #[serde(skip)]
    #[schemars(skip)]
    plan: Plan,
}

impl ExperimentFile {
    /// Builds an experiment file.
    ///
    /// # Errors
    /// Returns [`ExecutionError::Zero`] or [`ExecutionError::NotPositive`] if `replicates` or a
    /// cap is 0 or not above 0, [`ExecutionError::NoLevels`] if a factor has no level,
    /// [`ExecutionError::EmptyTable`], [`ExecutionError::Format`] or [`ExecutionError::Values`]
    /// if a level cannot fill its paths, and [`ExecutionError::PathOverlap`] if a key of `fixed`
    /// and a path of a factor, or the paths of two factors, overlap.
    pub fn new(
        fixed: BTreeMap<FieldPath, JsonValue>,
        factors: Vec<Factor>,
        replicates: u32,
        budget: Budget,
        harness: HarnessRequest,
        treatment: TreatmentRef,
        secrets: Vec<SecretName>,
    ) -> Result<Self, ExecutionError> {
        if replicates == 0 {
            return Err(ExecutionError::Zero {
                field: "replicates",
            });
        }
        budget.check()?;
        let mut levels = Vec::with_capacity(factors.len());
        // Every path an owner touches: the keys of `fixed` first, then one set per factor.
        // BORROW: .cloned() copies the keys into the footprint set
        let mut footprints: Vec<BTreeSet<FieldPath>> = vec![fixed.keys().cloned().collect()];
        for factor in &factors {
            if factor.levels.is_empty() {
                return Err(ExecutionError::NoLevels {
                    // BORROW: .clone() keeps the factor's path in the error
                    factor: factor.path.clone(),
                });
            }
            let filled = factor
                .levels
                .iter()
                .map(|level| level.fills(&factor.path))
                .collect::<Result<Vec<_>, _>>()?;
            footprints.push(
                filled
                    .iter()
                    .flatten()
                    // BORROW: .clone() copies the path into the footprint set
                    .map(|value| value.path().clone())
                    .collect(),
            );
            levels.push(filled);
        }
        check_overlaps(&footprints)?;
        let fixed_values = fixed
            .iter()
            // BORROW: .clone() gives the cell its own path and value
            .map(|(path, value)| ResolvedValue::recorded(path.clone(), value.clone()))
            .collect();
        let plan = Plan {
            fixed: fixed_values,
            levels,
            caps: cap_values(&budget.per_run)?,
        };
        Ok(Self {
            schema: ExperimentTag,
            fixed,
            factors,
            replicates,
            budget,
            harness,
            treatment,
            secrets,
            plan,
        })
    }

    /// Reads an experiment file from TOML, converts it to JSON and validates it.
    ///
    /// A table inside `fixed` fills the paths its leaves name, so `plan.model = "m"` and
    /// `"plan.model" = "m"` are the same value. A factor is a table of the `factors` array of
    /// tables, which keeps the order it is written in.
    ///
    /// # Errors
    /// Returns [`ExecutionError::Toml`] if `text` is not TOML, [`ExecutionError::Unrepresentable`]
    /// if it holds a date or time or a number that is not finite, [`ExecutionError::Document`] if
    /// it is not an experiment file, and the errors of [`ExperimentFile::new`] if it is one that
    /// does not hold together.
    pub fn from_toml_str(text: &str) -> Result<Self, ExecutionError> {
        let table: toml::Table = toml::from_str(text).map_err(|e| ExecutionError::Toml {
            detail: e.to_string(),
        })?;
        let mut document = Map::new();
        for (key, value) in table {
            let converted = if key == "fixed" {
                flatten_fixed(value)?
            } else {
                // BORROW: explicit .as_str() instead of Deref coercion
                toml_to_json(value, key.as_str())?
            };
            document.insert(key, converted);
        }
        serde_json::from_value(Value::Object(document)).map_err(|e| ExecutionError::Document {
            detail: e.to_string(),
        })
    }

    /// The values every cell holds, by path.
    #[must_use]
    pub const fn fixed<'a>(&'a self) -> &'a BTreeMap<FieldPath, JsonValue> {
        &self.fixed
    }

    /// The factors, in written order.
    #[must_use]
    pub fn factors<'a>(&'a self) -> &'a [Factor] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.factors.as_slice()
    }

    /// The runs of each cell.
    #[must_use]
    pub const fn replicates(&self) -> u32 {
        self.replicates
    }

    /// The caps on a run and on the job.
    #[must_use]
    pub const fn budget<'a>(&'a self) -> &'a Budget {
        &self.budget
    }

    /// The harness the runs use.
    #[must_use]
    pub const fn harness<'a>(&'a self) -> &'a HarnessRequest {
        &self.harness
    }

    /// The treatment the runs apply.
    #[must_use]
    pub const fn treatment<'a>(&'a self) -> &'a TreatmentRef {
        &self.treatment
    }

    /// The names of the secrets the runs need.
    #[must_use]
    pub fn secrets<'a>(&'a self) -> &'a [SecretName] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.secrets.as_slice()
    }

    /// The cells of the experiment: the cartesian product of the levels of its factors.
    ///
    /// The first factor varies slowest and the first level of a factor comes first. A cell holds
    /// the `fixed` values in path order, the values of one level of each factor in written order,
    /// and the per-run caps as `budget.per_run.wall_s`, `budget.per_run.turns` and
    /// `budget.per_run.max_cost_usd`; the caps of the job are not cell values. An experiment with
    /// no factor has one cell.
    #[must_use]
    pub fn cells(&self) -> Vec<Vec<ResolvedValue>> {
        // BORROW: .clone() starts the first cell from the fixed values
        let mut cells = vec![self.plan.fixed.clone()];
        for levels in &self.plan.levels {
            let mut next = Vec::with_capacity(cells.len().saturating_mul(levels.len()));
            for cell in &cells {
                for level in levels {
                    // BORROW: .clone() gives each product cell its own prefix
                    let mut extended = cell.clone();
                    // BORROW: .cloned() gives each product cell its own level values
                    extended.extend(level.iter().cloned());
                    next.push(extended);
                }
            }
            cells = next;
        }
        for cell in &mut cells {
            // BORROW: .cloned() gives each cell its own cap values
            cell.extend(self.plan.caps.iter().cloned());
        }
        cells
    }
}

/// Refuses two footprints whose paths overlap, and two overlapping paths inside one footprint.
fn check_overlaps(footprints: &[BTreeSet<FieldPath>]) -> Result<(), ExecutionError> {
    let overlap = |first: &FieldPath, second: &FieldPath| {
        first.starts_with(second) || second.starts_with(first)
    };
    let refuse = |first: &FieldPath, second: &FieldPath| ExecutionError::PathOverlap {
        // BORROW: .clone() keeps the paths in the error
        first: first.clone(),
        // BORROW: .clone() keeps the paths in the error
        second: second.clone(),
    };
    for (index, own) in footprints.iter().enumerate() {
        let paths: Vec<&FieldPath> = own.iter().collect();
        for (at, first) in paths.iter().enumerate() {
            for second in paths.iter().skip(at.saturating_add(1)) {
                if overlap(first, second) {
                    return Err(refuse(first, second));
                }
            }
        }
        for other in footprints.iter().skip(index.saturating_add(1)) {
            for first in own {
                if let Some(second) = other.iter().find(|second| overlap(first, second)) {
                    return Err(refuse(first, second));
                }
            }
        }
    }
    Ok(())
}

/// The per-run caps as the values of `budget.per_run.*`.
fn cap_values(caps: &PerRunCaps) -> Result<Vec<ResolvedValue>, ExecutionError> {
    let mut out = vec![ResolvedValue::recorded(
        FieldPath::from_str("budget.per_run.wall_s")?,
        JsonValue::new(Value::from(caps.wall_s))?,
    )];
    if let Some(turns) = caps.turns {
        out.push(ResolvedValue::recorded(
            FieldPath::from_str("budget.per_run.turns")?,
            JsonValue::new(Value::from(turns))?,
        ));
    }
    if let Some(cost) = caps.max_cost_usd {
        let number = Number::from_f64(cost).ok_or(ExecutionError::NotPositive {
            field: "budget.per_run.max_cost_usd",
        })?;
        out.push(ResolvedValue::recorded(
            FieldPath::from_str("budget.per_run.max_cost_usd")?,
            JsonValue::new(Value::Number(number))?,
        ));
    }
    Ok(out)
}

/// Converts a TOML value to JSON, refusing a date or time and a number that is not finite.
fn toml_to_json(value: toml::Value, at: &str) -> Result<Value, ExecutionError> {
    let refuse = |problem| ExecutionError::Unrepresentable {
        // BORROW: .to_owned() keeps the path in the error
        path: at.to_owned(),
        problem,
    };
    match value {
        toml::Value::String(text) => Ok(Value::String(text)),
        toml::Value::Integer(number) => Ok(Value::from(number)),
        toml::Value::Float(number) => Number::from_f64(number)
            .map(Value::Number)
            .ok_or_else(|| refuse("is not a finite number")),
        toml::Value::Boolean(flag) => Ok(Value::Bool(flag)),
        toml::Value::Datetime(_) => Err(refuse("is a date or time")),
        toml::Value::Array(items) => items
            .into_iter()
            .map(|item| toml_to_json(item, at))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        toml::Value::Table(table) => table
            .into_iter()
            .map(|(key, inner)| {
                // BORROW: explicit .as_str() instead of Deref coercion
                let path = format!("{at}.{}", key.as_str());
                // BORROW: explicit .as_str() instead of Deref coercion
                toml_to_json(inner, path.as_str()).map(|json| (key, json))
            })
            .collect::<Result<Map<_, _>, _>>()
            .map(Value::Object),
    }
}

/// Converts the `fixed` table, flattening nested tables into dotted keys.
fn flatten_fixed(value: toml::Value) -> Result<Value, ExecutionError> {
    let toml::Value::Table(table) = value else {
        return toml_to_json(value, "fixed");
    };
    let mut flat = Map::new();
    flatten_into(table, "", &mut flat)?;
    Ok(Value::Object(flat))
}

fn flatten_into(
    table: toml::Table,
    prefix: &str,
    out: &mut Map<String, Value>,
) -> Result<(), ExecutionError> {
    for (key, value) in table {
        // BORROW: explicit .as_str() instead of Deref coercion
        let path = format!("{prefix}{}", key.as_str());
        match value {
            toml::Value::Table(inner) => {
                if inner.is_empty() {
                    return Err(ExecutionError::EmptyTable { path });
                }
                // BORROW: explicit .as_str() instead of Deref coercion
                flatten_into(inner, format!("{path}.").as_str(), out)?;
            }
            toml::Value::String(_)
            | toml::Value::Integer(_)
            | toml::Value::Float(_)
            | toml::Value::Boolean(_)
            | toml::Value::Datetime(_)
            | toml::Value::Array(_) => {
                // BORROW: explicit .as_str() instead of Deref coercion
                let json = toml_to_json(value, path.as_str())?;
                // BORROW: .clone() keeps the path for the duplicate error
                if out.insert(path.clone(), json).is_some() {
                    return Err(ExecutionError::Duplicate {
                        list: "fixed",
                        value: path,
                    });
                }
            }
        }
    }
    Ok(())
}

/// The digest of an experiment: the digest of the RFC 8785 form of the sorted, deduplicated list of its cell digests.
///
/// This is the only function that computes the `experiment` field of a manifest, for live runs
/// and for imported ones alike.
#[must_use]
pub fn experiment_digest(cells: &[Digest]) -> Digest {
    let sorted: BTreeSet<&Digest> = cells.iter().collect();
    let list = sorted
        .into_iter()
        // BORROW: explicit .as_str() and .to_owned() make the owned strings of the list
        .map(|digest| Value::String(digest.as_str().to_owned()))
        .collect();
    match Digest::of_canonical(&Value::Array(list)) {
        Ok(digest) => digest,
        Err(error) => unreachable!("a list of strings has a canonical text: {error}"),
    }
}

/// The state a cell gives a path: absent, recorded as unrecorded, or recorded with a canonical text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum PathState {
    Absent,
    Unrecorded,
    Recorded(String),
}

/// The paths whose values are not the same in every cell: those a cell records with a value that
/// another cell records differently, records as `unrecorded`, or does not hold at all.
///
/// A manifest's `factors` are exactly these, and this is the only function that computes them,
/// for live runs and for imported ones alike. The paths come sorted and unique. Two values are the
/// same when their RFC 8785 texts are, so `1` and `1.0` do not vary.
#[must_use]
pub fn varying_paths(cells: &[Vec<ResolvedValue>]) -> Vec<FieldPath> {
    let mut seen: BTreeMap<&FieldPath, BTreeSet<PathState>> = BTreeMap::new();
    for cell in cells {
        for value in cell {
            let state = value.value().map_or(PathState::Unrecorded, |json| {
                PathState::Recorded(
                    canonical::to_jcs(json.as_value())
                        .unwrap_or_else(|_| json.as_value().to_string()),
                )
            });
            seen.entry(value.path()).or_default().insert(state);
        }
    }
    for cell in cells {
        let held: BTreeSet<&FieldPath> = cell.iter().map(ResolvedValue::path).collect();
        for (path, states) in &mut seen {
            if !held.contains(path) {
                states.insert(PathState::Absent);
            }
        }
    }
    seen.into_iter()
        .filter(|(_, states)| states.len() > 1)
        // BORROW: .clone() copies the path into the result
        .map(|(path, _)| path.clone())
        .collect()
}

schema_tag! {
    /// The constant `schema` field of a run manifest.
    pub RunManifestTag = RUN_MANIFEST_SCHEMA
}

/// The harness a run used, as it identified itself: its name, its version and, when one drove it, the driver as `name@version`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct HarnessIdentity {
    /// The name of the harness.
    pub name: Name,
    /// The version of the harness.
    #[serde(with = "version_text")]
    #[schemars(with = "VersionText")]
    pub version: semver::Version,
    /// The driver that ran the harness, as `name@version`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub driver: Option<NameVersion>,
}

object_only!(HarnessIdentity);

impl HarnessIdentity {
    /// A harness without a driver.
    #[must_use]
    pub const fn new(name: Name, version: semver::Version) -> Self {
        Self {
            name,
            version,
            driver: None,
        }
    }
}

/// A digest and the kind of thing it identifies: a catalog snapshot or an input.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct DigestRef {
    /// What the digest identifies, as a plain name.
    pub kind: Name,
    /// The digest.
    pub digest: Digest,
}

object_only!(DigestRef);

impl DigestRef {
    /// A digest of a `kind` of thing.
    #[must_use]
    pub const fn new(kind: Name, digest: Digest) -> Self {
        Self { kind, digest }
    }
}

/// The lock digest of a plug-in environment, with the plug-in as `name@version`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct EnvironmentRef {
    /// The plug-in, as `name@version`.
    pub plugin: NameVersion,
    /// The digest of the plug-in's lock file.
    pub lock_digest: Digest,
}

object_only!(EnvironmentRef);

impl EnvironmentRef {
    /// The environment of `plugin`, locked at `lock_digest`.
    #[must_use]
    pub const fn new(plugin: NameVersion, lock_digest: Digest) -> Self {
        Self {
            plugin,
            lock_digest,
        }
    }
}

/// The roots a preflight step derives into the typed fields of a [`Comparable`].
const RESERVED_ROOTS: [&str; 5] = [
    "harness",
    "treatment",
    "snapshots",
    "inputs",
    "environments",
];

/// The fields of a cell as the hash sees them, borrowed.
#[derive(Serialize)]
struct ComparableBody<'a> {
    values: &'a [ResolvedValue],
    harness: &'a HarnessIdentity,
    treatment: &'a TreatmentRef,
    snapshots: &'a [DigestRef],
    inputs: &'a [DigestRef],
    environments: &'a [EnvironmentRef],
}

/// Checks that `items` rise strictly by `key`: an equal neighbour is a duplicate, a lower one is out of order.
fn check_ascending<'a, T, K: Ord>(
    list: &'static str,
    by: &'static str,
    items: &'a [T],
    key: impl Fn(&'a T) -> K,
    show: impl Fn(&'a T) -> String,
) -> Result<(), ExecutionError> {
    for pair in items.windows(2) {
        if let [first, second] = pair {
            match key(first).cmp(&key(second)) {
                // EXPLICIT: an ascending pair has nothing to report
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => {
                    return Err(ExecutionError::Duplicate {
                        list,
                        value: show(second),
                    });
                }
                std::cmp::Ordering::Greater => {
                    return Err(ExecutionError::Unsorted { list, key: by });
                }
            }
        }
    }
    Ok(())
}

fn environment_order(first: &EnvironmentRef, second: &EnvironmentRef) -> std::cmp::Ordering {
    first
        .plugin
        // BORROW: explicit .as_str() instead of Deref coercion
        .as_str()
        .cmp(second.plugin.as_str())
        .then_with(|| first.lock_digest.cmp(&second.lock_digest))
}

/// What the cell digest hashes: every resolved value, the harness, the treatment, the catalog snapshots, the inputs and the plug-in environments of a run, each list sorted and without duplicates.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "Comparable")]
struct RawComparable {
    /// Every resolved value by field path, sorted by path; a value without `value` was not recorded.
    #[schemars(extend(
        "$comment" = "Sorted by path in code point order, without a repeated path; JSON Schema cannot state this, and the Rust reader refuses a list that is not."
    ))]
    values: Vec<ResolvedValue>,
    /// The harness that ran the agent.
    harness: HarnessIdentity,
    /// The treatment the run applied.
    treatment: TreatmentRef,
    /// The digests of the catalog snapshots, sorted by kind and digest.
    #[schemars(extend(
        "$comment" = "Sorted by kind then digest in code point order, without a repeated entry; the Rust reader refuses a list that is not."
    ))]
    snapshots: Vec<DigestRef>,
    /// The digests of the inputs, sorted by kind and digest.
    #[schemars(extend(
        "$comment" = "Sorted by kind then digest in code point order, without a repeated entry; the Rust reader refuses a list that is not."
    ))]
    inputs: Vec<DigestRef>,
    /// The plug-in environments, sorted by plug-in text and lock digest.
    #[schemars(extend(
        "$comment" = "Sorted by the text of the plug-in then the lock digest in code point order, without a repeated entry; the Rust reader refuses a list that is not."
    ))]
    environments: Vec<EnvironmentRef>,
}

impl TryFrom<MapOnly<RawComparable>> for Comparable {
    type Error = ExecutionError;

    fn try_from(raw: MapOnly<RawComparable>) -> Result<Self, Self::Error> {
        let RawComparable {
            values,
            harness,
            treatment,
            snapshots,
            inputs,
            environments,
        } = raw.0;
        Self::assemble(values, harness, treatment, snapshots, inputs, environments)
    }
}

/// What the cell digest hashes: every resolved value, the harness, the treatment, the catalog snapshots, the inputs and the plug-in environments of a run, each list sorted and without duplicates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawComparable>")]
#[non_exhaustive]
pub struct Comparable {
    values: Vec<ResolvedValue>,
    harness: HarnessIdentity,
    treatment: TreatmentRef,
    snapshots: Vec<DigestRef>,
    inputs: Vec<DigestRef>,
    environments: Vec<EnvironmentRef>,
    #[serde(skip)]
    #[schemars(skip)]
    digest: Digest,
}

impl Comparable {
    /// Starts a cell; the harness and the treatment must be set before it builds.
    #[must_use]
    pub fn builder() -> ComparableBuilder {
        ComparableBuilder::default()
    }

    /// Checks the order of every list and computes the digest.
    fn assemble(
        values: Vec<ResolvedValue>,
        harness: HarnessIdentity,
        treatment: TreatmentRef,
        snapshots: Vec<DigestRef>,
        inputs: Vec<DigestRef>,
        environments: Vec<EnvironmentRef>,
    ) -> Result<Self, ExecutionError> {
        check_ascending(
            "values",
            "path",
            // BORROW: explicit .as_slice() instead of Deref coercion
            values.as_slice(),
            ResolvedValue::path,
            |value| value.path().to_string(),
        )?;
        for (list, items) in [("snapshots", &snapshots), ("inputs", &inputs)] {
            check_ascending(
                list,
                "kind and digest",
                // BORROW: explicit .as_slice() instead of Deref coercion
                items.as_slice(),
                |entry| (entry.kind.as_str(), entry.digest.as_str()),
                |entry| entry.digest.to_string(),
            )?;
        }
        check_ascending(
            "environments",
            "plugin and lock digest",
            // BORROW: explicit .as_slice() instead of Deref coercion
            environments.as_slice(),
            |entry| (entry.plugin.as_str(), entry.lock_digest.as_str()),
            |entry| entry.plugin.to_string(),
        )?;
        let body = ComparableBody {
            // BORROW: explicit .as_slice() instead of Deref coercion
            values: values.as_slice(),
            harness: &harness,
            treatment: &treatment,
            snapshots: snapshots.as_slice(),
            // BORROW: explicit .as_slice() instead of Deref coercion
            inputs: inputs.as_slice(),
            // BORROW: explicit .as_slice() instead of Deref coercion
            environments: environments.as_slice(),
        };
        let json = serde_json::to_value(&body).map_err(|e| CanonicalError::Syntax {
            detail: e.to_string(),
        })?;
        let digest = Digest::of_canonical(&json)?;
        Ok(Self {
            values,
            harness,
            treatment,
            snapshots,
            inputs,
            environments,
            digest,
        })
    }

    /// Builds a cell from resolved values.
    ///
    /// The path `harness` fills the harness, `treatment` the treatment, `snapshots` the snapshot
    /// digests, `inputs` the input digests and `environments` the plug-in environments; each
    /// takes one value, read as the typed field reads. Every other path goes to `values`, and a
    /// path below one of the five roots is refused. A settings path is not a value and never
    /// reaches this function.
    ///
    /// # Errors
    /// Returns [`ExecutionError::ReservedRoot`] if a reserved root has no value or a path below
    /// it, [`ExecutionError::ReservedValue`] if its value is not the shape the root takes,
    /// [`ExecutionError::MissingField`] if `harness` or `treatment` is absent,
    /// [`ExecutionError::Duplicate`] if a path or a root appears twice,
    /// [`ExecutionError::AbsolutePath`] if a value holds an absolute path as a string, at the top
    /// or at any depth of an array or object, and the errors of [`ComparableBuilder::build`].
    pub fn from_resolved(resolved: &[ResolvedValue]) -> Result<Self, ExecutionError> {
        let mut builder = Self::builder();
        let mut seen: BTreeSet<&'static str> = BTreeSet::new();
        for value in resolved {
            let root = value.path().segments().next().unwrap_or_default();
            let Some(reserved) = RESERVED_ROOTS.iter().copied().find(|name| *name == root) else {
                if value
                    .value()
                    .is_some_and(|json| holds_absolute_path(json.as_value()))
                {
                    return Err(ExecutionError::AbsolutePath {
                        path: value.path().to_string(),
                    });
                }
                // BORROW: .clone() gives the cell its own value
                builder = builder.value(value.clone());
                continue;
            };
            // BORROW: explicit .as_str() instead of Deref coercion
            if value.path().as_str() != reserved {
                return Err(ExecutionError::ReservedRoot {
                    path: value.path().to_string(),
                    problem: "is a path below a reserved root, which takes one value",
                });
            }
            let Some(json) = value.value() else {
                return Err(ExecutionError::ReservedRoot {
                    path: value.path().to_string(),
                    problem: "has no value",
                });
            };
            if !seen.insert(reserved) {
                return Err(ExecutionError::Duplicate {
                    list: "reserved roots",
                    // BORROW: .to_owned() makes the owned value
                    value: reserved.to_owned(),
                });
            }
            // BORROW: .clone() hands the value to the typed reader
            builder = builder.reserved(reserved, json.as_value().clone())?;
        }
        builder.build()
    }

    /// Builds a cell and the manifest `sources` from a resolution, in one step.
    ///
    /// `resolved` and `sources` are the values and the sources of the resolution, and `settings`
    /// are the paths it marks as settings. A setting never enters the manifest, so a value at a
    /// setting path, or below one, is dropped before the cell is built, and so is the source of
    /// such a path. The sources that remain are those of the cell's values and those at a typed
    /// root or below it, ready for [`RunManifestBuilder::source`]. The cell is built as
    /// [`Comparable::from_resolved`] builds it.
    ///
    /// # Errors
    /// Returns [`ExecutionError::SourcesMismatch`] if a source is of a path that is neither a
    /// value path of the cell nor at or below a typed root (a source at or below a setting path
    /// is dropped, not refused), and the errors of [`Comparable::from_resolved`].
    pub fn from_resolution(
        resolved: &[ResolvedValue],
        sources: &[SourceEntry],
        settings: &[FieldPath],
    ) -> Result<(Self, Vec<SourceEntry>), ExecutionError> {
        let is_setting = |path: &FieldPath| {
            settings.iter().any(|setting| {
                // BORROW: explicit .as_str() instead of Deref coercion
                path.as_str()
                    // BORROW: explicit .as_str() instead of Deref coercion
                    .strip_prefix(setting.as_str())
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
            })
        };
        let values: Vec<ResolvedValue> = resolved
            .iter()
            .filter(|value| !is_setting(value.path()))
            // BORROW: .cloned() gives the cell its own values
            .cloned()
            .collect();
        let comparable = Self::from_resolved(values.as_slice())?;
        let held: BTreeSet<&FieldPath> = values.iter().map(ResolvedValue::path).collect();
        let mut kept = Vec::new();
        for source in sources {
            let path = source.path();
            if is_setting(path) {
                continue;
            }
            if held.contains(path) || under_typed_root(path) {
                // BORROW: .clone() gives the manifest its own entry
                kept.push(source.clone());
            } else {
                return Err(stray_source(path));
            }
        }
        Ok((comparable, kept))
    }

    /// Every resolved value, sorted by path.
    #[must_use]
    pub fn values<'a>(&'a self) -> &'a [ResolvedValue] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.values.as_slice()
    }

    /// The harness.
    #[must_use]
    pub const fn harness<'a>(&'a self) -> &'a HarnessIdentity {
        &self.harness
    }

    /// The treatment.
    #[must_use]
    pub const fn treatment<'a>(&'a self) -> &'a TreatmentRef {
        &self.treatment
    }

    /// The catalog snapshot digests, sorted.
    #[must_use]
    pub fn snapshots<'a>(&'a self) -> &'a [DigestRef] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.snapshots.as_slice()
    }

    /// The input digests, sorted.
    #[must_use]
    pub fn inputs<'a>(&'a self) -> &'a [DigestRef] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.inputs.as_slice()
    }

    /// The plug-in environments, sorted.
    #[must_use]
    pub fn environments<'a>(&'a self) -> &'a [EnvironmentRef] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.environments.as_slice()
    }

    /// The digest of the cell: `sha256` of the RFC 8785 form of the cell, as `comparable` is
    /// written. A change to the digest an existing manifest yields requires a new schema tag.
    #[must_use]
    pub fn digest(&self) -> Digest {
        // BORROW: .clone() hands out an owned digest
        self.digest.clone()
    }
}

/// Reports whether `value`, or a string nested in it at any depth, is an absolute path.
fn holds_absolute_path(value: &Value) -> bool {
    match value {
        Value::String(text) => is_absolute_path(text),
        Value::Array(items) => items.iter().any(holds_absolute_path),
        Value::Object(fields) => fields.values().any(holds_absolute_path),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

/// Reports whether `text` is an absolute path: it starts with `/` or `\`, or with a drive letter and a separator.
fn is_absolute_path(text: &str) -> bool {
    let mut bytes = text.bytes();
    match (bytes.next(), bytes.next(), bytes.next()) {
        (Some(b'/' | b'\\'), _, _) => true,
        (Some(drive), Some(b':'), Some(b'/' | b'\\')) => drive.is_ascii_alphabetic(),
        _ => false,
    }
}

/// Builds a [`Comparable`]: it sorts every list and refuses a duplicate.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ComparableBuilder {
    values: Vec<ResolvedValue>,
    harness: Option<HarnessIdentity>,
    treatment: Option<TreatmentRef>,
    snapshots: Vec<DigestRef>,
    inputs: Vec<DigestRef>,
    environments: Vec<EnvironmentRef>,
}

impl ComparableBuilder {
    /// Adds a resolved value.
    #[must_use]
    pub fn value(mut self, value: ResolvedValue) -> Self {
        self.values.push(value);
        self
    }

    /// Sets the harness.
    #[must_use]
    pub fn harness(mut self, harness: HarnessIdentity) -> Self {
        self.harness = Some(harness);
        self
    }

    /// Sets the treatment.
    #[must_use]
    pub fn treatment(mut self, treatment: TreatmentRef) -> Self {
        self.treatment = Some(treatment);
        self
    }

    /// Adds a catalog snapshot digest.
    #[must_use]
    pub fn snapshot(mut self, snapshot: DigestRef) -> Self {
        self.snapshots.push(snapshot);
        self
    }

    /// Adds an input digest.
    #[must_use]
    pub fn input(mut self, input: DigestRef) -> Self {
        self.inputs.push(input);
        self
    }

    /// Adds a plug-in environment.
    #[must_use]
    pub fn environment(mut self, environment: EnvironmentRef) -> Self {
        self.environments.push(environment);
        self
    }

    /// Reads the value of a reserved root into its typed field.
    fn reserved(mut self, root: &'static str, json: Value) -> Result<Self, ExecutionError> {
        let malformed = |e: serde_json::Error| ExecutionError::ReservedValue {
            root,
            detail: e.to_string(),
        };
        match root {
            "harness" => self.harness = Some(serde_json::from_value(json).map_err(malformed)?),
            "treatment" => self.treatment = Some(serde_json::from_value(json).map_err(malformed)?),
            "snapshots" => {
                // The list is read as written; `build` sorts and refuses a duplicate.
                self.snapshots = serde_json::from_value(json).map_err(malformed)?;
            }
            "inputs" => self.inputs = serde_json::from_value(json).map_err(malformed)?,
            "environments" => {
                self.environments = serde_json::from_value(json).map_err(malformed)?;
            }
            _ => {
                return Err(ExecutionError::ReservedRoot {
                    // BORROW: .to_owned() makes the owned value
                    path: root.to_owned(),
                    problem: "is not a reserved root",
                });
            }
        }
        Ok(self)
    }

    /// Sorts every list and builds the cell.
    ///
    /// # Errors
    /// Returns [`ExecutionError::MissingField`] if the harness or the treatment is not set,
    /// [`ExecutionError::Duplicate`] if a list holds an entry twice, and
    /// [`ExecutionError::Canonical`] if the cell has no canonical form.
    pub fn build(self) -> Result<Comparable, ExecutionError> {
        let Self {
            mut values,
            harness,
            treatment,
            mut snapshots,
            mut inputs,
            mut environments,
        } = self;
        let harness = harness.ok_or(ExecutionError::MissingField { field: "harness" })?;
        let treatment = treatment.ok_or(ExecutionError::MissingField { field: "treatment" })?;
        values.sort_by(|a, b| a.path().cmp(b.path()));
        snapshots.sort();
        inputs.sort();
        environments.sort_by(environment_order);
        Comparable::assemble(values, harness, treatment, snapshots, inputs, environments)
    }
}

/// Whether a secret the run needs was present in the environment, by name; the value is never written.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct SecretPresence {
    /// The name of the environment variable.
    pub name: SecretName,
    /// Whether the variable was set.
    pub present: bool,
}

object_only!(SecretPresence);

impl SecretPresence {
    /// Records whether the secret `name` was present.
    #[must_use]
    pub const fn new(name: SecretName, present: bool) -> Self {
        Self { name, present }
    }
}

/// A re-checked value that a forced resume accepted although it differed from the stored one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct Deviation {
    /// The path of the value.
    pub path: FieldPath,
    /// The value the manifest stored; absent when none was recorded.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub stored: Option<JsonValue>,
    /// The value the resume observed; absent when none was recorded.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub observed: Option<JsonValue>,
}

object_only!(Deviation);

impl Deviation {
    /// A deviation at `path`.
    #[must_use]
    pub const fn new(
        path: FieldPath,
        stored: Option<JsonValue>,
        observed: Option<JsonValue>,
    ) -> Self {
        Self {
            path,
            stored,
            observed,
        }
    }
}

/// Why a run is invalid: a cause that is not the arm's doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InvalidReason {
    /// The gateway or the provider failed.
    Gateway,
    /// The job budget stopped the run.
    BudgetStop,
    /// The run was killed.
    Killed,
    /// The supervisor was lost while the run ran.
    Interrupted,
    /// A secret or ground truth reached the run's isolation boundary.
    InputLeak,
    /// The collection of the run's files failed.
    CollectionError,
    /// A reader of the run's output failed.
    ReaderError,
    /// The format of the run's output is not one a reader knows.
    FormatUnknown,
}

/// Whether a run counts: `valid`, or `invalid` with the reason it does not.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum Integrity {
    /// The run counts.
    Valid,
    /// The run does not count, and the reason says why.
    #[non_exhaustive]
    Invalid {
        /// Why the run is invalid.
        reason: InvalidReason,
    },
}

object_only!(Integrity);

/// A file the import read, kept as a blob: its run-relative path and its `sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct ImportSource {
    /// The run-relative path of the file in the legacy run.
    pub path: RelPath,
    /// The digest of the file's bytes.
    pub sha256: Digest,
}

object_only!(ImportSource);

impl ImportSource {
    /// A source file with its digest.
    #[must_use]
    pub const fn new(path: RelPath, sha256: Digest) -> Self {
        Self { path, sha256 }
    }
}

/// An outcome file the import wrote, found by its evaluator and request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct ImportedRequest {
    /// The evaluator, as `name@version`.
    pub evaluator: NameVersion,
    /// The request id of the outcome file.
    pub request: RequestId,
}

object_only!(ImportedRequest);

impl ImportedRequest {
    /// An imported request of `evaluator`.
    #[must_use]
    pub const fn new(evaluator: NameVersion, request: RequestId) -> Self {
        Self { evaluator, request }
    }
}

/// How an imported run was read: the format label, the base commit, the legacy integrity block, the source files and the outcome files written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct ImportRecord {
    /// The label of the legacy format, as `name@token`.
    pub format: NameToken,
    /// The commit of the legacy repository the run was read at.
    pub base_commit: GitCommit,
    /// The legacy integrity block as it was written; any JSON value except `null`.
    pub legacy_integrity: JsonValue,
    /// The files the import read, each stored as a blob.
    pub sources: Vec<ImportSource>,
    /// The outcome files the import wrote.
    pub requests: Vec<ImportedRequest>,
}

object_only!(ImportRecord);

impl ImportRecord {
    /// An import record with no source and no request yet.
    #[must_use]
    pub const fn new(
        format: NameToken,
        base_commit: GitCommit,
        legacy_integrity: JsonValue,
    ) -> Self {
        Self {
            format,
            base_commit,
            legacy_integrity,
            sources: Vec::new(),
            requests: Vec::new(),
        }
    }
}

/// A run's manifest: what it was, what it resolved and how it ended, with the version marker `run-manifest/1` in `schema`.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "RunManifest")]
struct RawRunManifest {
    /// The version marker of the document: always `run-manifest/1`.
    schema: RunManifestTag,
    /// The key of the run.
    run: RunKey,
    /// The digest of the experiment's sorted cell digests.
    experiment: Digest,
    /// The field paths that vary across the experiment's cells.
    factors: Vec<FieldPath>,
    /// What the cell digest hashes.
    comparable: Comparable,
    /// Where each value came from: one entry per value, and one or more entries under each typed root the cell holds; never an entry for a setting.
    #[schemars(extend(
        "$comment" = "Each path of `comparable.values` has exactly one entry, and each of `harness`, `treatment` and the non-empty lists `snapshots`, `inputs` and `environments` has an entry at the root path or at a path under it; no other path appears and no path repeats. JSON Schema cannot state this, and the Rust reader refuses a list that differs."
    ))]
    sources: Vec<SourceEntry>,
    /// The secrets the run needed and whether each was present; never their values.
    secrets: Vec<SecretPresence>,
    /// The values observed during the run, with the run's end under `run.ended_by` and `run.exit`.
    measured: Vec<MeasuredValue>,
    /// The re-checked values a forced resume accepted.
    deviations: Vec<Deviation>,
    /// When the run started.
    #[serde(default, deserialize_with = "present")]
    started_at: Option<Timestamp>,
    /// When the run ended.
    #[serde(default, deserialize_with = "present")]
    ended_at: Option<Timestamp>,
    /// The host that ran it.
    #[serde(default, deserialize_with = "present")]
    host: Option<String>,
    /// The job that ran it.
    #[serde(default, deserialize_with = "present")]
    job: Option<JobId>,
    /// Whether the run counts.
    integrity: Integrity,
    /// How an imported run was read.
    #[serde(default, deserialize_with = "present")]
    import: Option<ImportRecord>,
}

impl TryFrom<MapOnly<RawRunManifest>> for RunManifest {
    type Error = ExecutionError;

    fn try_from(raw: MapOnly<RawRunManifest>) -> Result<Self, Self::Error> {
        let RawRunManifest {
            schema: RunManifestTag,
            run,
            experiment,
            factors,
            comparable,
            sources,
            secrets,
            measured,
            deviations,
            started_at,
            ended_at,
            host,
            job,
            integrity,
            import,
        } = raw.0;
        // BORROW: explicit .as_slice() instead of Deref coercion
        check_sources(&comparable, sources.as_slice())?;
        Ok(Self {
            schema: RunManifestTag,
            run,
            experiment,
            factors,
            comparable,
            sources,
            secrets,
            measured,
            deviations,
            started_at,
            ended_at,
            host,
            job,
            integrity,
            import,
        })
    }
}

/// Reports whether `path` is a typed root or lies below one.
fn under_typed_root(path: &FieldPath) -> bool {
    let root = path.segments().next().unwrap_or_default();
    RESERVED_ROOTS.contains(&root)
}

/// The error for a source whose path is neither a value path nor at or below a typed root.
fn stray_source(path: &FieldPath) -> ExecutionError {
    ExecutionError::SourcesMismatch {
        path: path.to_string(),
        problem: "names no value and lies under no typed root",
    }
}

/// Checks that `sources` covers the cell: one entry for each value path, and an entry at the root path or below it for `harness`, `treatment` and each non-empty list among `snapshots`, `inputs` and `environments`. A source of any other path is refused, and so is a repeated path (I12).
fn check_sources(comparable: &Comparable, sources: &[SourceEntry]) -> Result<(), ExecutionError> {
    let mut own: BTreeSet<&FieldPath> = BTreeSet::new();
    for source in sources {
        if !own.insert(source.path()) {
            return Err(ExecutionError::Duplicate {
                list: "sources",
                value: source.path().to_string(),
            });
        }
    }
    let held: BTreeSet<&FieldPath> = comparable
        .values()
        .iter()
        .map(ResolvedValue::path)
        .collect();
    if let Some(path) = held.difference(&own).next() {
        return Err(ExecutionError::SourcesMismatch {
            path: path.to_string(),
            problem: "is missing",
        });
    }
    let root_of = |path: &FieldPath| path.segments().next().unwrap_or_default().to_owned(); // BORROW: .to_owned() keeps the root text
    let roots_with_source: BTreeSet<String> = own
        .iter()
        .filter(|path| !held.contains(**path))
        .map(|path| root_of(path))
        .collect();
    for path in own.iter().filter(|path| !held.contains(**path)) {
        if !under_typed_root(path) {
            return Err(stray_source(path));
        }
    }
    let required = [
        ("harness", true),
        ("treatment", true),
        ("snapshots", !comparable.snapshots().is_empty()),
        ("inputs", !comparable.inputs().is_empty()),
        ("environments", !comparable.environments().is_empty()),
    ];
    for (root, needed) in required {
        if needed && !roots_with_source.contains(root) {
            return Err(ExecutionError::SourcesMismatch {
                path: root.to_owned(), // BORROW: .to_owned() makes the owned path text
                problem: "is missing",
            });
        }
    }
    Ok(())
}

/// A run's manifest: what it was, what it resolved and how it ended, with the version marker `run-manifest/1` in `schema`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawRunManifest>")]
#[non_exhaustive]
pub struct RunManifest {
    schema: RunManifestTag,
    run: RunKey,
    experiment: Digest,
    factors: Vec<FieldPath>,
    comparable: Comparable,
    sources: Vec<SourceEntry>,
    secrets: Vec<SecretPresence>,
    measured: Vec<MeasuredValue>,
    deviations: Vec<Deviation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ended_at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    job: Option<JobId>,
    integrity: Integrity,
    #[serde(skip_serializing_if = "Option::is_none")]
    import: Option<ImportRecord>,
}

impl RunManifest {
    /// Starts a manifest; the run, the experiment digest, the cell and the integrity must be set
    /// before it builds.
    #[must_use]
    pub fn builder() -> RunManifestBuilder {
        RunManifestBuilder::default()
    }

    /// The digest of the cell: the only function that hashes one.
    #[must_use]
    pub fn cell_digest(&self) -> Digest {
        self.comparable.digest()
    }

    /// The key of the run.
    #[must_use]
    pub const fn run<'a>(&'a self) -> &'a RunKey {
        &self.run
    }

    /// The digest of the experiment.
    #[must_use]
    pub const fn experiment<'a>(&'a self) -> &'a Digest {
        &self.experiment
    }

    /// The field paths that vary across the experiment's cells.
    #[must_use]
    pub fn factors<'a>(&'a self) -> &'a [FieldPath] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.factors.as_slice()
    }

    /// What the cell digest hashes.
    #[must_use]
    pub const fn comparable<'a>(&'a self) -> &'a Comparable {
        &self.comparable
    }

    /// Where each value came from.
    #[must_use]
    pub fn sources<'a>(&'a self) -> &'a [SourceEntry] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.sources.as_slice()
    }

    /// The secrets the run needed.
    #[must_use]
    pub fn secrets<'a>(&'a self) -> &'a [SecretPresence] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.secrets.as_slice()
    }

    /// The values observed during the run.
    #[must_use]
    pub fn measured<'a>(&'a self) -> &'a [MeasuredValue] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.measured.as_slice()
    }

    /// The values a forced resume accepted although they differed.
    #[must_use]
    pub fn deviations<'a>(&'a self) -> &'a [Deviation] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.deviations.as_slice()
    }

    /// When the run started, if known.
    #[must_use]
    pub const fn started_at<'a>(&'a self) -> Option<&'a Timestamp> {
        self.started_at.as_ref()
    }

    /// When the run ended, if known.
    #[must_use]
    pub const fn ended_at<'a>(&'a self) -> Option<&'a Timestamp> {
        self.ended_at.as_ref()
    }

    /// The host that ran it, if known.
    #[must_use]
    pub fn host<'a>(&'a self) -> Option<&'a str> {
        self.host.as_deref()
    }

    /// The job that ran it, if any.
    #[must_use]
    pub const fn job<'a>(&'a self) -> Option<&'a JobId> {
        self.job.as_ref()
    }

    /// Whether the run counts.
    #[must_use]
    pub const fn integrity<'a>(&'a self) -> &'a Integrity {
        &self.integrity
    }

    /// How an imported run was read; absent for a live run.
    #[must_use]
    pub const fn import<'a>(&'a self) -> Option<&'a ImportRecord> {
        self.import.as_ref()
    }
}

/// Builds a [`RunManifest`].
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct RunManifestBuilder {
    run: Option<RunKey>,
    experiment: Option<Digest>,
    factors: Vec<FieldPath>,
    comparable: Option<Comparable>,
    sources: Vec<SourceEntry>,
    secrets: Vec<SecretPresence>,
    measured: Vec<MeasuredValue>,
    deviations: Vec<Deviation>,
    started_at: Option<Timestamp>,
    ended_at: Option<Timestamp>,
    host: Option<String>,
    job: Option<JobId>,
    integrity: Option<Integrity>,
    import: Option<ImportRecord>,
}

impl RunManifestBuilder {
    /// Sets the key of the run.
    #[must_use]
    pub fn run(mut self, run: RunKey) -> Self {
        self.run = Some(run);
        self
    }

    /// Sets the digest of the experiment.
    #[must_use]
    pub fn experiment(mut self, experiment: Digest) -> Self {
        self.experiment = Some(experiment);
        self
    }

    /// Adds a field path that varies across the experiment's cells.
    #[must_use]
    pub fn factor(mut self, factor: FieldPath) -> Self {
        self.factors.push(factor);
        self
    }

    /// Sets what the cell digest hashes.
    #[must_use]
    pub fn comparable(mut self, comparable: Comparable) -> Self {
        self.comparable = Some(comparable);
        self
    }

    /// Adds the source of a value.
    #[must_use]
    pub fn source(mut self, source: SourceEntry) -> Self {
        self.sources.push(source);
        self
    }

    /// Adds a secret the run needed.
    #[must_use]
    pub fn secret(mut self, secret: SecretPresence) -> Self {
        self.secrets.push(secret);
        self
    }

    /// Adds a value observed during the run.
    #[must_use]
    pub fn measured(mut self, measured: MeasuredValue) -> Self {
        self.measured.push(measured);
        self
    }

    /// Adds a deviation a forced resume accepted.
    #[must_use]
    pub fn deviation(mut self, deviation: Deviation) -> Self {
        self.deviations.push(deviation);
        self
    }

    /// Sets when the run started.
    #[must_use]
    pub fn started_at(mut self, started_at: Timestamp) -> Self {
        self.started_at = Some(started_at);
        self
    }

    /// Sets when the run ended.
    #[must_use]
    pub fn ended_at(mut self, ended_at: Timestamp) -> Self {
        self.ended_at = Some(ended_at);
        self
    }

    /// Sets the host that ran it.
    #[must_use]
    pub fn host(mut self, host: String) -> Self {
        self.host = Some(host);
        self
    }

    /// Sets the job that ran it.
    #[must_use]
    pub fn job(mut self, job: JobId) -> Self {
        self.job = Some(job);
        self
    }

    /// Sets whether the run counts.
    #[must_use]
    pub fn integrity(mut self, integrity: Integrity) -> Self {
        self.integrity = Some(integrity);
        self
    }

    /// Sets how an imported run was read.
    #[must_use]
    pub fn import(mut self, import: ImportRecord) -> Self {
        self.import = Some(import);
        self
    }

    /// Builds the manifest.
    ///
    /// # Errors
    /// Returns [`ExecutionError::MissingField`] if the run, the experiment digest, the cell or the
    /// integrity is not set, [`ExecutionError::SourcesMismatch`] if the sources do not cover the cell's values and
    /// typed roots or name another path, and [`ExecutionError::Duplicate`] if a path has two
    /// sources.
    pub fn build(self) -> Result<RunManifest, ExecutionError> {
        let run = self
            .run
            .ok_or(ExecutionError::MissingField { field: "run" })?;
        let experiment = self.experiment.ok_or(ExecutionError::MissingField {
            field: "experiment",
        })?;
        let comparable = self.comparable.ok_or(ExecutionError::MissingField {
            field: "comparable",
        })?;
        let integrity = self
            .integrity
            .ok_or(ExecutionError::MissingField { field: "integrity" })?;
        // BORROW: explicit .as_slice() instead of Deref coercion
        check_sources(&comparable, self.sources.as_slice())?;
        Ok(RunManifest {
            schema: RunManifestTag,
            run,
            experiment,
            factors: self.factors,
            comparable,
            sources: self.sources,
            secrets: self.secrets,
            measured: self.measured,
            deviations: self.deviations,
            started_at: self.started_at,
            ended_at: self.ended_at,
            host: self.host,
            job: self.job,
            integrity,
            import: self.import,
        })
    }
}

schema_tag! {
    /// The constant `schema` field of a run tree.
    pub RunTreeTag = RUN_TREE_SCHEMA
}

/// One path of a run's workspace when the run ended: a file with its content digest, or a directory that holds nothing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum TreeEntry {
    /// A file: the store keeps its bytes as the blob `blobs/<hex of sha256>`.
    #[non_exhaustive]
    File {
        /// The run-relative path of the file.
        path: RelPath,
        /// The digest of the file's bytes.
        sha256: Digest,
        /// Whether the file had an execute bit.
        executable: bool,
    },
    /// A directory that holds nothing; a directory with content is implied by its entries.
    #[non_exhaustive]
    Dir {
        /// The run-relative path of the directory.
        path: RelPath,
    },
}

object_only!(TreeEntry);

impl TreeEntry {
    /// A file entry.
    #[must_use]
    pub const fn file(path: RelPath, sha256: Digest, executable: bool) -> Self {
        Self::File {
            path,
            sha256,
            executable,
        }
    }

    /// An empty directory entry.
    #[must_use]
    pub const fn dir(path: RelPath) -> Self {
        Self::Dir { path }
    }

    /// The run-relative path of the entry.
    #[must_use]
    pub const fn path<'a>(&'a self) -> &'a RelPath {
        match self {
            Self::File { path, .. } | Self::Dir { path } => path,
        }
    }

    /// Reports whether the entry is a file.
    #[must_use]
    pub const fn is_file(&self) -> bool {
        matches!(self, Self::File { .. })
    }
}

/// The workspace of a run when it ended: every file with its content digest and every empty directory, sorted by path, with the version marker `run-tree/1` in `schema`.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "RunTree")]
struct RawRunTree {
    /// The version marker of the document: always `run-tree/1`.
    schema: RunTreeTag,
    /// The entries, sorted by path text without a repeat; no entry lies below a file or below an empty directory.
    entries: Vec<TreeEntry>,
}

/// The workspace of a run when it ended: every file with its content digest and every empty directory, sorted by path, with the version marker `run-tree/1` in `schema`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawRunTree>")]
#[non_exhaustive]
pub struct RunTree {
    schema: RunTreeTag,
    entries: Vec<TreeEntry>,
}

impl TryFrom<MapOnly<RawRunTree>> for RunTree {
    type Error = ExecutionError;

    fn try_from(raw: MapOnly<RawRunTree>) -> Result<Self, Self::Error> {
        let RawRunTree {
            schema: RunTreeTag,
            entries,
        } = raw.0;
        check_ascending(
            "entries",
            "path",
            entries.as_slice(), // BORROW: explicit .as_slice() instead of Deref coercion
            TreeEntry::path,
            |entry| entry.path().to_string(),
        )?;
        check_ancestors(entries.as_slice())?; // BORROW: explicit .as_slice()
        Ok(Self {
            schema: RunTreeTag,
            entries,
        })
    }
}

/// Checks that no entry lies below a file or below an entry listed as an empty directory.
fn check_ancestors(entries: &[TreeEntry]) -> Result<(), ExecutionError> {
    let by_path: BTreeMap<&str, &TreeEntry> = entries
        .iter()
        .map(|entry| (entry.path().as_str(), entry)) // BORROW: explicit .as_str()
        .collect();
    for entry in entries {
        let path = entry.path().as_str(); // BORROW: explicit .as_str()
        for (slash, _) in path.match_indices('/') {
            let Some(ancestor) = path.get(..slash) else {
                continue;
            };
            match by_path.get(ancestor) {
                Some(found) if found.is_file() => {
                    return Err(ExecutionError::PathUnderFile {
                        path: path.to_owned(),     // BORROW: error carries owned text
                        file: ancestor.to_owned(), // BORROW: error carries owned text
                    });
                }
                Some(_) => {
                    return Err(ExecutionError::DirNotEmpty {
                        dir: ancestor.to_owned(), // BORROW: error carries owned text
                        path: path.to_owned(),    // BORROW: error carries owned text
                    });
                }
                // EXPLICIT: no entry at this ancestor, so nothing is in the way
                None => {}
            }
        }
    }
    Ok(())
}

impl RunTree {
    /// Starts a tree with no entry.
    #[must_use]
    pub fn builder() -> RunTreeBuilder {
        RunTreeBuilder::default()
    }

    /// The entries, sorted by path.
    #[must_use]
    pub fn entries<'a>(&'a self) -> &'a [TreeEntry] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.entries.as_slice()
    }
}

/// Builds a [`RunTree`]: it sorts the entries by path.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct RunTreeBuilder {
    entries: Vec<TreeEntry>,
}

impl RunTreeBuilder {
    /// Adds an entry.
    #[must_use]
    pub fn entry(mut self, entry: TreeEntry) -> Self {
        self.entries.push(entry);
        self
    }

    /// Builds the tree.
    ///
    /// # Errors
    /// Returns [`ExecutionError::Duplicate`] if a path is listed twice,
    /// [`ExecutionError::PathUnderFile`] if an entry lies below a file and
    /// [`ExecutionError::DirNotEmpty`] if an entry lies below an empty directory.
    pub fn build(mut self) -> Result<RunTree, ExecutionError> {
        self.entries.sort_by(|a, b| a.path().cmp(b.path()));
        check_ascending(
            "entries",
            "path",
            self.entries.as_slice(), // BORROW: explicit .as_slice() instead of Deref coercion
            TreeEntry::path,
            |entry| entry.path().to_string(),
        )?;
        check_ancestors(self.entries.as_slice())?; // BORROW: explicit .as_slice()
        Ok(RunTree {
            schema: RunTreeTag,
            entries: self.entries,
        })
    }
}

schema_tag! {
    /// The constant `schema` field of a job record.
    pub JobRecordTag = JOB_RECORD_SCHEMA
}

/// Whether the supervisor of a job is still running it: `running` or `ended`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum JobState {
    /// The supervisor is running the job.
    Running,
    /// The job ended; its events say how.
    Ended,
}

/// How a job ended: `completed`, `budget_stop`, `killed` or `failed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum JobEnd {
    /// Every run finished.
    Completed,
    /// The budget stopped the job.
    BudgetStop,
    /// The job was killed.
    Killed,
    /// The job failed.
    Failed,
}

/// A step of the ladder that stops a process: `pre_stop`, `signal`, `terminate_group`, `kill_group` or `sweep`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StopStep {
    /// The harness's own stop command ran.
    PreStop,
    /// The process was signalled.
    Signal,
    /// The process group was asked to terminate.
    TerminateGroup,
    /// The process group was killed.
    KillGroup,
    /// The sweep killed what was left of the group.
    Sweep,
}

/// The supervisor of a job: its process id, which is at least 1, and when it started.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "SupervisorIdentity")]
struct RawSupervisorIdentity {
    /// The process id of the supervisor, at least 1.
    #[schemars(range(min = 1))]
    pid: u32,
    /// When the supervisor started, so a reused process id is never taken for it.
    start: ProcessStart,
}

/// The supervisor of a job: its process id, which is at least 1, and when it started.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawSupervisorIdentity>")]
#[non_exhaustive]
pub struct SupervisorIdentity {
    pid: u32,
    start: ProcessStart,
}

impl SupervisorIdentity {
    /// A supervisor identity.
    ///
    /// # Errors
    /// Returns [`ExecutionError::Zero`] if `pid` is 0.
    pub fn new(pid: u32, start: ProcessStart) -> Result<Self, ExecutionError> {
        if pid == 0 {
            return Err(ExecutionError::Zero { field: "pid" });
        }
        Ok(Self { pid, start })
    }

    /// The process id of the supervisor.
    #[must_use]
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// When the supervisor started.
    #[must_use]
    pub const fn start<'a>(&'a self) -> &'a ProcessStart {
        &self.start
    }
}

impl TryFrom<MapOnly<RawSupervisorIdentity>> for SupervisorIdentity {
    type Error = ExecutionError;

    fn try_from(raw: MapOnly<RawSupervisorIdentity>) -> Result<Self, Self::Error> {
        let RawSupervisorIdentity { pid, start } = raw.0;
        Self::new(pid, start)
    }
}

/// The record a job directory keeps of its supervisor: the job id, the supervisor, the digest of the job spec and whether the job still runs, with the version marker `job-record/1` in `schema`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct JobRecord {
    /// The version marker of the document: always `job-record/1`.
    schema: JobRecordTag,
    /// The id of the job.
    id: JobId,
    /// The process that supervises the job.
    supervisor: SupervisorIdentity,
    /// The digest of the job spec the supervisor was started with.
    spec_digest: Digest,
    /// Whether the job still runs.
    state: JobState,
}

object_only!(JobRecord);

impl JobRecord {
    /// Starts a record; every field must be set before it builds.
    #[must_use]
    pub fn builder() -> JobRecordBuilder {
        JobRecordBuilder::default()
    }

    /// The id of the job.
    #[must_use]
    pub const fn id<'a>(&'a self) -> &'a JobId {
        &self.id
    }

    /// The supervisor of the job.
    #[must_use]
    pub const fn supervisor<'a>(&'a self) -> &'a SupervisorIdentity {
        &self.supervisor
    }

    /// The digest of the job spec.
    #[must_use]
    pub const fn spec_digest<'a>(&'a self) -> &'a Digest {
        &self.spec_digest
    }

    /// Whether the job still runs.
    #[must_use]
    pub const fn state(&self) -> JobState {
        self.state
    }
}

/// Builds a [`JobRecord`].
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct JobRecordBuilder {
    id: Option<JobId>,
    supervisor: Option<SupervisorIdentity>,
    spec_digest: Option<Digest>,
    state: Option<JobState>,
}

impl JobRecordBuilder {
    /// Sets the id of the job.
    #[must_use]
    pub fn id(mut self, id: JobId) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the supervisor.
    #[must_use]
    pub fn supervisor(mut self, supervisor: SupervisorIdentity) -> Self {
        self.supervisor = Some(supervisor);
        self
    }

    /// Sets the digest of the job spec.
    #[must_use]
    pub fn spec_digest(mut self, spec_digest: Digest) -> Self {
        self.spec_digest = Some(spec_digest);
        self
    }

    /// Sets whether the job still runs.
    #[must_use]
    pub fn state(mut self, state: JobState) -> Self {
        self.state = Some(state);
        self
    }

    /// Builds the record.
    ///
    /// # Errors
    /// Returns [`ExecutionError::MissingField`] if a field is not set.
    pub fn build(self) -> Result<JobRecord, ExecutionError> {
        Ok(JobRecord {
            schema: JobRecordTag,
            id: self
                .id
                .ok_or(ExecutionError::MissingField { field: "id" })?,
            supervisor: self.supervisor.ok_or(ExecutionError::MissingField {
                field: "supervisor",
            })?,
            spec_digest: self.spec_digest.ok_or(ExecutionError::MissingField {
                field: "spec_digest",
            })?,
            state: self
                .state
                .ok_or(ExecutionError::MissingField { field: "state" })?,
        })
    }
}

/// The constant `v` field of an event envelope: the number 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnvelopeVersion;

impl Serialize for EnvelopeVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(1)
    }
}

impl<'de> Deserialize<'de> for EnvelopeVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match u64::deserialize(deserializer)? {
            1 => Ok(Self),
            other => Err(<D::Error as serde::de::Error>::custom(format!(
                "envelope version is {other} (this reader reads 1)"
            ))),
        }
    }
}

impl JsonSchema for EnvelopeVersion {
    fn schema_name() -> Cow<'static, str> {
        "EnvelopeVersion".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "integer",
            "const": 1,
        })
    }
}

/// A process a run left behind: its process id, which is at least 1, and when it started.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "StrayProcess")]
struct RawStrayProcess {
    /// The process id, at least 1.
    #[schemars(range(min = 1))]
    pid: u32,
    /// When the process started, so a reused process id is never taken for it.
    start: ProcessStart,
}

/// A process a run left behind: its process id, which is at least 1, and when it started.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawStrayProcess>")]
#[non_exhaustive]
pub struct StrayProcess {
    pid: u32,
    start: ProcessStart,
}

impl StrayProcess {
    /// A stray process.
    ///
    /// # Errors
    /// Returns [`ExecutionError::Zero`] if `pid` is 0.
    pub fn new(pid: u32, start: ProcessStart) -> Result<Self, ExecutionError> {
        if pid == 0 {
            return Err(ExecutionError::Zero { field: "pid" });
        }
        Ok(Self { pid, start })
    }

    /// The process id.
    #[must_use]
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// When the process started.
    #[must_use]
    pub const fn start<'a>(&'a self) -> &'a ProcessStart {
        &self.start
    }
}

impl TryFrom<MapOnly<RawStrayProcess>> for StrayProcess {
    type Error = ExecutionError;

    fn try_from(raw: MapOnly<RawStrayProcess>) -> Result<Self, Self::Error> {
        let RawStrayProcess { pid, start } = raw.0;
        Self::new(pid, start)
    }
}

/// What happened, as an object with the event's name under `kind` and its members under `payload`; a kind this version does not know is read as [`EventPayload::Unknown`], which keeps its whole payload, so that writing reproduces what was read.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum EventPayload {
    /// A job started.
    #[non_exhaustive]
    JobStarted {},
    /// A job ended.
    #[non_exhaustive]
    JobEnded {
        /// How it ended.
        reason: JobEnd,
    },
    /// A run started.
    #[non_exhaustive]
    RunStarted {},
    /// A run ended.
    #[non_exhaustive]
    RunEnded {
        /// How the session ended.
        ended_by: EndedBy,
        /// How the harness process ended, if it did.
        exit: Option<ExitStatus>,
    },
    /// A run could not be carried out.
    #[non_exhaustive]
    RunFailed {
        /// Why it failed.
        reason: ReasonCode,
    },
    /// Processes of a run outlived it.
    #[non_exhaustive]
    RunStray {
        /// The processes found, each by process id and start time.
        processes: Vec<StrayProcess>,
    },
    /// An evaluator gave its result for a run.
    #[non_exhaustive]
    EvaluationOutcome {
        /// The result.
        record: Box<OutcomeRecord>,
    },
    /// A child process was spawned.
    #[non_exhaustive]
    ProcessSpawned {
        /// The record the supervisor wrote for the child.
        record: ChildRecord,
    },
    /// A step of the ladder that stops a process was taken.
    #[non_exhaustive]
    ProcessStopStep {
        /// The process id of the child the step was taken on, at least 1.
        pid: u32,
        /// When the child started, which with `pid` tells it from a reused process id.
        start: ProcessStart,
        /// The step.
        step: StopStep,
        /// When it was taken.
        at: Timestamp,
        /// What came of it, if the step reports; free text, such as `no sweeper`.
        outcome: Option<String>,
    },
    /// A child process exited.
    #[non_exhaustive]
    ProcessExited {
        /// The process id of the child, at least 1.
        pid: u32,
        /// When the child started, which with `pid` tells it from a reused process id.
        start: ProcessStart,
        /// How it ended.
        status: ExitStatus,
        /// Whether its process group was empty afterwards.
        group_empty: bool,
    },
    /// A kind this version does not know, with its payload as written.
    #[non_exhaustive]
    Unknown {
        /// The kind as written.
        kind: String,
        /// The payload as written; any JSON value.
        payload: Value,
    },
}

/// The kinds this version knows, as written.
const KNOWN_EVENT_KINDS: [&str; 10] = [
    "job.started",
    "job.ended",
    "run.started",
    "run.ended",
    "run.failed",
    "run.stray",
    "evaluation.outcome",
    "process.spawned",
    "process.stop_step",
    "process.exited",
];

/// Refuses a process id of 0.
const fn check_pid(pid: u32) -> Result<(), ExecutionError> {
    if pid == 0 {
        Err(ExecutionError::Zero { field: "pid" })
    } else {
        Ok(())
    }
}

impl EventPayload {
    /// A job started.
    #[must_use]
    pub const fn job_started() -> Self {
        Self::JobStarted {}
    }

    /// A job ended.
    #[must_use]
    pub const fn job_ended(reason: JobEnd) -> Self {
        Self::JobEnded { reason }
    }

    /// A run started.
    #[must_use]
    pub const fn run_started() -> Self {
        Self::RunStarted {}
    }

    /// A run ended.
    #[must_use]
    pub const fn run_ended(ended_by: EndedBy, exit: Option<ExitStatus>) -> Self {
        Self::RunEnded { ended_by, exit }
    }

    /// A run failed.
    #[must_use]
    pub const fn run_failed(reason: ReasonCode) -> Self {
        Self::RunFailed { reason }
    }

    /// Processes of a run outlived it.
    #[must_use]
    pub const fn run_stray(processes: Vec<StrayProcess>) -> Self {
        Self::RunStray { processes }
    }

    /// An evaluator's result.
    #[must_use]
    pub fn evaluation_outcome(record: OutcomeRecord) -> Self {
        Self::EvaluationOutcome {
            record: Box::new(record),
        }
    }

    /// A child process was spawned.
    #[must_use]
    pub const fn process_spawned(record: ChildRecord) -> Self {
        Self::ProcessSpawned { record }
    }

    /// A step of the stop ladder was taken on the child `pid` that started at `start`.
    ///
    /// # Errors
    /// Returns [`ExecutionError::Zero`] if `pid` is 0.
    pub fn process_stop_step(
        pid: u32,
        start: ProcessStart,
        step: StopStep,
        at: Timestamp,
        outcome: Option<String>,
    ) -> Result<Self, ExecutionError> {
        check_pid(pid)?;
        Ok(Self::ProcessStopStep {
            pid,
            start,
            step,
            at,
            outcome,
        })
    }

    /// The child `pid` that started at `start` exited.
    ///
    /// # Errors
    /// Returns [`ExecutionError::Zero`] if `pid` is 0.
    pub fn process_exited(
        pid: u32,
        start: ProcessStart,
        status: ExitStatus,
        group_empty: bool,
    ) -> Result<Self, ExecutionError> {
        check_pid(pid)?;
        Ok(Self::ProcessExited {
            pid,
            start,
            status,
            group_empty,
        })
    }

    /// A kind this version does not know.
    ///
    /// # Errors
    /// Returns [`ExecutionError::KnownKind`] if `kind` is a kind this version knows.
    pub fn unknown(kind: String, payload: Value) -> Result<Self, ExecutionError> {
        // BORROW: explicit .as_str() instead of Deref coercion
        if KNOWN_EVENT_KINDS.contains(&kind.as_str()) {
            Err(ExecutionError::KnownKind { kind })
        } else {
            Ok(Self::Unknown { kind, payload })
        }
    }

    /// The kind of the event as written.
    #[must_use]
    pub fn kind<'a>(&'a self) -> &'a str {
        match self {
            Self::JobStarted {} => "job.started",
            Self::JobEnded { .. } => "job.ended",
            Self::RunStarted {} => "run.started",
            Self::RunEnded { .. } => "run.ended",
            Self::RunFailed { .. } => "run.failed",
            Self::RunStray { .. } => "run.stray",
            Self::EvaluationOutcome { .. } => "evaluation.outcome",
            Self::ProcessSpawned { .. } => "process.spawned",
            Self::ProcessStopStep { .. } => "process.stop_step",
            Self::ProcessExited { .. } => "process.exited",
            Self::Unknown { kind, .. } => kind.as_str(), // BORROW: explicit .as_str()
        }
    }
}

/// What happened, as an object whose `kind` is one of the names below and whose `payload` is an object with the members of that kind.
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(
    remote = "EventPayload",
    tag = "kind",
    content = "payload",
    rename_all = "snake_case"
)]
#[schemars(deny_unknown_fields, rename = "EventPayload")]
enum KnownPayload {
    /// A job started.
    #[serde(rename = "job.started")]
    JobStarted {},
    /// A job ended.
    #[serde(rename = "job.ended")]
    JobEnded {
        /// How it ended.
        reason: JobEnd,
    },
    /// A run started.
    #[serde(rename = "run.started")]
    RunStarted {},
    /// A run ended.
    #[serde(rename = "run.ended")]
    RunEnded {
        /// How the session ended.
        ended_by: EndedBy,
        /// How the harness process ended, if it did.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        exit: Option<ExitStatus>,
    },
    /// A run could not be carried out.
    #[serde(rename = "run.failed")]
    RunFailed {
        /// Why it failed.
        reason: ReasonCode,
    },
    /// Processes of a run outlived it.
    #[serde(rename = "run.stray")]
    RunStray {
        /// The processes found, each by process id and start time.
        processes: Vec<StrayProcess>,
    },
    /// An evaluator gave its result for a run.
    #[serde(rename = "evaluation.outcome")]
    EvaluationOutcome {
        /// The result.
        record: Box<OutcomeRecord>,
    },
    /// A child process was spawned.
    #[serde(rename = "process.spawned")]
    ProcessSpawned {
        /// The record the supervisor wrote for the child.
        record: ChildRecord,
    },
    /// A step of the ladder that stops a process was taken.
    #[serde(rename = "process.stop_step")]
    ProcessStopStep {
        /// The process id of the child the step was taken on, at least 1.
        #[schemars(range(min = 1))]
        pid: u32,
        /// When the child started, which with `pid` tells it from a reused process id.
        start: ProcessStart,
        /// The step.
        step: StopStep,
        /// When it was taken.
        at: Timestamp,
        /// What came of it, if the step reports; free text, such as `no sweeper`.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        outcome: Option<String>,
    },
    /// A child process exited.
    #[serde(rename = "process.exited")]
    ProcessExited {
        /// The process id of the child, at least 1.
        #[schemars(range(min = 1))]
        pid: u32,
        /// When the child started, which with `pid` tells it from a reused process id.
        start: ProcessStart,
        /// How it ended.
        status: ExitStatus,
        /// Whether its process group was empty afterwards.
        group_empty: bool,
    },
    /// A kind this version does not know; it is not part of the schema.
    #[serde(skip)]
    Unknown { kind: String, payload: Value },
}

impl Serialize for EventPayload {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Unknown { kind, payload } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("kind", kind)?;
                map.serialize_entry("payload", payload)?;
                map.end()
            }
            Self::JobStarted {}
            | Self::JobEnded { .. }
            | Self::RunStarted {}
            | Self::RunEnded { .. }
            | Self::RunFailed { .. }
            | Self::RunStray { .. }
            | Self::EvaluationOutcome { .. }
            | Self::ProcessSpawned { .. }
            | Self::ProcessStopStep { .. }
            | Self::ProcessExited { .. } => KnownPayload::serialize(self, serializer),
        }
    }
}

impl<'de> Deserialize<'de> for EventPayload {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let mut map = Map::<String, Value>::deserialize(deserializer)?;
        let kind = match map.remove("kind") {
            Some(Value::String(kind)) => kind,
            Some(_) => return Err(D::Error::custom("`kind` is not a string")),
            None => return Err(D::Error::custom("missing field `kind`")),
        };
        let payload = map
            .remove("payload")
            .ok_or_else(|| D::Error::custom("missing field `payload`"))?;
        // BORROW: explicit .as_str() instead of Deref coercion
        if KNOWN_EVENT_KINDS.contains(&kind.as_str()) {
            let mut tagged = Map::new();
            tagged.insert(String::from("kind"), Value::String(kind));
            tagged.insert(String::from("payload"), payload);
            let event =
                KnownPayload::deserialize(Value::Object(tagged)).map_err(D::Error::custom)?;
            match &event {
                Self::ProcessStopStep { pid, .. } | Self::ProcessExited { pid, .. } => {
                    check_pid(*pid).map_err(D::Error::custom)?;
                }
                Self::JobStarted {}
                | Self::JobEnded { .. }
                | Self::RunStarted {}
                | Self::RunEnded { .. }
                | Self::RunFailed { .. }
                | Self::RunStray { .. }
                | Self::EvaluationOutcome { .. }
                | Self::ProcessSpawned { .. }
                | Self::Unknown { .. } => {} // EXPLICIT: no process id to check
            }
            Ok(event)
        } else {
            Ok(Self::Unknown { kind, payload })
        }
    }
}

impl JsonSchema for EventPayload {
    fn schema_name() -> Cow<'static, str> {
        KnownPayload::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        KnownPayload::schema_id()
    }

    fn inline_schema() -> bool {
        KnownPayload::inline_schema()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        KnownPayload::json_schema(generator)
    }
}

/// One line of a job's event log: the format version, the place of the event in the log, when it happened, the job and run it belongs to, and what happened, whose `kind` and `payload` are written beside the other members.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[non_exhaustive]
pub struct Envelope {
    /// The format version: always 1.
    v: EnvelopeVersion,
    /// The position of the event in the log of its job, counting from 0 without gaps.
    #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
    seq: u64,
    /// When the event happened.
    at: Timestamp,
    /// The job the event belongs to.
    job_id: JobId,
    /// The run the event belongs to, if it belongs to one.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    run: Option<RunKey>,
    /// What happened.
    #[serde(flatten)]
    event: EventPayload,
}

object_only!(Envelope);

impl Envelope {
    /// Starts an envelope; the place, the time, the job and the event must be set before it builds.
    #[must_use]
    pub fn builder() -> EnvelopeBuilder {
        EnvelopeBuilder::default()
    }

    /// The position of the event in the log of its job.
    #[must_use]
    pub const fn seq(&self) -> u64 {
        self.seq
    }

    /// When the event happened.
    #[must_use]
    pub const fn at<'a>(&'a self) -> &'a Timestamp {
        &self.at
    }

    /// The job the event belongs to.
    #[must_use]
    pub const fn job_id<'a>(&'a self) -> &'a JobId {
        &self.job_id
    }

    /// The run the event belongs to, if it belongs to one.
    #[must_use]
    pub const fn run<'a>(&'a self) -> Option<&'a RunKey> {
        self.run.as_ref()
    }

    /// What happened.
    #[must_use]
    pub const fn event<'a>(&'a self) -> &'a EventPayload {
        &self.event
    }
}

/// Builds an [`Envelope`].
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct EnvelopeBuilder {
    seq: Option<u64>,
    at: Option<Timestamp>,
    job_id: Option<JobId>,
    run: Option<RunKey>,
    event: Option<EventPayload>,
}

impl EnvelopeBuilder {
    /// Sets the position of the event in the log.
    #[must_use]
    pub const fn seq(mut self, seq: u64) -> Self {
        self.seq = Some(seq);
        self
    }

    /// Sets when the event happened.
    #[must_use]
    pub fn at(mut self, at: Timestamp) -> Self {
        self.at = Some(at);
        self
    }

    /// Sets the job.
    #[must_use]
    pub fn job_id(mut self, job_id: JobId) -> Self {
        self.job_id = Some(job_id);
        self
    }

    /// Sets the run.
    #[must_use]
    pub fn run(mut self, run: RunKey) -> Self {
        self.run = Some(run);
        self
    }

    /// Sets what happened.
    #[must_use]
    pub fn event(mut self, event: EventPayload) -> Self {
        self.event = Some(event);
        self
    }

    /// Builds the envelope.
    ///
    /// # Errors
    /// Returns [`ExecutionError::MissingField`] if the position, the time, the job or the event is
    /// not set.
    pub fn build(self) -> Result<Envelope, ExecutionError> {
        Ok(Envelope {
            v: EnvelopeVersion,
            seq: self
                .seq
                .ok_or(ExecutionError::MissingField { field: "seq" })?,
            at: self
                .at
                .ok_or(ExecutionError::MissingField { field: "at" })?,
            job_id: self
                .job_id
                .ok_or(ExecutionError::MissingField { field: "job_id" })?,
            run: self.run,
            event: self
                .event
                .ok_or(ExecutionError::MissingField { field: "event" })?,
        })
    }
}

#[cfg(test)]
mod guards {
    use super::{Envelope, EventPayload, ExecutionError};
    use serde_json::json;

    /// Guards: I18 — an envelope whose kind this version does not know is read with its payload intact and written back as the same bytes, and a known kind is never read as an unknown one. Oracle: a hand-written envelope line whose payload keys are in the sorted order the writer produces, with a `null` and a nested list inside the payload.
    #[test]
    fn unknown_event_kind() {
        let line = concat!(
            r#"{"v":1,"seq":7,"at":"2026-01-02T03:04:05Z","job_id":"018f2b3c-4d5e-7a6b-8c7d-9e0f1a2b3c4d","#,
            r#""run":{"cell":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","replicate":1,"attempt":2},"#,
            r#""kind":"cache.warmed","payload":{"a":[1,2,{"b":null}],"z":"x"}}"#,
        );
        let envelope: Envelope = serde_json::from_str(line).expect("the line reads");
        assert_eq!(envelope.seq(), 7);
        let EventPayload::Unknown { kind, payload } = envelope.event() else {
            panic!("read as a known kind: {:?}", envelope.event());
        };
        assert_eq!(kind, "cache.warmed");
        assert_eq!(payload, &json!({"a": [1, 2, {"b": null}], "z": "x"}));
        assert_eq!(serde_json::to_string(&envelope).expect("writes"), line);

        // A known kind with a payload of the wrong shape is refused, not kept as unknown.
        let wrong = line.replace("cache.warmed", "job.ended");
        let refused = serde_json::from_str::<Envelope>(wrong.as_str()).expect_err("wrong payload"); // BORROW: explicit .as_str()
        assert!(refused.to_string().contains("reason"), "{refused}");
        for listed in [
            r#""kind":"job.started","payload":[]"#,
            r#""kind":"job.ended","payload":["completed"]"#,
        ] {
            let text = line.replace(
                r#""kind":"cache.warmed","payload":{"a":[1,2,{"b":null}],"z":"x"}"#,
                listed,
            );
            // BORROW: explicit .as_str() instead of Deref coercion
            assert!(
                serde_json::from_str::<Envelope>(text.as_str()).is_err(),
                "{listed}"
            );
        }

        // The constructor refuses a known kind, which would not read back as unknown.
        assert!(matches!(
            EventPayload::unknown(String::from("run.ended"), json!({})),
            Err(ExecutionError::KnownKind { .. })
        ));
        assert!(EventPayload::unknown(String::from("cache.warmed"), json!(null)).is_ok());
    }
}
