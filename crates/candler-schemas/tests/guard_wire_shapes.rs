#![cfg(test)]
//! The tagged enums of the wire are read from objects and from nothing else.

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use candler_schemas::catalog::{CatalogSnapshot, SCHEMA_TAG};
use candler_schemas::primitives::NameToken;
use candler_schemas::process::{ExitStatus, ProcessStart};
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
