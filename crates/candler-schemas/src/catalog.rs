//! The tool-catalog snapshot: the tools the MCP servers of a run offered, in the shape of MCP
//! `tools/list` results.
//!
//! A [`Tool`] keeps every key its server gave it: the named keys of the MCP `Tool` definition are
//! typed, and any other key is held as given. The document is read through
//! [`CatalogSnapshot`], which refuses any other version marker than [`SCHEMA_TAG`] and refuses an
//! explicit `null` for an optional field. Inside `inputSchema`, `outputSchema`, `annotations`,
//! `ext` and the extra keys of a tool, `null` is ordinary JSON data and is kept.
//!
//! Server names are non-empty and unique within a snapshot, and tool names are non-empty and unique
//! within a server. Names are compared byte for byte: they are case-sensitive and are not
//! Unicode-normalised. The schema states that names are non-empty; uniqueness is enforced when a
//! snapshot is built or read.
//!
//! The reader keeps every key of a tool, `ext` and `environment`, and every value, with these
//! exceptions. Keys not defined for a snapshot or a server are dropped. Of repeated keys the last
//! is kept. Object keys are sorted, so the order in which they were written is not kept. A number
//! is held as `i64`, `u64` or `f64`: an integer beyond both becomes a float, a decimal beyond
//! `f64` precision is rounded, and a number whose magnitude is above the range of `f64` is
//! refused. Text nested deeper than about 120 levels is refused by the text readers.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::primitives::{Digest, NameToken, RelPath, Timestamp};

/// The version marker every snapshot carries in its `schema` field.
pub const SCHEMA_TAG: &str = "tool-catalog-snapshot/1";

/// The keys of a [`Tool`] that are fields rather than extra keys.
const NAMED_TOOL_KEYS: [&str; 6] = [
    "name",
    "title",
    "description",
    "inputSchema",
    "outputSchema",
    "annotations",
];

/// Why a snapshot or one of its members could not be built or read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CatalogError {
    /// A `schema` field that is not [`SCHEMA_TAG`].
    #[error("schema tag differs from `{SCHEMA_TAG}` (got {value:?})")]
    SchemaTag {
        /// The text the field held.
        value: String,
    },
    /// A builder that was asked to build before a required field was set.
    #[error("snapshot field is missing (`{field}` was not set)")]
    MissingField {
        /// The name of the field.
        field: &'static str,
    },
    /// An extra key of a tool that is the name of one of its fields.
    #[error("tool extra key names a field of the tool (got {key:?})")]
    ReservedKey {
        /// The key.
        key: String,
    },
    /// An optional field that was written as `null` instead of being omitted.
    #[error("optional field is null (an absent field is omitted)")]
    Null,
    /// A server or tool name that is empty.
    #[error("{owner} name is empty (names have at least one character)")]
    EmptyName {
        /// What the name belongs to: `server` or `tool`.
        owner: &'static str,
    },
    /// A server or tool name that another member of the same scope already has.
    #[error("{owner} name is repeated within a {scope} (got {name:?})")]
    DuplicateName {
        /// What the name belongs to: `server` or `tool`.
        owner: &'static str,
        /// What the names are unique within: `snapshot` or `server`.
        scope: &'static str,
        /// The repeated name.
        name: String,
    },
}

/// Refuses an empty name.
fn non_empty(name: String, owner: &'static str) -> Result<String, CatalogError> {
    if name.is_empty() {
        Err(CatalogError::EmptyName { owner })
    } else {
        Ok(name)
    }
}

/// Refuses a name that occurs twice in `names`.
fn all_distinct<'a, I>(
    names: I,
    owner: &'static str,
    scope: &'static str,
) -> Result<(), CatalogError>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut seen = BTreeSet::new();
    match names.into_iter().find(|name| !seen.insert(*name)) {
        Some(name) => Err(CatalogError::DuplicateName {
            owner,
            scope,
            name: String::from(name),
        }),
        None => Ok(()),
    }
}

fn distinct_tools(tools: &[Tool]) -> Result<(), CatalogError> {
    all_distinct(tools.iter().map(Tool::name), "tool", "server")
}

fn distinct_servers(servers: &[CatalogServer]) -> Result<(), CatalogError> {
    all_distinct(
        servers.iter().map(CatalogServer::name),
        "server",
        "snapshot",
    )
}

/// Reads a tool name, refusing an empty one.
fn tool_name<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    non_empty(String::deserialize(deserializer)?, "tool").map_err(D::Error::custom)
}

/// Reads a server name, refusing an empty one.
fn server_name<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    non_empty(String::deserialize(deserializer)?, "server").map_err(D::Error::custom)
}

/// Reads the tools of a server, refusing a repeated name.
fn distinct_tools_of<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Tool>, D::Error> {
    let tools = Vec::<Tool>::deserialize(deserializer)?;
    distinct_tools(&tools).map_err(D::Error::custom)?;
    Ok(tools)
}

/// Reads an optional field, refusing an explicit `null`.
///
/// Used with `#[serde(default)]`, so that an absent field is `None` and a present one is `Some`.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    match Option::<T>::deserialize(deserializer)? {
        Some(value) => Ok(Some(value)),
        None => Err(D::Error::custom(CatalogError::Null)),
    }
}

/// The constant `schema` field of a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SchemaTag;

impl Serialize for SchemaTag {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(SCHEMA_TAG)
    }
}

impl JsonSchema for SchemaTag {
    fn schema_name() -> Cow<'static, str> {
        "SchemaTag".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "const": SCHEMA_TAG,
        })
    }
}

/// How a client reaches an MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Transport {
    /// The server runs as a subprocess and is spoken to over its standard input and output.
    Stdio,
    /// The server is reached over streamable HTTP.
    Http,
    /// The server is reached over server-sent events.
    Sse,
}

/// A tool in MCP `tools/list` shape: the named keys of the MCP `Tool` definition, and every other key as the server gave it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("additionalProperties" = true))]
#[non_exhaustive]
pub struct Tool {
    /// The name of the tool, as the server gave it: at least one character, and unique among the tools of its server.
    #[serde(deserialize_with = "tool_name")]
    #[schemars(length(min = 1))]
    name: String,
    /// A human-readable title of the tool.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    title: Option<String>,
    /// A human-readable description of the tool.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    description: Option<String>,
    /// The JSON Schema object that describes the arguments of the tool. Its `type` is not required to be `object`.
    input_schema: Map<String, Value>,
    /// The JSON Schema object that describes the structured output of the tool.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    output_schema: Option<Map<String, Value>>,
    /// Hints about the behaviour of the tool: an open object whose keys are kept as given.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    annotations: Option<Map<String, Value>>,
    // Every key that is not named above, as given.
    #[serde(flatten)]
    extra: Map<String, Value>,
}

impl Tool {
    /// Builds a tool with the two keys MCP requires.
    ///
    /// # Errors
    /// Returns [`CatalogError::EmptyName`] if `name` is empty.
    pub fn new(name: String, input_schema: Map<String, Value>) -> Result<Self, CatalogError> {
        Ok(Self {
            name: non_empty(name, "tool")?,
            title: None,
            description: None,
            input_schema,
            output_schema: None,
            annotations: None,
            extra: Map::new(),
        })
    }

    /// Sets the human-readable title.
    #[must_use]
    pub fn with_title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    /// Sets the human-readable description.
    #[must_use]
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Sets the JSON Schema object of the structured output.
    #[must_use]
    pub fn with_output_schema(mut self, output_schema: Map<String, Value>) -> Self {
        self.output_schema = Some(output_schema);
        self
    }

    /// Sets the annotations.
    #[must_use]
    pub fn with_annotations(mut self, annotations: Map<String, Value>) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Adds a key the tool keeps as given, replacing an earlier value of the same key.
    ///
    /// # Errors
    /// Returns [`CatalogError::ReservedKey`] if `key` is the name of a field of the tool.
    pub fn with_extra(mut self, key: String, value: Value) -> Result<Self, CatalogError> {
        // BORROW: explicit .as_str() to match the key against the field names
        if NAMED_TOOL_KEYS.contains(&key.as_str()) {
            return Err(CatalogError::ReservedKey { key });
        }
        self.extra.insert(key, value);
        Ok(self)
    }

    /// The name of the tool.
    #[must_use]
    pub const fn name<'a>(&'a self) -> &'a str {
        // BORROW: explicit .as_str() instead of Deref coercion
        self.name.as_str()
    }

    /// The human-readable title, if the server gave one.
    #[must_use]
    pub fn title<'a>(&'a self) -> Option<&'a str> {
        // BORROW: explicit .as_deref() instead of Deref coercion
        self.title.as_deref()
    }

    /// The human-readable description, if the server gave one.
    #[must_use]
    pub fn description<'a>(&'a self) -> Option<&'a str> {
        // BORROW: explicit .as_deref() instead of Deref coercion
        self.description.as_deref()
    }

    /// The JSON Schema object that describes the arguments.
    #[must_use]
    pub const fn input_schema<'a>(&'a self) -> &'a Map<String, Value> {
        &self.input_schema
    }

    /// The JSON Schema object that describes the structured output, if the server gave one.
    #[must_use]
    pub const fn output_schema<'a>(&'a self) -> Option<&'a Map<String, Value>> {
        self.output_schema.as_ref()
    }

    /// The annotations, if the server gave any.
    #[must_use]
    pub const fn annotations<'a>(&'a self) -> Option<&'a Map<String, Value>> {
        self.annotations.as_ref()
    }

    /// The keys that are not fields of the tool, as the server gave them.
    #[must_use]
    pub const fn extra<'a>(&'a self) -> &'a Map<String, Value> {
        &self.extra
    }
}

/// One MCP server of a run and the tools it offered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct CatalogServer {
    /// The name of the server: at least one character, and unique among the servers of its snapshot.
    #[serde(deserialize_with = "server_name")]
    #[schemars(length(min = 1))]
    name: String,
    /// The path of the server's source file: a POSIX path relative to the project root.
    source_path: RelPath,
    /// The digest of the server's source file.
    source_digest: Digest,
    /// How a client reaches the server.
    transport: Transport,
    /// The tools the server offered, with names unique among them.
    #[serde(deserialize_with = "distinct_tools_of")]
    #[schemars(extend(
        "$comment" = "Tool names are unique within the array, compared byte for byte (case-sensitive, without Unicode normalisation); JSON Schema cannot state this, and the Rust reader refuses a repeated name."
    ))]
    tools: Vec<Tool>,
}

impl CatalogServer {
    /// Builds a server entry.
    ///
    /// # Errors
    /// Returns [`CatalogError::EmptyName`] if `name` is empty.
    /// Returns [`CatalogError::DuplicateName`] if two of the tools have the same name.
    pub fn new(
        name: String,
        source_path: RelPath,
        source_digest: Digest,
        transport: Transport,
        tools: Vec<Tool>,
    ) -> Result<Self, CatalogError> {
        distinct_tools(&tools)?;
        Ok(Self {
            name: non_empty(name, "server")?,
            source_path,
            source_digest,
            transport,
            tools,
        })
    }

    /// The name of the server.
    #[must_use]
    pub const fn name<'a>(&'a self) -> &'a str {
        // BORROW: explicit .as_str() instead of Deref coercion
        self.name.as_str()
    }

    /// The path of the server's source file.
    #[must_use]
    pub const fn source_path<'a>(&'a self) -> &'a RelPath {
        &self.source_path
    }

    /// The digest of the server's source file.
    #[must_use]
    pub const fn source_digest<'a>(&'a self) -> &'a Digest {
        &self.source_digest
    }

    /// How a client reaches the server.
    #[must_use]
    pub const fn transport(&self) -> Transport {
        self.transport
    }

    /// The tools the server offered.
    #[must_use]
    pub const fn tools<'a>(&'a self) -> &'a [Tool] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.tools.as_slice()
    }
}

/// What the MCP servers targeted by a run offered: for each server, its tools in the shape of an MCP `tools/list` result.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct CatalogSnapshot {
    /// The version marker of the document: always `tool-catalog-snapshot/1`.
    schema: SchemaTag,
    /// When the snapshot was captured.
    captured_at: Timestamp,
    /// The catalog provider that captured the snapshot, as `name@token`.
    provider: NameToken,
    /// The versions the capture depended on, as an object from a name to its version text.
    #[serde(skip_serializing_if = "Option::is_none")]
    environment: Option<BTreeMap<String, String>>,
    /// Provider-specific data: an open JSON object that Candler does not interpret.
    #[serde(skip_serializing_if = "Option::is_none")]
    ext: Option<Map<String, Value>>,
    /// The servers the run targeted, each with the tools it offered, with names unique among them.
    #[schemars(extend(
        "$comment" = "Server names are unique within the array, compared byte for byte (case-sensitive, without Unicode normalisation); JSON Schema cannot state this, and the Rust reader refuses a repeated name."
    ))]
    servers: Vec<CatalogServer>,
}

impl CatalogSnapshot {
    /// Starts a snapshot; `captured_at` and `provider` must be set before it builds.
    #[must_use]
    pub fn builder() -> CatalogSnapshotBuilder {
        CatalogSnapshotBuilder::default()
    }

    /// When the snapshot was captured.
    #[must_use]
    pub const fn captured_at<'a>(&'a self) -> &'a Timestamp {
        &self.captured_at
    }

    /// The catalog provider that captured the snapshot.
    #[must_use]
    pub const fn provider<'a>(&'a self) -> &'a NameToken {
        &self.provider
    }

    /// The versions the capture depended on, if any were recorded.
    #[must_use]
    pub const fn environment<'a>(&'a self) -> Option<&'a BTreeMap<String, String>> {
        self.environment.as_ref()
    }

    /// The provider-specific data, if the provider wrote any.
    #[must_use]
    pub const fn ext<'a>(&'a self) -> Option<&'a Map<String, Value>> {
        self.ext.as_ref()
    }

    /// The servers the run targeted.
    #[must_use]
    pub const fn servers<'a>(&'a self) -> &'a [CatalogServer] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.servers.as_slice()
    }
}

/// The fields of a snapshot as written, before the version marker is checked.
#[derive(Deserialize)]
struct CatalogSnapshotWire {
    schema: String,
    captured_at: Timestamp,
    provider: NameToken,
    #[serde(default, deserialize_with = "present")]
    environment: Option<BTreeMap<String, String>>,
    #[serde(default, deserialize_with = "present")]
    ext: Option<Map<String, Value>>,
    servers: Vec<CatalogServer>,
}

impl TryFrom<CatalogSnapshotWire> for CatalogSnapshot {
    type Error = CatalogError;

    fn try_from(wire: CatalogSnapshotWire) -> Result<Self, Self::Error> {
        // BORROW: explicit .as_str() to compare with the tag
        if wire.schema.as_str() != SCHEMA_TAG {
            return Err(CatalogError::SchemaTag { value: wire.schema });
        }
        distinct_servers(&wire.servers)?;
        Ok(Self {
            schema: SchemaTag,
            captured_at: wire.captured_at,
            provider: wire.provider,
            environment: wire.environment,
            ext: wire.ext,
            servers: wire.servers,
        })
    }
}

impl<'de> Deserialize<'de> for CatalogSnapshot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(CatalogSnapshotWire::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// Builds a [`CatalogSnapshot`].
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct CatalogSnapshotBuilder {
    captured_at: Option<Timestamp>,
    provider: Option<NameToken>,
    environment: Option<BTreeMap<String, String>>,
    ext: Option<Map<String, Value>>,
    servers: Vec<CatalogServer>,
}

impl CatalogSnapshotBuilder {
    /// Sets when the snapshot was captured.
    #[must_use]
    pub fn captured_at(mut self, captured_at: Timestamp) -> Self {
        self.captured_at = Some(captured_at);
        self
    }

    /// Sets the catalog provider.
    #[must_use]
    pub fn provider(mut self, provider: NameToken) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Sets the versions the capture depended on.
    #[must_use]
    pub fn environment(mut self, environment: BTreeMap<String, String>) -> Self {
        self.environment = Some(environment);
        self
    }

    /// Sets the provider-specific data.
    #[must_use]
    pub fn ext(mut self, ext: Map<String, Value>) -> Self {
        self.ext = Some(ext);
        self
    }

    /// Appends a server.
    #[must_use]
    pub fn server(mut self, server: CatalogServer) -> Self {
        self.servers.push(server);
        self
    }

    /// Builds the snapshot.
    ///
    /// # Errors
    /// Returns [`CatalogError::MissingField`] if `captured_at` or `provider` was not set.
    /// Returns [`CatalogError::DuplicateName`] if two of the servers have the same name.
    pub fn build(self) -> Result<CatalogSnapshot, CatalogError> {
        let captured_at = self.captured_at.ok_or(CatalogError::MissingField {
            field: "captured_at",
        })?;
        let provider = self
            .provider
            .ok_or(CatalogError::MissingField { field: "provider" })?;
        distinct_servers(&self.servers)?;
        Ok(CatalogSnapshot {
            schema: SchemaTag,
            captured_at,
            provider,
            environment: self.environment,
            ext: self.ext,
            servers: self.servers,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn tool_value() -> Value {
        json!({
            "name": "t",
            "title": "T",
            "description": "d",
            "inputSchema": {"type": "object"},
            "outputSchema": {"type": "object"},
            "annotations": {"readOnlyHint": true},
            "icons": [],
            "_meta": {"k": null},
        })
    }

    fn tool(name: &str) -> Tool {
        Tool::new(String::from(name), Map::new()).expect("a tool with a name")
    }

    fn server(name: &str, tools: Vec<Tool>) -> Result<CatalogServer, CatalogError> {
        CatalogServer::new(
            String::from(name),
            "a/b.py".parse().expect("path"),
            Digest::from_hex(&"ab".repeat(32)).expect("digest"),
            Transport::Stdio,
            tools,
        )
    }

    fn snapshot_value() -> Value {
        json!({
            "schema": SCHEMA_TAG,
            "captured_at": "2026-10-28T01:09:04+00:00",
            "provider": "p/q@1",
            "servers": [{
                "name": "s",
                "source_path": "a/b.py",
                "source_digest": format!("sha256:{}", "ab".repeat(32)),
                "transport": "stdio",
                "tools": [tool_value()],
            }],
        })
    }

    #[test]
    fn named_keys_are_exactly_the_fields_a_full_tool_writes() {
        let tool: Tool = serde_json::from_value(tool_value()).expect("a full tool reads");
        let written = serde_json::to_value(&tool).expect("a tool writes");
        let mut named: Vec<&str> = written
            .as_object()
            .expect("a tool is an object")
            .keys()
            // BORROW: explicit .as_str() to compare with the named keys
            .map(String::as_str)
            .filter(|key| !tool.extra().contains_key(*key))
            .collect();
        named.sort_unstable();
        let mut expected = NAMED_TOOL_KEYS.to_vec();
        expected.sort_unstable();
        assert_eq!(named, expected);
        assert_eq!(tool.extra().len(), 2);
    }

    #[test]
    fn a_tool_keeps_null_inside_its_values_and_extra_keys() {
        let value = json!({
            "name": "t",
            "inputSchema": {"default": null, "properties": {"a": {"enum": [null, 1]}}},
            "annotations": {"title": null},
            "outputSchema": {"const": null},
            "x": null,
        });
        let tool: Tool = serde_json::from_value(value.clone()).expect("null is data here");
        assert_eq!(serde_json::to_value(&tool).expect("writes"), value);
        assert_eq!(tool.extra().get("x"), Some(&Value::Null));
    }

    #[test]
    fn a_tool_refuses_null_for_its_optional_fields() {
        for key in ["title", "description", "outputSchema", "annotations"] {
            let mut value = tool_value();
            value[key] = Value::Null;
            let error = serde_json::from_value::<Tool>(value).expect_err(key);
            assert!(
                error.to_string().contains("optional field is null"),
                "{key}"
            );
        }
    }

    #[test]
    fn a_tool_without_its_required_keys_is_refused() {
        for key in ["name", "inputSchema"] {
            let mut value = tool_value();
            value.as_object_mut().expect("object").remove(key);
            assert!(serde_json::from_value::<Tool>(value).is_err(), "{key}");
        }
        let mut value = tool_value();
        value["inputSchema"] = json!("object");
        assert!(serde_json::from_value::<Tool>(value).is_err());
    }

    #[test]
    fn an_extra_key_cannot_take_the_name_of_a_field() {
        for key in NAMED_TOOL_KEYS {
            let result = tool("t").with_extra(String::from(key), json!(1));
            assert_eq!(
                result.expect_err(key),
                CatalogError::ReservedKey {
                    key: String::from(key)
                }
            );
        }
        let tool = tool("t")
            .with_extra(String::from("icons"), json!([]))
            .expect("icons is an extra key");
        assert_eq!(tool.extra().len(), 1);
    }

    #[test]
    fn a_snapshot_refuses_a_foreign_tag_and_null_optionals() {
        let snapshot: CatalogSnapshot =
            serde_json::from_value(snapshot_value()).expect("a snapshot reads");
        assert_eq!(snapshot.servers().len(), 1);

        let mut value = snapshot_value();
        value["schema"] = json!("tool-catalog-snapshot/0");
        let error = serde_json::from_value::<CatalogSnapshot>(value).expect_err("tag");
        assert!(error.to_string().contains("schema tag differs"));

        for key in ["environment", "ext"] {
            let mut value = snapshot_value();
            value[key] = Value::Null;
            let error = serde_json::from_value::<CatalogSnapshot>(value).expect_err(key);
            assert!(
                error.to_string().contains("optional field is null"),
                "{key}"
            );
        }
    }

    #[test]
    fn the_builder_requires_the_capture_time_and_the_provider() {
        let timestamp: Timestamp = "2026-10-28T01:09:04Z".parse().expect("timestamp");
        let provider: NameToken = "p@1".parse().expect("provider");
        assert_eq!(
            CatalogSnapshot::builder()
                .provider(provider.clone())
                .build()
                .expect_err("no time"),
            CatalogError::MissingField {
                field: "captured_at"
            }
        );
        assert_eq!(
            CatalogSnapshot::builder()
                .captured_at(timestamp.clone())
                .build()
                .expect_err("no provider"),
            CatalogError::MissingField { field: "provider" }
        );
        let built = CatalogSnapshot::builder()
            .captured_at(timestamp)
            .provider(provider)
            .build()
            .expect("minimal snapshot builds");
        assert_eq!(built.servers(), []);
        assert_eq!(
            serde_json::to_value(&built).expect("writes"),
            json!({
                "schema": SCHEMA_TAG,
                "captured_at": "2026-10-28T01:09:04Z",
                "provider": "p@1",
                "servers": [],
            })
        );
    }

    #[test]
    fn names_are_non_empty_and_unique_within_their_scope() {
        assert_eq!(
            Tool::new(String::new(), Map::new()).expect_err("empty tool name"),
            CatalogError::EmptyName { owner: "tool" }
        );
        assert_eq!(
            server("", Vec::new()).expect_err("empty server name"),
            CatalogError::EmptyName { owner: "server" }
        );
        assert_eq!(
            server("s", vec![tool("a"), tool("b"), tool("a")]).expect_err("repeated tool"),
            CatalogError::DuplicateName {
                owner: "tool",
                scope: "server",
                name: String::from("a")
            }
        );
        let timestamp: Timestamp = "2026-10-28T01:09:04Z".parse().expect("timestamp");
        let provider: NameToken = "p@1".parse().expect("provider");
        let repeated = CatalogSnapshot::builder()
            .captured_at(timestamp)
            .provider(provider)
            .server(server("s", vec![tool("a")]).expect("server"))
            .server(server("s", vec![tool("b")]).expect("server"))
            .build()
            .expect_err("repeated server");
        assert_eq!(
            repeated,
            CatalogError::DuplicateName {
                owner: "server",
                scope: "snapshot",
                name: String::from("s")
            }
        );
        // The same name in different scopes is allowed.
        server("s", vec![tool("a")]).expect("tool names repeat across servers");
    }

    #[test]
    fn the_reader_refuses_empty_and_repeated_names() {
        let mut empty_tool = tool_value();
        empty_tool["name"] = json!("");
        assert!(serde_json::from_value::<Tool>(empty_tool).is_err());

        let mut value = snapshot_value();
        value["servers"][0]["name"] = json!("");
        let error = serde_json::from_value::<CatalogSnapshot>(value).expect_err("empty server");
        assert!(error.to_string().contains("server name is empty"));

        let mut value = snapshot_value();
        value["servers"][0]["tools"] = json!([tool_value(), tool_value()]);
        let error = serde_json::from_value::<CatalogSnapshot>(value).expect_err("repeated tool");
        assert!(error.to_string().contains("tool name is repeated"));

        let mut value = snapshot_value();
        let first = value["servers"][0].clone();
        value["servers"] = json!([first.clone(), first]);
        let error = serde_json::from_value::<CatalogSnapshot>(value).expect_err("repeated server");
        assert!(error.to_string().contains("server name is repeated"));
    }

    #[test]
    fn every_error_has_its_wording() {
        let cases = [
            (
                CatalogError::SchemaTag {
                    value: String::from("x/1"),
                },
                r#"schema tag differs from `tool-catalog-snapshot/1` (got "x/1")"#,
            ),
            (
                CatalogError::MissingField { field: "provider" },
                "snapshot field is missing (`provider` was not set)",
            ),
            (
                CatalogError::ReservedKey {
                    key: String::from("name"),
                },
                r#"tool extra key names a field of the tool (got "name")"#,
            ),
            (
                CatalogError::Null,
                "optional field is null (an absent field is omitted)",
            ),
            (
                CatalogError::EmptyName { owner: "server" },
                "server name is empty (names have at least one character)",
            ),
            (
                CatalogError::DuplicateName {
                    owner: "tool",
                    scope: "server",
                    name: String::from("a"),
                },
                r#"tool name is repeated within a server (got "a")"#,
            ),
        ];
        for (error, text) in cases {
            assert_eq!(error.to_string(), text);
        }
    }
}
