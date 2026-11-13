#![cfg(test)]
//! Guards for `candler analyze` through the binary, run from a directory that is no project and
//! over a store seeded by hand with the layout of the leaf: three cells of ten valid runs, and two
//! extra runs of `alpha`, one invalid for the gateway and one ended by the experimenter, that each
//! carry a pass the report must count nowhere. Every expected count is a hand count, and every
//! interval, p-value and difference is a row of the second engine's vector files in the core crate.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use candler_core::store::Store;
use candler_core::store::outcome::OutcomeWriter;
use candler_core::store::run::{Existing, RunWriter};
use candler_schemas::execution::RunManifest;
use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::{NameVersion, RequestId, RunKey};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tempfile::TempDir;

const EVALUATOR: &str = "suite-eval@1.0.0";

fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

/// What one fabricated run holds besides its model.
#[derive(Clone)]
struct Spec {
    model: &'static str,
    replicate: u32,
    integrity: Value,
    /// Further `comparable` values, `(path, value)`.
    extra: Vec<(&'static str, Value)>,
    /// Further measured values, `(path, value)`.
    measured: Vec<(&'static str, Value)>,
    /// Overrides of the clean provenance block; `null` drops the path.
    provenance: Vec<(&'static str, Value)>,
    /// Paths of the cell whose value was not recorded.
    unrecorded: Vec<&'static str>,
    /// The factors the manifest records.
    factors: Vec<&'static str>,
    /// The character the experiment digest is made of.
    experiment: char,
}

impl Spec {
    fn new(model: &'static str, replicate: u32) -> Self {
        Self {
            model,
            replicate,
            integrity: json!({"kind": "valid"}),
            extra: Vec::new(),
            measured: Vec::new(),
            provenance: Vec::new(),
            unrecorded: Vec::new(),
            factors: vec!["plan.model"],
            experiment: '0',
        }
    }
}

/// The manifest of one run, whose key carries the cell's real digest. Every run carries the clean
/// provenance block that a verdict needs, unless its spec overrides a part of it.
fn manifest(spec: &Spec) -> RunManifest {
    let mut values = vec![json!({"path": "plan.model", "value": spec.model})];
    for (path, value) in &spec.extra {
        values.push(json!({"path": path, "value": value}));
    }
    for path in &spec.unrecorded {
        values.push(json!({"path": path}));
    }
    values.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    let mut sources: Vec<Value> = values
        .iter()
        .map(|v| {
            let kind = if v.get("value").is_some() {
                "experiment_file"
            } else {
                "unrecorded"
            };
            json!({"path": v["path"], "source": {"kind": kind}})
        })
        .collect();
    sources.push(json!({"path": "harness", "source": {"kind": "experiment_file"}}));
    sources.push(json!({"path": "treatment", "source": {"kind": "experiment_file"}}));
    let mut block: Vec<(String, Value)> = vec![
        ("eval_infra.commit".to_owned(), json!("abc1234")),
        ("eval_infra.dirty".to_owned(), json!(false)),
        ("sandbox_contract".to_owned(), json!("candler-container@1")),
    ];
    for (key, value) in &spec.provenance {
        block.retain(|(held, _)| held != key);
        if !value.is_null() {
            block.push(((*key).to_owned(), value.clone()));
        }
    }
    let mut measured: Vec<Value> = block
        .into_iter()
        .map(|(key, value)| {
            json!({"path": format!("provenance.{key}"), "value": value, "source": {"kind": "experiment_file"}})
        })
        .collect();
    for (path, value) in &spec.measured {
        measured.push(json!({"path": path, "value": value, "source": {"kind": "experiment_file"}}));
    }
    let body = |cell: &str| {
        json!({
            "schema": "run-manifest/1",
            "run": {"cell": cell, "replicate": spec.replicate, "attempt": 1},
            "experiment": digest_text(spec.experiment),
            "factors": spec.factors,
            "comparable": {
                "values": values,
                "harness": {"name": "h", "version": "1.0.0"},
                "treatment": {"name": "t"},
                "snapshots": [],
                "inputs": [],
                "environments": []
            },
            "sources": sources,
            "secrets": [],
            "measured": measured,
            "deviations": [],
            "integrity": spec.integrity
        })
    };
    let probe: RunManifest = serde_json::from_value(body(&digest_text('f'))).expect("a manifest");
    let cell = probe.cell_digest().to_string();
    serde_json::from_value(body(cell.as_str())).expect("a manifest") // BORROW: explicit .as_str()
}

/// One run to store: its manifest and the records of each evaluator, `(evaluator, record)`.
struct Seeded {
    manifest: RunManifest,
    records: Vec<(String, OutcomeRecord)>,
}

/// One outcome record of `evaluator`. `outcome` is the status, the weight as `(num, den)` (absent
/// for the default of the status) and the reason code of a `blocked`, `not_measured` or
/// `not_applicable`.
fn record(
    evaluator: &str,
    run: &RunKey,
    request: &RequestId,
    expectation: &str,
    role: &str,
    outcome: (&str, Option<(u32, u32)>, Option<&str>),
) -> OutcomeRecord {
    let (status, weight, cause) = outcome;
    let mut doc = json!({
        "run": {"cell": run.cell().to_string(), "replicate": run.replicate(), "attempt": run.attempt()},
        "expectation_id": expectation,
        "role": role,
        "evaluator": evaluator,
        "instruments": [],
        "suite_entry_digest": digest_text('1'),
        "context_digest": digest_text('2'),
        "request_id": request.to_string(),
        "status": status,
        "evidence": [],
    });
    if let Some((num, den)) = weight {
        doc["weight"] = json!({"num": num, "den": den});
    }
    if let Some(reason) = cause {
        doc["cause"] = json!({"reason": reason});
    }
    serde_json::from_value(doc).expect("an outcome record")
}

/// A graded row of one run: `(status, cause)`.
type Row = (&'static str, Option<&'static str>);

/// The rows of `E1` and `E2` of one cell over runs 1 to 10, as the layout of the leaf lists them.
fn rows_of(model: &str) -> (Vec<Row>, Vec<Row>) {
    let rep = |status: &'static str, cause: Option<&'static str>, count: usize| {
        vec![(status, cause); count]
    };
    let mut e1: Vec<Row> = Vec::new();
    let mut e2: Vec<Row> = Vec::new();
    match model {
        "alpha" => {
            e1.extend(rep("pass", None, 6));
            e1.extend(rep("fail", None, 2));
            e1.extend(rep("blocked", Some("missing_input"), 2));
            e2.extend(rep("pass", None, 3));
            e2.extend(rep("fail", None, 2));
            e2.extend(rep("not_measured", Some("instrument_error"), 3));
            e2.extend(rep("not_measured", Some("deadline"), 2));
        }
        "beta" => {
            e1.extend(rep("pass", None, 2));
            e1.extend(rep("fail", None, 8));
            e2.extend(rep("fail", None, 5));
            e2.extend(rep("not_applicable", Some("not_applicable_to_arm"), 5));
        }
        _ => {
            e1.extend(rep("pass", None, 5));
            e1.extend(rep("fail", None, 5));
            e2.extend(rep("pass", None, 5));
            e2.extend(rep("not_measured", Some("instrument_error"), 5));
        }
    }
    (e1, e2)
}

fn request(n: usize) -> RequestId {
    format!("01926b3a-7c3e-7a1b-8c4d-{n:012x}")
        .parse()
        .expect("a request id")
}

/// The runs of the layout for the evaluators given: each evaluator grades every run in its own
/// request, `E1` and `E2` as the layout lists them and `E3` at weights 5/5 and 2/5.
fn layout(evaluators: &[&str], with_invalid: bool) -> Vec<Seeded> {
    let mut seeded = Vec::new();
    let mut n = 0_usize;
    let mut one = |spec: &Spec, e1: Row, e2: Row, e3: (&'static str, (u32, u32))| {
        let manifest = manifest(spec);
        let mut records = Vec::new();
        for evaluator in evaluators {
            n += 1;
            let id = request(n);
            let key = manifest.run().clone();
            let (s1, c1) = e1;
            let (s2, c2) = e2;
            let rows = [
                ("E1", "gate", (s1, None, c1)),
                ("E2", "gate", (s2, None, c2)),
                ("E3", "quality", (e3.0, Some(e3.1), None)),
            ];
            for (expectation, role, outcome) in rows {
                records.push((
                    (*evaluator).to_owned(),
                    record(evaluator, &key, &id, expectation, role, outcome),
                ));
            }
        }
        seeded.push(Seeded { manifest, records });
    };
    for model in ["alpha", "beta", "gamma"] {
        let (e1, e2) = rows_of(model);
        for replicate in 1..=10_u32 {
            let at = usize::try_from(replicate).expect("small") - 1;
            let e3 = match (model, replicate) {
                ("alpha", 9 | 10) => ("fail", (2, 5)),
                _ => ("pass", (5, 5)),
            };
            one(&Spec::new(model, replicate), e1[at], e2[at], e3);
        }
    }
    if with_invalid {
        for (replicate, reason) in [(11, "gateway"), (12, "killed")] {
            let mut spec = Spec::new("alpha", replicate);
            spec.integrity = json!({"kind": "invalid", "reason": reason});
            one(&spec, ("pass", None), ("pass", None), ("pass", (5, 5)));
        }
    }
    seeded
}

/// The store of `runs`, written through the store's own writers.
fn seed(root: &Path, runs: &[Seeded]) {
    let store = Store::init(root).expect("the store initialises");
    for run in runs {
        RunWriter::create(&store, &run.manifest, Existing::Refuse)
            .expect("the run starts")
            .done()
            .expect("the run seals");
        let key = run.manifest.run();
        let mut evaluators: Vec<&str> = run.records.iter().map(|(e, _)| e.as_str()).collect();
        evaluators.dedup();
        for evaluator in evaluators {
            let name: NameVersion = evaluator.parse().expect("name@version");
            let mut ours = run.records.iter().filter(|(e, _)| e == evaluator);
            let first = ours.next().expect("a record");
            let id = first.1.request_id().clone();
            let mut writer = OutcomeWriter::create(&store, key, &name, &id).expect("a writer");
            writer.append(&first.1).expect("a record appends");
            for (_, held) in ours {
                writer.append(held).expect("a record appends");
            }
            writer.commit().expect("the outcomes commit");
        }
    }
}

/// Runs `candler analyze` over `store` from `home`, a directory that is no project.
fn analyze(home: &Path, store: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .current_dir(home)
        .env_remove("CANDLER_STORE")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .arg("--store")
        .arg(store)
        .arg("analyze")
        .args(args)
        .output()
        .expect("the binary runs")
}

fn json_of(out: &Output) -> Value {
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("the output is JSON")
}

/// An experiment file with the factors given as `(path, levels)`.
fn experiment_of(dir: &Path, factors: &[(&str, &[&str])]) -> PathBuf {
    let path = dir.join("experiment.toml");
    let mut text = String::from(
        "schema = \"experiment/1\"\nreplicates = 10\n\n[fixed]\npermission_mode = \"plan\"\n\n",
    );
    for (factor, levels) in factors {
        let list: Vec<String> = levels.iter().map(|l| format!("{l:?}")).collect();
        writeln!(
            text,
            "[[factors]]\npath = {factor:?}\nlevels = [{}]\n",
            list.join(", ")
        )
        .expect("a string writes");
    }
    text.push_str(
        "[budget.per_run]\nturns = 30\nwall_s = 600\nmax_cost_usd = 1.0\n\n[harness]\nname = \"h\"\n\n[treatment]\nname = \"t\"\n",
    );
    fs::write(&path, text).expect("the experiment file writes");
    path
}

/// An experiment file whose one factor is `plan.model`, with the three models as levels.
fn experiment(dir: &Path) -> PathBuf {
    experiment_of(dir, &[("plan.model", &["alpha", "beta", "gamma"])])
}

/// A store of `runs` and a home directory, both in one temporary directory.
fn fixture(runs: &[Seeded]) -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("a temporary directory");
    let store = dir.path().join("store");
    seed(&store, runs);
    (dir, store)
}

/// The cell of a model, by its hex.
fn cell_of(runs: &[Seeded], model: &str) -> String {
    runs.iter()
        .find(|run| {
            run.manifest.comparable().values().iter().any(|v| {
                v.value()
                    .is_some_and(|held| held.as_value() == &json!(model))
            })
        })
        .map(|run| run.manifest.run().cell().hex().to_owned())
        .expect("a run of the model")
}

#[derive(Deserialize)]
struct Vectors<R> {
    rows: Vec<R>,
}

/// The rows of one vector file of the core crate's second engine.
fn vector<R: DeserializeOwned>(file: &str) -> Vec<R> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../candler-core/tests/vectors/stats")
        .join(file);
    let text = fs::read_to_string(&path).expect("the vector file reads");
    toml::from_str::<Vectors<R>>(&text)
        .expect("the vector file parses")
        .rows
}

#[derive(Deserialize)]
struct WilsonRow {
    k: u64,
    n: u64,
    lo: f64,
    hi: f64,
}

#[derive(Deserialize)]
struct FisherRow {
    k1: u64,
    n1: u64,
    k2: u64,
    n2: u64,
    p: f64,
}

#[derive(Deserialize)]
struct NewcombeRow {
    k1: u64,
    n1: u64,
    k2: u64,
    n2: u64,
    diff: f64,
    lo: f64,
    hi: f64,
}

fn close(label: &str, got: f64, want: f64) {
    assert!(
        (got - want).abs() <= 1e-9,
        "{label}: got {got:?}, the second engine has {want:?}"
    );
}

fn wilson_row(k: u64, n: u64) -> WilsonRow {
    vector::<WilsonRow>("wilson.toml")
        .into_iter()
        .find(|r| (r.k, r.n) == (k, n))
        .expect("a wilson row")
}

fn pair_rows(a: (u64, u64), b: (u64, u64)) -> (FisherRow, NewcombeRow) {
    let fisher = vector::<FisherRow>("fisher.toml")
        .into_iter()
        .find(|r| (r.k1, r.n1, r.k2, r.n2) == (a.0, a.1, b.0, b.1))
        .expect("a fisher row");
    let newcombe = vector::<NewcombeRow>("newcombe.toml")
        .into_iter()
        .find(|r| (r.k1, r.n1, r.k2, r.n2) == (a.0, a.1, b.0, b.1))
        .expect("a newcombe row");
    (fisher, newcombe)
}

fn arm<'a>(report: &'a Value, cell: &str) -> &'a Value {
    report["arms"]
        .as_array()
        .expect("arms")
        .iter()
        .find(|a| a["cell"] == cell)
        .expect("the arm")
}

fn expectation<'a>(arm: &'a Value, id: &str) -> &'a Value {
    arm["expectations"]
        .as_array()
        .expect("expectations")
        .iter()
        .find(|e| e["id"] == id)
        .expect("the expectation")
}

/// Guards: A1 — the JSON equals the hand counts of the layout: the evaluator, alpha with its
/// source, per arm the runs by validity, the gate and quality sums as `num` and `den`, the status
/// totals, per expectation the counts, causes and Wilson interval, E3 `weighted`, and with
/// `--experiment` the three comparable pairs, their p, Holm adjustment and differences from the
/// vector rows, one family `gate, 6`. The binary runs outside any project.
/// Oracle: the hand counts written in the leaf and the rows of the second engine.
/// Bite: count the invalid runs, leave `blocked` out of `n`, adjust per pair, or list `killed`
/// under `invalid`, and a count turns red.
#[test]
fn a1_the_json_equals_the_hand_counts() {
    let runs = layout(&[EVALUATOR], true);
    let (dir, store) = fixture(&runs);
    let report = json_of(&analyze(dir.path(), &store, &[]));
    assert_eq!(report["evaluator"], EVALUATOR);
    assert_eq!(
        report["alpha"],
        json!({"value": 0.05, "source": "core_default"})
    );
    assert_eq!(report["i4_violations"], 0);
    // Without an experiment file the factors are the ones the manifests record.
    assert_eq!(report["comparisons"].as_array().expect("pairs").len(), 3);
    assert_eq!(report["pairs_not_one_factor"], 0);
    for held in report["arms"].as_array().expect("arms") {
        assert_eq!(held["experiment"], digest_text('0'));
    }
    assert_eq!(report["runs"].as_array().expect("runs").len(), 32);
    assert_eq!(report["arms"].as_array().expect("arms").len(), 3);
    let alpha = cell_of(&runs, "alpha");
    let a = arm(&report, alpha.as_str());
    assert_eq!(
        a["runs"],
        json!({"valid": 10, "unsealed": 0, "invalid": {"gateway": 1}, "experimenter_ends": {"killed": 1}})
    );
    assert_eq!(
        a["rates"],
        json!({"gate": {"num": 9, "den": 15}, "quality": {"num": 44, "den": 50}})
    );
    assert_eq!(
        a["status"],
        json!({"pass": 17, "fail": 6, "blocked": 2, "not_measured": 5, "not_applicable": 0})
    );
    let e1 = expectation(a, "E1");
    assert_eq!(
        (&e1["pass"], &e1["fail"], &e1["blocked"], &e1["role"]),
        (&json!(6), &json!(2), &json!(2), &json!("gate"))
    );
    let row = wilson_row(6, 10);
    close("lo", e1["wilson"]["lo"].as_f64().expect("lo"), row.lo);
    close("hi", e1["wilson"]["hi"].as_f64().expect("hi"), row.hi);
    assert_eq!(e1["omitted"], Value::Null);
    let e2 = expectation(a, "E2");
    assert_eq!(
        e2["not_measured"],
        json!({"reason:instrument_error": 3, "reason:deadline": 2})
    );
    assert_eq!(e2["weight"], json!({"num": 3, "den": 5}));
    let e3 = expectation(a, "E3");
    assert_eq!(e3["omitted"], "weighted");
    assert_eq!(e3["wilson"], Value::Null);
    assert_eq!(e3["weight"], json!({"num": 44, "den": 50}));
    let beta = cell_of(&runs, "beta");
    let b = arm(&report, beta.as_str());
    assert_eq!(b["rates"]["gate"], json!({"num": 2, "den": 15}));
    assert_eq!(
        expectation(b, "E2")["not_applicable"],
        json!({"reason:not_applicable_to_arm": 5})
    );
    let gamma = cell_of(&runs, "gamma");
    assert_eq!(
        arm(&report, gamma.as_str())["rates"],
        json!({"gate": {"num": 10, "den": 15}, "quality": {"num": 50, "den": 50}})
    );
    // With the experiment file, the three pairs are compared and Holm is one family of six.
    let file = experiment(dir.path());
    let file_arg = file.to_str().expect("a UTF-8 path");
    let compared = json_of(&analyze(dir.path(), &store, &["--experiment", file_arg]));
    assert_eq!(compared["families"], json!([{"role": "gate", "m": 6}]));
    assert_eq!(compared["pairs_not_one_factor"], 0);
    let pairs = compared["comparisons"].as_array().expect("comparisons");
    assert_eq!(pairs.len(), 3);
    let counts = |cell: &str, id: &str| -> (u64, u64) {
        match (cell == alpha, cell == beta, id) {
            (true, _, "E1") => (6, 10),
            (true, _, _) => (3, 5),
            (_, true, "E1") => (2, 10),
            (_, true, _) => (0, 5),
            (_, _, "E1") => (5, 10),
            _ => (5, 5),
        }
    };
    let mut raw = Vec::new();
    let mut adjusted = Vec::new();
    for pair in pairs {
        assert_eq!(pair["factor"], "plan.model");
        assert_eq!(
            pair["stratum"],
            json!({"verdict": "comparable", "causes": []})
        );
        let (ca, cb) = (
            pair["a"].as_str().expect("a"),
            pair["b"].as_str().expect("b"),
        );
        for entry in pair["entries"].as_array().expect("entries") {
            if entry["id"] == "E3" {
                assert_eq!(entry["omitted"], "weighted");
                continue;
            }
            let id = entry["id"].as_str().expect("id");
            let (ka, kb) = (counts(ca, id), counts(cb, id));
            assert_eq!(entry["a"], json!({"k": ka.0, "n": ka.1}));
            assert_eq!(entry["b"], json!({"k": kb.0, "n": kb.1}));
            let (fisher, newcombe) = pair_rows(ka, kb);
            close("p", entry["p"].as_f64().expect("p"), fisher.p);
            close("diff", entry["diff"].as_f64().expect("diff"), newcombe.diff);
            close(
                "diff_lo",
                entry["diff_lo"].as_f64().expect("lo"),
                newcombe.lo,
            );
            close(
                "diff_hi",
                entry["diff_hi"].as_f64().expect("hi"),
                newcombe.hi,
            );
            raw.push(fisher.p);
            adjusted.push(entry["p_holm"].as_f64().expect("p_holm"));
        }
    }
    assert_eq!(raw.len(), 6);
    // The step-down adjustment, written out here: sort ascending, multiply by (m - rank), keep the
    // running maximum, cap at 1. The entries came out in pair order, which is the order of `raw`.
    let m = raw.len();
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&i, &j| raw[i].total_cmp(&raw[j]).then(i.cmp(&j)));
    let mut want = vec![0.0_f64; m];
    let mut running = 0.0_f64;
    for (rank, &at) in order.iter().enumerate() {
        running = running.max(f64::from(u32::try_from(m - rank).expect("small")) * raw[at]);
        want[at] = running.min(1.0);
    }
    for (at, got) in adjusted.iter().enumerate() {
        close("p_holm", *got, want[at]);
    }
}

/// The records of a CSV table as RFC 4180 writes them: fields separated by commas, a field in
/// double quotes when it holds a comma, a quote (doubled) or a line break, records ended CRLF.
fn table(text: &str) -> Vec<Vec<String>> {
    assert!(text.ends_with("\r\n"), "RFC 4180: every record ends CRLF");
    let mut records = Vec::new();
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            (true, '"') => quoted = false,
            (false, '"') => quoted = true,
            (false, ',') => fields.push(std::mem::take(&mut field)),
            (false, '\r') => {
                assert_eq!(chars.next(), Some('\n'), "a bare CR");
                fields.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut fields));
            }
            (_, other) => field.push(other),
        }
    }
    records
}

fn csv(home: &Path, store: &Path, args: &[&str]) -> Vec<Vec<String>> {
    let mut all = vec!["--format", "csv"];
    all.extend_from_slice(args);
    let out = analyze(home, store, &all);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    table(&String::from_utf8(out.stdout).expect("UTF-8"))
}

/// Guards: A1 — the three CSV tables equal the hand counts: `arms` has one row per cell and
/// expectation with the header of the leaf, `comparisons` one row per pair and expectation, and
/// `runs` one row per run with a column for every recorded path, empty where a run records none.
/// Oracle: the hand counts written in the leaf and the vector rows of the second engine.
/// Bite: drop a column, swap `num` and `den`, or list the invalid runs in the arm's counts, and a
/// cell of a row turns red.
#[test]
fn a1_the_csv_tables_equal_the_hand_counts() {
    let runs = layout(&[EVALUATOR], true);
    let (dir, store) = fixture(&runs);
    let arms = csv(dir.path(), &store, &[]);
    assert_eq!(
        arms[0].join(","),
        "cell,experiment,role,expectation,pass,fail,blocked,not_measured,not_applicable,zero_weight,num,den,wilson_lo,wilson_hi,omitted"
    );
    assert_eq!(arms.len(), 1 + 9, "three cells of three expectations");
    let alpha = cell_of(&runs, "alpha");
    let find = |id: &str| {
        arms.iter()
            .find(|r| r[0] == alpha && r[3] == id)
            .expect("a row")
            .clone()
    };
    let e1 = find("E1");
    assert_eq!(
        e1[2..12],
        ["gate", "E1", "6", "2", "2", "0", "0", "0", "6", "10"].map(str::to_owned)
    );
    let row = wilson_row(6, 10);
    close("lo", e1[12].parse().expect("lo"), row.lo);
    close("hi", e1[13].parse().expect("hi"), row.hi);
    assert_eq!(e1[1], digest_text('0'));
    assert_eq!(e1[14], "");
    let e2 = find("E2");
    assert_eq!(e2[4..10], ["3", "2", "0", "5", "0", "0"].map(str::to_owned));
    let e3 = find("E3");
    assert_eq!(
        e3[10..],
        ["44", "50", "", "", "weighted"].map(str::to_owned)
    );
    // The comparisons table.
    let file = experiment(dir.path());
    let file_arg = file.to_str().expect("a UTF-8 path");
    let pairs = csv(
        dir.path(),
        &store,
        &["--table", "comparisons", "--experiment", file_arg],
    );
    assert_eq!(
        pairs[0].join(","),
        "a,b,factor,verdict,expectation,role,a_k,a_n,b_k,b_n,p,p_holm,reject,diff,diff_lo,diff_hi,omitted,experiment,causes"
    );
    assert_eq!(pairs.len(), 1 + 9, "three pairs of three expectations");
    let tested: Vec<&Vec<String>> = pairs[1..].iter().filter(|r| r[16].is_empty()).collect();
    assert_eq!(tested.len(), 6);
    assert_eq!(pairs[1..].iter().filter(|r| r[16] == "weighted").count(), 3);
    for r in &tested {
        assert_eq!(
            (r[2].as_str(), r[3].as_str(), r[5].as_str()),
            ("plan.model", "comparable", "gate")
        );
        let a = (r[6].parse().expect("a_k"), r[7].parse().expect("a_n"));
        let b = (r[8].parse().expect("b_k"), r[9].parse().expect("b_n"));
        let (fisher, newcombe) = pair_rows(a, b);
        close("p", r[10].parse().expect("p"), fisher.p);
        close("diff", r[13].parse().expect("diff"), newcombe.diff);
        assert!(r[11].parse::<f64>().expect("p_holm") >= fisher.p);
        assert!(r[12] == "true" || r[12] == "false");
    }
    // The summary table holds what no other row does.
    let summary = csv(dir.path(), &store, &["--table", "summary"]);
    let items: Vec<String> = summary
        .iter()
        .map(|r| format!("{}={}", r[0], r[1]))
        .collect();
    assert_eq!(
        items,
        [
            "item=value",
            &format!("evaluator={EVALUATOR}"),
            "alpha=0.05",
            "i4_violations=0",
            "pairs_not_one_factor=0",
            "pairs_compared=3",
            "family.gate=6",
        ]
    );
    // The runs table.
    let listed = csv(dir.path(), &store, &["--table", "runs"]);
    assert_eq!(listed.len(), 1 + 32);
    assert_eq!(
        listed[0][..3],
        ["run", "cell", "integrity"].map(str::to_owned)
    );
    let paths: Vec<&String> = listed[0][3..].iter().collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths.len(), listed[0].len() - 3);
    assert!(
        paths
            .iter()
            .all(|c| c.starts_with("comparable.") || c.starts_with("measured."))
    );
    assert!(listed[0].contains(&"comparable.plan.model".to_owned()));
    assert!(listed[0].contains(&"measured.provenance.eval_infra.commit".to_owned()));
    let model = listed[0]
        .iter()
        .position(|c| c == "comparable.plan.model")
        .expect("a column");
    assert!(
        listed[1..]
            .iter()
            .all(|r| ["alpha", "beta", "gamma"].contains(&r[model].as_str()))
    );
    assert!(listed[1..].iter().any(|r| r[2].contains("gateway")));
}

/// Guards: A2 — a refused pair is reported through the binary with its cause and its rates, and
/// no test: a different `eval_infra.commit`, a value no factor declares and a dirty build, each
/// planted in live manifests of one experiment file declaring `plan.model`.
/// Oracle: the paths `fixtures/comparability/verdicts.json` expects for its rows C48, C03 and
/// C45.
/// Bite: compare across the refusal, or drop the cause, and the verdict or its causes turn red.
#[test]
fn a2_a_refused_pair_is_reported_through_the_binary() {
    type Shape = fn(&mut Spec);
    let cases: [(&str, Shape, &str); 3] = [
        (
            "C48",
            |s| s.provenance = vec![("eval_infra.commit", json!("def5678"))],
            "provenance.eval_infra.commit",
        ),
        (
            "C03",
            |s| s.extra = vec![("plan.other", json!("x"))],
            "plan.other",
        ),
        (
            "C45",
            |s| s.provenance = vec![("eval_infra.dirty", json!(true))],
            "provenance.eval_infra.dirty",
        ),
    ];
    for (label, shape, cause) in cases {
        let mut seeded = Vec::new();
        for (model, odd) in [("alpha", false), ("beta", true)] {
            for replicate in 1..=5_u32 {
                let mut spec = Spec::new(model, replicate);
                // C45 plants the dirty build in both arms; the others in the second arm.
                if odd || label == "C45" {
                    shape(&mut spec);
                }
                let manifest = manifest(&spec);
                let key = manifest.run().clone();
                let pass = if odd { replicate == 1 } else { replicate <= 4 };
                let at = usize::try_from(replicate).expect("small") + if odd { 100 } else { 0 };
                let outcome = (if pass { "pass" } else { "fail" }, None, None);
                let rec = record(EVALUATOR, &key, &request(at), "E1", "gate", outcome);
                seeded.push(Seeded {
                    manifest,
                    records: vec![(EVALUATOR.to_owned(), rec)],
                });
            }
        }
        let (dir, store) = fixture(&seeded);
        let file = experiment(dir.path());
        let file_arg = file.to_str().expect("a UTF-8 path");
        let report = json_of(&analyze(dir.path(), &store, &["--experiment", file_arg]));
        let pairs = report["comparisons"].as_array().expect("comparisons");
        assert_eq!(pairs.len(), 1, "{label}");
        assert_eq!(
            pairs[0]["stratum"],
            json!({"verdict": "refused", "causes": [cause]}),
            "{label}"
        );
        assert_eq!(
            pairs[0]["entries"],
            json!([]),
            "{label}: a refused pair has no test"
        );
        assert_eq!(report["families"], json!([]), "{label}");
        // The CSV keeps the refused pair, with its verdict and cause and no statistic.
        let pairs_csv = csv(
            dir.path(),
            &store,
            &["--table", "comparisons", "--experiment", file_arg],
        );
        assert_eq!(
            pairs_csv.len(),
            2,
            "{label}: the header and the refused pair"
        );
        let held = &pairs_csv[1];
        assert_eq!(
            (held[3].as_str(), held[18].as_str()),
            ("refused", cause),
            "{label}"
        );
        assert!(
            held[4..17].iter().all(String::is_empty),
            "{label}: {held:?}"
        );
        let gates: Vec<Value> = report["arms"]
            .as_array()
            .expect("arms")
            .iter()
            .map(|a| a["rates"]["gate"].clone())
            .collect();
        assert!(
            gates.contains(&json!({"num": 4, "den": 5})),
            "{label}: {gates:?}"
        );
        assert!(
            gates.contains(&json!({"num": 1, "den": 5})),
            "{label}: {gates:?}"
        );
    }
}

/// Guards: A3 — a store with the outcomes of two evaluator versions is refused through the binary
/// with exit 2, each version listed and nothing on stdout; naming one accepts it; naming a
/// version the store does not hold is refused; the report holds the named evaluator's records
/// alone.
/// Oracle: the evaluators written into the store and the exit codes of `main`.
/// Bite: pick the first evaluator when several exist, and the mix is analysed in silence, so this
/// turns red.
#[test]
fn a3_several_versions_are_refused_through_the_binary() {
    let runs = layout(&["suite-eval@1.0.0", "suite-eval@2.0.0"], false);
    let (dir, store) = fixture(&runs);
    let out = analyze(dir.path(), &store, &[]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        out.stdout,
        Vec::<u8>::new(),
        "nothing is written for a refusal"
    );
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        stderr.contains("suite-eval@1.0.0") && stderr.contains("suite-eval@2.0.0"),
        "{stderr}"
    );
    let named = json_of(&analyze(
        dir.path(),
        &store,
        &["--evaluator", "suite-eval@2.0.0"],
    ));
    assert_eq!(named["evaluator"], "suite-eval@2.0.0");
    let alpha = cell_of(&runs, "alpha");
    assert_eq!(
        arm(&named, alpha.as_str())["rates"]["gate"],
        json!({"num": 9, "den": 15}),
        "the other version's records are not added"
    );
    let missing = analyze(dir.path(), &store, &["--evaluator", "suite-eval@3.0.0"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("suite-eval@3.0.0"));
}

/// Guards: A3 — a bare version is accepted through the binary when one evaluator holds it, and
/// refused with every holder named when two evaluators do.
/// Oracle: the evaluators written into the stores.
/// Bite: take the first holder of a bare version, and the ambiguous store is analysed.
#[test]
fn a3_a_bare_version_is_accepted_through_the_binary() {
    let runs = layout(&["a-eval@1.0.0", "b-eval@2.0.0"], false);
    let (dir, store) = fixture(&runs);
    let accepted = json_of(&analyze(dir.path(), &store, &["--evaluator", "2.0.0"]));
    assert_eq!(accepted["evaluator"], "b-eval@2.0.0");
    let runs = layout(&["a-eval@1.0.0", "b-eval@2.0.0", "c-eval@2.0.0"], false);
    let (dir, store) = fixture(&runs);
    let out = analyze(dir.path(), &store, &["--evaluator", "2.0.0"]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        stderr.contains("b-eval@2.0.0") && stderr.contains("c-eval@2.0.0"),
        "{stderr}"
    );
    assert_eq!(out.stdout, Vec::<u8>::new());
}

/// The one record on stderr of a refused call under `--error-format json`.
fn record_of(out: &Output) -> Value {
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        out.stdout,
        Vec::<u8>::new(),
        "nothing is written for a refusal"
    );
    let text = String::from_utf8(out.stderr.clone()).expect("UTF-8");
    assert_eq!(text.lines().count(), 1, "one record on one line: {text}");
    serde_json::from_str(text.trim_end()).expect("the record is JSON")
}

/// Guards: R2 — a store with two evaluator versions prints as one `refusal/1` record under
/// `--error-format json`, with the code `analysis.mixed_evaluator_versions`, the side `config`
/// (the fix is a setting of the command line, `--evaluator`) and both versions in the message.
/// Oracle: a record written by hand from the contract.
/// Bite: route the refusal through `CliError::refused`, and the record's code becomes
/// `candler.internal`; give the code the side `suite`, and the side turns red.
#[test]
fn r2_a_mixed_store_prints_as_a_record() {
    let runs = layout(&["suite-eval@1.0.0", "suite-eval@2.0.0"], false);
    let (dir, store) = fixture(&runs);
    let out = analyze(dir.path(), &store, &["--error-format", "json"]);
    let record = record_of(&out);
    assert_eq!(record["schema"], "refusal/1", "{record}");
    assert_eq!(
        record["code"], "analysis.mixed_evaluator_versions",
        "{record}"
    );
    assert_eq!(record["side"], "config", "{record}");
    let message = record["message"].as_str().expect("a message");
    assert!(message.starts_with("refused: "), "{message}");
    assert!(message.contains("suite-eval@1.0.0") && message.contains("suite-eval@2.0.0"));
}

/// Guards: R2 — an experiment file that cannot be read prints as the record of
/// `analysis.experiment_unreadable`, side `experiment`, naming the file because no pointer into a
/// document exists.
/// Oracle: a record written by hand from the contract.
/// Bite: leave the file out of the record, and this turns red.
#[test]
fn r2_an_unreadable_experiment_names_the_file() {
    let runs = layout(&[EVALUATOR], false);
    let (dir, store) = fixture(&runs);
    let file = dir.path().join("broken.toml");
    fs::write(&file, "replicates = [\n").expect("the file writes");
    let file_arg = file.to_str().expect("a UTF-8 path");
    let out = analyze(
        dir.path(),
        &store,
        &["--error-format", "json", "--experiment", file_arg],
    );
    let record = record_of(&out);
    assert_eq!(record["code"], "analysis.experiment_unreadable", "{record}");
    assert_eq!(record["side"], "experiment", "{record}");
    assert_eq!(record["file"], json!(file_arg), "{record}");
    assert!(record.get("path").is_none(), "no path: {record}");
}

/// The cells of a 2 by 2 store, `plan.model` by `plan.other`, two runs each, whose manifests record
/// the factors given.
fn two_by_two(factors: &[&'static str]) -> Vec<Seeded> {
    let mut seeded = Vec::new();
    let mut n = 0_usize;
    for (model, other) in [("alpha", "x"), ("alpha", "y"), ("beta", "x"), ("beta", "y")] {
        for replicate in 1..=2_u32 {
            let mut spec = Spec::new(model, replicate);
            spec.extra = vec![("plan.other", json!(other))];
            spec.factors = factors.to_vec();
            let manifest = manifest(&spec);
            n += 1;
            let key = manifest.run().clone();
            let rec = record(
                EVALUATOR,
                &key,
                &request(n),
                "E1",
                "gate",
                ("pass", None, None),
            );
            seeded.push(Seeded {
                manifest,
                records: vec![(EVALUATOR.to_owned(), rec)],
            });
        }
    }
    seeded
}

/// Guards: A2 — through the binary, a 2 by 2 store compares the four pairs that differ in one
/// factor and counts the two that differ in two, from the factors its manifests record; a file
/// that declares the parent path `plan` or an unrelated path is refused with the record of
/// `analysis.experiment_disagrees` (side `experiment`); the file with the recorded factors is
/// accepted; and manifests that record the parent `plan` are refused with the record of
/// `analysis.factor_broader_than_variation`.
/// Oracle: the factors written into the manifests and the files by the test.
/// Bite: skip the cross-check, and the wrong files run; skip the prefix check, and the broad
/// record runs.
#[test]
fn a2_a_wrong_experiment_is_refused_through_the_binary() {
    let (dir, store) = fixture(&two_by_two(&["plan.model", "plan.other"]));
    let report = json_of(&analyze(dir.path(), &store, &[]));
    assert_eq!(report["comparisons"].as_array().expect("pairs").len(), 4);
    assert_eq!(report["pairs_not_one_factor"], 2);
    let right = experiment_of(
        dir.path(),
        &[
            ("plan.model", &["alpha", "beta"]),
            ("plan.other", &["x", "y"]),
        ],
    );
    let right_arg = right.to_str().expect("a UTF-8 path");
    json_of(&analyze(dir.path(), &store, &["--experiment", right_arg]));
    for (label, wrong) in [
        (
            "parent",
            experiment_of(dir.path(), &[("plan", &["a", "b"])]),
        ),
        (
            "unrelated",
            experiment_of(dir.path(), &[("plan.zzz", &["a", "b"])]),
        ),
    ] {
        let wrong_arg = wrong.to_str().expect("a UTF-8 path");
        let out = analyze(
            dir.path(),
            &store,
            &["--error-format", "json", "--experiment", wrong_arg],
        );
        let record = record_of(&out);
        assert_eq!(
            record["code"], "analysis.experiment_disagrees",
            "{label}: {record}"
        );
        assert_eq!(record["side"], "experiment", "{label}: {record}");
    }
    let (dir, store) = fixture(&two_by_two(&["plan"]));
    let out = analyze(dir.path(), &store, &["--error-format", "json"]);
    let record = record_of(&out);
    assert_eq!(
        record["code"], "analysis.factor_broader_than_variation",
        "{record}"
    );
}

/// Guards: A1 — a cell in two strata is two arms through the binary: each arm names its
/// experiment digest, the counts are per stratum, there is one comparison per stratum and one
/// Holm family of two tests.
/// Oracle: the runs written into the store, three per cell and stratum.
/// Bite: key the tally by cell alone, and the arms pool six runs.
#[test]
fn a1_a_cell_in_two_strata_is_two_arms() {
    let mut seeded = Vec::new();
    let mut n = 0_usize;
    for (at, experiment) in ['0', '1'].into_iter().enumerate() {
        for (model, status) in [("alpha", "pass"), ("beta", "fail")] {
            for replicate in 1..=3_u32 {
                let mut spec = Spec::new(model, 3 * u32::try_from(at).expect("small") + replicate);
                spec.experiment = experiment;
                let manifest = manifest(&spec);
                n += 1;
                let key = manifest.run().clone();
                let rec = record(
                    EVALUATOR,
                    &key,
                    &request(n),
                    "E1",
                    "gate",
                    (status, None, None),
                );
                seeded.push(Seeded {
                    manifest,
                    records: vec![(EVALUATOR.to_owned(), rec)],
                });
            }
        }
    }
    let (dir, store) = fixture(&seeded);
    let report = json_of(&analyze(dir.path(), &store, &[]));
    let arms = report["arms"].as_array().expect("arms");
    assert_eq!(arms.len(), 4);
    for held in arms {
        assert_eq!(held["runs"]["valid"], 3, "{held}");
        let den = held["rates"]["gate"]["den"].as_u64().expect("den");
        assert_eq!(den, 3, "{held}");
    }
    let pairs = report["comparisons"].as_array().expect("pairs");
    assert_eq!(pairs.len(), 2);
    let experiments: Vec<&Value> = pairs.iter().map(|p| &p["experiment"]).collect();
    assert_eq!(
        experiments,
        [&json!(digest_text('0')), &json!(digest_text('1'))]
    );
    assert_eq!(report["families"], json!([{"role": "gate", "m": 2}]));
}

/// Guards: A1 — a run without a seal does not fail the call through the binary: with the layout
/// and two unsealed runs, one whose manifest was written and one with none, the call exits 0, the
/// arm of the first shows `runs.unsealed` 1, a stratum `unknown` holds the second, and the counts
/// of the sealed runs are the hand counts.
/// Oracle: the layout's hand counts.
/// Bite: list only the sealed runs while the unsealed one has records, and the call fails with a
/// run that has records and no listing.
#[test]
fn a1_an_unsealed_run_does_not_fail_the_call() {
    let runs = layout(&[EVALUATOR], false);
    let (dir, store) = fixture(&runs);
    let opened = Store::open(&store).expect("the store opens");
    let with_manifest = manifest(&Spec::new("alpha", 50));
    let mut writer =
        RunWriter::create(&opened, &with_manifest, Existing::Refuse).expect("the run starts");
    writer
        .manifest(&with_manifest)
        .expect("the manifest writes");
    drop(writer);
    let without = manifest(&Spec::new("gamma", 51));
    drop(RunWriter::create(&opened, &without, Existing::Refuse).expect("the run starts"));
    fs::remove_file(opened.layout().manifest(without.run())).expect("the manifest is removed");
    let report = json_of(&analyze(dir.path(), &store, &[]));
    let alpha = cell_of(&runs, "alpha");
    let held = arm(&report, alpha.as_str());
    assert_eq!(held["runs"]["unsealed"], 1);
    assert_eq!(held["rates"]["gate"], json!({"num": 9, "den": 15}));
    let unknown = report["arms"]
        .as_array()
        .expect("arms")
        .iter()
        .find(|a| a["experiment"] == "unknown")
        .expect("the unknown stratum");
    assert_eq!(unknown["runs"]["unsealed"], 1);
    assert_eq!(unknown["runs"]["valid"], 0);
}
