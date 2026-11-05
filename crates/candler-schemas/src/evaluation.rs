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
use crate::outcome::Role;
use crate::primitives::{
    Digest, ExpectationId, FieldPath, Name, RelPath, SEMVER_PATTERN, object_only, present,
    schema_tag,
};
use crate::values::JsonValue;

/// The version marker every plug-in manifest carries in its `schema` field.
pub const PLUGIN_MANIFEST_SCHEMA: &str = "plugin-manifest/1";

/// The version marker every suite file carries in its `schema` field.
pub const SUITE_SCHEMA: &str = "suite/1";

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    Vec::new()
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
