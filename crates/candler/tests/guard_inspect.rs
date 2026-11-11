#![cfg(test)]
//! Guards for `candler inspect`: a run directory, a key prefix or a job id is all it needs, the
//! document shows outcomes with their evidence, the provenance and the integrity of the run, and
//! reading writes nothing. The stores are seeded by hand and the binary is the real one, run from
//! an unrelated directory.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use candler_core::jobs::dir::JobDir;
use candler_core::store::Store;
use candler_core::store::layout::StoreLayout;
use candler_core::store::outcome::OutcomeWriter;
use candler_core::store::run::{Existing, RunWriter};
use candler_schemas::execution::{JobRecord, RunManifest};
use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::{JobId, NameVersion, RequestId, RunKey};
use serde_json::{Value, json};
use tempfile::TempDir;

const EVALUATOR: &str = "eval@1.0.0";

/// The job every seeded run names.
const JOB: &str = "018f2b3c-4d5e-7a6b-8c7d-000000000001";

/// A job the seeded store holds whose supervisor was lost.
const LOST_JOB: &str = "018f2b3c-4d5e-7a6b-8c7d-000000000002";

fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

/// The manifest of a one-cell run: `plan_a` sets the cell, `replicate` the key, `job` the job it
/// names, `integrity` whether it counts. Its key carries the cell's real digest.
fn manifest(plan_a: u32, replicate: u32, job: Option<&str>, integrity: &Value) -> RunManifest {
    let body = |cell: &str| {
        let mut doc = json!({
            "schema": "run-manifest/1",
            "run": {"cell": cell, "replicate": replicate, "attempt": 1},
            "experiment": digest_text('0'),
            "factors": [],
            "comparable": {
                "values": [{"path": "plan.a", "value": plan_a}],
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
            "measured": [
                {"path": "provenance.sandbox_contract", "value": "v5",
                 "source": {"kind": "experiment_file"}},
                {"path": "tokens.input", "value": 7, "source": {"kind": "core_default"}}
            ],
            "deviations": [],
            "host": "host-1",
            "integrity": integrity,
        });
        if let Some(job) = job {
            doc["job"] = json!(job);
        }
        doc
    };
    let probe: RunManifest = serde_json::from_value(body(&digest_text('f'))).expect("a manifest");
    let cell = probe.cell_digest().to_string();
    serde_json::from_value(body(cell.as_str())).expect("a manifest") // BORROW: explicit .as_str()
}

/// The rows of an outcome file: `(expectation, status, evidence note)`.
type Rows<'a> = &'a [(&'a str, &'a str, &'a str)];

/// One outcome record with `evidence` notes, as request number `n`.
fn record(
    run: &RunKey,
    request: &RequestId,
    expectation: &str,
    status: &str,
    note: &str,
) -> OutcomeRecord {
    let num = u32::from(status == "pass");
    serde_json::from_value(json!({
        "run": {"cell": run.cell().to_string(), "replicate": run.replicate(), "attempt": run.attempt()},
        "expectation_id": expectation,
        "role": "gate",
        "evaluator": EVALUATOR,
        "instruments": [],
        "suite_entry_digest": digest_text('1'),
        "context_digest": digest_text('2'),
        "request_id": request.to_string(),
        "status": status,
        "weight": {"num": num, "den": 1},
        "evidence": [{"note": note}],
    }))
    .expect("an outcome record")
}

/// The id of request number `n`.
fn request(n: u32) -> RequestId {
    format!("01926b3a-7c3e-7a1b-8c4d-{n:012x}")
        .parse()
        .expect("a request id")
}

/// Seals the run of `manifest`, then writes one outcome file per `(request number, rows)`.
fn seed_run(store: &Store, manifest: &RunManifest, files: &[(u32, Rows<'_>)]) -> RunKey {
    let run = manifest.run().clone(); // BORROW: the key is used after the manifest moves
    RunWriter::create(store, manifest, Existing::Refuse)
        .expect("the run starts")
        .done()
        .expect("the run seals");
    let evaluator: NameVersion = EVALUATOR.parse().expect("name@version");
    for (n, rows) in files {
        let id = request(*n);
        let mut writer = OutcomeWriter::create(store, &run, &evaluator, &id).expect("a writer");
        for (expectation, status, note) in *rows {
            writer
                .append(&record(&run, &id, expectation, status, note))
                .expect("a record appends");
        }
        writer.commit().expect("the outcomes commit");
    }
    run
}

/// Creates the job `id` with its lock and, when `ended`, a `job.ended` event: a job as a
/// supervisor leaves it.
fn seed_job(root: &Path, id: &str, ended: bool) {
    let layout = StoreLayout::new(root);
    let id: JobId = id.parse().expect("a job id");
    let record: JobRecord = serde_json::from_value(json!({
        "schema": "job-record/1",
        "id": id.to_string(),
        "supervisor": {"pid": 4242, "start": {"kind": "unavailable"}},
        "spec_digest": digest_text('b'),
        "state": "running",
    }))
    .expect("a job record");
    let dir = JobDir::create(&layout, id.clone(), &record).expect("the job directory");
    fs::write(dir.path().join("lock"), b"").expect("the lock file");
    let log = if ended {
        format!(
            "{{\"v\":1,\"seq\":1,\"at\":\"2026-11-06T10:00:00Z\",\"job_id\":\"{id}\",\"kind\":\"job.ended\",\"payload\":{{\"reason\":\"completed\"}}}}\n"
        )
    } else {
        String::new()
    };
    fs::write(dir.path().join("events.jsonl"), log).expect("the log");
}

/// The real binary with `args`, run from `home` with no configuration from the environment and,
/// when `store` is given, `--store <store>`.
fn candler(home: &Path, store: Option<&Path>, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_candler"));
    command
        .current_dir(home)
        .env_remove("CANDLER_STORE")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"));
    if let Some(store) = store {
        command.arg("--store").arg(store);
    }
    command.args(args).output().expect("the binary runs")
}

/// The JSON a successful `inspect` printed.
fn document(out: &Output) -> Value {
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    serde_json::from_slice(&out.stdout).expect("one JSON document")
}

/// Every file under `dir`, by relative path, with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("a directory lists") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .expect("under the base")
                    .to_path_buf();
                out.insert(rel, fs::read(&path).expect("a file reads"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// Guards: I3 — `candler inspect <store>/runs/<key>`, with no `--store` and no environment, shows
/// that run, by an absolute path and by a path relative to the caller's directory; the store is
/// derived from the path.
/// Oracle: the key the test sealed, and the run directory it made.
/// Bite: a verb that needs `--store` to find the store of a run directory looks in the default
/// store, which holds no such run, and is refused.
#[test]
fn i3_a_run_directory_needs_no_flag() {
    let dir = TempDir::new().expect("a directory");
    let root = dir.path().join("somewhere/deep/store");
    let store = Store::init(&root).expect("a store");
    let run = seed_run(
        &store,
        &manifest(1, 1, None, &json!({"kind": "valid"})),
        &[(1, &[("A1", "pass", "ok")])],
    );
    let absolute = root.join("runs").join(run.to_string());
    let shown = document(&candler(
        dir.path(),
        None,
        &["inspect", absolute.to_str().expect("a UTF-8 path")],
    ));
    assert_eq!(shown["run"], run.to_string());
    assert_eq!(shown["sealed"], true);
    let relative = format!("runs/{run}");
    let again = document(&candler(&root, None, &["inspect", relative.as_str()]));
    assert_eq!(again, shown);
}

/// Guards: I3 — a unique prefix of a key selects its run; an ambiguous prefix is refused and lists
/// at most five of its candidates; a missing prefix and a target of no form are refused.
/// Oracle: the keys the test sealed (seven replicates of one cell, one run of another) and the rule
/// of the leaf: at least 6 characters, at most 5 candidates.
/// Bite: a prefix that picks the first match shows a run for the ambiguous one; a list that is not
/// cut shows seven.
#[test]
fn i3_a_key_prefix_selects_one_run() {
    let dir = TempDir::new().expect("a directory");
    let root = dir.path().join("store");
    let store = Store::init(&root).expect("a store");
    let valid = json!({"kind": "valid"});
    let mut shared = Vec::new();
    for replicate in 1..=7 {
        shared.push(seed_run(&store, &manifest(1, replicate, None, &valid), &[]));
    }
    let lone = seed_run(&store, &manifest(2, 1, None, &valid), &[]);
    let cell_of = |key: &RunKey| key.cell().hex().to_owned();

    let unique = cell_of(&lone)[..10].to_owned();
    let shown = document(&candler(
        dir.path(),
        Some(&root),
        &["inspect", unique.as_str()],
    ));
    assert_eq!(shown["run"], lone.to_string());

    let one_of_seven = format!("{}-r3", cell_of(&shared[0]));
    let shown = document(&candler(
        dir.path(),
        Some(&root),
        &["inspect", one_of_seven.as_str()],
    ));
    assert_eq!(shown["run"], shared[2].to_string());

    let ambiguous = cell_of(&shared[0])[..8].to_owned();
    let out = candler(dir.path(), Some(&root), &["inspect", ambiguous.as_str()]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let listed = shared
        .iter()
        .filter(|key| stderr.contains(key.to_string().as_str()))
        .count();
    assert_eq!(listed, 5, "{stderr}");
    assert!(stderr.contains("2 more"), "{stderr}");

    let out = candler(dir.path(), Some(&root), &["inspect", "ffffffffff"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("no run"));

    let out = candler(dir.path(), Some(&root), &["inspect", "nonsense target"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    for form in ["run directory", "prefix of a run key", "job id"] {
        assert!(stderr.contains(form), "{form}: {stderr}");
    }
}

/// Guards: I3 — a job id shows the job's state and the runs whose manifest names it, each with its
/// integrity and the counts of its current outcomes; a run that names no job is not listed.
/// Oracle: the job directory and the three runs the test wrote: two name the job (one with two
/// passes and a fail, one invalid with none), one names none.
/// Bite: a listing of every run of the store shows three; a state read from `job.json` shows
/// `running` instead of `ended (completed)`.
#[test]
fn i3_a_job_id_shows_its_runs() {
    let dir = TempDir::new().expect("a directory");
    let root = dir.path().join("store");
    let store = Store::init(&root).expect("a store");
    let valid = json!({"kind": "valid"});
    let first = seed_run(
        &store,
        &manifest(1, 1, Some(JOB), &valid),
        &[(
            1,
            &[
                ("A1", "pass", "a"),
                ("A2", "pass", "b"),
                ("A3", "fail", "c"),
            ],
        )],
    );
    let second = seed_run(
        &store,
        &manifest(
            1,
            2,
            Some(JOB),
            &json!({"kind": "invalid", "reason": "killed"}),
        ),
        &[],
    );
    seed_run(&store, &manifest(1, 3, None, &valid), &[]);
    seed_job(&root, JOB, true);
    let shown = document(&candler(dir.path(), Some(&root), &["inspect", JOB]));
    assert_eq!(shown["id"], JOB);
    assert_eq!(shown["state"], "ended (completed)");
    let runs = shown["runs"].as_array().expect("a list of runs");
    assert_eq!(runs.len(), 2, "{shown}");
    let by_key: BTreeMap<&str, &Value> = runs
        .iter()
        .map(|run| (run["key"].as_str().expect("a key"), run))
        .collect();
    let counts = |pass, fail| json!({"pass": pass, "fail": fail, "blocked": 0, "not_measured": 0, "not_applicable": 0});
    let first_text = first.to_string();
    let second_text = second.to_string();
    assert_eq!(by_key[first_text.as_str()]["outcomes"], counts(2, 1));
    assert_eq!(
        by_key[first_text.as_str()]["integrity"],
        json!({"kind": "valid"})
    );
    assert_eq!(by_key[second_text.as_str()]["outcomes"], counts(0, 0));
    assert_eq!(
        by_key[second_text.as_str()]["integrity"],
        json!({"kind": "invalid", "reason": "killed"})
    );
}

/// Guards: I3 — the document of a run shows each outcome with its evaluator, expectation, role,
/// status, weight, evidence and request, the latest request first for one key, the `provenance.*`
/// measured values apart from the other measured values, the integrity (with its reason) and the
/// host.
/// Oracle: the hand-written manifest and the two outcome files the test sealed, one request a
/// failure and the later one a pass of the same expectation.
/// Bite: outcomes without their evidence, provenance mixed into `measured`, or the earliest request
/// first fails an assertion.
#[test]
fn i3_outcomes_come_with_evidence_provenance_and_integrity() {
    let dir = TempDir::new().expect("a directory");
    let root = dir.path().join("store");
    let store = Store::init(&root).expect("a store");
    let run = seed_run(
        &store,
        &manifest(1, 1, None, &json!({"kind": "invalid", "reason": "gateway"})),
        &[
            (
                1,
                &[
                    ("A1", "fail", "the file is missing"),
                    ("A2", "pass", "present"),
                ],
            ),
            (2, &[("A1", "pass", "the file is there")]),
        ],
    );
    let shown = document(&candler(
        dir.path(),
        Some(&root),
        &["inspect", run.to_string().as_str()],
    ));
    assert_eq!(
        shown["integrity"],
        json!({"kind": "invalid", "reason": "gateway"})
    );
    assert_eq!(shown["host"], "host-1");
    assert_eq!(
        shown["provenance"],
        json!({"provenance.sandbox_contract": "v5"})
    );
    assert_eq!(shown["measured"], json!({"tokens.input": 7}));
    assert_eq!(shown["experiment"], digest_text('0'));
    let outcomes = shown["outcomes"].as_array().expect("a list of outcomes");
    let rows: Vec<(String, String, String)> = outcomes
        .iter()
        .map(|o| {
            (
                o["expectation"]
                    .as_str()
                    .expect("an expectation")
                    .to_owned(),
                o["status"].as_str().expect("a status").to_owned(),
                o["evidence"][0]["note"]
                    .as_str()
                    .expect("a note")
                    .to_owned(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (
                "A1".to_owned(),
                "pass".to_owned(),
                "the file is there".to_owned()
            ),
            (
                "A1".to_owned(),
                "fail".to_owned(),
                "the file is missing".to_owned()
            ),
            ("A2".to_owned(), "pass".to_owned(), "present".to_owned()),
        ]
    );
    for outcome in outcomes {
        assert_eq!(outcome["evaluator"], EVALUATOR);
        assert_eq!(outcome["role"], "gate");
        assert!(outcome["request"].is_string());
        assert!(outcome["weight"].is_object());
    }
}

/// Guards: I3 — inspecting a run directory, a key prefix, an ended job and a lost job writes
/// nothing: every file of the store has the bytes it had and no file is added, and a lost job is
/// shown as `lost` and not recovered.
/// Oracle: the byte snapshot of the whole store before and after.
/// Bite: a verb that recovers a lost job appends a `job.ended` event, and one that probes a job
/// without a lock creates it; either moves the snapshot.
#[test]
fn i3_inspect_writes_nothing() {
    let dir = TempDir::new().expect("a directory");
    let root = dir.path().join("store");
    let store = Store::init(&root).expect("a store");
    let run = seed_run(
        &store,
        &manifest(1, 1, Some(JOB), &json!({"kind": "valid"})),
        &[(1, &[("A1", "pass", "ok")])],
    );
    seed_job(&root, JOB, true);
    seed_job(&root, LOST_JOB, false);
    let before = snapshot(&root);
    let run_dir = root.join("runs").join(run.to_string());
    let prefix = &run.to_string()[..12];
    for target in [run_dir.to_str().expect("a UTF-8 path"), prefix, JOB] {
        let out = candler(dir.path(), Some(&root), &["inspect", target]);
        assert_eq!(out.status.code(), Some(0), "{target}: {out:?}");
    }
    let lost = document(&candler(dir.path(), Some(&root), &["inspect", LOST_JOB]));
    assert_eq!(lost["state"], "lost");
    assert_eq!(snapshot(&root), before);
}
