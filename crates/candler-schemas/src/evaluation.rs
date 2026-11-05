//! The plug-in manifest, the suite file and the evaluator protocol messages.
//!
//! A [`PluginManifest`] and a [`SuiteFile`] are standalone documents read from TOML or JSON; each
//! carries its version marker in `schema`. A suite's dependency graph is checked when the file is
//! read: an id appears once, every dependency names an entry, and no entry depends on itself
//! through others. [`SuiteEntry::digest`] is the digest of what an entry asks for, and leaves out
//! the fields that only name or place it.
//!
//! An optional field is omitted when absent and never written as `null`; the reader refuses an
//! explicit `null`. Unknown fields are ignored on input, and every struct and every tagged enum is
//! read from an object only, never from a list.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::canonical::{self, CanonicalError};
use crate::generate::GeneratedSchema;
use crate::map_only::MapOnly;
use crate::outcome::{Cause, Instrument, OutcomeRecord, Role};
use crate::primitives::{
    Digest, ExpectationId, FieldPath, Name, RelPath, RequestId, RunKey, SEMVER_PATTERN,
    object_only, present, schema_tag,
};
use crate::raw::StreamName;
use crate::values::JsonValue;

/// The version marker every plug-in manifest carries in its `schema` field.
pub const PLUGIN_MANIFEST_SCHEMA: &str = "plugin-manifest/1";

/// The version marker every suite file carries in its `schema` field.
pub const SUITE_SCHEMA: &str = "suite/1";

/// The method of the request that asks an evaluator to describe itself.
pub const METHOD_DESCRIBE: &str = "describe";

/// The method of the request that asks an evaluator to grade items.
pub const METHOD_EVALUATE: &str = "evaluate";

/// The method of the notification that carries one outcome record.
pub const METHOD_OUTCOME: &str = "outcome";

/// The method of the notification that reports progress.
pub const METHOD_PROGRESS: &str = "progress";

/// The method of the notification that carries a log line.
pub const METHOD_LOG: &str = "log";

/// The method of the notification that cancels a request.
pub const METHOD_CANCEL: &str = "cancel";

/// The JSON-RPC error code of a request that was cancelled.
pub const ERROR_REQUEST_CANCELLED: i32 = -32800;

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    vec![
        GeneratedSchema::of::<PluginManifest>("plugin-manifest"),
        GeneratedSchema::of::<SuiteFile>("suite"),
        GeneratedSchema::of::<DescribeResult>("evaluator-describe-result"),
        GeneratedSchema::of::<EvaluateParams>("evaluator-evaluate-params"),
        GeneratedSchema::of::<Notification>("evaluator-notification"),
        GeneratedSchema::of::<RequestSide>("evaluator-request-side"),
    ]
}

/// Why an evaluation record could not be built or read.
///
/// Each variant names what is wrong and the ids or fields involved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EvaluationError {
    /// A document that is not valid TOML or does not have the shape of its type.
    #[error("document is not readable ({detail})")]
    Toml {
        /// The reader's explanation.
        detail: String,
    },
    /// A builder that was asked to build before a required field was set.
    #[error("evaluation record field is missing (`{field}` was not set)")]
    MissingField {
        /// The name of the field.
        field: &'static str,
    },
    /// A list that must hold at least one member and holds none.
    #[error("list is empty (`{field}` needs at least one member)")]
    EmptyList {
        /// The name of the field.
        field: &'static str,
    },
    /// Two suite entries that share an id.
    #[error("suite repeats an id (got {id})")]
    DuplicateId {
        /// The id that appears more than once.
        id: ExpectationId,
    },
    /// A suite entry that depends on an id no entry has.
    #[error("suite entry {id} depends on {dependency}, which no entry has")]
    UnknownDependency {
        /// The entry that names the dependency.
        id: ExpectationId,
        /// The id that names no entry.
        dependency: ExpectationId,
    },
    /// Suite entries that depend on each other in a loop.
    #[error("suite entries depend on each other in a cycle ({})", join_ids(ids))]
    Cycle {
        /// The ids on the cycle, each depending on the next and the last on the first.
        ids: Vec<ExpectationId>,
    },
    /// An `outcome` notification whose record carries the legacy cause, which evaluators never emit.
    #[error(
        "outcome carries the legacy cause (expectation {expectation_id}; evaluators emit expectation, artifact or reason)"
    )]
    LegacyCause {
        /// The expectation the record is about.
        expectation_id: ExpectationId,
    },
    /// A value that has no canonical form, such as an integer outside ±(2^53 − 1).
    #[error(transparent)]
    Canonical(#[from] CanonicalError),
}

/// Joins ids with ` -> ` for an error message.
fn join_ids(ids: &[ExpectationId]) -> String {
    ids.iter()
        .map(ExpectationId::as_str)
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// Reads and writes a `SemVer` version as its text.
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

/// Reads and writes a `SemVer` version requirement as its text.
mod range_text {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(
        range: &semver::VersionReq,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(range)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<semver::VersionReq, D::Error> {
        let text = String::deserialize(deserializer)?;
        // BORROW: explicit .as_str() instead of Deref coercion
        semver::VersionReq::parse(text.as_str()).map_err(serde::de::Error::custom)
    }
}

/// The protocol version a plug-in or the core speaks, written as an object with the integers `major` and `minor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct ProtocolVersion {
    /// The major version; a different major is incompatible.
    #[schemars(extend("$comment" = "Fits in 32 bits; the reader enforces it."))]
    pub major: u32,
    /// The minor version; a plug-in may offer a minor no newer than the core's.
    #[schemars(extend("$comment" = "Fits in 32 bits; the reader enforces it."))]
    pub minor: u32,
}

object_only!(ProtocolVersion);

impl ProtocolVersion {
    /// A protocol version.
    #[must_use]
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }
}

impl ProtocolVersion {
    /// Checks that a plug-in speaking `offered` can talk to a core speaking `self`: the major is
    /// the same and the offered minor is no newer than ours.
    ///
    /// # Errors
    /// Returns a [`VersionMismatch`] that names the side to upgrade: the core when the plug-in
    /// offers a newer major or minor, the plug-in when it offers an older major.
    pub const fn accepts(&self, offered: &Self) -> Result<(), VersionMismatch> {
        let upgrade = if offered.major > self.major
            || (offered.major == self.major && offered.minor > self.minor)
        {
            Side::Core
        } else if offered.major < self.major {
            Side::Plugin
        } else {
            return Ok(());
        };
        Err(VersionMismatch {
            ours: *self,
            offered: *offered,
            upgrade,
        })
    }
}

/// The side of a connection that has to be upgraded to make two protocol versions compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Side {
    /// The core, which is older than the plug-in.
    Core,
    /// The plug-in, which is older than the core.
    Plugin,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Core => "core",
            Self::Plugin => "plug-in",
        })
    }
}

/// Two protocol versions that cannot talk to each other, with the side that has to be upgraded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
#[error("protocol {offered} is not accepted by protocol {ours} (upgrade the {upgrade})")]
#[non_exhaustive]
pub struct VersionMismatch {
    /// The version the core speaks.
    pub ours: ProtocolVersion,
    /// The version the plug-in offered.
    pub offered: ProtocolVersion,
    /// The side to upgrade.
    pub upgrade: Side,
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// The environment a plug-in runs in: an object whose `kind` is `uv` (with the `project` and `lock` paths, relative to the plug-in directory) or `none`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum PluginEnvironment {
    /// A `uv` project: the directory that holds `pyproject.toml` and the lock file, both relative to the plug-in directory.
    #[non_exhaustive]
    Uv {
        /// The `uv` project directory, relative to the plug-in directory.
        project: RelPath,
        /// The lock file, relative to the plug-in directory.
        lock: RelPath,
    },
    /// The plug-in needs no prepared environment.
    None,
}

object_only!(PluginEnvironment);

impl PluginEnvironment {
    /// A `uv` environment.
    #[must_use]
    pub const fn uv(project: RelPath, lock: RelPath) -> Self {
        Self::Uv { project, lock }
    }
}

schema_tag! {
    /// The constant `schema` field of a plug-in manifest.
    ManifestSchemaTag = PLUGIN_MANIFEST_SCHEMA
}

/// The static description of a plug-in, read without running anything: a manifest with the version marker `plugin-manifest/1` in `schema`.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "PluginManifest")]
struct RawPluginManifest {
    /// The version marker of the document: always `plugin-manifest/1`.
    schema: ManifestSchemaTag,
    /// The plug-in's name; `describe` returns the same name.
    name: Name,
    /// The plug-in's version, a `SemVer` 2.0 string; `describe` returns the same version.
    #[serde(with = "version_text")]
    #[schemars(with = "String", regex(pattern = SEMVER_PATTERN))]
    version: semver::Version,
    /// The protocol version the plug-in speaks.
    protocol: ProtocolVersion,
    /// The command that starts the plug-in as an argument vector, with at least one member, relative to the plug-in directory.
    #[schemars(length(min = 1))]
    command: Vec<String>,
    /// The environment the plug-in runs in.
    environment: PluginEnvironment,
}

impl TryFrom<MapOnly<RawPluginManifest>> for PluginManifest {
    type Error = EvaluationError;

    fn try_from(raw: MapOnly<RawPluginManifest>) -> Result<Self, Self::Error> {
        let RawPluginManifest {
            schema: ManifestSchemaTag,
            name,
            version,
            protocol,
            command,
            environment,
        } = raw.0;
        Self::new(name, version, protocol, command, environment)
    }
}

/// The static description of a plug-in, read without running anything: a manifest with the version marker `plugin-manifest/1` in `schema`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawPluginManifest>")]
#[non_exhaustive]
pub struct PluginManifest {
    schema: ManifestSchemaTag,
    name: Name,
    #[serde(with = "version_text")]
    #[schemars(with = "String")]
    version: semver::Version,
    protocol: ProtocolVersion,
    command: Vec<String>,
    environment: PluginEnvironment,
}

impl PluginManifest {
    /// Builds a manifest.
    ///
    /// # Errors
    /// Returns [`EvaluationError::EmptyList`] if `command` has no member.
    pub fn new(
        name: Name,
        version: semver::Version,
        protocol: ProtocolVersion,
        command: Vec<String>,
        environment: PluginEnvironment,
    ) -> Result<Self, EvaluationError> {
        if command.is_empty() {
            return Err(EvaluationError::EmptyList { field: "command" });
        }
        Ok(Self {
            schema: ManifestSchemaTag,
            name,
            version,
            protocol,
            command,
            environment,
        })
    }

    /// Reads a manifest from TOML text.
    ///
    /// # Errors
    /// Returns [`EvaluationError::Toml`] if the text is not TOML, does not have the shape of a
    /// manifest or carries another `schema` marker, and [`EvaluationError::EmptyList`] if
    /// `command` has no member.
    pub fn from_toml_str(text: &str) -> Result<Self, EvaluationError> {
        let raw: RawPluginManifest = toml::from_str(text).map_err(|error| toml_error(&error))?;
        Self::try_from(MapOnly(raw))
    }

    /// The plug-in's name.
    #[must_use]
    pub const fn name<'a>(&'a self) -> &'a Name {
        &self.name
    }

    /// The plug-in's version.
    #[must_use]
    pub const fn version<'a>(&'a self) -> &'a semver::Version {
        &self.version
    }

    /// The protocol version the plug-in speaks.
    #[must_use]
    pub const fn protocol<'a>(&'a self) -> &'a ProtocolVersion {
        &self.protocol
    }

    /// The command that starts the plug-in, relative to the plug-in directory.
    #[must_use]
    pub const fn command<'a>(&'a self) -> &'a [String] {
        // BORROW: .as_slice() views the owned argument vector
        self.command.as_slice()
    }

    /// The environment the plug-in runs in.
    #[must_use]
    pub const fn environment<'a>(&'a self) -> &'a PluginEnvironment {
        &self.environment
    }
}

/// Wraps a TOML reader error.
fn toml_error(error: &toml::de::Error) -> EvaluationError {
    EvaluationError::Toml {
        detail: error.to_string(),
    }
}

/// The evaluator a suite entry asks for: a name and the range of versions that may grade it, written as an object with the string `name` and the `SemVer` requirement `range`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct EvaluatorRef {
    /// The evaluator's name.
    pub name: Name,
    /// The versions that may grade the entry, a `SemVer` requirement such as `^0.4`.
    #[serde(with = "range_text")]
    #[schemars(with = "String")]
    #[schemars(extend("$comment" = "The text is a SemVer requirement; the Rust reader enforces the grammar."))]
    pub range: semver::VersionReq,
}

object_only!(EvaluatorRef);

impl EvaluatorRef {
    /// A reference to the evaluator `name` at any version in `range`.
    #[must_use]
    pub const fn new(name: Name, range: semver::VersionReq) -> Self {
        Self { name, range }
    }
}

/// A condition on a run: the run's value at `path` is one of the listed values, written as an object with the string `path` and the non-empty list `any_of`.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "Condition")]
struct RawCondition {
    /// The dotted path of the run field the condition reads.
    path: FieldPath,
    /// The values the field may hold, at least one.
    #[schemars(length(min = 1))]
    any_of: Vec<JsonValue>,
}

impl TryFrom<MapOnly<RawCondition>> for Condition {
    type Error = EvaluationError;

    fn try_from(raw: MapOnly<RawCondition>) -> Result<Self, Self::Error> {
        Self::new(raw.0.path, raw.0.any_of)
    }
}

/// A condition on a run: the run's value at `path` is one of the listed values, written as an object with the string `path` and the non-empty list `any_of`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawCondition>")]
#[non_exhaustive]
pub struct Condition {
    path: FieldPath,
    any_of: Vec<JsonValue>,
}

impl Condition {
    /// Builds a condition.
    ///
    /// # Errors
    /// Returns [`EvaluationError::EmptyList`] if `any_of` has no member.
    pub fn new(path: FieldPath, any_of: Vec<JsonValue>) -> Result<Self, EvaluationError> {
        if any_of.is_empty() {
            return Err(EvaluationError::EmptyList { field: "any_of" });
        }
        Ok(Self { path, any_of })
    }

    /// The path of the run field the condition reads.
    #[must_use]
    pub const fn path<'a>(&'a self) -> &'a FieldPath {
        &self.path
    }

    /// The values the field may hold, in the order written.
    #[must_use]
    pub const fn any_of<'a>(&'a self) -> &'a [JsonValue] {
        // BORROW: .as_slice() views the owned list
        self.any_of.as_slice()
    }
}

/// One expectation of a suite, written as an object: what to check (`kind` and `params`), who checks it (`evaluator`), how it counts (`role`), what it waits for (`depends_on`) and when it applies (`applies_to`).
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "SuiteEntry")]
struct RawSuiteEntry {
    /// The expectation's identifier, unique in the suite.
    id: ExpectationId,
    /// The group the expectation belongs to.
    #[serde(default, deserialize_with = "present")]
    group: Option<Name>,
    /// The part the expectation plays in grading.
    role: Role,
    /// The evaluator that grades the expectation.
    evaluator: EvaluatorRef,
    /// The kind of check, one of the kinds the evaluator describes.
    kind: Name,
    /// The parameters of the check, read by the evaluator and never expanded as templates.
    #[schemars(extend("$comment" = "Integers lie within ±(2^53 - 1); JSON Schema cannot state this, and the Rust reader refuses a larger one."))]
    params: Map<String, Value>,
    /// The ids of the expectations that must not fail or be blocked for this one to be graded.
    depends_on: Vec<ExpectationId>,
    /// The conditions that must all hold for the expectation to apply to a run; absent when it applies to every run, otherwise at least one.
    #[serde(default, deserialize_with = "present")]
    #[schemars(length(min = 1))]
    applies_to: Option<Vec<Condition>>,
    /// The time the evaluator may spend on the expectation, in milliseconds; absent when the evaluator's default for the kind applies.
    #[serde(default, deserialize_with = "present")]
    #[schemars(extend("$comment" = "Fits in 64 bits; a reader that holds numbers as doubles is exact only below 2^53."))]
    deadline_ms: Option<u64>,
}

impl TryFrom<MapOnly<RawSuiteEntry>> for SuiteEntry {
    type Error = EvaluationError;

    fn try_from(raw: MapOnly<RawSuiteEntry>) -> Result<Self, Self::Error> {
        let RawSuiteEntry {
            id,
            group,
            role,
            evaluator,
            kind,
            params,
            depends_on,
            applies_to,
            deadline_ms,
        } = raw.0;
        Self::checked(
            id,
            group,
            role,
            evaluator,
            kind,
            params,
            depends_on,
            applies_to,
            deadline_ms,
        )
    }
}

/// One expectation of a suite, written as an object: what to check (`kind` and `params`), who checks it (`evaluator`), how it counts (`role`), what it waits for (`depends_on`) and when it applies (`applies_to`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawSuiteEntry>")]
#[non_exhaustive]
pub struct SuiteEntry {
    id: ExpectationId,
    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<Name>,
    role: Role,
    evaluator: EvaluatorRef,
    kind: Name,
    params: Map<String, Value>,
    depends_on: Vec<ExpectationId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    applies_to: Option<Vec<Condition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deadline_ms: Option<u64>,
    #[serde(skip)]
    digest: Digest,
}

impl SuiteEntry {
    /// Starts a builder.
    #[must_use]
    pub fn builder() -> SuiteEntryBuilder {
        SuiteEntryBuilder::default()
    }

    #[allow(clippy::too_many_arguments)]
    fn checked(
        id: ExpectationId,
        group: Option<Name>,
        role: Role,
        evaluator: EvaluatorRef,
        kind: Name,
        params: Map<String, Value>,
        depends_on: Vec<ExpectationId>,
        applies_to: Option<Vec<Condition>>,
        deadline_ms: Option<u64>,
    ) -> Result<Self, EvaluationError> {
        if applies_to.as_ref().is_some_and(Vec::is_empty) {
            return Err(EvaluationError::EmptyList {
                field: "applies_to",
            });
        }
        params.values().try_for_each(canonical::check)?;
        let digest = entry_digest(&kind, &params, role, &depends_on, applies_to.as_deref())?;
        Ok(Self {
            id,
            group,
            role,
            evaluator,
            kind,
            params,
            depends_on,
            applies_to,
            deadline_ms,
            digest,
        })
    }

    /// The digest of what the entry asks for: `sha256` of the RFC 8785 form of `{kind, params,
    /// role, depends_on, applies_to}`, with `depends_on` sorted and `applies_to` left out when
    /// absent and kept in the order written otherwise.
    ///
    /// The id, the group, the evaluator and the deadline are outside it, so renaming an
    /// expectation or moving its deadline keeps its digest.
    #[must_use]
    pub fn digest(&self) -> Digest {
        // BORROW: .clone() hands out the digest computed when the entry was built
        self.digest.clone()
    }

    /// The expectation's identifier.
    #[must_use]
    pub const fn id<'a>(&'a self) -> &'a ExpectationId {
        &self.id
    }

    /// The group the expectation belongs to, if it has one.
    #[must_use]
    pub const fn group<'a>(&'a self) -> Option<&'a Name> {
        self.group.as_ref()
    }

    /// The part the expectation plays in grading.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    /// The evaluator that grades the expectation.
    #[must_use]
    pub const fn evaluator<'a>(&'a self) -> &'a EvaluatorRef {
        &self.evaluator
    }

    /// The kind of check.
    #[must_use]
    pub const fn kind<'a>(&'a self) -> &'a Name {
        &self.kind
    }

    /// The parameters of the check.
    #[must_use]
    pub const fn params<'a>(&'a self) -> &'a Map<String, Value> {
        &self.params
    }

    /// The ids of the expectations this one depends on, in the order written.
    #[must_use]
    pub const fn depends_on<'a>(&'a self) -> &'a [ExpectationId] {
        // BORROW: .as_slice() views the owned list
        self.depends_on.as_slice()
    }

    /// The conditions that must all hold for the expectation to apply, or `None` when it applies
    /// to every run.
    #[must_use]
    pub fn applies_to<'a>(&'a self) -> Option<&'a [Condition]> {
        self.applies_to.as_deref()
    }

    /// The deadline in milliseconds, if the entry sets one.
    #[must_use]
    pub const fn deadline_ms(&self) -> Option<u64> {
        self.deadline_ms
    }
}

/// Computes [`SuiteEntry::digest`].
fn entry_digest(
    kind: &Name,
    params: &Map<String, Value>,
    role: Role,
    depends_on: &[ExpectationId],
    applies_to: Option<&[Condition]>,
) -> Result<Digest, EvaluationError> {
    let mut sorted: Vec<&ExpectationId> = depends_on.iter().collect();
    sorted.sort();
    let mut projection = json!({
        "kind": kind,
        "params": params,
        "role": role,
        "depends_on": sorted,
    });
    if let (Some(conditions), Some(object)) = (applies_to, projection.as_object_mut()) {
        // BORROW: .to_owned() makes the owned key `Map::insert` takes
        object.insert("applies_to".to_owned(), json!(conditions));
    }
    Ok(Digest::of_canonical(&projection)?)
}

/// Builds a [`SuiteEntry`].
///
/// `id`, `role`, `evaluator` and `kind` are required. `params` is empty, `depends_on` is empty
/// and `applies_to` is absent until set; `depends_on` and `applies_to` take one member per call.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct SuiteEntryBuilder {
    id: Option<ExpectationId>,
    group: Option<Name>,
    role: Option<Role>,
    evaluator: Option<EvaluatorRef>,
    kind: Option<Name>,
    params: Map<String, Value>,
    depends_on: Vec<ExpectationId>,
    applies_to: Vec<Condition>,
    deadline_ms: Option<u64>,
}

impl SuiteEntryBuilder {
    /// Sets the identifier.
    #[must_use]
    pub fn id(mut self, id: ExpectationId) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the group.
    #[must_use]
    pub fn group(mut self, group: Name) -> Self {
        self.group = Some(group);
        self
    }

    /// Sets the role.
    #[must_use]
    pub const fn role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }

    /// Sets the evaluator.
    #[must_use]
    pub fn evaluator(mut self, evaluator: EvaluatorRef) -> Self {
        self.evaluator = Some(evaluator);
        self
    }

    /// Sets the kind of check.
    #[must_use]
    pub fn kind(mut self, kind: Name) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Sets the parameters of the check.
    #[must_use]
    pub fn params(mut self, params: Map<String, Value>) -> Self {
        self.params = params;
        self
    }

    /// Adds a dependency.
    #[must_use]
    pub fn depends_on(mut self, id: ExpectationId) -> Self {
        self.depends_on.push(id);
        self
    }

    /// Adds a condition of `applies_to`.
    #[must_use]
    pub fn applies_to(mut self, condition: Condition) -> Self {
        self.applies_to.push(condition);
        self
    }

    /// Sets the deadline in milliseconds.
    #[must_use]
    pub const fn deadline_ms(mut self, deadline_ms: u64) -> Self {
        self.deadline_ms = Some(deadline_ms);
        self
    }

    /// Builds the entry.
    ///
    /// # Errors
    /// Returns [`EvaluationError::MissingField`] if `id`, `role`, `evaluator` or `kind` was not
    /// set, and [`EvaluationError::Canonical`] if a parameter holds an integer outside
    /// ±(2^53 − 1).
    pub fn build(self) -> Result<SuiteEntry, EvaluationError> {
        let missing = |field| EvaluationError::MissingField { field };
        SuiteEntry::checked(
            self.id.ok_or_else(|| missing("id"))?,
            self.group,
            self.role.ok_or_else(|| missing("role"))?,
            self.evaluator.ok_or_else(|| missing("evaluator"))?,
            self.kind.ok_or_else(|| missing("kind"))?,
            self.params,
            self.depends_on,
            (!self.applies_to.is_empty()).then_some(self.applies_to),
            self.deadline_ms,
        )
    }
}

schema_tag! {
    /// The constant `schema` field of a suite file.
    SuiteSchemaTag = SUITE_SCHEMA
}

/// The expectations a run is graded against: a suite with the version marker `suite/1` in `schema` and the list `expectations`, whose ids are unique, whose dependencies name entries of the list and whose dependency graph has no cycle.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "SuiteFile")]
struct RawSuiteFile {
    /// The version marker of the document: always `suite/1`.
    schema: SuiteSchemaTag,
    /// The expectations, in the order written.
    #[schemars(extend("$comment" = "Unique ids, dependencies that name entries and the absence of a dependency cycle are enforced in Rust; JSON Schema cannot state them."))]
    expectations: Vec<SuiteEntry>,
}

impl TryFrom<MapOnly<RawSuiteFile>> for SuiteFile {
    type Error = EvaluationError;

    fn try_from(raw: MapOnly<RawSuiteFile>) -> Result<Self, Self::Error> {
        let RawSuiteFile {
            schema: SuiteSchemaTag,
            expectations,
        } = raw.0;
        Self::new(expectations)
    }
}

/// The expectations a run is graded against: a suite with the version marker `suite/1` in `schema` and the list `expectations`, whose ids are unique, whose dependencies name entries of the list and whose dependency graph has no cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawSuiteFile>")]
#[non_exhaustive]
pub struct SuiteFile {
    schema: SuiteSchemaTag,
    expectations: Vec<SuiteEntry>,
}

impl SuiteFile {
    /// Builds a suite.
    ///
    /// # Errors
    /// Returns [`EvaluationError::DuplicateId`] if two entries share an id,
    /// [`EvaluationError::UnknownDependency`] if an entry depends on an id no entry has, and
    /// [`EvaluationError::Cycle`] if entries depend on each other in a loop. Each names the ids
    /// involved.
    pub fn new(expectations: Vec<SuiteEntry>) -> Result<Self, EvaluationError> {
        check_graph(&expectations)?;
        Ok(Self {
            schema: SuiteSchemaTag,
            expectations,
        })
    }

    /// Reads a suite from TOML text and checks its dependency graph.
    ///
    /// # Errors
    /// Returns [`EvaluationError::Toml`] if the text is not TOML, does not have the shape of a
    /// suite or carries another `schema` marker, and the errors of [`SuiteFile::new`] for a
    /// graph that is not valid.
    pub fn from_toml_str(text: &str) -> Result<Self, EvaluationError> {
        let raw: RawSuiteFile = toml::from_str(text).map_err(|error| toml_error(&error))?;
        Self::try_from(MapOnly(raw))
    }

    /// The expectations, in the order written.
    #[must_use]
    pub const fn expectations<'a>(&'a self) -> &'a [SuiteEntry] {
        // BORROW: .as_slice() views the owned list
        self.expectations.as_slice()
    }
}

/// Checks that ids are unique, that every dependency names an entry, and that no cycle exists.
fn check_graph(entries: &[SuiteEntry]) -> Result<(), EvaluationError> {
    let mut known: BTreeMap<&ExpectationId, &[ExpectationId]> = BTreeMap::new();
    for entry in entries {
        // BORROW: .as_slice() views the entry's dependency list
        if known
            .insert(&entry.id, entry.depends_on.as_slice())
            .is_some()
        {
            return Err(EvaluationError::DuplicateId {
                // BORROW: .clone() keeps the repeated id in the error
                id: entry.id.clone(),
            });
        }
    }
    for entry in entries {
        if let Some(dependency) = entry.depends_on.iter().find(|d| !known.contains_key(d)) {
            return Err(EvaluationError::UnknownDependency {
                // BORROW: .clone() keeps the ids in the error
                id: entry.id.clone(),
                // BORROW: .clone() keeps the ids in the error
                dependency: dependency.clone(),
            });
        }
    }
    match find_cycle(entries, &known) {
        Some(ids) => Err(EvaluationError::Cycle {
            // BORROW: .cloned() keeps the ids of the cycle in the error
            ids: ids.into_iter().cloned().collect(),
        }),
        None => Ok(()),
    }
}

/// Finds one dependency cycle by a depth-first walk in the order the entries are written, and
/// returns its ids in dependency order.
fn find_cycle<'a>(
    entries: &'a [SuiteEntry],
    known: &BTreeMap<&'a ExpectationId, &'a [ExpectationId]>,
) -> Option<Vec<&'a ExpectationId>> {
    let dependencies = |id: &ExpectationId| known.get(id).copied().unwrap_or_default();
    let mut finished: BTreeSet<&ExpectationId> = BTreeSet::new();
    for entry in entries {
        if finished.contains(&entry.id) {
            continue;
        }
        let mut path: Vec<&ExpectationId> = vec![&entry.id];
        let mut frames = vec![dependencies(&entry.id).iter()];
        while let Some(frame) = frames.last_mut() {
            if let Some(next) = frame.next() {
                if let Some(start) = path.iter().position(|id| *id == next) {
                    return path.get(start..).map(<[&ExpectationId]>::to_vec);
                }
                if !finished.contains(next) {
                    path.push(next);
                    frames.push(dependencies(next).iter());
                }
            } else {
                frames.pop();
                if let Some(done) = path.pop() {
                    finished.insert(done);
                }
            }
        }
    }
    None
}

/// How an evaluator reaches a verdict: always the same one for the same input, or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EvaluatorKind {
    /// The same input always gives the same verdict.
    Deterministic,
    /// The verdict can differ between runs on the same input, as with a model that grades.
    Probabilistic,
}

/// What an evaluator reports about the environment it runs in: the digest of its lock file when it has one, and the versions of its packages, written as an object with `lock_digest` omitted when absent and the map `packages` always present.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct EvaluatorEnvironment {
    /// The digest of the lock file the environment was built from; absent when there is none.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub lock_digest: Option<Digest>,
    /// The version of each installed package, keyed by package name.
    pub packages: BTreeMap<String, String>,
}

object_only!(EvaluatorEnvironment);

/// What one kind of check needs from a run, written as an object whose five lists are all present, empty when the kind needs nothing of that sort.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct Needs {
    /// Whether the check reads the normalised trace.
    pub trace: bool,
    /// Globs, relative to the run, of the artifacts the check reads.
    pub artifacts: Vec<String>,
    /// The kinds of snapshot the check reads.
    pub snapshots: Vec<Name>,
    /// The kinds of input the check reads.
    pub inputs: Vec<Name>,
    /// The paths of the metadata fields the check reads.
    pub metadata: Vec<FieldPath>,
}

object_only!(Needs);

/// One kind of check an evaluator performs, written as an object: the kind, the schema of its parameters, what it needs from a run and the deadline it gets by default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct KindSpec {
    /// The name of the kind, as an entry's `kind` names it.
    pub kind: Name,
    /// The JSON Schema the parameters of an entry of this kind satisfy.
    pub params_schema: JsonValue,
    /// What the kind needs from a run.
    pub needs: Needs,
    /// The time the evaluator gives an item of this kind, in milliseconds, when the entry sets no deadline.
    #[schemars(extend("$comment" = "Fits in 64 bits; a reader that holds numbers as doubles is exact only below 2^53."))]
    pub default_deadline_ms: u64,
}

object_only!(KindSpec);

impl KindSpec {
    /// A kind of check.
    #[must_use]
    pub const fn new(
        kind: Name,
        params_schema: JsonValue,
        needs: Needs,
        default_deadline_ms: u64,
    ) -> Self {
        Self {
            kind,
            params_schema,
            needs,
            default_deadline_ms,
        }
    }
}

/// What an evaluator says about itself in answer to `describe`, written as an object: the protocol it speaks, its name and version, the instruments it uses, its environment, whether it is deterministic, the kinds of check it performs and, when it has one, its reuse rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct DescribeResult {
    /// The protocol version the evaluator speaks.
    pub protocol: ProtocolVersion,
    /// The evaluator's name; the plug-in manifest carries the same name.
    pub name: Name,
    /// The evaluator's version, a `SemVer` 2.0 string; the plug-in manifest carries the same version.
    #[serde(with = "version_text")]
    #[schemars(with = "String", regex(pattern = SEMVER_PATTERN))]
    pub version: semver::Version,
    /// The tools that make the evaluator's measurements.
    pub instruments: Vec<Instrument>,
    /// The environment the evaluator runs in.
    pub environment: EvaluatorEnvironment,
    /// Whether the evaluator is deterministic.
    pub kind: EvaluatorKind,
    /// The kinds of check the evaluator performs.
    pub kinds: Vec<KindSpec>,
    /// The rule that decides when an earlier verdict can be reused; absent when the evaluator has none.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub reuse: Option<JsonValue>,
}

object_only!(DescribeResult);

impl DescribeResult {
    /// Starts a builder.
    #[must_use]
    pub fn builder() -> DescribeResultBuilder {
        DescribeResultBuilder::default()
    }
}

/// Builds a [`DescribeResult`].
///
/// `protocol`, `name`, `version`, `environment` and `kind` are required. `instrument` and
/// `kind_spec` add one member per call to `instruments` and `kinds`, which are empty until then;
/// `reuse` is absent until set. The setter for the field `kind` keeps its name, so the one that adds
/// to `kinds` is `kind_spec`.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct DescribeResultBuilder {
    protocol: Option<ProtocolVersion>,
    name: Option<Name>,
    version: Option<semver::Version>,
    instruments: Vec<Instrument>,
    environment: Option<EvaluatorEnvironment>,
    kind: Option<EvaluatorKind>,
    kinds: Vec<KindSpec>,
    reuse: Option<JsonValue>,
}

impl DescribeResultBuilder {
    /// Sets the protocol version.
    #[must_use]
    pub const fn protocol(mut self, protocol: ProtocolVersion) -> Self {
        self.protocol = Some(protocol);
        self
    }

    /// Sets the name.
    #[must_use]
    pub fn name(mut self, name: Name) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the version.
    #[must_use]
    pub fn version(mut self, version: semver::Version) -> Self {
        self.version = Some(version);
        self
    }

    /// Adds an instrument.
    #[must_use]
    pub fn instrument(mut self, instrument: Instrument) -> Self {
        self.instruments.push(instrument);
        self
    }

    /// Sets the environment.
    #[must_use]
    pub fn environment(mut self, environment: EvaluatorEnvironment) -> Self {
        self.environment = Some(environment);
        self
    }

    /// Sets whether the evaluator is deterministic.
    #[must_use]
    pub const fn kind(mut self, kind: EvaluatorKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Adds a kind of check.
    #[must_use]
    pub fn kind_spec(mut self, spec: KindSpec) -> Self {
        self.kinds.push(spec);
        self
    }

    /// Sets the reuse rule.
    #[must_use]
    pub fn reuse(mut self, reuse: JsonValue) -> Self {
        self.reuse = Some(reuse);
        self
    }

    /// Builds the result.
    ///
    /// # Errors
    /// Returns [`EvaluationError::MissingField`] if `protocol`, `name`, `version`, `environment`
    /// or `kind` was not set.
    pub fn build(self) -> Result<DescribeResult, EvaluationError> {
        let missing = |field| EvaluationError::MissingField { field };
        Ok(DescribeResult {
            protocol: self.protocol.ok_or_else(|| missing("protocol"))?,
            name: self.name.ok_or_else(|| missing("name"))?,
            version: self.version.ok_or_else(|| missing("version"))?,
            instruments: self.instruments,
            environment: self.environment.ok_or_else(|| missing("environment"))?,
            kind: self.kind.ok_or_else(|| missing("kind"))?,
            kinds: self.kinds,
            reuse: self.reuse,
        })
    }
}

/// One raw stream of a run: its name and the path where the evaluator reads it, written as an object with the strings `stream` and `path`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct ContextRaw {
    /// The name of the raw stream.
    pub stream: StreamName,
    /// The absolute path of the stream, valid where the evaluator runs.
    pub path: String,
}

object_only!(ContextRaw);

impl ContextRaw {
    /// A raw stream of a run.
    #[must_use]
    pub const fn new(stream: StreamName, path: String) -> Self {
        Self { stream, path }
    }
}

/// One artifact of a run: its path relative to the run, where the evaluator reads it and its digest, written as an object with the strings `path`, `abs` and `sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct ContextArtifact {
    /// The path of the artifact relative to the run.
    pub path: RelPath,
    /// The absolute path of the artifact, valid where the evaluator runs.
    pub abs: String,
    /// The digest of the artifact's bytes.
    pub sha256: Digest,
}

object_only!(ContextArtifact);

impl ContextArtifact {
    /// An artifact of a run.
    #[must_use]
    pub const fn new(path: RelPath, abs: String, sha256: Digest) -> Self {
        Self { path, abs, sha256 }
    }
}

/// A snapshot or an input of a run: its kind, where the evaluator reads it and its digest, written as an object with the strings `kind`, `path` and `digest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct ContextRef {
    /// The kind of snapshot or input.
    pub kind: Name,
    /// The absolute path of the file, valid where the evaluator runs.
    pub path: String,
    /// The digest of the file's bytes.
    pub digest: Digest,
}

object_only!(ContextRef);

impl ContextRef {
    /// A snapshot or an input of a run.
    #[must_use]
    pub const fn new(kind: Name, path: String, digest: Digest) -> Self {
        Self { kind, path, digest }
    }
}

/// What an evaluator may read of one run, written as an object: the run's key and root directory, the trace when there is one, the raw streams, the artifacts, the snapshots, the inputs and the metadata. Every list and the metadata are present, empty when there is nothing of that sort.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "RunContext")]
struct RawRunContext {
    /// The key of the run.
    run: RunKey,
    /// The absolute path of the run's directory, valid where the evaluator runs.
    root: String,
    /// The absolute path of the normalised trace; absent when the run has none.
    #[serde(default, deserialize_with = "present")]
    trace: Option<String>,
    /// The raw streams of the run.
    raw: Vec<ContextRaw>,
    /// The artifacts of the run.
    artifacts: Vec<ContextArtifact>,
    /// The snapshots the run was made against.
    snapshots: Vec<ContextRef>,
    /// The inputs the run was made from.
    inputs: Vec<ContextRef>,
    /// The metadata of the run, keyed by the dotted path of the field.
    metadata: BTreeMap<FieldPath, JsonValue>,
}

impl TryFrom<MapOnly<RawRunContext>> for RunContext {
    type Error = EvaluationError;

    fn try_from(raw: MapOnly<RawRunContext>) -> Result<Self, Self::Error> {
        let RawRunContext {
            run,
            root,
            trace,
            raw,
            artifacts,
            snapshots,
            inputs,
            metadata,
        } = raw.0;
        Self::checked(
            run, root, trace, raw, artifacts, snapshots, inputs, metadata,
        )
    }
}

/// What an evaluator may read of one run, written as an object: the run's key and root directory, the trace when there is one, the raw streams, the artifacts, the snapshots, the inputs and the metadata. Every list and the metadata are present, empty when there is nothing of that sort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawRunContext>")]
#[non_exhaustive]
pub struct RunContext {
    run: RunKey,
    root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<String>,
    raw: Vec<ContextRaw>,
    artifacts: Vec<ContextArtifact>,
    snapshots: Vec<ContextRef>,
    inputs: Vec<ContextRef>,
    metadata: BTreeMap<FieldPath, JsonValue>,
    #[serde(skip)]
    context_digest: Digest,
}

impl RunContext {
    /// Starts a builder.
    #[must_use]
    pub fn builder() -> RunContextBuilder {
        RunContextBuilder::default()
    }

    #[allow(clippy::too_many_arguments)]
    fn checked(
        run: RunKey,
        root: String,
        trace: Option<String>,
        raw: Vec<ContextRaw>,
        artifacts: Vec<ContextArtifact>,
        snapshots: Vec<ContextRef>,
        inputs: Vec<ContextRef>,
        metadata: BTreeMap<FieldPath, JsonValue>,
    ) -> Result<Self, EvaluationError> {
        let context_digest = context_digest(&artifacts, &snapshots, &inputs, &metadata)?;
        Ok(Self {
            run,
            root,
            trace,
            raw,
            artifacts,
            snapshots,
            inputs,
            metadata,
            context_digest,
        })
    }

    /// The digest of the part of the context that decides a verdict: `sha256` of the RFC 8785
    /// form of `{artifacts: [{path, sha256}], snapshots: [{kind, digest}], inputs: [{kind,
    /// digest}], metadata}`, with the artifacts sorted by `path` and the snapshots and the inputs
    /// by `(kind, digest)`, both by code point, and every key present.
    ///
    /// The run key, the root, the trace, the raw streams and the absolute paths are outside it,
    /// so moving a run to another directory keeps its digest.
    #[must_use]
    pub fn context_digest(&self) -> Digest {
        // BORROW: .clone() hands out the digest computed when the context was built
        self.context_digest.clone()
    }

    /// The key of the run.
    #[must_use]
    pub const fn run<'a>(&'a self) -> &'a RunKey {
        &self.run
    }

    /// The absolute path of the run's directory.
    #[must_use]
    pub const fn root<'a>(&'a self) -> &'a str {
        // BORROW: .as_str() views the owned path
        self.root.as_str()
    }

    /// The absolute path of the normalised trace, if the run has one.
    #[must_use]
    pub fn trace<'a>(&'a self) -> Option<&'a str> {
        self.trace.as_deref()
    }

    /// The raw streams of the run.
    #[must_use]
    pub const fn raw<'a>(&'a self) -> &'a [ContextRaw] {
        // BORROW: .as_slice() views the owned list
        self.raw.as_slice()
    }

    /// The artifacts of the run, in the order written.
    #[must_use]
    pub const fn artifacts<'a>(&'a self) -> &'a [ContextArtifact] {
        // BORROW: .as_slice() views the owned list
        self.artifacts.as_slice()
    }

    /// The snapshots of the run, in the order written.
    #[must_use]
    pub const fn snapshots<'a>(&'a self) -> &'a [ContextRef] {
        // BORROW: .as_slice() views the owned list
        self.snapshots.as_slice()
    }

    /// The inputs of the run, in the order written.
    #[must_use]
    pub const fn inputs<'a>(&'a self) -> &'a [ContextRef] {
        // BORROW: .as_slice() views the owned list
        self.inputs.as_slice()
    }

    /// The metadata of the run.
    #[must_use]
    pub const fn metadata<'a>(&'a self) -> &'a BTreeMap<FieldPath, JsonValue> {
        &self.metadata
    }
}

/// Computes [`RunContext::context_digest`].
fn context_digest(
    artifacts: &[ContextArtifact],
    snapshots: &[ContextRef],
    inputs: &[ContextRef],
    metadata: &BTreeMap<FieldPath, JsonValue>,
) -> Result<Digest, EvaluationError> {
    let mut artifacts: Vec<&ContextArtifact> = artifacts.iter().collect();
    artifacts.sort_by(|a, b| a.path.cmp(&b.path));
    let artifacts: Vec<Value> = artifacts
        .iter()
        .map(|a| json!({"path": a.path, "sha256": a.sha256}))
        .collect();
    let refs = |refs: &[ContextRef]| -> Vec<Value> {
        let mut sorted: Vec<&ContextRef> = refs.iter().collect();
        sorted.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.digest.cmp(&b.digest)));
        sorted
            .iter()
            .map(|r| json!({"kind": r.kind, "digest": r.digest}))
            .collect()
    };
    let projection = json!({
        "artifacts": artifacts,
        "snapshots": refs(snapshots),
        "inputs": refs(inputs),
        "metadata": metadata,
    });
    Ok(Digest::of_canonical(&projection)?)
}

/// Builds a [`RunContext`].
///
/// `run` and `root` are required. `trace` is absent until set; the lists and the metadata are
/// empty until set, and `raw`, `artifact`, `snapshot`, `input` and `metadata` add one member per
/// call.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct RunContextBuilder {
    run: Option<RunKey>,
    root: Option<String>,
    trace: Option<String>,
    raw: Vec<ContextRaw>,
    artifacts: Vec<ContextArtifact>,
    snapshots: Vec<ContextRef>,
    inputs: Vec<ContextRef>,
    metadata: BTreeMap<FieldPath, JsonValue>,
}

impl RunContextBuilder {
    /// Sets the run key.
    #[must_use]
    pub fn run(mut self, run: RunKey) -> Self {
        self.run = Some(run);
        self
    }

    /// Sets the root directory.
    #[must_use]
    pub fn root(mut self, root: String) -> Self {
        self.root = Some(root);
        self
    }

    /// Sets the path of the trace.
    #[must_use]
    pub fn trace(mut self, trace: String) -> Self {
        self.trace = Some(trace);
        self
    }

    /// Adds a raw stream.
    #[must_use]
    pub fn raw(mut self, raw: ContextRaw) -> Self {
        self.raw.push(raw);
        self
    }

    /// Adds an artifact.
    #[must_use]
    pub fn artifact(mut self, artifact: ContextArtifact) -> Self {
        self.artifacts.push(artifact);
        self
    }

    /// Adds a snapshot.
    #[must_use]
    pub fn snapshot(mut self, snapshot: ContextRef) -> Self {
        self.snapshots.push(snapshot);
        self
    }

    /// Adds an input.
    #[must_use]
    pub fn input(mut self, input: ContextRef) -> Self {
        self.inputs.push(input);
        self
    }

    /// Adds a metadata field, replacing an earlier value at the same path.
    #[must_use]
    pub fn metadata(mut self, path: FieldPath, value: JsonValue) -> Self {
        self.metadata.insert(path, value);
        self
    }

    /// Builds the context.
    ///
    /// # Errors
    /// Returns [`EvaluationError::MissingField`] if `run` or `root` was not set, and
    /// [`EvaluationError::Canonical`] if a metadata value has no canonical form.
    pub fn build(self) -> Result<RunContext, EvaluationError> {
        let missing = |field| EvaluationError::MissingField { field };
        RunContext::checked(
            self.run.ok_or_else(|| missing("run"))?,
            self.root.ok_or_else(|| missing("root"))?,
            self.trace,
            self.raw,
            self.artifacts,
            self.snapshots,
            self.inputs,
            self.metadata,
        )
    }
}

/// One expectation to grade: its id, the kind of check, the parameters and the deadline, written as an object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct Item {
    /// The expectation to grade.
    pub expectation_id: ExpectationId,
    /// The kind of check, one of the kinds the evaluator describes.
    pub kind: Name,
    /// The parameters of the check, as the suite entry holds them.
    pub params: Map<String, Value>,
    /// The time the evaluator may spend on the item, in milliseconds.
    #[schemars(extend("$comment" = "Fits in 64 bits; a reader that holds numbers as doubles is exact only below 2^53."))]
    pub deadline_ms: u64,
}

object_only!(Item);

impl Item {
    /// An item to grade.
    #[must_use]
    pub const fn new(
        expectation_id: ExpectationId,
        kind: Name,
        params: Map<String, Value>,
        deadline_ms: u64,
    ) -> Self {
        Self {
            expectation_id,
            kind,
            params,
            deadline_ms,
        }
    }
}

/// The params of an `evaluate` request, written as an object: the run's context, the items to grade, the outcomes already recorded for the run and a scratch directory the evaluator may write in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct EvaluateParams {
    /// What the evaluator may read of the run.
    pub context: RunContext,
    /// The expectations to grade.
    pub items: Vec<Item>,
    /// The outcome records already made for the run, which dependent items may read.
    pub prior: Vec<OutcomeRecord>,
    /// The absolute path of a directory the evaluator may write in.
    pub scratch: String,
}

object_only!(EvaluateParams);

impl EvaluateParams {
    /// The params of an `evaluate` request.
    #[must_use]
    pub const fn new(
        context: RunContext,
        items: Vec<Item>,
        prior: Vec<OutcomeRecord>,
        scratch: String,
    ) -> Self {
        Self {
            context,
            items,
            prior,
            scratch,
        }
    }
}

/// The result of an `evaluate` request, written as an object with the number of outcomes the evaluator reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct EvaluateResult {
    /// The number of `outcome` notifications the evaluator sent for the request.
    #[schemars(extend("$comment" = "Fits in 64 bits; a reader that holds numbers as doubles is exact only below 2^53."))]
    pub reported: u64,
}

object_only!(EvaluateResult);

impl EvaluateResult {
    /// The result of an `evaluate` request.
    #[must_use]
    pub const fn new(reported: u64) -> Self {
        Self { reported }
    }
}

/// The params of an `outcome` notification, written as an object with one outcome record, which carries no legacy cause.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "OutcomeParams")]
struct RawOutcomeParams {
    /// The outcome record the evaluator made.
    #[schemars(extend("$comment" = "The record's cause is never the legacy cause; the Rust reader refuses it and JSON Schema cannot state it."))]
    record: OutcomeRecord,
}

impl TryFrom<MapOnly<RawOutcomeParams>> for OutcomeParams {
    type Error = EvaluationError;

    fn try_from(raw: MapOnly<RawOutcomeParams>) -> Result<Self, Self::Error> {
        Self::new(raw.0.record)
    }
}

/// The params of an `outcome` notification, written as an object with one outcome record, which carries no legacy cause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawOutcomeParams>")]
#[non_exhaustive]
pub struct OutcomeParams {
    record: OutcomeRecord,
}

impl OutcomeParams {
    /// Starts a builder.
    #[must_use]
    pub fn builder() -> OutcomeParamsBuilder {
        OutcomeParamsBuilder::default()
    }

    /// Wraps an outcome record.
    ///
    /// # Errors
    /// Returns [`EvaluationError::LegacyCause`] if the record carries the legacy cause.
    pub fn new(record: OutcomeRecord) -> Result<Self, EvaluationError> {
        if record.cause().is_some_and(Cause::is_legacy) {
            return Err(EvaluationError::LegacyCause {
                // BORROW: .clone() keeps the expectation id in the error
                expectation_id: record.expectation_id().clone(),
            });
        }
        Ok(Self { record })
    }

    /// The outcome record.
    #[must_use]
    pub const fn record<'a>(&'a self) -> &'a OutcomeRecord {
        &self.record
    }
}

/// Builds an [`OutcomeParams`]; `record` is required.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct OutcomeParamsBuilder {
    record: Option<OutcomeRecord>,
}

impl OutcomeParamsBuilder {
    /// Sets the outcome record.
    #[must_use]
    pub fn record(mut self, record: OutcomeRecord) -> Self {
        self.record = Some(record);
        self
    }

    /// Builds the params.
    ///
    /// # Errors
    /// Returns [`EvaluationError::MissingField`] if `record` was not set, and
    /// [`EvaluationError::LegacyCause`] if the record carries the legacy cause.
    pub fn build(self) -> Result<OutcomeParams, EvaluationError> {
        OutcomeParams::new(
            self.record
                .ok_or(EvaluationError::MissingField { field: "record" })?,
        )
    }
}

/// The params of a `progress` notification: a free JSON object that tells a person how far the evaluator is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[non_exhaustive]
pub struct ProgressParams(Map<String, Value>);

impl ProgressParams {
    /// Wraps a free object.
    #[must_use]
    pub const fn new(fields: Map<String, Value>) -> Self {
        Self(fields)
    }

    /// The free object.
    #[must_use]
    pub const fn fields<'a>(&'a self) -> &'a Map<String, Value> {
        &self.0
    }
}

/// The params of a `log` notification: a free JSON object that carries one log line of the evaluator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[non_exhaustive]
pub struct LogParams(Map<String, Value>);

impl LogParams {
    /// Wraps a free object.
    #[must_use]
    pub const fn new(fields: Map<String, Value>) -> Self {
        Self(fields)
    }

    /// The free object.
    #[must_use]
    pub const fn fields<'a>(&'a self) -> &'a Map<String, Value> {
        &self.0
    }
}

/// The params of a `cancel` notification, written as an object with the id of the request to cancel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct CancelParams {
    /// The id of the request to cancel.
    pub request_id: RequestId,
}

object_only!(CancelParams);

impl CancelParams {
    /// The params of a `cancel` notification.
    #[must_use]
    pub const fn new(request_id: RequestId) -> Self {
        Self { request_id }
    }
}

/// A notification an evaluator or the core sends during an `evaluate` request: an object whose `method` is `outcome`, `progress`, `log` or `cancel` and whose `params` holds the params of that method.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "method", content = "params")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum Notification {
    /// An evaluator reports one outcome record.
    #[serde(rename = "outcome")]
    Outcome(Box<OutcomeParams>),
    /// An evaluator reports how far it is.
    #[serde(rename = "progress")]
    Progress(ProgressParams),
    /// An evaluator sends a log line.
    #[serde(rename = "log")]
    Log(LogParams),
    /// The core cancels a request.
    #[serde(rename = "cancel")]
    Cancel(CancelParams),
}

object_only!(Notification);

/// The params of a `describe` request, written as an empty object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct DescribeParams {}

object_only!(DescribeParams);

impl DescribeParams {
    /// The params of a `describe` request.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }
}

impl Default for DescribeParams {
    fn default() -> Self {
        Self::new()
    }
}

/// A request the core sends to an evaluator: an object whose `method` is `describe` or `evaluate` and whose `params` holds the params of that method.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "method", content = "params")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum Request {
    /// The core asks the evaluator to describe itself.
    #[serde(rename = "describe")]
    Describe(DescribeParams),
    /// The core asks the evaluator to grade items.
    #[serde(rename = "evaluate")]
    Evaluate(Box<EvaluateParams>),
}

object_only!(Request);

/// The error code `-32800` of a request that was cancelled, written as the integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CancelledCode;

impl Serialize for CancelledCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(ERROR_REQUEST_CANCELLED)
    }
}

impl<'de> Deserialize<'de> for CancelledCode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let code = i32::deserialize(deserializer)?;
        if code == ERROR_REQUEST_CANCELLED {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom(format!(
                "error code is not {ERROR_REQUEST_CANCELLED} (got {code})"
            )))
        }
    }
}

impl JsonSchema for CancelledCode {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "CancelledCode".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "integer",
            "const": ERROR_REQUEST_CANCELLED,
        })
    }
}

/// The error a request answers with when it was cancelled, written as an object with the code `-32800` and a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct RequestCancelled {
    /// The error code, always `-32800`.
    pub code: CancelledCode,
    /// A sentence that says what was cancelled.
    pub message: String,
}

object_only!(RequestCancelled);

impl RequestCancelled {
    /// The error of a cancelled request.
    #[must_use]
    pub const fn new(message: String) -> Self {
        Self {
            code: CancelledCode,
            message,
        }
    }
}

/// The request side of the evaluator protocol, written as an object of one of three shapes: a request from the core, the result of an `evaluate` request, or the error of a request that was cancelled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[non_exhaustive]
pub enum RequestSide {
    /// A request the core sends.
    Request(Request),
    /// The result of an `evaluate` request: how many outcomes the evaluator reported.
    EvaluateResult(EvaluateResult),
    /// The error of a request that was cancelled.
    Cancelled(RequestCancelled),
}
