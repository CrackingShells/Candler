//! Properties of the tool-catalog snapshot: generated snapshots are valid by the committed schema
//! and by the Rust reader, and round-trip without loss; each rule the format states is refused by
//! both. The cases where the schema and the Rust reader differ are pinned at the end.

// Marks the file as test code, so that clippy's test allowances reach its helpers.
#![cfg(test)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use candler_schemas::catalog::{CatalogServer, CatalogSnapshot, SCHEMA_TAG, Tool, Transport};
use candler_schemas::primitives::{Digest, NameToken, RelPath, Timestamp};
use jsonschema::Validator;
use proptest::prelude::*;
use serde_json::{Map, Value, json};

const NAMED_TOOL_KEYS: [&str; 6] = [
    "name",
    "title",
    "description",
    "inputSchema",
    "outputSchema",
    "annotations",
];

const TRANSPORTS: [(&str, Transport); 3] = [
    ("stdio", Transport::Stdio),
    ("http", Transport::Http),
    ("sse", Transport::Sse),
];

fn schema_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schema")
        .join("catalog-snapshot.schema.json")
}

fn read_schema() -> Value {
    let text = fs::read_to_string(schema_path()).expect("the committed schema is readable");
    serde_json::from_str(&text).expect("the committed schema is JSON")
}

static SCHEMA: LazyLock<Validator> = LazyLock::new(|| {
    jsonschema::validator_for(&read_schema()).expect("the committed schema compiles")
});

// ---- generated snapshots --------------------------------------------------------------------

/// Any JSON value: `null`, booleans, integers, finite floats (zeros of both signs and subnormals
/// included), strings, arrays and objects.
fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        any::<u64>().prop_map(Value::from),
        (prop::num::f64::NORMAL | prop::num::f64::SUBNORMAL | prop::num::f64::ZERO)
            .prop_filter_map("finite", |f| {
                serde_json::Number::from_f64(f).map(Value::Number)
            }),
        Just(Value::from(-0.0_f64)),
        Just(Value::from(0.0_f64)),
        "\\PC{0,8}".prop_map(Value::String),
    ];
    leaf.prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
            prop::collection::btree_map("\\PC{0,6}", inner, 0..4)
                .prop_map(|map| Value::Object(map.into_iter().collect())),
        ]
    })
}

fn arb_object() -> impl Strategy<Value = Map<String, Value>> {
    prop::collection::btree_map("\\PC{0,6}", arb_json(), 0..4)
        .prop_map(|map| map.into_iter().collect())
}

/// Objects whose keys are none of the named keys of a tool.
fn arb_extras() -> impl Strategy<Value = Map<String, Value>> {
    let key = "\\PC{0,6}".prop_filter("a named key", |key| {
        !NAMED_TOOL_KEYS.iter().any(|named| named == key)
    });
    prop::collection::btree_map(key, arb_json(), 0..4).prop_map(|map| map.into_iter().collect())
}

#[derive(Debug, Clone)]
struct ToolParts {
    name: String,
    title: Option<String>,
    description: Option<String>,
    input_schema: Map<String, Value>,
    output_schema: Option<Map<String, Value>>,
    annotations: Option<Map<String, Value>>,
    extra: Map<String, Value>,
}

impl ToolParts {
    /// The JSON the parts stand for, assembled without the library.
    fn value(&self) -> Value {
        let mut tool = self.extra.clone();
        tool.insert(String::from("name"), Value::String(self.name.clone()));
        tool.insert(
            String::from("inputSchema"),
            Value::Object(self.input_schema.clone()),
        );
        for (key, text) in [("title", &self.title), ("description", &self.description)] {
            if let Some(text) = text {
                tool.insert(String::from(key), Value::String(text.clone()));
            }
        }
        for (key, object) in [
            ("outputSchema", &self.output_schema),
            ("annotations", &self.annotations),
        ] {
            if let Some(object) = object {
                tool.insert(String::from(key), Value::Object(object.clone()));
            }
        }
        Value::Object(tool)
    }

    /// The tool the parts stand for, built through the library.
    fn built(&self) -> Tool {
        let mut tool = Tool::new(self.name.clone(), self.input_schema.clone())
            .expect("generated tool names are not empty");
        if let Some(title) = &self.title {
            tool = tool.with_title(title.clone());
        }
        if let Some(description) = &self.description {
            tool = tool.with_description(description.clone());
        }
        if let Some(output_schema) = &self.output_schema {
            tool = tool.with_output_schema(output_schema.clone());
        }
        if let Some(annotations) = &self.annotations {
            tool = tool.with_annotations(annotations.clone());
        }
        for (key, value) in &self.extra {
            tool = tool
                .with_extra(key.clone(), value.clone())
                .expect("generated extra keys are not named keys");
        }
        tool
    }
}

/// The items whose key has not occurred before, in order.
fn distinct_by<T>(items: Vec<T>, key: impl Fn(&T) -> String) -> Vec<T> {
    let mut seen = BTreeSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(key(item)))
        .collect()
}

fn arb_tool() -> impl Strategy<Value = ToolParts> {
    (
        "\\PC{1,8}",
        proptest::option::of("\\PC{0,8}"),
        proptest::option::of("\\PC{0,8}"),
        arb_object(),
        proptest::option::of(arb_object()),
        proptest::option::of(arb_object()),
        arb_extras(),
    )
        .prop_map(
            |(name, title, description, input_schema, output_schema, annotations, extra)| {
                ToolParts {
                    name,
                    title,
                    description,
                    input_schema,
                    output_schema,
                    annotations,
                    extra,
                }
            },
        )
}

#[derive(Debug, Clone)]
struct ServerParts {
    name: String,
    source_path: String,
    source_digest: String,
    transport: (&'static str, Transport),
    tools: Vec<ToolParts>,
}

impl ServerParts {
    fn value(&self) -> Value {
        json!({
            "name": self.name,
            "source_path": self.source_path,
            "source_digest": self.source_digest,
            "transport": self.transport.0,
            "tools": self.tools.iter().map(ToolParts::value).collect::<Vec<_>>(),
        })
    }

    fn built(&self) -> CatalogServer {
        CatalogServer::new(
            self.name.clone(),
            self.source_path
                .parse::<RelPath>()
                .expect("generated paths are valid"),
            self.source_digest
                .parse::<Digest>()
                .expect("generated digests are valid"),
            self.transport.1,
            self.tools.iter().map(ToolParts::built).collect(),
        )
        .expect("generated server and tool names are valid")
    }
}

fn arb_server(min_tools: usize) -> impl Strategy<Value = ServerParts> {
    (
        "\\PC{1,8}",
        "[A-Za-z0-9_-][A-Za-z0-9._-]{0,6}(/[A-Za-z0-9_-][A-Za-z0-9._-]{0,6}){0,3}",
        "[0-9a-f]{64}".prop_map(|hex| format!("sha256:{hex}")),
        prop::sample::select(TRANSPORTS.to_vec()),
        prop::collection::vec(arb_tool(), min_tools..4),
    )
        .prop_map(
            |(name, source_path, source_digest, transport, tools)| ServerParts {
                name,
                source_path,
                source_digest,
                transport,
                tools: distinct_by(tools, |tool| tool.name.clone()),
            },
        )
}

#[derive(Debug, Clone)]
struct SnapshotParts {
    captured_at: String,
    provider: String,
    environment: Option<BTreeMap<String, String>>,
    ext: Option<Map<String, Value>>,
    servers: Vec<ServerParts>,
}

impl SnapshotParts {
    fn value(&self) -> Value {
        let mut snapshot = json!({
            "schema": SCHEMA_TAG,
            "captured_at": self.captured_at,
            "provider": self.provider,
            "servers": self.servers.iter().map(ServerParts::value).collect::<Vec<_>>(),
        });
        if let Some(environment) = &self.environment {
            snapshot["environment"] = json!(environment);
        }
        if let Some(ext) = &self.ext {
            snapshot["ext"] = Value::Object(ext.clone());
        }
        snapshot
    }

    fn built(&self) -> CatalogSnapshot {
        let mut builder = CatalogSnapshot::builder()
            .captured_at(
                self.captured_at
                    .parse::<Timestamp>()
                    .expect("generated timestamps are valid"),
            )
            .provider(
                self.provider
                    .parse::<NameToken>()
                    .expect("generated providers are valid"),
            );
        if let Some(environment) = &self.environment {
            builder = builder.environment(environment.clone());
        }
        if let Some(ext) = &self.ext {
            builder = builder.ext(ext.clone());
        }
        for server in &self.servers {
            builder = builder.server(server.built());
        }
        builder.build().expect("both required fields are set")
    }
}

fn arb_timestamp() -> impl Strategy<Value = String> {
    let offset = prop_oneof![
        Just(String::from("Z")),
        (any::<bool>(), 0u32..24, 0u32..60).prop_map(|(negative, hours, minutes)| {
            let sign = if negative { '-' } else { '+' };
            format!("{sign}{hours:02}:{minutes:02}")
        }),
    ];
    (
        (1970u32..2100, 1u32..=12, 1u32..=28),
        (0u32..24, 0u32..60, 0u32..60),
        proptest::option::of("[0-9]{1,9}"),
        offset,
    )
        .prop_map(
            |((year, month, day), (hour, minute, second), fraction, offset)| {
                let date = format!("{year:04}-{month:02}-{day:02}");
                let time = format!("{hour:02}:{minute:02}:{second:02}");
                let fraction = fraction
                    .map(|digits| format!(".{digits}"))
                    .unwrap_or_default();
                format!("{date}T{time}{fraction}{offset}")
            },
        )
}

fn arb_snapshot(min_servers: usize, min_tools: usize) -> impl Strategy<Value = SnapshotParts> {
    (
        arb_timestamp(),
        "[A-Za-z0-9_./-]{1,12}@[0-9A-Za-z.+-]{1,8}",
        proptest::option::of(prop::collection::btree_map("\\PC{0,6}", "\\PC{0,6}", 0..4)),
        proptest::option::of(arb_object()),
        prop::collection::vec(arb_server(min_tools), min_servers..3),
    )
        .prop_map(
            |(captured_at, provider, environment, ext, servers)| SnapshotParts {
                captured_at,
                provider,
                environment,
                ext,
                servers: distinct_by(servers, |server| server.name.clone()),
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// JSON of any shape the format allows is valid by the schema, is read by Rust and is written
    /// back as the same value, through text.
    #[test]
    fn catalog_snapshot_json_with_arbitrary_extra_keys_is_valid_and_round_trips(
        parts in arb_snapshot(0, 0),
    ) {
        let value = parts.value();
        prop_assert!(SCHEMA.is_valid(&value), "the schema rejects {value:#}");
        let text = serde_json::to_string(&value)?;
        let snapshot: CatalogSnapshot = serde_json::from_str(&text)?;
        prop_assert_eq!(serde_json::to_value(&snapshot)?, value);
        // The text is the same too, so a float keeps its sign and its digits.
        prop_assert_eq!(serde_json::to_string(&serde_json::to_value(&snapshot)?)?, text);
    }

    /// A snapshot built through the builders writes the same JSON, which the schema accepts and
    /// Rust reads back as an equal snapshot.
    #[test]
    fn catalog_snapshot_built_through_the_builders_writes_valid_json_and_reads_back_equal(
        parts in arb_snapshot(0, 0),
    ) {
        let built = parts.built();
        let written = serde_json::to_value(&built)?;
        prop_assert_eq!(&written, &parts.value());
        prop_assert!(SCHEMA.is_valid(&written));
        let again: CatalogSnapshot = serde_json::from_str(&serde_json::to_string(&built)?)?;
        prop_assert_eq!(again, built);
    }
}

// ---- one mutation per rule ------------------------------------------------------------------

/// Both the schema and the Rust reader refuse `value`.
fn assert_rejected(value: &Value) -> Result<(), TestCaseError> {
    prop_assert!(!SCHEMA.is_valid(value), "the schema accepts {value:#}");
    prop_assert!(
        serde_json::from_value::<CatalogSnapshot>(value.clone()).is_err(),
        "Rust accepts {value:#}"
    );
    Ok(())
}

/// The value at `pointer`, which every snapshot of `arb_snapshot(1, 1)` has.
fn at<'a>(value: &'a mut Value, pointer: &str) -> &'a mut Value {
    value
        .pointer_mut(pointer)
        .expect("a snapshot of one server and one tool has this path")
}

fn set(value: &mut Value, pointer: &str, key: &str, replacement: Value) {
    if let Some(object) = at(value, pointer).as_object_mut() {
        object.insert(String::from(key), replacement);
    }
}

fn remove(value: &mut Value, pointer: &str, key: &str) {
    if let Some(object) = at(value, pointer).as_object_mut() {
        object.remove(key);
    }
}

const TOOL: &str = "/servers/0/tools/0";
const SERVER: &str = "/servers/0";

fn arb_non_object() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        "\\PC{0,8}".prop_map(Value::String),
        prop::collection::vec(arb_json(), 0..3).prop_map(Value::Array),
    ]
}

fn arb_bad_digest() -> impl Strategy<Value = String> {
    prop_oneof![
        "[0-9a-f]{64}",
        "[0-9a-f]{64}".prop_map(|hex| format!("md5:{hex}")),
        "[A-F]{64}".prop_map(|hex| format!("sha256:{hex}")),
        "[0-9a-f]{0,63}".prop_map(|hex| format!("sha256:{hex}")),
        "[0-9a-f]{65,70}".prop_map(|hex| format!("sha256:{hex}")),
    ]
}

fn arb_bad_source_path() -> impl Strategy<Value = String> {
    prop_oneof![
        "/[a-z]{1,5}(/[a-z]{1,5}){0,2}",
        "[a-z]{1,4}/\\.\\./[a-z]{1,4}",
        Just(String::new()),
        "[a-z]{1,4}\\\\[a-z]{1,4}",
        "[a-z]{1,4}//[a-z]{1,4}",
    ]
}

fn arb_unknown_transport() -> impl Strategy<Value = String> {
    "[a-zA-Z]{1,8}".prop_filter("a known transport", |text| {
        !TRANSPORTS.iter().any(|(known, _)| known == text)
    })
}

/// A field the format names, with the object that holds it, as a pointer and a key.
const NULLABLE: [(&str, &str); 6] = [
    ("", "environment"),
    ("", "ext"),
    (TOOL, "title"),
    (TOOL, "description"),
    (TOOL, "outputSchema"),
    (TOOL, "annotations"),
];

const REQUIRED: [(&str, &str); 11] = [
    ("", "schema"),
    ("", "captured_at"),
    ("", "provider"),
    ("", "servers"),
    (SERVER, "name"),
    (SERVER, "source_path"),
    (SERVER, "source_digest"),
    (SERVER, "transport"),
    (SERVER, "tools"),
    (TOOL, "name"),
    (TOOL, "inputSchema"),
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn catalog_snapshot_without_input_schema_is_rejected(parts in arb_snapshot(1, 1)) {
        let mut value = parts.value();
        remove(&mut value, TOOL, "inputSchema");
        assert_rejected(&value)?;
    }

    #[test]
    fn catalog_snapshot_with_a_non_object_input_schema_is_rejected(
        parts in arb_snapshot(1, 1),
        bad in arb_non_object(),
    ) {
        let mut value = parts.value();
        set(&mut value, TOOL, "inputSchema", bad);
        assert_rejected(&value)?;
    }

    #[test]
    fn catalog_snapshot_with_a_bad_digest_is_rejected(
        parts in arb_snapshot(1, 1),
        bad in arb_bad_digest(),
    ) {
        let mut value = parts.value();
        set(&mut value, SERVER, "source_digest", Value::String(bad));
        assert_rejected(&value)?;
    }

    #[test]
    fn catalog_snapshot_with_a_bad_source_path_is_rejected(
        parts in arb_snapshot(1, 1),
        bad in arb_bad_source_path(),
    ) {
        let mut value = parts.value();
        set(&mut value, SERVER, "source_path", Value::String(bad));
        assert_rejected(&value)?;
    }

    #[test]
    fn catalog_snapshot_with_an_unknown_transport_is_rejected(
        parts in arb_snapshot(1, 1),
        bad in arb_unknown_transport(),
    ) {
        let mut value = parts.value();
        set(&mut value, SERVER, "transport", Value::String(bad));
        assert_rejected(&value)?;
    }

    #[test]
    fn catalog_snapshot_with_a_null_optional_field_is_rejected(
        parts in arb_snapshot(1, 1),
        (pointer, key) in prop::sample::select(NULLABLE.to_vec()),
    ) {
        let mut value = parts.value();
        set(&mut value, pointer, key, Value::Null);
        assert_rejected(&value)?;
    }

    #[test]
    fn catalog_snapshot_without_a_required_field_is_rejected(
        parts in arb_snapshot(1, 1),
        (pointer, key) in prop::sample::select(REQUIRED.to_vec()),
    ) {
        let mut value = parts.value();
        remove(&mut value, pointer, key);
        assert_rejected(&value)?;
    }

    #[test]
    fn catalog_snapshot_with_another_schema_tag_is_rejected(
        parts in arb_snapshot(0, 0),
        bad in "[a-z/0-9-]{0,30}".prop_filter("the tag", |text| text != SCHEMA_TAG),
    ) {
        let mut value = parts.value();
        set(&mut value, "", "schema", Value::String(bad));
        assert_rejected(&value)?;
    }

    /// `null` is data inside the open objects of a tool and of the snapshot, so it never makes a
    /// snapshot invalid there.
    #[test]
    fn catalog_snapshot_keeps_null_inside_open_objects(parts in arb_snapshot(1, 1)) {
        let mut value = parts.value();
        for (pointer, key) in [
            (TOOL, "inputSchema"),
            (TOOL, "outputSchema"),
            (TOOL, "annotations"),
            ("", "ext"),
        ] {
            set(&mut value, pointer, key, json!({"nested": null, "list": [null]}));
        }
        set(&mut value, TOOL, "extra", Value::Null);
        prop_assert!(SCHEMA.is_valid(&value));
        let snapshot: CatalogSnapshot = serde_json::from_value(value.clone())?;
        prop_assert_eq!(serde_json::to_value(&snapshot)?, value);
    }
}

// ---- where the schema and the Rust reader differ --------------------------------------------

fn minimal() -> Value {
    json!({
        "schema": SCHEMA_TAG,
        "captured_at": "2026-10-28T01:09:04+00:00",
        "provider": "capture@1",
        "servers": [{
            "name": "s",
            "source_path": "servers/s.py",
            "source_digest": format!("sha256:{}", "ab".repeat(32)),
            "transport": "stdio",
            "tools": [{"name": "t", "inputSchema": {"type": "object"}}],
        }],
    })
}

#[test]
fn catalog_minimal_snapshot_is_valid_in_both() {
    let value = minimal();
    assert!(SCHEMA.is_valid(&value));
    let snapshot: CatalogSnapshot = serde_json::from_value(value.clone()).expect("reads");
    assert_eq!(serde_json::to_value(&snapshot).expect("writes"), value);
}

/// The schemas of the snapshot and of a server are strict: they list what a writer emits. The
/// reader ignores a field it does not know, so the field is not written back.
#[test]
fn catalog_unknown_fields_fail_the_schema_and_are_dropped_by_the_reader() {
    for (pointer, key) in [("", "servers_dir"), (SERVER, "mode")] {
        let mut value = minimal();
        set(&mut value, pointer, key, json!("x"));
        assert!(!SCHEMA.is_valid(&value), "{key}");
        let snapshot: CatalogSnapshot = serde_json::from_value(value.clone()).expect("reads");
        assert_eq!(serde_json::to_value(&snapshot).expect("writes"), minimal());
    }
}

/// A tool is open: an unknown key is valid by the schema and kept by the reader.
#[test]
fn catalog_unknown_tool_keys_pass_the_schema_and_are_kept_by_the_reader() {
    let mut value = minimal();
    set(
        &mut value,
        TOOL,
        "icons",
        json!([{"src": "https://example.com/i.png"}]),
    );
    set(&mut value, TOOL, "x", json!(null));
    assert!(SCHEMA.is_valid(&value));
    let snapshot: CatalogSnapshot = serde_json::from_value(value.clone()).expect("reads");
    assert_eq!(serde_json::to_value(&snapshot).expect("writes"), value);
}

/// The schema states the lexical form of a timestamp; the parser also refuses a date or time
/// that names no instant.
#[test]
fn catalog_timestamp_with_no_instant_passes_the_schema_and_fails_the_reader() {
    for text in [
        "2026-13-01T00:00:00Z",
        "2026-02-30T00:00:00Z",
        "2026-01-01T24:00:00Z",
    ] {
        let mut value = minimal();
        set(&mut value, "", "captured_at", json!(text));
        assert!(SCHEMA.is_valid(&value), "{text}");
        assert!(
            serde_json::from_value::<CatalogSnapshot>(value).is_err(),
            "{text}"
        );
    }
}

/// An integer beyond `u64` is an integer to the schema; `serde_json` holds it as a float, so it
/// is written back as a different number.
#[test]
fn catalog_integer_beyond_u64_is_not_kept_exactly() {
    let text =
        r#"{"name": "t", "inputSchema": {"type": "object", "maximum": 18446744073709551616}}"#;
    let tool: Tool = serde_json::from_str(text).expect("reads");
    assert_eq!(
        serde_json::to_string(tool.input_schema()).expect("writes"),
        r#"{"maximum":1.8446744073709552e+19,"type":"object"}"#
    );
}

/// A repeated key of the JSON text is refused when it names a field of a tool and the last value
/// wins when it is an extra key.
#[test]
fn catalog_repeated_keys_are_refused_for_fields_and_overwritten_for_extra_keys() {
    let field = r#"{"name": "a", "name": "b", "inputSchema": {}}"#;
    assert!(serde_json::from_str::<Tool>(field).is_err());
    let extra = r#"{"name": "a", "inputSchema": {}, "x": 1, "x": 2}"#;
    let tool: Tool = serde_json::from_str(extra).expect("reads");
    assert_eq!(tool.extra().get("x"), Some(&json!(2)));
}

/// An integer-valued float is an integer to the schema; the reader keeps the literal as written.
#[test]
fn catalog_integral_float_keeps_its_literal() {
    let tool: Tool =
        serde_json::from_str(r#"{"name": "t", "inputSchema": {"minimum": 3.0}}"#).expect("reads");
    assert_eq!(
        serde_json::to_string(tool.input_schema()).expect("writes"),
        r#"{"minimum":3.0}"#
    );
}

/// Names are non-empty by the schema and by the reader.
#[test]
fn catalog_empty_names_fail_the_schema_and_the_reader() {
    for pointer in [SERVER, TOOL] {
        let mut value = minimal();
        set(&mut value, pointer, "name", json!(""));
        assert!(!SCHEMA.is_valid(&value), "{pointer}");
        assert!(
            serde_json::from_value::<CatalogSnapshot>(value).is_err(),
            "{pointer}"
        );
    }
}

/// Uniqueness of names is not expressible in JSON Schema: the schema accepts a repeated name, the
/// reader refuses it, and the schema says so in a `$comment`.
#[test]
fn catalog_repeated_names_pass_the_schema_and_fail_the_reader() {
    let mut tools = minimal();
    let tool = tools
        .pointer("/servers/0/tools/0")
        .cloned()
        .expect("a tool");
    set(&mut tools, SERVER, "tools", json!([tool.clone(), tool]));
    let mut servers = minimal();
    let server = servers.pointer(SERVER).cloned().expect("a server");
    set(&mut servers, "", "servers", json!([server.clone(), server]));
    for (what, value) in [("tools", tools), ("servers", servers)] {
        assert!(SCHEMA.is_valid(&value), "{what}");
        let error = serde_json::from_value::<CatalogSnapshot>(value).expect_err(what);
        assert!(error.to_string().contains("is repeated within a"), "{what}");
    }

    let schema = read_schema();
    for pointer in [
        "/properties/servers/$comment",
        "/$defs/CatalogServer/properties/tools/$comment",
    ] {
        let comment = schema.pointer(pointer).and_then(Value::as_str);
        assert!(
            comment.is_some_and(|text| text.contains("JSON Schema cannot state this")),
            "{pointer}"
        );
    }
}

/// The environment maps names to text: any other value is refused by both.
#[test]
fn catalog_environment_with_a_non_string_value_is_rejected() {
    let mut value = minimal();
    set(&mut value, "", "environment", json!({"python": "3.12"}));
    assert!(SCHEMA.is_valid(&value));
    assert!(serde_json::from_value::<CatalogSnapshot>(value).is_ok());
    for bad in [
        json!(3),
        json!(null),
        json!(true),
        json!(["3"]),
        json!({"a": 1}),
    ] {
        let mut value = minimal();
        set(&mut value, "", "environment", json!({"python": bad}));
        assert!(!SCHEMA.is_valid(&value), "{bad}");
        assert!(
            serde_json::from_value::<CatalogSnapshot>(value).is_err(),
            "{bad}"
        );
    }
    for bad in [json!(["python"]), json!("python"), json!(3)] {
        let mut value = minimal();
        set(&mut value, "", "environment", bad.clone());
        assert!(!SCHEMA.is_valid(&value), "{bad}");
        assert!(
            serde_json::from_value::<CatalogSnapshot>(value).is_err(),
            "{bad}"
        );
    }
}

/// Object keys are held in sorted order: the order of the text is not kept, the value is.
#[test]
fn catalog_object_key_order_is_not_kept() {
    let text = r#"{"name": "t", "inputSchema": {"type": "object", "properties": {"z": {}, "a": {}, "m": {}}}}"#;
    let tool: Tool = serde_json::from_str(text).expect("reads");
    let written = serde_json::to_string(&tool).expect("writes");
    assert_eq!(
        written,
        r#"{"name":"t","inputSchema":{"properties":{"a":{},"m":{},"z":{}},"type":"object"}}"#
    );
    let before: Value = serde_json::from_str(text).expect("parses");
    let after: Value = serde_json::from_str(&written).expect("parses");
    assert_eq!(before, after);
}

/// An object nested `depth` levels deep: `{"a": {"a": … {} …}}`.
fn nested(depth: usize) -> Map<String, Value> {
    let mut inner = Map::new();
    for _ in 0..depth {
        let mut outer = Map::new();
        outer.insert(String::from("a"), Value::Object(inner));
        inner = outer;
    }
    inner
}

/// A document nested deeper than the reader's limit is written but not read back. The limit is
/// 128 levels of the whole document, so the depth left for an `inputSchema` is below that.
#[test]
fn catalog_deeply_nested_documents_are_written_and_refused_on_reading() {
    let snapshot_with = |depth: usize| {
        let tool = Tool::new(String::from("t"), nested(depth)).expect("a tool");
        let server = CatalogServer::new(
            String::from("s"),
            "a.py".parse::<RelPath>().expect("path"),
            Digest::from_hex(&"ab".repeat(32)).expect("digest"),
            Transport::Stdio,
            vec![tool],
        )
        .expect("a server");
        CatalogSnapshot::builder()
            .captured_at("2026-10-28T01:09:04Z".parse().expect("timestamp"))
            .provider("p@1".parse().expect("provider"))
            .server(server)
            .build()
            .expect("a snapshot")
    };

    let shallow = snapshot_with(50);
    let text = serde_json::to_string(&shallow).expect("writes");
    assert_eq!(
        serde_json::from_str::<CatalogSnapshot>(&text).expect("reads"),
        shallow
    );

    let deep = snapshot_with(200);
    let text = serde_json::to_string(&deep).expect("writes without a limit");
    let error = serde_json::from_str::<CatalogSnapshot>(&text).expect_err("refused on reading");
    assert!(error.to_string().contains("recursion limit"), "{error}");
}

/// A number outside the range of `f64` is refused; one outside the 64-bit integers is read as a
/// float (see `catalog_integer_beyond_u64_is_not_kept_exactly`).
#[test]
fn catalog_number_beyond_f64_is_refused() {
    let text = r#"{"name": "t", "inputSchema": {"maximum": 1e400}}"#;
    let error = serde_json::from_str::<Tool>(text).expect_err("out of range");
    assert!(error.to_string().contains("number out of range"), "{error}");
}
