#![cfg(test)]
//! The stratum verdict and the nuisance set. The verdict rows are data, written by hand from the
//! R01 sentence each cites (`fixtures/comparability/verdicts.json`); no row computes its
//! expectation from `compare`. Nothing here spawns a process or a container.

#[path = "support/legacy_import.rs"]
mod legacy_import;
#[path = "support/legacy_rules.rs"]
mod legacy_rules;
#[path = "oracle/mod.rs"]
mod oracle;

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

use candler_core::import::assemble::import;
use candler_core::manifest::compare::{
    ArmView, NUISANCE_PATHS, Verdict, VerdictError, stratum_verdict,
};
use candler_core::store::Store;
use candler_schemas::execution::RunManifest;
use candler_schemas::primitives::Digest;
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
    // A live arm carries a clean provenance block unless its row overrides parts of it. An
    // imported arm carries none: the importer records the legacy build in `comparable.values`.
    let mut block: Vec<(String, Value)> = if get("import").is_some() {
        Vec::new()
    } else {
        [
            ("eval_infra.tag", json!("v0.1.0")),
            ("eval_infra.commits_ahead", json!(0)),
            ("eval_infra.commit", json!("abc1234")),
            ("eval_infra.dirty", json!(false)),
            ("sandbox_contract", json!("candler-container@1")),
        ]
        // BORROW: explicit conversion makes the owned or borrowed form the next line needs
        .map(|(key, value)| (key.to_owned(), value))
        .into()
    };
    if let Some(Value::Object(overrides)) = get("provenance") {
        for (key, value) in overrides {
            block.retain(|(held, _)| *held != key);
            if !value.is_null() {
                block.push((key, value));
            }
        }
    }
    for (key, value) in block {
        measured.push(json!({"path": format!("provenance.{key}"), "value": value, "source": {"kind": "experiment_file"}}));
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
    if let Some(import) = get("import") {
        let mut record = json!({
            "format": "legacy@1",
            "base_commit": "0e84a75000000000000000000000000000000000",
            "legacy_integrity": {"kind": "valid"},
            "sources": [],
            "requests": [],
        });
        if let Some(rules) = import.get("rules").filter(|rules| !rules.is_null()) {
            // BORROW: explicit conversion makes the owned or borrowed form the next line needs
            record["rules"] = rules.clone();
        }
        doc.insert("import".into(), record);
    }
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
/// unreadable arm gives no verdict. The provenance rows (C45 to C59) gate on the build's identity
/// alone: `eval_infra.commit` recorded and equal, `eval_infra.dirty` false everywhere,
/// `sandbox_contract` equal; `eval_infra.tag`, `commits_ahead` and `packages.*` gate nothing, and
/// an unrecorded commit or dirty flag is refused and named, never counted as equal. The
/// `imported_` rows (an arm with an `import` key is imported; its legacy build sits in
/// `comparable.values`, as the importer writes it) gate an imported stratum on `import.rules`
/// recorded and equal and on the legacy commit, dirty flag and contract recorded; a legacy dirty
/// build pools with arms of the same commit, flag, contract and rules; a stratum mixing imported
/// and live arms is refused naming `import`; the live rows follow D8.
/// Oracle: the R01 sentence each row of `verdicts.json` cites (C01 to C23 the lead's, C24 on
/// added); the expected verdict is a literal in the row, written from decision D8, and for the
/// `imported_` rows from decision D7.
/// Bite: compare paths with `str::starts_with` instead of whole segments (`covers`); gate every
/// `provenance.eval_infra.*` path (the tag rows); count an unrecorded commit as equal (the commit
/// rows); apply the imported rule to live arms too (the live rows); pool a dirty legacy build
/// across commits (`imported_commit_differs_refused`).
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
    // BORROW: explicit conversion makes the owned or borrowed form the next line needs
    let imported: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| id.starts_with("imported_"))
        .collect();
    assert!(
        imported.len() >= 25,
        "the D7 rows are in the table, one or more per class: {imported:?}"
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

/// The factor the runs of the mini-corpus' two real cells differ in: the version of the skill, with
/// the value paths and typed roots that carry it.
const SKILL_FACTORS: [&str; 5] = [
    "inputs",
    "plan.skill_version",
    "provenance.skill_version",
    "resolved_skill_version",
    "treatment",
];

/// Imports the mini-corpus into a fresh store under rules whose file digested to `rules` and
/// reads three runs of it as the arms of one stratum: runs 6 and 17 of the v0.2.3 cell and run
/// 12 of the v0.2.6 cell. Every run is asserted sealed first.
fn mini_arms(rules: &str) -> Vec<Result<ArmView, VerdictError>> {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(dir.path()).expect("the store initialises");
    let digest =
        Digest::from_str(format!("sha256:{}", rules.repeat(64)).as_str()).expect("a sha256 digest");
    let rules = legacy_rules::builder("resolved_skill_version")
        .entry_params(legacy_rules::entry_params(&["scale", "text", "tier"]))
        .source_digest(digest)
        .build()
        .expect("the rules of the mini-corpus build");
    import(
        &legacy_import::mini(),
        &rules,
        &legacy_import::reader,
        &store,
    )
    .expect("the mini-corpus imports");
    let entries = store.runs().expect("the runs list");
    assert!(
        entries.iter().all(|entry| entry.done),
        "every imported run is sealed: {entries:?}"
    );
    [6, 17, 12]
        .into_iter()
        .map(|replicate| {
            let key = entries
                .iter()
                .find(|entry| entry.key.replicate() == replicate)
                .unwrap_or_else(|| panic!("no run {replicate} in the store"))
                .key
                .clone(); // BORROW: .clone() gives the lookup its own key
            let manifest = store.read_manifest(&key).expect("the manifest reads");
            ArmView::from_manifest(&manifest)
        })
        .collect()
}

/// The declared factors of the mini-corpus' stratum.
fn skill_factors() -> Vec<FieldPath> {
    SKILL_FACTORS
        .iter()
        .map(|path| FieldPath::from_str(path).expect("a declared path"))
        .collect()
}

/// Guards: I7 and R01 §Run manifest (imported strata, D7) — three runs of the mini-corpus, read
/// by the real importer under one import-rules digest, differ only in the skill version of their
/// two cells; with that factor declared the stratum is comparable, though the legacy build they
/// record is dirty.
/// Oracle: the mini-corpus' two `experiment.json` files, diffed by hand with `jq -S`: they differ
/// in `plan.out_dir` (which the importer files under `import`), `plan.skill_version`,
/// `provenance.skill_version` and `resolved_skill_version`; both record `eval_infra` commit
/// `0e84a75`, `dirty: true` and sandbox contract `v5`. The skill snapshot and the treatment are
/// the typed fields that version travels in.
/// Bite: read the measured provenance only (the importer records none there), so the stratum is
/// refused for its unrecorded commit.
#[test]
fn imported_stratum_of_the_mini_corpus_is_comparable() {
    let arms = mini_arms("1");
    assert!(
        arms.iter()
            .all(|arm| arm.as_ref().is_ok_and(ArmView::is_imported)),
        "every arm reads as imported: {arms:?}"
    );
    // BORROW: explicit conversion makes the owned or borrowed form the next line needs
    let got = stratum_verdict(arms.as_slice(), skill_factors().as_slice());
    assert_eq!(got, Verdict::Comparable);
}

/// Guards: R01 §Run manifest (imported strata, D7) — an arm read under other import rules than
/// its stratum's refuses the stratum, naming `import.rules`, though every legacy value agrees.
/// Oracle: the two imports are given the digests `1` repeated and `2` repeated by the test; D7
/// says differing rules are refused at `import.rules`, and nothing else differs between the arms.
/// Bite: leave `import.rules` out of the imported gate.
#[test]
fn imported_stratum_with_other_rules_is_refused() {
    let mut arms = mini_arms("1");
    let other = mini_arms("2");
    let flipped = other
        .into_iter()
        .next()
        .expect("an arm of the second import");
    arms.truncate(2);
    arms.push(flipped);
    // BORROW: explicit conversion makes the owned or borrowed form the next line needs
    let got = stratum_verdict(arms.as_slice(), skill_factors().as_slice());
    let want = FieldPath::from_str("import.rules").expect("a path");
    assert_eq!(got, Verdict::Refused(vec![want]));
}
