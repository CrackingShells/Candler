//! The experiment file, the run manifest and its cell digest, the job record and the event envelope.
//!
//! An [`ExperimentFile`] is the TOML document that declares fixed values, factors with their
//! levels, replicates and a budget; its cells are a pure function of the file. An optional field
//! is omitted when absent and never written as `null`; the reader refuses an explicit `null`.
//! Unknown fields are ignored on input, and every struct and every tagged enum is read from an
//! object only, never from a list.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Number, Value};

use crate::canonical::{self, CanonicalError};
use crate::generate::GeneratedSchema;
use crate::map_only::MapOnly;
use crate::primitives::{
    Digest, FieldPath, FormatError, Name, SecretName, object_only, present, schema_tag, text_type,
};
use crate::values::{JsonValue, ResolvedValue, ValuesError};

/// The version marker of an experiment file, written in its `schema` field.
pub const EXPERIMENT_SCHEMA: &str = "experiment/1";

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
