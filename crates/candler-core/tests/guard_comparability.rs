#![cfg(test)]
//! The stratum verdict and the nuisance set. The verdict rows are data, written by hand from the
//! R01 sentence each cites (`fixtures/comparability/verdicts.json`); no row computes its
//! expectation from `compare`. Nothing here spawns a process or a container.

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

use candler_core::manifest::compare::{
    ArmView, NUISANCE_PATHS, Verdict, VerdictError, stratum_verdict,
};
use candler_schemas::execution::RunManifest;
use candler_schemas::primitives::FieldPath;
use serde::Deserialize;
use serde_json::{Map, Value, json};

/// One row of `verdicts.json`.
#[derive(Deserialize)]
struct Row {
    id: String,
    source: String,
    declared: Vec<String>,
    arms: Vec<Value>,
    expected: Value,
    #[serde(default)]
    malformed_because: Option<String>,
}

/// The rows of the table, from the data file.
fn rows() -> Vec<Row> {
    #[derive(Deserialize)]
    struct Table {
        rows: Vec<Row>,
    }
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/comparability/verdicts.json");
    let text = std::fs::read_to_string(&path).expect("the verdict table is readable");
    // BORROW: explicit conversion makes the owned or borrowed form the next line needs
    serde_json::from_str::<Table>(text.as_str())
        .expect("the verdict table parses")
        .rows
}

/// A digest text of sixty-four copies of `c`.
fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

/// The manifest document of one arm of a row: its literal `comparable`, a source for every value
/// and typed root the cell holds, the measured tool list when the arm records one, and the
/// manifest fields outside the hash the arm sets.
fn manifest_doc(arm: &Value) -> Value {
    // BORROW: explicit conversion makes the owned or borrowed form the next line needs
    let get = |key: &str| arm.get(key).cloned();
    let mut comparable = get("comparable").expect("an arm holds a comparable");
    if get("unsorted_values") == Some(Value::Bool(true)) {
        let values = comparable
            .get_mut("values")
            .and_then(Value::as_array_mut)
            .expect("values");
        values.swap(0, 1);
    }
    if get("dup_snapshots") == Some(Value::Bool(true)) {
        let list = comparable
            .get_mut("snapshots")
            .and_then(Value::as_array_mut)
            .expect("snapshots");
        // BORROW: explicit conversion makes the owned or borrowed form the next line needs
        let again = list.first().cloned().expect("a snapshot");
        list.push(again);
    }
    let mut paths: Vec<String> = comparable
        .get("values")
        .and_then(Value::as_array)
        .expect("values")
        .iter()
        .map(|v| {
            v.get("path")
                .and_then(Value::as_str)
                .expect("path")
                // BORROW: explicit conversion makes the owned or borrowed form the next line needs
                .to_owned()
        })
        .collect();
    // BORROW: explicit conversion makes the owned or borrowed form the next line needs
    paths.extend(["harness", "treatment"].map(str::to_owned));
    for list in ["snapshots", "inputs", "environments"] {
        let held = comparable.get(list).and_then(Value::as_array);
        if held.is_some_and(|items| !items.is_empty()) {
            // BORROW: explicit conversion makes the owned or borrowed form the next line needs
            paths.push(list.to_owned());
        }
    }
    let outside = get("outside").unwrap_or_else(|| json!({}));
    let source_of = |path: &str| {
        let own = outside
            .get("extra_sources")
            .and_then(Value::as_array)
            .and_then(|list| {
                list.iter()
                    .find(|e| e.get("path").and_then(Value::as_str) == Some(path))
            });
        match own {
            // BORROW: explicit conversion makes the owned or borrowed form the next line needs
            Some(entry) => entry.get("source").cloned().expect("source"),
            None => json!({"kind": "experiment_file"}),
        }
    };
    let sources: Vec<Value> = paths
        .iter()
        // BORROW: explicit conversion makes the owned or borrowed form the next line needs
        .map(|p| json!({"path": p, "source": source_of(p.as_str())}))
        .collect();
    let mut measured: Vec<Value> = Vec::new();
    if let Some(Value::Array(tools)) = get("init_tools") {
        measured.push(json!({"path": "census.init_tools", "value": tools, "source": {"kind": "experiment_file"}}));
    }
    if let Some(Value::Array(more)) = outside.get("measured") {
        // BORROW: explicit conversion makes the owned or borrowed form the next line needs
        measured.extend(more.iter().cloned());
    }
    let int = |key: &str, default: u64| outside.get(key).and_then(Value::as_u64).unwrap_or(default);
    let mut doc = Map::new();
    doc.insert("schema".into(), json!("run-manifest/1"));
    doc.insert(
        "run".into(),
        json!({"cell": digest_text('f'), "replicate": int("run_replicate", 1), "attempt": int("run_attempt", 1)}),
    );
    doc.insert(
        "experiment".into(),
        outside
            .get("experiment")
            // BORROW: explicit conversion makes the owned or borrowed form the next line needs
            .cloned()
            .unwrap_or_else(|| json!(digest_text('0'))),
    );
    doc.insert(
        "factors".into(),
        get("claimed").unwrap_or_else(|| json!([])),
    );
    doc.insert("comparable".into(), comparable);
    doc.insert("sources".into(), Value::Array(sources));
    doc.insert(
        "secrets".into(),
        // BORROW: explicit conversion makes the owned or borrowed form the next line needs
        outside.get("secrets").cloned().unwrap_or_else(|| json!([])),
    );
    doc.insert("measured".into(), Value::Array(measured));
    doc.insert(
        "deviations".into(),
        outside
            .get("deviations")
            // BORROW: explicit conversion makes the owned or borrowed form the next line needs
            .cloned()
            .unwrap_or_else(|| json!([])),
    );
    doc.insert("integrity".into(), json!({"kind": "valid"}));
    for key in ["host", "started_at", "ended_at", "job"] {
        if let Some(value) = outside.get(key) {
            // BORROW: explicit conversion makes the owned or borrowed form the next line needs
            doc.insert(key.to_owned(), value.clone());
        }
    }
    Value::Object(doc)
}

/// What reading one arm gives: its view, or the reason it is malformed. A document the manifest
/// reader refuses is malformed, with the reader's own words.
fn read_arm(arm: &Value) -> Result<ArmView, VerdictError> {
    match serde_json::from_value::<RunManifest>(manifest_doc(arm)) {
        Ok(manifest) => ArmView::from_manifest(&manifest),
        Err(error) => Err(VerdictError::Malformed {
            problem: "the manifest reader refused the arm",
            path: error.to_string(),
        }),
    }
}

/// Guards: I7 — arms are compared only when they differ in declared factors alone: every other
/// path is refused and named, by whole path segments, whatever the order or the number of arms; an
/// unreadable arm gives no verdict.
/// Oracle: the R01 sentence each row of `verdicts.json` cites (C01 to C23 the lead's, C24 on
/// added); the expected verdict is a literal in the row.
/// Bite: compare paths with `str::starts_with` instead of whole segments (`covers`).
#[test]
fn stratum_verdict_table() {
    let rows = rows();
    // BORROW: explicit conversion makes the owned or borrowed form the next line needs
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let lead: Vec<String> = (1..=23).map(|n| format!("C{n:02}")).collect();
    assert_eq!(
        ids.get(..23).map(<[&str]>::to_vec),
        // BORROW: explicit conversion makes the owned or borrowed form the next line needs
        Some(lead.iter().map(String::as_str).collect::<Vec<&str>>()),
        "the lead's rows C01 to C23 open the table, in order"
    );
    let mut failures = Vec::new();
    for row in &rows {
        let declared: Vec<FieldPath> = row
            .declared
            .iter()
            // BORROW: explicit conversion makes the owned or borrowed form the next line needs
            .map(|p| FieldPath::from_str(p.as_str()).expect("a declared path"))
            .collect();
        let arms: Vec<Result<ArmView, VerdictError>> = row.arms.iter().map(read_arm).collect();
        // BORROW: explicit conversion makes the owned or borrowed form the next line needs
        let got = stratum_verdict(arms.as_slice(), declared.as_slice());
        let ok = match (&row.expected, &got) {
            (Value::String(word), Verdict::Comparable) => word == "comparable",
            (Value::String(word), Verdict::Malformed(errors)) => {
                word == "malformed"
                    && row.malformed_because.as_ref().is_none_or(|needle| {
                        errors
                            .iter()
                            // BORROW: explicit conversion makes the owned or borrowed form the next line needs
                            .any(|e| e.to_string().contains(needle.as_str()))
                    })
            }
            (Value::Object(map), Verdict::Refused(paths)) => {
                let want: Vec<&str> = map
                    .get("refused")
                    .and_then(Value::as_array)
                    .expect("refused paths")
                    .iter()
                    // BORROW: explicit conversion makes the owned or borrowed form the next line needs
                    .map(|p| p.as_str().expect("a path"))
                    .collect();
                // BORROW: explicit .as_str() instead of Deref coercion
                paths.iter().map(FieldPath::as_str).collect::<Vec<&str>>() == want
            }
            _ => false,
        };
        if !ok {
            failures.push(format!(
                "{} ({}): expected {}, got {got:?}",
                row.id, row.source, row.expected
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "rows that disagree:\n{}",
        failures.join("\n")
    );
}

/// Guards: I7 — the crate's nuisance set is the one written by hand in the schemas crate, so no
/// value can hide as nuisance: it equals the `nuisance` entries of `hashed_paths.toml` as sets, no
/// entry of either is a `comparable` path, and every `hashed` entry is one.
/// Oracle: `crates/candler-schemas/tests/vectors/manifest/hashed_paths.toml`, written by hand from
/// R01 §Run manifest.
/// Bite: add `comparable.inputs` to `NUISANCE_PATHS`.
#[test]
fn nuisance_set_is_the_hand_written_one() {
    #[derive(Deserialize)]
    struct Entry {
        name: String,
        class: String,
    }
    #[derive(Deserialize)]
    struct Table {
        path: Vec<Entry>,
    }
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../candler-schemas/tests/vectors/manifest/hashed_paths.toml");
    let text = std::fs::read_to_string(&file).expect("the hashed-paths table is readable");
    // BORROW: explicit conversion makes the owned or borrowed form the next line needs
    let table: Table = toml::from_str(text.as_str()).expect("the table parses");
    let nuisance: BTreeSet<&str> = table
        .path
        .iter()
        .filter(|e| e.class == "nuisance")
        // BORROW: explicit conversion makes the owned or borrowed form the next line needs
        .map(|e| e.name.as_str())
        .collect();
    let ours: BTreeSet<&str> = NUISANCE_PATHS.iter().copied().collect();
    assert_eq!(ours, nuisance, "the crate's set is the hand-written one");
    assert_eq!(
        NUISANCE_PATHS.len(),
        ours.len(),
        "the crate's set holds no entry twice"
    );
    // BORROW: explicit conversion makes the owned or borrowed form the next line needs
    let root = |name: &str| name.split('.').next().unwrap_or_default().to_owned();
    assert!(
        ours.iter().all(|name| root(name) != "comparable"),
        "no `comparable.*` path is nuisance"
    );
    assert!(
        table
            .path
            .iter()
            .filter(|e| e.class == "hashed")
            // BORROW: explicit conversion makes the owned or borrowed form the next line needs
            .all(|e| root(e.name.as_str()) == "comparable"),
        "every hashed path is a `comparable.*` path"
    );
}
