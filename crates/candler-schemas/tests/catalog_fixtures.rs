//! The tool-catalog snapshot against the MCP specification and two catalogs captured from real
//! servers.
//!
//! The specification fixtures are tool objects the MCP `Tool` definition accepts; the snapshot
//! must accept them and keep every key. The real catalogs carry `servers_dir` and
//! `stubbed_modules` at the top level and the tag `tool-catalog-snapshot/0`; these tests rewrite
//! the tag to `SCHEMA_TAG` and move the two keys under `ext`, then check the result.

// Marks the file as test code, so that clippy's test allowances reach its helpers.
#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};
use std::slice;
use std::sync::LazyLock;

use candler_schemas::catalog::{CatalogSnapshot, SCHEMA_TAG, Tool};
use jsonschema::Validator;
use proptest::prelude::*;
use serde_json::{Map, Value, json};

fn manifest_path(parts: &[&str]) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(parts);
    path
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The snapshot schema as committed.
static SNAPSHOT_SCHEMA: LazyLock<Value> =
    LazyLock::new(|| read_json(&manifest_path(&["schema", "catalog-snapshot.schema.json"])));

static SNAPSHOT: LazyLock<Validator> = LazyLock::new(|| {
    jsonschema::validator_for(&SNAPSHOT_SCHEMA).expect("the snapshot schema compiles")
});

/// The `Tool` definition inside the snapshot schema.
static OUR_TOOL: LazyLock<Validator> = LazyLock::new(|| {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/Tool",
        "$defs": SNAPSHOT_SCHEMA["$defs"].clone(),
    });
    jsonschema::validator_for(&schema).expect("the Tool definition compiles")
});

/// The `Tool` definition of the MCP specification, validated by the vendored schema file.
static MCP_TOOL: LazyLock<Validator> = LazyLock::new(|| {
    let mut schema = read_json(&manifest_path(&[
        "tests",
        "fixtures",
        "spec",
        "mcp-2026-07-28.schema.json",
    ]));
    schema["$ref"] = json!("#/$defs/Tool");
    jsonschema::validator_for(&schema).expect("the MCP schema compiles")
});

/// A snapshot of one server around `tools`.
fn wrap(tools: &[Value]) -> Value {
    json!({
        "schema": SCHEMA_TAG,
        "captured_at": "2026-10-28T01:09:04+00:00",
        "provider": "capture@1",
        "servers": [{
            "name": "server",
            "source_path": "servers/server.py",
            "source_digest": format!("sha256:{}", "cd".repeat(32)),
            "transport": "stdio",
            "tools": tools,
        }],
    })
}

// ---- the MCP specification ------------------------------------------------------------------

fn spec_tools() -> Vec<(String, Value)> {
    let dir = manifest_path(&["tests", "fixtures", "spec", "tools"]);
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("the spec tool directory is readable")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| {
            let name = path
                .file_name()
                // BORROW: .into_owned() makes the owned file name the pair holds
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            (name, read_json(path))
        })
        .collect()
}

#[test]
fn catalog_spec_tools_are_valid_mcp_tools() {
    let tools = spec_tools();
    assert_eq!(tools.len(), 10);
    for (name, tool) in &tools {
        assert!(MCP_TOOL.is_valid(tool), "MCP rejects {name}");
    }
}

#[test]
fn catalog_spec_tools_validate_and_round_trip_inside_a_snapshot() {
    for (name, tool) in spec_tools() {
        let snapshot = wrap(slice::from_ref(&tool));
        assert!(SNAPSHOT.is_valid(&snapshot), "the schema rejects {name}");
        assert!(
            OUR_TOOL.is_valid(&tool),
            "the Tool definition rejects {name}"
        );

        let read: CatalogSnapshot =
            serde_json::from_value(snapshot.clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            serde_json::to_value(&read).expect("writes"),
            snapshot,
            "{name}"
        );

        let alone: Tool = serde_json::from_value(tool.clone()).expect("a tool reads");
        assert_eq!(
            serde_json::to_value(&alone).expect("writes"),
            tool,
            "{name}"
        );
    }
}

#[test]
fn catalog_full_spec_tool_keeps_icons_and_meta_as_extra_keys() {
    let full = spec_tools()
        .into_iter()
        .find(|(name, _)| name == "full.json")
        .map(|(_, tool)| tool)
        .expect("full.json is among the spec tools");
    let tool: Tool = serde_json::from_value(full.clone()).expect("reads");
    assert_eq!(tool.title(), Some("Document Search"));
    assert!(tool.output_schema().is_some());
    assert_eq!(tool.annotations().map(Map::len), Some(5));
    // BORROW: explicit .as_str() to compare the keys with literals
    let mut extra: Vec<&str> = tool.extra().keys().map(String::as_str).collect();
    extra.sort_unstable();
    assert_eq!(extra, ["_meta", "icons"]);
    assert_eq!(tool.extra().get("icons"), full.get("icons"));
}

/// Where the snapshot's `Tool` and the MCP `Tool` agree, where the snapshot accepts more (it records
/// what a server offered) and where it accepts less (an empty name).
///
/// Each row: the tool, whether MCP accepts it, whether the snapshot accepts it. The schema and
/// the Rust reader agree on every row.
#[test]
fn catalog_tool_verdicts_of_mcp_and_the_snapshot() {
    let object = json!({"type": "object"});
    let rows: Vec<(&str, Value, bool, bool)> = vec![
        (
            "required keys only",
            json!({"name": "t", "inputSchema": object}),
            true,
            true,
        ),
        (
            "empty name",
            json!({"name": "", "inputSchema": object}),
            true,
            false,
        ),
        (
            "an unknown key",
            json!({"name": "t", "inputSchema": object, "x-y": [1]}),
            true,
            true,
        ),
        (
            "an unknown key of null",
            json!({"name": "t", "inputSchema": object, "x": null}),
            true,
            true,
        ),
        (
            "an array output schema",
            json!({"name": "t", "inputSchema": object, "outputSchema": {"type": "array"}}),
            true,
            true,
        ),
        (
            "an annotation of an unknown key",
            json!({"name": "t", "inputSchema": object, "annotations": {"mine": null}}),
            true,
            true,
        ),
        ("no input schema", json!({"name": "t"}), false, false),
        ("no name", json!({"inputSchema": object}), false, false),
        (
            "a string input schema",
            json!({"name": "t", "inputSchema": "object"}),
            false,
            false,
        ),
        (
            "a numeric title",
            json!({"name": "t", "inputSchema": object, "title": 5}),
            false,
            false,
        ),
        (
            "a null description",
            json!({"name": "t", "inputSchema": object, "description": null}),
            false,
            false,
        ),
        (
            "a null output schema",
            json!({"name": "t", "inputSchema": object, "outputSchema": null}),
            false,
            false,
        ),
        (
            "a null annotations",
            json!({"name": "t", "inputSchema": object, "annotations": null}),
            false,
            false,
        ),
        (
            "an input schema without a type",
            json!({"name": "t", "inputSchema": {}}),
            false,
            true,
        ),
        (
            "an input schema of another type",
            json!({"name": "t", "inputSchema": {"type": "string"}}),
            false,
            true,
        ),
        (
            "a mistyped annotation hint",
            json!({"name": "t", "inputSchema": object, "annotations": {"readOnlyHint": "yes"}}),
            false,
            true,
        ),
        (
            "icons that are not an array",
            json!({"name": "t", "inputSchema": object, "icons": "x"}),
            false,
            true,
        ),
        (
            "an icon without a source",
            json!({"name": "t", "inputSchema": object, "icons": [{}]}),
            false,
            true,
        ),
        (
            "a meta that is not an object",
            json!({"name": "t", "inputSchema": object, "_meta": 5}),
            false,
            true,
        ),
    ];
    for (what, tool, mcp, ours) in rows {
        assert_eq!(MCP_TOOL.is_valid(&tool), mcp, "MCP, {what}");
        assert_eq!(OUR_TOOL.is_valid(&tool), ours, "the schema, {what}");
        assert_eq!(
            serde_json::from_value::<Tool>(tool.clone()).is_ok(),
            ours,
            "the reader, {what}"
        );
        let snapshot = wrap(slice::from_ref(&tool));
        assert_eq!(SNAPSHOT.is_valid(&snapshot), ours, "the snapshot, {what}");
        assert_eq!(
            serde_json::from_value::<CatalogSnapshot>(snapshot).is_ok(),
            ours,
            "the snapshot reader, {what}"
        );
    }
}

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        "\\PC{0,8}".prop_map(Value::String),
    ];
    leaf.prop_recursive(2, 12, 3, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..3).prop_map(Value::Array),
            prop::collection::btree_map("\\PC{0,5}", inner, 0..3)
                .prop_map(|map| Value::Object(map.into_iter().collect())),
        ]
    })
}

/// A tool of the MCP `Tool` definition with every optional key drawn independently.
fn arb_mcp_tool() -> impl Strategy<Value = Value> {
    let icon = (
        "https://example\\.com/[a-z]{1,8}\\.png",
        proptest::option::of("image/[a-z]{3,4}"),
        proptest::option::of(prop::collection::vec("[0-9]{2}x[0-9]{2}|any", 0..3)),
        proptest::option::of(prop_oneof![Just("dark"), Just("light")]),
    )
        .prop_map(|(src, mime, sizes, theme)| {
            let mut icon = json!({"src": src});
            if let Some(mime) = mime {
                icon["mimeType"] = json!(mime);
            }
            if let Some(sizes) = sizes {
                icon["sizes"] = json!(sizes);
            }
            if let Some(theme) = theme {
                icon["theme"] = json!(theme);
            }
            icon
        });
    let hints = (
        proptest::option::of(any::<bool>()),
        proptest::option::of(any::<bool>()),
        proptest::option::of(any::<bool>()),
        proptest::option::of(any::<bool>()),
        proptest::option::of("\\PC{0,8}"),
    )
        .prop_map(|(read_only, destructive, idempotent, open_world, title)| {
            let mut hints = Map::new();
            for (key, flag) in [
                ("readOnlyHint", read_only),
                ("destructiveHint", destructive),
                ("idempotentHint", idempotent),
                ("openWorldHint", open_world),
            ] {
                if let Some(flag) = flag {
                    hints.insert(String::from(key), json!(flag));
                }
            }
            if let Some(title) = title {
                hints.insert(String::from("title"), json!(title));
            }
            Value::Object(hints)
        });
    (
        "\\PC{1,8}",
        proptest::option::of("\\PC{0,8}"),
        proptest::option::of("\\PC{0,8}"),
        prop::collection::btree_map("\\PC{0,5}", arb_json(), 0..3),
        proptest::option::of(prop::collection::btree_map("\\PC{0,5}", arb_json(), 0..3)),
        proptest::option::of(hints),
        proptest::option::of(prop::collection::vec(icon, 0..3)),
        proptest::option::of(prop::collection::btree_map("\\PC{0,5}", arb_json(), 0..3)),
    )
        .prop_map(
            |(name, title, description, input, output, annotations, icons, meta)| {
                let mut input: Map<String, Value> = input.into_iter().collect();
                input.insert(String::from("type"), json!("object"));
                let mut tool = json!({"name": name, "inputSchema": input});
                if let Some(title) = title {
                    tool["title"] = json!(title);
                }
                if let Some(description) = description {
                    tool["description"] = json!(description);
                }
                if let Some(output) = output {
                    tool["outputSchema"] = Value::Object(output.into_iter().collect());
                }
                if let Some(annotations) = annotations {
                    tool["annotations"] = annotations;
                }
                if let Some(icons) = icons {
                    tool["icons"] = json!(icons);
                }
                if let Some(meta) = meta {
                    tool["_meta"] = Value::Object(meta.into_iter().collect());
                }
                tool
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// The snapshot narrows MCP only by requiring a non-empty name: any other tool the MCP `Tool`
    /// definition accepts is accepted by the schema and the reader and is written back unchanged.
    #[test]
    fn catalog_accepts_every_tool_mcp_accepts(tool in arb_mcp_tool()) {
        prop_assert!(MCP_TOOL.is_valid(&tool), "the generator makes MCP tools: {tool:#}");
        let snapshot = wrap(slice::from_ref(&tool));
        prop_assert!(SNAPSHOT.is_valid(&snapshot), "{snapshot:#}");
        let read: CatalogSnapshot = serde_json::from_str(&serde_json::to_string(&snapshot)?)?;
        prop_assert_eq!(serde_json::to_value(&read)?, snapshot);
    }
}

// ---- real catalogs --------------------------------------------------------------------------

/// The text of a captured catalog.
fn catalog_text(name: &str) -> String {
    let path = manifest_path(&["tests", "fixtures", "upstream", name]);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A captured catalog as JSON.
fn catalog(name: &str) -> Value {
    serde_json::from_str(&catalog_text(name)).expect("a captured catalog is JSON")
}

/// Rewrites a captured catalog to the shape of `SCHEMA_TAG`: sets the tag and moves
/// `servers_dir` and `stubbed_modules` under `ext`.
fn rewrite(mut snapshot: Value) -> Value {
    let object = snapshot.as_object_mut().expect("a snapshot is an object");
    assert_eq!(
        object.get("schema"),
        Some(&json!("tool-catalog-snapshot/0")),
        "the catalog carries the `/0` tag"
    );
    object.insert(String::from("schema"), json!(SCHEMA_TAG));
    let mut ext = Map::new();
    for key in ["servers_dir", "stubbed_modules"] {
        let value = object
            .remove(key)
            .unwrap_or_else(|| panic!("the catalog has {key}"));
        ext.insert(String::from(key), value);
    }
    object.insert(String::from("ext"), Value::Object(ext));
    snapshot
}

fn tool_counts(snapshot: &Value) -> Vec<usize> {
    snapshot["servers"]
        .as_array()
        .expect("servers is an array")
        .iter()
        .map(|server| server["tools"].as_array().map_or(0, Vec::len))
        .collect()
}

fn assert_catalog_round_trips(file: &str, counts: &[usize]) {
    let captured = catalog(file);
    let rewritten = rewrite(captured.clone());
    assert_eq!(tool_counts(&rewritten), counts, "{file}");

    assert!(SNAPSHOT.is_valid(&rewritten), "the schema rejects {file}");
    let snapshot: CatalogSnapshot =
        serde_json::from_value(rewritten.clone()).unwrap_or_else(|e| panic!("{file}: {e}"));
    assert_eq!(
        serde_json::to_value(&snapshot).expect("writes"),
        rewritten,
        "{file} does not round-trip"
    );
    // The same snapshot read from text, pretty-printed and compact.
    for text in [
        serde_json::to_string_pretty(&rewritten).expect("writes"),
        serde_json::to_string(&rewritten).expect("writes"),
    ] {
        let from_text: CatalogSnapshot =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(from_text, snapshot, "{file} reads differently from text");
    }
    assert_eq!(snapshot.servers().len(), counts.len());
    let read_counts: Vec<usize> = snapshot.servers().iter().map(|s| s.tools().len()).collect();
    assert_eq!(read_counts, counts);
    let ext = snapshot.ext().expect("the rewrite writes ext");
    assert_eq!(ext.get("servers_dir"), captured.get("servers_dir"));
    assert_eq!(ext.get("stubbed_modules"), captured.get("stubbed_modules"));

    // The catalog as captured carries the `/0` tag and top-level keys the schema does not allow.
    assert!(
        !SNAPSHOT.is_valid(&captured),
        "the schema accepts the `/0` catalog {file}"
    );
    assert!(
        serde_json::from_value::<CatalogSnapshot>(captured.clone()).is_err(),
        "Rust accepts the `/0` catalog {file}"
    );
    assert!(serde_json::from_str::<CatalogSnapshot>(&catalog_text(file)).is_err());

    // The tag and the moved keys are each checked on their own.
    let mut tag_only = captured;
    tag_only["schema"] = json!(SCHEMA_TAG);
    assert!(
        !SNAPSHOT.is_valid(&tag_only),
        "the schema accepts the top-level keys of {file}"
    );
    let mut moved_only = rewritten;
    moved_only["schema"] = json!("tool-catalog-snapshot/0");
    assert!(!SNAPSHOT.is_valid(&moved_only));
    assert!(serde_json::from_value::<CatalogSnapshot>(moved_only).is_err());
}

#[test]
fn catalog_rikyu_snapshot_validates_and_round_trips() {
    assert_catalog_round_trips("rikyu_mcp.json", &[26, 3]);
    let total: usize = tool_counts(&catalog("rikyu_mcp.json")).iter().sum();
    assert_eq!(total, 29);
}

#[test]
fn catalog_bio_opentrons_snapshot_validates_and_round_trips() {
    assert_catalog_round_trips("bio_opentrons.json", &[1, 10, 19, 6]);
}

/// The reader ignores the top-level keys it does not know: `servers_dir` and `stubbed_modules`
/// under the tag `SCHEMA_TAG` are read and not written back, and only the schema refuses them.
#[test]
fn catalog_unknown_top_level_keys_are_dropped_by_the_reader() {
    let mut tag_only = catalog("bio_opentrons.json");
    tag_only["schema"] = json!(SCHEMA_TAG);
    assert!(!SNAPSHOT.is_valid(&tag_only));
    let snapshot: CatalogSnapshot = serde_json::from_value(tag_only).expect("reads");
    assert!(snapshot.ext().is_none());
    let written = serde_json::to_value(&snapshot).expect("writes");
    assert!(written.get("servers_dir").is_none());
    assert!(written.get("stubbed_modules").is_none());
}
