#![cfg(test)]
//! The tagged enums of the wire are read from objects and from nothing else.

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use candler_schemas::catalog::{CatalogSnapshot, SCHEMA_TAG};
use candler_schemas::evaluation::PluginManifest;
use candler_schemas::primitives::NameToken;
use candler_schemas::process::{ExitStatus, ProcessStart};
use candler_schemas::refusal::Refusal;
use candler_schemas::values::ValueSource;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

fn read<T: DeserializeOwned>(text: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(text)
}

/// Guards: I12, I15 — a value source, a process start time and an exit status are written as
/// objects, so a list that happens to start with a kind name does not read as a record of that
/// kind. Oracle: the hand-written list and object texts below, each object being the positive
/// control of its list.
#[test]
fn tagged_enums_refuse_the_list_form_and_accept_the_object_form() {
    assert_eq!(
        read::<ExitStatus>(r#"{"kind":"code","code":3}"#).expect("object form"),
        ExitStatus::Code { code: 3 }
    );
    assert!(
        read::<ExitStatus>(r#"["code",3]"#).is_err(),
        "exit status as a list"
    );
    assert!(
        read::<ExitStatus>(r#"["signal",9]"#).is_err(),
        "signal as a list"
    );

    assert_eq!(
        read::<ProcessStart>(r#"{"kind":"unavailable"}"#).expect("object form"),
        ProcessStart::Unavailable
    );
    assert!(
        read::<ProcessStart>(r#"["unavailable"]"#).is_err(),
        "unavailable start as a list"
    );
    assert!(
        read::<ProcessStart>(r#"["darwin",1,2]"#).is_err(),
        "darwin start as a list"
    );
    assert!(
        read::<ProcessStart>(r#"["linux","3f2a9c1e-5b7d-4e08-9a6c-0123456789ab",7]"#).is_err(),
        "linux start as a list"
    );

    assert_eq!(
        read::<ValueSource>(r#"{"kind":"core_default"}"#).expect("object form"),
        ValueSource::CoreDefault
    );
    assert!(
        read::<ValueSource>(r#"["core_default"]"#).is_err(),
        "core default as a list"
    );
    assert!(
        read::<ValueSource>(r#"["derived","probe@1"]"#).is_err(),
        "derived source as a list"
    );
}

fn fixture_path(parts: &[&str]) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "fixtures"]);
    path.extend(parts);
    path
}

fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    files
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Every valid catalog snapshot the fixtures hold: the two captured catalogs, rewritten to the
/// current tag with their two extra top-level keys under `ext`, and one snapshot around each
/// MCP specification tool.
fn catalog_snapshot_fixtures() -> Vec<(String, Value)> {
    let mut snapshots = Vec::new();
    for path in json_files(&fixture_path(&["upstream"])) {
        let mut snapshot = read_json(&path);
        let object = snapshot.as_object_mut().expect("a catalog is an object");
        object.insert(String::from("schema"), json!(SCHEMA_TAG));
        let mut ext = Map::new();
        for key in ["servers_dir", "stubbed_modules"] {
            ext.insert(
                String::from(key),
                object.remove(key).expect("a captured key"),
            );
        }
        object.insert(String::from("ext"), Value::Object(ext));
        snapshots.push((path.display().to_string(), snapshot));
    }
    for path in json_files(&fixture_path(&["spec", "tools"])) {
        let snapshot = json!({
            "schema": SCHEMA_TAG,
            "captured_at": "2026-10-28T01:09:04+00:00",
            "provider": "capture@1",
            "servers": [{
                "name": "server",
                "source_path": "servers/server.py",
                "source_digest": format!("sha256:{}", "cd".repeat(32)),
                "transport": "stdio",
                "tools": [read_json(&path)],
            }],
        });
        snapshots.push((path.display().to_string(), snapshot));
    }
    snapshots
}

/// JSON pointers to every struct of the catalog wire in `snapshot`: the snapshot, each server
/// and each tool.
fn catalog_struct_pointers(snapshot: &Value) -> Vec<String> {
    let mut pointers = vec![String::new()];
    let servers = snapshot["servers"].as_array().expect("servers is a list");
    for (s, server) in servers.iter().enumerate() {
        pointers.push(format!("/servers/{s}"));
        let tools = server["tools"].as_array().expect("tools is a list");
        for t in 0..tools.len() {
            pointers.push(format!("/servers/{s}/tools/{t}"));
        }
    }
    pointers
}

/// Guards: R01 §Wire encodings, for the catalog wire only (snapshot, server, tool) — a struct is
/// never written as a list of its values. Oracle: every committed valid catalog snapshot reads;
/// with any one of its snapshot, server or tool objects replaced by the list of that object's
/// values, the reader refuses it; the counts of fixtures and of objects tried are above zero.
/// Bite: removing `object_only!` from `CatalogServer` or `MapOnly` from the snapshot reader turns
/// this test red, because the lists are written in the order of the struct's fields.
#[test]
fn catalog_wire_structs_are_refused_as_lists_in_every_fixture() {
    let fixtures = catalog_snapshot_fixtures();
    assert!(!fixtures.is_empty(), "no catalog fixture was found");
    let mut tried = 0_usize;
    for (name, snapshot) in &fixtures {
        serde_json::from_value::<CatalogSnapshot>(snapshot.clone())
            .unwrap_or_else(|e| panic!("{name} is a valid snapshot: {e}"));
        for pointer in catalog_struct_pointers(snapshot) {
            let mut mutated = snapshot.clone();
            let node = mutated
                .pointer_mut(&pointer)
                .expect("the pointer leads to an object");
            // A derived reader that took a list would take the values in the order of the
            // struct's fields, an absent optional field being `{}`; a tool has a flattened
            // remainder, so any order is a list of its values.
            let fields: &[&str] = match pointer.matches('/').count() {
                0 => &[
                    "schema",
                    "captured_at",
                    "provider",
                    "environment",
                    "ext",
                    "servers",
                ],
                2 => &["name", "source_path", "source_digest", "transport", "tools"],
                // EXPLICIT: any other depth is a tool, whose flattened extra keys give it no fixed
                // field order
                _ => &[],
            };
            let list: Vec<Value> = if fields.is_empty() {
                node.as_object()
                    .expect("an object")
                    .values()
                    .cloned()
                    .collect()
            } else {
                fields
                    .iter()
                    .map(|field| node.get(*field).cloned().unwrap_or_else(|| json!({})))
                    .collect()
            };
            *node = Value::Array(list);
            assert!(
                serde_json::from_value::<CatalogSnapshot>(mutated).is_err(),
                "{name}: the object at '{pointer}' written as a list was read"
            );
            tried += 1;
        }
    }
    assert!(tried > 0, "no object was tried");
}

/// Guards: Run manifest — a derived source may record the location its step read; without one it writes no `from`, and an explicit `null` is refused. Oracle: the three wire texts written by hand.
#[test]
fn derived_source_carries_an_optional_location() {
    let step = || NameToken::from_str("probe@1").expect("a step");
    let with = read::<ValueSource>(r#"{"kind":"derived","step":"probe@1","from":"../skills"}"#)
        .expect("a derived source with a location");
    assert_eq!(
        with,
        ValueSource::derived_from(step(), "../skills".to_owned())
    );
    assert_eq!(
        serde_json::to_string(&with).expect("JSON"),
        r#"{"kind":"derived","step":"probe@1","from":"../skills"}"#
    );
    let without = ValueSource::derived(step());
    assert_eq!(
        serde_json::to_string(&without).expect("JSON"),
        r#"{"kind":"derived","step":"probe@1"}"#
    );
    assert!(
        read::<ValueSource>(r#"{"kind":"derived","step":"probe@1","from":null}"#).is_err(),
        "a null location"
    );
}

const MANIFEST_HEAD: &str = r#"schema = "plugin-manifest/1"
name = "rubric"
version = "0.4.2"
command = ["python", "run.py"]
protocol = { major = 1, minor = 0 }
environment = { kind = "none" }
"#;

fn manifest_with(
    tail: &str,
) -> Result<PluginManifest, candler_schemas::evaluation::EvaluationError> {
    PluginManifest::from_toml_str(format!("{MANIFEST_HEAD}{tail}").as_str())
}

/// Guards: R01 §Plug-in manifest — a manifest written before agent environments existed still reads and writes no `agent_environments`; with the map set, each environment keeps its values and its agent-only inputs. Oracle: the manifest texts written by hand, and the manifests the core's committed fixtures hold.
/// Bite: `agent_environments` made a required field, or written as `{}` when empty.
#[test]
fn agent_environments_are_optional_and_read_when_set() {
    let bare = manifest_with("").expect("a manifest without the map");
    assert!(bare.agent_environments().is_empty());
    let written = serde_json::to_value(&bare).expect("JSON");
    assert!(
        written.get("agent_environments").is_none(),
        "an empty map is omitted: {written}"
    );

    for fixture in [
        "engine/plugin.toml",
        "engine/plugin_bare.toml",
        "engine/plugin_pool.toml",
        "engine/plugin_second.toml",
        "registry/plugin/plugin.toml",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../candler-core/tests/fixtures")
            .join(fixture);
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let manifest = PluginManifest::from_toml_str(text.as_str())
            .unwrap_or_else(|e| panic!("{fixture} is refused: {e}"));
        assert!(manifest.agent_environments().is_empty(), "{fixture}");
    }

    let set = manifest_with(
        r#"
[agent_environments.web.values]
"isolation.egress" = ["pypi.org:443"]
"isolation.build.dockerfile" = "docker/Dockerfile"
"mcp.servers.tools.command" = "tools"

[[agent_environments.web.inputs]]
kind = "tool_source"
path = "tools"
at = "tools"
"#,
    )
    .expect("a manifest with an environment");
    let wire = serde_json::to_value(&set).expect("JSON");
    assert_eq!(
        wire["agent_environments"],
        json!({"web": {
            "values": {
                "isolation.egress": ["pypi.org:443"],
                "isolation.build.dockerfile": "docker/Dockerfile",
                "mcp.servers.tools.command": "tools",
            },
            "inputs": [{"kind": "tool_source", "path": "tools", "at": "tools"}],
        }})
    );
    let back: PluginManifest = serde_json::from_value(wire).expect("the written form reads");
    assert_eq!(back, set);
}

/// Guards: R01 §Plug-in manifest — an environment is a closed table, an agent input carries no audience (it is agent-only by construction), and an input obeys the experiment's rules for `at` and the reserved kind. Oracle: the table written by hand.
/// Bite: `deny_unknown_fields` removed from the environment or from the input.
#[test]
fn agent_environments_refuse_unknown_keys_and_audiences() {
    for (name, tail) in [
        (
            "an unknown key in an environment",
            "\n[agent_environments.web]\nvalues = {}\ninputs = []\nextra = 1\n",
        ),
        (
            "an audience on an input",
            "\n[agent_environments.web]\nvalues = {}\n[[agent_environments.web.inputs]]\nkind = \"k\"\npath = \"p\"\naudience = \"evaluators\"\n",
        ),
        (
            "an input mounted outside the workspace",
            "\n[agent_environments.web]\nvalues = {}\n[[agent_environments.web.inputs]]\nkind = \"k\"\npath = \"p\"\nat = \"../x\"\n",
        ),
        (
            "an input of the reserved kind",
            "\n[agent_environments.web]\nvalues = {}\n[[agent_environments.web.inputs]]\nkind = \"build_inputs\"\npath = \"p\"\n",
        ),
    ] {
        assert!(manifest_with(tail).is_err(), "{name} is accepted");
    }
    let unknown =
        manifest_with("\n[agent_environments.web]\nvalues = {}\ninputs = []\nextra = 1\n")
            .expect_err("unknown key");
    assert_eq!(unknown.code().as_str(), "plugin.unknown_field");
}

/// Guards: R01 §Plug-in manifest — an environment may set only `isolation.build.*`, `isolation.egress` and `mcp.*`; any other path is refused at read with `plugin.environment_path_unsupported`, naming the path. Oracle: the allowed set as R06 D4 writes it (prefixes `isolation.build.` and `mcp.`, path `isolation.egress`), worked as a table; segments, not text prefixes, decide (`mcp` alone, `mcpx.a` and `isolation.egressx` are outside).
/// Bite: the allowed set widened to a text prefix (`mcp`), or the check removed.
#[test]
fn agent_environments_refuse_a_value_path_outside_the_allowed_set() {
    let table = |path: &str| format!("\n[agent_environments.web.values]\n\"{path}\" = \"x\"\n");
    for path in [
        "isolation.build.dockerfile",
        "isolation.build.requirements",
        "isolation.build.plugin",
        "isolation.egress",
        "mcp.servers.a.command",
        "mcp.servers",
    ] {
        manifest_with(table(path).as_str())
            .unwrap_or_else(|error| panic!("`{path}` is refused: {error}"));
    }
    for path in [
        "model.id",
        "isolation.network",
        "isolation.image",
        "isolation.limits.pids",
        "isolation.build",
        "isolation.egressx",
        "isolation.egress.extra",
        "mcp",
        "mcpx.a",
        "tools.allowed",
        "budget.per_run.turns",
        "request.inputs",
    ] {
        let error =
            manifest_with(table(path).as_str()).expect_err(&format!("`{path}` is accepted"));
        assert_eq!(
            error.code().as_str(),
            "plugin.environment_path_unsupported",
            "`{path}`"
        );
        assert!(
            error.to_string().contains(path),
            "the message names `{path}`: {error}"
        );
    }
}
