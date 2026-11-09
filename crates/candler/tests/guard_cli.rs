#![cfg(test)]
//! Guards for the `candler` command line: the JSON of `diff` and its exit status, over a store
//! seeded by hand. The expected key sets are the hand-written literal in
//! `fixtures/cli/diff_keys.json`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use candler_core::store::Store;
use candler_core::store::outcome::OutcomeWriter;
use candler_core::store::run::{Existing, RunWriter};
use candler_schemas::execution::RunManifest;
use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::{NameVersion, RequestId, RunKey};
use serde_json::{Value, json};
use tempfile::TempDir;

const OLD: &str = "old-eval@1.0.0";
const NEW: &str = "new-eval@2.0.0";

fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

/// The manifest of a one-cell run, whose key carries the cell's real digest.
fn manifest() -> RunManifest {
    let body = |cell: &str| {
        json!({
            "schema": "run-manifest/1",
            "run": {"cell": cell, "replicate": 1, "attempt": 1},
            "experiment": digest_text('0'),
            "factors": [],
            "comparable": {
                "values": [{"path": "plan.a", "value": 1}],
                "harness": {"name": "h", "version": "1.0.0"},
                "treatment": {"name": "t"},
                "snapshots": [],
                "inputs": [],
                "environments": []
            },
            "sources": [
                {"path": "plan.a", "source": {"kind": "experiment_file"}},
                {"path": "harness", "source": {"kind": "experiment_file"}},
                {"path": "treatment", "source": {"kind": "experiment_file"}}
            ],
            "secrets": [],
            "measured": [],
            "deviations": [],
            "integrity": {"kind": "valid"}
        })
    };
    let probe: RunManifest = serde_json::from_value(body(&digest_text('f'))).expect("a manifest");
    let cell = probe.cell_digest().to_string();
    serde_json::from_value(body(cell.as_str())).expect("a manifest") // BORROW: explicit .as_str()
}

/// One outcome record: `expectation` graded `status` (`pass` or `fail`) by `evaluator`.
fn record(
    run: &RunKey,
    request: &RequestId,
    evaluator: &str,
    expectation: &str,
    status: &str,
) -> OutcomeRecord {
    let num = u32::from(status == "pass");
    serde_json::from_value(json!({
        "run": {"cell": run.cell().to_string(), "replicate": run.replicate(), "attempt": run.attempt()},
        "expectation_id": expectation,
        "role": "gate",
        "evaluator": evaluator,
        "instruments": [],
        "suite_entry_digest": digest_text('1'),
        "context_digest": digest_text('2'),
        "request_id": request.to_string(),
        "status": status,
        "weight": {"num": num, "den": 1},
        "evidence": [],
    }))
    .expect("an outcome record")
}

/// A store of one run holding, per evaluator, the rows `(expectation, status)` in one request.
fn seed(root: &Path, rows: &[(&str, &[(&str, &str)])]) -> RunKey {
    let store = Store::init(root).expect("the store initialises");
    let manifest = manifest();
    let run = manifest.run().clone(); // BORROW: the key is used after the manifest moves
    RunWriter::create(&store, &manifest, Existing::Refuse)
        .expect("the run starts")
        .done()
        .expect("the run seals");
    for (n, (evaluator, graded)) in rows.iter().enumerate() {
        let request: RequestId = format!("01926b3a-7c3e-7a1b-8c4d-{:012x}", n + 1)
            .parse()
            .expect("a request id");
        let name: NameVersion = evaluator.parse().expect("name@version");
        let mut writer = OutcomeWriter::create(&store, &run, &name, &request).expect("a writer");
        for (expectation, status) in *graded {
            writer
                .append(&record(&run, &request, evaluator, expectation, status))
                .expect("a record appends");
        }
        writer.commit().expect("the outcomes commit");
    }
    run
}

/// Runs the binary with `args` over `store`, with no configuration from the environment.
fn candler(home: &Path, store: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .current_dir(home)
        .env_remove("CANDLER_STORE")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .arg("--store")
        .arg(store)
        .args(args)
        .output()
        .expect("the binary runs")
}

fn keys(value: &Value) -> BTreeSet<String> {
    value
        .as_object()
        .expect("a JSON object")
        .keys()
        .cloned()
        .collect()
}

fn literal(name: &str) -> BTreeSet<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli/diff_keys.json");
    let text = fs::read_to_string(path).expect("the key literal reads");
    let all: Value = serde_json::from_str(&text).expect("the key literal is JSON");
    all[name]
        .as_array()
        .expect("a list of keys")
        .iter()
        .map(|k| k.as_str().expect("a key").to_owned())
        .collect()
}

/// An equivalence file that excuses the one row `(run, A3)`, graded `fail` by the old evaluator
/// and `pass` by the new one.
fn equivalence(dir: &Path, run: &RunKey) -> PathBuf {
    let path = dir.join("equivalence.json");
    let body = json!({
        "rules": [{
            "name": "fail-to-pass",
            "from": {"status": "fail"},
            "to": {"status": "pass"},
            "rows": [{"run": run.to_string(), "expectation": "A3"}]
        }]
    });
    fs::write(&path, body.to_string()).expect("the equivalence file writes");
    path
}

fn diff_json(home: &Path, store: &Path, extra: &[&str]) -> (i32, Value) {
    let mut args = vec!["diff", "--from", OLD, "--to", NEW, "--format", "json"];
    args.extend_from_slice(extra);
    let out = candler(home, store, &args);
    let code = out.status.code().expect("the binary exits with a code");
    let value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    (code, value)
}

/// Guards: R2 — `diff --format json` has exactly the documented key set at the top level, in each
/// change, in each uncompared entry and in `equivalence`, and the binary exits 1 on a planted
/// change. Oracle: the hand-written key literal in `fixtures/cli/diff_keys.json`.
#[test]
fn diff_json_has_the_literal_key_sets_and_exits_one_on_a_change() {
    let dir = TempDir::new().expect("a temporary directory");
    let store = dir.path().join("store");
    let run = seed(
        &store,
        &[
            (
                OLD,
                &[
                    ("A1", "pass"),
                    ("A2", "pass"),
                    ("A3", "fail"),
                    ("U1", "pass"),
                ],
            ),
            (NEW, &[("A1", "pass"), ("A2", "fail"), ("A3", "pass")]),
        ],
    );
    let file = equivalence(dir.path(), &run);
    let file_arg = file.to_str().expect("a UTF-8 path");
    let (code, report) = diff_json(dir.path(), &store, &["--equivalence", file_arg]);
    assert_eq!(code, 1, "one planted change: {report}");
    assert_eq!(keys(&report), literal("top"));
    assert_eq!(report["changed"], 1);
    assert_eq!(report["excused"], 1);
    let changes = report["changes"].as_array().expect("changes is a list");
    assert_eq!(changes.len(), 1);
    assert!(changes.iter().all(|c| keys(c) == literal("change")));
    let uncompared = report["uncompared"].as_array().expect("a list");
    assert_eq!(uncompared.len(), 1);
    assert!(uncompared.iter().all(|u| keys(u) == literal("uncompared")));
    assert_eq!(keys(&report["equivalence"]), literal("equivalence"));
}

/// Guards: R2 — with no change `diff` exits 0 and `equivalence` is a present `null`; when the
/// `to` version has no record nothing is compared and it exits 2, never 0. Oracle: the
/// `diff(1)` convention and the hand-written key literal in `fixtures/cli/diff_keys.json`.
#[test]
fn diff_exits_zero_without_a_change_and_two_without_a_to_record() {
    let same = TempDir::new().expect("a temporary directory");
    let store = same.path().join("store");
    seed(
        &store,
        &[
            (OLD, &[("A1", "pass"), ("A2", "fail")]),
            (NEW, &[("A1", "pass"), ("A2", "fail")]),
        ],
    );
    let (code, report) = diff_json(same.path(), &store, &[]);
    assert_eq!(code, 0, "no change: {report}");
    assert_eq!(keys(&report), literal("top"));
    assert_eq!(report["compared"], 2);
    assert!(report["equivalence"].is_null());

    let absent = TempDir::new().expect("a temporary directory");
    let store = absent.path().join("store");
    seed(&store, &[(OLD, &[("A1", "pass")])]);
    let (code, _) = diff_json(absent.path(), &store, &[]);
    assert_eq!(code, 2, "the to version has no record");
}

/// The sha256 of `fixtures/cli/mini-corpus-rules.toml`, from `shasum -a 256` on the file.
const RULES_SHA256: &str =
    "sha256:f26fef6c53ee026c52c08ac9de11cc47263bd289f61b0c2bf9b12000f45cd87e";

/// Guards: CLI2 stage one (import, show, diff; `evaluate` waits for the engine) — importing the M1
/// mini-corpus with the fixture rules gives `show` the counts of the corpus's grading files, `diff`
/// compares the imported evaluator with itself (exit 0, 136 compared) and with a version the store
/// lacks (exit 2), the stored manifests carry the sha256 of the rules file as `import.rules`, and a
/// re-import under a one-byte-different rules file exits 2 naming `import.rules`. Oracle: hand
/// literals counted from the 42 `eval-N-grading.json` files of the corpus (outside `__MACOSX` and
/// the contaminated synthetic run-1, which grades nothing) with
/// `jq -r '.expectations[].status'`, one status per row: runs 6, 12 and 17 hold 43 rows each
/// (38 pass, 1 fail, 4 `not_measured`; 32 pass, 5 fail, 6 `not_measured`; 29 pass, 1 fail, 13
/// blocked), the synthetic runs 2 and 3 hold 5 pass and 1 pass with 1 `not_applicable`; the
/// digest is `shasum -a 256` of the fixture; the importer's own output is never the oracle.
#[test]
fn import_show_and_diff_over_the_mini_corpus_match_the_hand_counts() {
    let dir = TempDir::new().expect("a temporary directory");
    let store = dir.path().join("store");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let corpus = root.join("../candler-core/tests/fixtures/mini-corpus");
    let rules = root.join("tests/fixtures/cli/mini-corpus-rules.toml");
    let corpus_arg = corpus.to_str().expect("a UTF-8 path");
    let rules_arg = rules.to_str().expect("a UTF-8 path");

    let out = candler(
        dir.path(),
        &store,
        &["import", corpus_arg, "--rules", rules_arg],
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let summary: Value = serde_json::from_slice(&out.stdout).expect("a JSON summary");
    assert_eq!(summary, json!({"runs": 6, "records": 136}));

    let out = candler(dir.path(), &store, &["show", "--format", "json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let shown: Value = serde_json::from_slice(&out.stdout).expect("JSON counts");
    let expected = json!({"counts": {"htn-creator-eval@0.4.2": {
        "pass": 105, "fail": 7, "blocked": 13, "not_measured": 10, "not_applicable": 1
    }}});
    assert_eq!(shown, expected);

    let same = "htn-creator-eval@0.4.2";
    let out = candler(
        dir.path(),
        &store,
        &["diff", "--from", same, "--to", same, "--format", "json"],
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let report: Value = serde_json::from_slice(&out.stdout).expect("a JSON report");
    assert_eq!(report["compared"], 136);
    assert_eq!(report["changed"], 0);
    let out = candler(
        dir.path(),
        &store,
        &[
            "diff",
            "--from",
            same,
            "--to",
            "htn-creator-eval@0.4.3",
            "--format",
            "json",
        ],
    );
    assert_eq!(out.status.code(), Some(2), "nothing to compare: {out:?}");

    let runs = store.join("runs");
    let mut seen = 0_usize;
    for entry in fs::read_dir(&runs).expect("the runs directory reads") {
        let path = entry
            .expect("a directory entry")
            .path()
            .join("manifest.json");
        let text = fs::read_to_string(&path).expect("a manifest reads");
        let manifest: Value = serde_json::from_str(&text).expect("a manifest is JSON");
        assert_eq!(manifest["import"]["rules"], RULES_SHA256, "{path:?}");
        seen += 1;
    }
    assert_eq!(seen, 6);

    let out = candler(
        dir.path(),
        &store,
        &["import", corpus_arg, "--rules", rules_arg],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "the same rules re-import: {out:?}"
    );
    let mut changed = fs::read(&rules).expect("the rules read");
    changed.push(b'\n');
    let other = dir.path().join("other-rules.toml");
    fs::write(&other, changed).expect("the copy writes");
    let other_arg = other.to_str().expect("a UTF-8 path");
    let out = candler(
        dir.path(),
        &store,
        &["import", corpus_arg, "--rules", other_arg],
    );
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("import.rules"), "{stderr}");
}
