#![cfg(test)]
//! Guards for the `candler` command line: the JSON of `diff` and its exit status, over a store
//! seeded by hand. The expected key sets are the hand-written literal in
//! `fixtures/cli/diff_keys.json`.

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use candler_core::evaluation::registry::EnvironmentKey;
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

/// Runs the binary as [`candler`] does, from the working directory `cwd`.
fn candler_in(cwd: &Path, home: &Path, store: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .current_dir(cwd)
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

/// A project directory for plug-in `version` under `parent`: a `candler.toml` that lists the
/// directory of `manifest` under `plugins.dirs`. Returns the project.
fn project_of(parent: &Path, version: &str, manifest: &Path) -> PathBuf {
    let project = parent.join(format!("project-{version}"));
    fs::create_dir_all(&project).expect("a project directory");
    let plugin_dir = manifest.parent().expect("a manifest directory");
    fs::write(
        project.join("candler.toml"),
        format!("[plugins]\ndirs = [\"{}\"]\n", plugin_dir.display()),
    )
    .expect("the project file writes");
    project
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
    "sha256:b4387a085012bd32701ecd9f968b419d719e9f33ff97fcbd21fa44164cc00ccb";

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

/// Guards: I12 — a rules file has no silent default for the entry params: the fixture rules
/// without their `entry_params` line are refused by `candler import` (exit 2), the refusal names
/// `entry_params`, and the store holds no run. Oracle: the fixture with that one line removed by
/// hand, and the exit-status contract of the binary.
#[test]
fn rules_without_entry_params_are_refused_naming_the_key() {
    let dir = TempDir::new().expect("a temporary directory");
    let store = dir.path().join("store");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let corpus = root.join("../candler-core/tests/fixtures/mini-corpus");
    let fixture = fs::read_to_string(root.join("tests/fixtures/cli/mini-corpus-rules.toml"))
        .expect("the rules read");
    let kept: Vec<&str> = fixture
        .lines()
        .filter(|line| !line.starts_with("entry_params"))
        .collect();
    let without = format!("{}\n", kept.join("\n"));
    assert_ne!(without, fixture, "the fixture names no `entry_params`");
    let rules = dir.path().join("rules-without-entry-params.toml");
    fs::write(&rules, without).expect("the copy writes");

    let out = candler(
        dir.path(),
        &store,
        &[
            "import",
            corpus.to_str().expect("a UTF-8 path"),
            "--rules",
            rules.to_str().expect("a UTF-8 path"),
        ],
    );
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("entry_params"), "{stderr}");
    assert!(
        !store.join("runs").exists()
            || fs::read_dir(store.join("runs")).expect("runs list").count() == 0,
        "a refused import wrote runs"
    );
}

/// The fixtures of the engine guards, read in place.
fn engine_fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../candler-core/tests/fixtures/engine")
}

/// The helper's `uv` project, from which the fixture plug-in runs.
fn helper() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../python/candler-evaluator")
}

/// A plug-in directory of `version` under `parent`: the engine fixture's manifest and kinds with
/// the version changed, and a link to the helper's project. Returns the manifest.
fn plugin_of(parent: &Path, version: &str) -> PathBuf {
    let dir = parent.join(format!("plugin-{version}"));
    let kinds = dir.join("kinds");
    fs::create_dir_all(&kinds).expect("a plug-in directory");
    symlink(helper(), dir.join("env")).expect("a link to the helper");
    let module = fs::read_to_string(engine_fixtures().join("kinds/engine_fixture.py"))
        .expect("the kinds read")
        .replace(
            "VERSION = \"0.1.0\"",
            format!("VERSION = \"{version}\"").as_str(), // BORROW: explicit .as_str()
        );
    fs::write(kinds.join("engine_fixture.py"), module).expect("the kinds write");
    let manifest = fs::read_to_string(engine_fixtures().join("plugin.toml"))
        .expect("the manifest reads")
        .replace(
            "version = \"0.1.0\"",
            format!("version = \"{version}\"").as_str(), // BORROW: explicit .as_str()
        )
        .replace(
            "@EXAMPLES@",
            &helper().join("examples").display().to_string(),
        )
        .replace("@KINDS@", &kinds.display().to_string());
    let path = dir.join("plugin.toml");
    fs::write(&path, manifest).expect("the manifest writes");
    path
}

/// Builds the helper's environment under `env_root` and marks it `ready`, as the engine guard does.
fn sync_environment(env_root: &Path) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let key = runtime
        .block_on(EnvironmentKey::of_files(
            &helper().join("uv.lock"),
            &helper().join("pyproject.toml"),
        ))
        .expect("the environment key");
    let dir = key.dir(env_root);
    fs::create_dir_all(&dir).expect("an environment directory");
    let synced = Command::new("uv")
        .args(["sync", "--frozen"])
        .env("UV_PROJECT_ENVIRONMENT", dir.join(".venv"))
        .current_dir(helper())
        .status()
        .expect("uv runs");
    assert!(synced.success(), "uv sync built the helper's environment");
    fs::write(dir.join("ready"), key.digest()).expect("the ready marker writes");
}

/// Guards: CLI2 stage two (`evaluate`, then `diff` and `show` over what it wrote) — the engine
/// fixture's plug-in graded over the imported runs under two manifest versions with identical
/// kinds: each `evaluate` reports the verdicts of the fixture suite, `show` lists both versions,
/// `diff 0.1.0 0.1.1` exits 0 with five rows compared and none changed, and a value edited in the
/// store's 0.1.1 outcome file makes `diff` exit 1 naming that row. Oracle: the hand-written
/// verdicts of the fixture kinds on run 6 (`N1` `applies_to` misses: `not_applicable`; `F1` fails;
/// `F2` depends on it: `blocked`; `P1` and `P2` pass), counted from `suite_dependencies.toml`
/// and `engine_fixture.py`, never from the engine's output. The store edit is a tamper at file
/// level: outcome files carry no seal or digest, so the edit is read back as written.
#[test]
fn evaluate_diff_and_show_over_two_plugin_versions_follow_the_fixture_verdicts() {
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

    // `candler` points the state home at `<dir>/state`, where the environments live.
    sync_environment(&dir.path().join("state/candler/environments"));
    let suite = engine_fixtures().join("suite_dependencies.toml");
    let suite_arg = suite.to_str().expect("a UTF-8 path");
    let verdicts = json!({"blocked": 1, "fail": 1, "not_applicable": 1, "pass": 2});
    for version in ["0.1.0", "0.1.1"] {
        let manifest = plugin_of(dir.path(), version);
        // Each version runs from a project of its own, whose `candler.toml` lists its directory.
        let project = project_of(dir.path(), version, &manifest);
        let candler =
            |home: &Path, store: &Path, args: &[&str]| candler_in(&project, home, store, args);
        #[rustfmt::skip]
        let out = candler(
            dir.path(),
            &store,
            &[
                "evaluate",
                suite_arg,
                "--runs",
                "*-r6-a1",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "evaluate {version}: {out:?}");
        let report: Value = serde_json::from_slice(&out.stdout).expect("a JSON report");
        assert_eq!(report["counts"], verdicts, "{version}");
        assert_eq!(report["failed_requests"], json!([]), "{version}");
    }

    let out = candler(dir.path(), &store, &["show", "--format", "json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let shown: Value = serde_json::from_slice(&out.stdout).expect("JSON counts");
    assert_eq!(shown["counts"]["engine@0.1.0"], verdicts);
    assert_eq!(shown["counts"]["engine@0.1.1"], verdicts);

    let compare = |dir: &Path| {
        let out = candler(
            dir,
            &store,
            &[
                "diff",
                "--from",
                "engine@0.1.0",
                "--to",
                "engine@0.1.1",
                "--format",
                "json",
            ],
        );
        let report: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
        (out.status.code(), report)
    };
    let (code, report) = compare(dir.path());
    assert_eq!(code, Some(0), "{report}");
    assert_eq!(report["compared"], 5);
    assert_eq!(report["changed"], 0);

    // The tamper: `P1` of the 0.1.1 outcome file reads `fail` instead of `pass`.
    let mut edited = 0_usize;
    for entry in walk(&store.join("outcomes")) {
        let in_new = entry.to_string_lossy().contains("engine@0.1.1");
        if !in_new || entry.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let text = fs::read_to_string(&entry).expect("an outcome file reads");
        let lines: Vec<String> = text
            .lines()
            .map(|line| {
                let mut record: Value = serde_json::from_str(line).expect("a record");
                if record["expectation_id"] == "P1" {
                    record["status"] = json!("fail");
                    edited += 1;
                }
                record.to_string()
            })
            .collect();
        fs::write(&entry, format!("{}\n", lines.join("\n"))).expect("the edit writes");
    }
    assert_eq!(edited, 1, "one P1 record was edited");
    let (code, report) = compare(dir.path());
    assert_eq!(code, Some(1), "{report}");
    assert_eq!(report["changed"], 1);
    assert_eq!(report["changes"][0]["expectation"], "P1");
    assert_eq!(report["changes"][0]["from"]["status"], "pass");
    assert_eq!(report["changes"][0]["to"]["status"], "fail");
}

/// The files under `root`, at any depth.
fn walk(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).expect("a directory reads") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found
}
