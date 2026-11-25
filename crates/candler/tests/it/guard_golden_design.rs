#![cfg(test)]
//! Guards for the HTN golden experiment's design: 3 models by 4 htn-creator versions by 20 runs is
//! one experiment file that plans 12 cells, and `candler analyze` over a store of that shape gives
//! 6 version comparisons blocked by model and 3 model comparisons blocked by version, each equal
//! to `oracle/golden_design/expected.toml`, which `second_engine.py` writes from scipy and
//! statsmodels alone. No golden run is read; the design is under test, not the numbers.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use candler_core::resolve::layers::Resolution;
use candler_core::runner::plan::{Force, Levels, PlanCtx, PlanMode, plan_runs};
use candler_core::store::Store;
use candler_core::store::outcome::OutcomeWriter;
use candler_core::store::run::{Existing, RunWriter};
use candler_schemas::execution::{ExperimentFile, RunManifest};
use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::{NameVersion, RequestId, RunKey};
use serde::Deserialize;
use serde_json::{Value, json};
use tempfile::TempDir;

const EVALUATOR: &str = "suite-eval@1.0.0";
const TOLERANCE: f64 = 1e-9;

fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

fn oracle_path(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/oracle/golden_design")
        .join(file)
}

#[derive(Deserialize)]
struct Cell {
    m: usize,
    v: usize,
    g1: u32,
    g2: u32,
    q1: u32,
    w: u32,
}

#[derive(Deserialize)]
struct Design {
    replicates: u32,
    weight_den: u32,
    weight_full: u32,
    weight_low: u32,
    models: Vec<String>,
    treatment_name: String,
    tags: Vec<String>,
    commits: Vec<String>,
    cell: Vec<Cell>,
}

fn design() -> Design {
    let text = fs::read_to_string(oracle_path("design.toml")).expect("design.toml reads");
    toml::from_str(&text).expect("design.toml parses")
}

#[derive(Deserialize)]
struct Block {
    key: String,
    a_k: Option<u64>,
    a_n: Option<u64>,
    b_k: Option<u64>,
    b_n: Option<u64>,
    diff: f64,
}

#[derive(Deserialize)]
struct Row {
    factor: String,
    a: String,
    b: String,
    expectation: String,
    role: String,
    battery: String,
    blocks: u64,
    fallback: Option<bool>,
    p: Option<f64>,
    p_holm: Option<f64>,
    or_mh: Option<f64>,
    or_lo: Option<f64>,
    or_hi: Option<f64>,
    reading: Option<String>,
    diff: Option<f64>,
    per_block: Vec<Block>,
}

#[derive(Deserialize)]
struct Expected {
    row: Vec<Row>,
}

fn expected() -> Vec<Row> {
    let text = fs::read_to_string(oracle_path("expected.toml")).expect("expected.toml reads");
    toml::from_str::<Expected>(&text)
        .expect("expected.toml parses")
        .row
}

/// The resolution of one treatment level: the harness, the image and the level's record.
fn level_resolution(design: &Design, version: usize) -> Resolution {
    serde_json::from_value(json!({
        "values": [
            {"path": "harness", "value": {"name": "claude-code", "version": "2.1.0"}},
            {"path": "isolation.image", "value": digest_text('a')},
            {"path": "treatment", "value": {
                "name": design.treatment_name,
                "tag": design.tags.get(version),
                "commit": design.commits.get(version),
            }},
        ],
        "settings": [],
        "sources": [],
        "preflight": [],
    }))
    .expect("a resolution")
}

/// Guards: the golden design is one experiment file: 3 `model.id` levels by 4 `request.treatment`
/// levels at 20 replicates plan 12 cells and 240 runs, each (model, version) pair one cell holding
/// 20 runs, and each cell's treatment record is its own level's.
/// Oracle: the arithmetic 3 x 4 x 20 and the tags and commits written in `design.toml`.
/// Bite: fold every cell to the first treatment level, or drop one table level from the count.
#[test]
fn gd_plan_twelve_cells() {
    let design = design();
    let text = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/golden_design/experiment.toml"),
    )
    .expect("the experiment file reads");
    let file = ExperimentFile::from_toml_str(&text).expect("an experiment file");
    let levels: Vec<Resolution> = (0..design.tags.len())
        .map(|v| level_resolution(&design, v))
        .collect();
    let dir = TempDir::new().expect("a temporary directory");
    let store = Store::init(&dir.path().join("store")).expect("a store");
    let ctx = PlanCtx {
        store,
        experiment_lock: dir.path().join("experiments").join("x").join("lock"),
        secrets_present: BTreeSet::new(),
        driver: None,
    };
    let out = plan_runs(
        &ctx,
        &file,
        Levels::from(&levels[..]),
        Force::No,
        PlanMode::Check,
    )
    .expect("a plan");
    assert_eq!(out.plans.len(), 240);
    let mut runs: BTreeMap<(String, String), usize> = BTreeMap::new();
    for plan in &out.plans {
        let cell = out.fresh.get(&plan.cell).expect("the plan's cell");
        let model = cell
            .values()
            .iter()
            .find(|held| held.path().as_str() == "model.id")
            .and_then(|held| held.value())
            .map(|value| value.as_value().clone()) // BORROW: the cell owns the level
            .expect("a model");
        let treatment = serde_json::to_value(cell.treatment()).expect("JSON");
        let tag = treatment["tag"].as_str().expect("a tag").to_owned();
        let at = design
            .tags
            .iter()
            .position(|t| *t == tag)
            .expect("a known tag");
        assert_eq!(
            treatment["commit"],
            json!(design.commits.get(at)),
            "the level's own record"
        );
        let key = (model.as_str().expect("a model id").to_owned(), tag);
        *runs.entry(key).or_default() += 1;
    }
    assert_eq!(runs.len(), 12, "one cell per (model, version) pair");
    assert!(runs.values().all(|count| *count == 20), "{runs:?}");
    let models: BTreeSet<&String> = runs.keys().map(|(m, _)| m).collect();
    assert_eq!(models.len(), 3);
}

fn request(n: usize) -> RequestId {
    format!("01926b3a-7c3e-7a1b-8c4d-{n:012x}")
        .parse()
        .expect("a request id")
}

/// What one fabricated run records: the values of its cell, the factors it declares, its
/// treatment record and its replicate.
struct Spec {
    values: Vec<(&'static str, Value)>,
    factors: Vec<&'static str>,
    treatment: Value,
    replicate: u32,
}

fn manifest(spec: &Spec) -> RunManifest {
    let mut values: Vec<Value> = spec
        .values
        .iter()
        .map(|(path, value)| json!({"path": path, "value": value}))
        .collect();
    values.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    let mut sources: Vec<Value> = values
        .iter()
        .map(|v| json!({"path": v["path"], "source": {"kind": "experiment_file"}}))
        .collect();
    sources.push(json!({"path": "harness", "source": {"kind": "experiment_file"}}));
    sources.push(json!({"path": "treatment", "source": {"kind": "experiment_file"}}));
    let measured: Vec<Value> = [
        ("provenance.eval_infra.commit", json!("abc1234")),
        ("provenance.eval_infra.dirty", json!(false)),
        ("provenance.sandbox_contract", json!("candler-container@1")),
    ]
    .into_iter()
    .map(|(path, value)| json!({"path": path, "value": value, "source": {"kind": "experiment_file"}}))
    .collect();
    let body = |cell: &str| {
        json!({
            "schema": "run-manifest/1",
            "run": {"cell": cell, "replicate": spec.replicate, "attempt": 1},
            "experiment": digest_text('0'),
            "factors": spec.factors,
            "comparable": {
                "values": values,
                "harness": {"name": "h", "version": "1.0.0"},
                "treatment": spec.treatment,
                "snapshots": [],
                "inputs": [],
                "environments": []
            },
            "sources": sources,
            "secrets": [],
            "measured": measured,
            "deviations": [],
            "integrity": {"kind": "valid"}
        })
    };
    let probe: RunManifest = serde_json::from_value(body(&digest_text('f'))).expect("a manifest");
    let cell = probe.cell_digest().to_string();
    serde_json::from_value(body(cell.as_str())).expect("a manifest") // BORROW: explicit .as_str()
}

fn record(
    run: &RunKey,
    request: &RequestId,
    expectation: &str,
    role: &str,
    status: &str,
    weight: Option<(u32, u32)>,
) -> OutcomeRecord {
    let mut doc = json!({
        "run": {"cell": run.cell().to_string(), "replicate": run.replicate(), "attempt": run.attempt()},
        "expectation_id": expectation,
        "role": role,
        "evaluator": EVALUATOR,
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
    serde_json::from_value(doc).expect("an outcome record")
}

/// A run to store: its manifest and the records of the one evaluator.
struct Seeded {
    manifest: RunManifest,
    records: Vec<OutcomeRecord>,
}

fn status(passed: bool) -> &'static str {
    if passed { "pass" } else { "fail" }
}

/// The store of the design's shape: in every cell the first `k` replicates of an expectation pass.
fn golden_runs(design: &Design) -> Vec<Seeded> {
    let mut seeded = Vec::new();
    let mut n = 0_usize;
    for cell in &design.cell {
        let model = design.models.get(cell.m).expect("a model");
        let treatment = json!({
            "name": design.treatment_name,
            "tag": design.tags.get(cell.v),
            "commit": design.commits.get(cell.v),
        });
        for replicate in 1..=design.replicates {
            let manifest = manifest(&Spec {
                values: vec![("model.id", json!(model))],
                factors: vec!["model.id", "request.treatment"],
                treatment: treatment.clone(), // BORROW: each run owns its record
                replicate,
            });
            let key = manifest.run().clone();
            n += 1;
            let id = request(n);
            let full = replicate <= cell.w;
            let weight = (
                if full {
                    design.weight_full
                } else {
                    design.weight_low
                },
                design.weight_den,
            );
            let records = vec![
                record(&key, &id, "G1", "gate", status(replicate <= cell.g1), None),
                record(&key, &id, "G2", "gate", status(replicate <= cell.g2), None),
                record(
                    &key,
                    &id,
                    "Q1",
                    "quality",
                    status(replicate <= cell.q1),
                    None,
                ),
                record(&key, &id, "Q2", "quality", status(full), Some(weight)),
            ];
            seeded.push(Seeded { manifest, records });
        }
    }
    seeded
}

fn seed(root: &Path, runs: &[Seeded]) {
    let store = Store::init(root).expect("the store initialises");
    for run in runs {
        RunWriter::create(&store, &run.manifest, Existing::Refuse)
            .expect("the run starts")
            .done()
            .expect("the run seals");
        let name: NameVersion = EVALUATOR.parse().expect("name@version");
        let first = run.records.first().expect("a record");
        let id = first.request_id().clone();
        let mut writer =
            OutcomeWriter::create(&store, run.manifest.run(), &name, &id).expect("a writer");
        for held in &run.records {
            writer.append(held).expect("a record appends");
        }
        writer.commit().expect("the outcomes commit");
    }
}

fn analyze(home: &Path, store: &Path) -> Value {
    let out: Output = Command::new(env!("CARGO_BIN_EXE_candler"))
        .current_dir(home)
        .env_remove("CANDLER_STORE")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .arg("--store")
        .arg(store)
        .arg("analyze")
        .output()
        .expect("the binary runs");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("the output is JSON")
}

fn fixture(runs: &[Seeded]) -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("a temporary directory");
    let store = dir.path().join("store");
    seed(&store, runs);
    (dir, store)
}

/// The report of `analyze` over the golden store.
fn golden_report() -> Value {
    let (dir, store) = fixture(&golden_runs(&design()));
    analyze(dir.path(), &store)
}

/// A level as the expected file names it: a treatment by its tag, a model by its id.
fn label(level: &Value) -> String {
    level
        .get("tag")
        .unwrap_or(level)
        .as_str()
        .expect("a label")
        .to_owned()
}

/// Guards: the golden shape gives 6 comparisons of `request.treatment` with 3 blocks each (the
/// models) and 3 of `model.id` with 4 blocks each (the versions), every entry blocked by the
/// other factor and never by the compared one.
/// Oracle: the count of pairs of 4 and of 3 levels, and the levels of the other factor.
/// Bite: take the blocks from every declared factor, the compared one included.
#[test]
fn gd_comparisons_shape() {
    let report = golden_report();
    let comparisons = report["comparisons"].as_array().expect("comparisons");
    assert_eq!(comparisons.len(), 9);
    for (factor, pairs, blocks, other) in [
        ("request.treatment", 6, 3, "model.id"),
        ("model.id", 3, 4, "request.treatment"),
    ] {
        let ours: Vec<&Value> = comparisons
            .iter()
            .filter(|c| c["factor"] == factor)
            .collect();
        assert_eq!(ours.len(), pairs, "{factor}");
        for comparison in ours {
            let entries = comparison["entries"].as_array().expect("entries");
            assert_eq!(entries.len(), 4);
            for entry in entries {
                assert_eq!(entry["blocks"], blocks, "{factor}");
                for block in entry["per_block"].as_array().expect("per_block") {
                    assert_eq!(block["key"].as_array().map(Vec::len), Some(1));
                    assert_eq!(block["key"][0]["factor"], other);
                }
            }
        }
    }
}

/// Whether the comparison is of the two levels named, in either order.
fn same_pair(comparison: &Value, a: &str, b: &str) -> bool {
    let (x, y) = (
        label(&comparison["levels"][0]),
        label(&comparison["levels"][1]),
    );
    (x == a && y == b) || (x == b && y == a)
}

/// A comparison by its factor and its two levels.
type Pair = (String, String, String);

fn close(held: Option<f64>, wanted: Option<f64>, what: &str) {
    match (held, wanted) {
        (Some(h), Some(w)) => assert!((h - w).abs() <= TOLERANCE, "{what}: {h} against {w}"),
        (None, None) => {}
        _ => panic!("{what}: {held:?} against {wanted:?}"),
    }
}

/// Holm over two p-values: the adjusted value of each, in the order given.
fn holm_two(p: [f64; 2]) -> [f64; 2] {
    let first_is_low = p[0] <= p[1];
    let (low, high) = if first_is_low {
        (p[0], p[1])
    } else {
        (p[1], p[0])
    };
    let adjusted_low = (2.0 * low).min(1.0);
    let adjusted_high = adjusted_low.max(high).min(1.0);
    if first_is_low {
        [adjusted_low, adjusted_high]
    } else {
        [adjusted_high, adjusted_low]
    }
}

/// Guards: A8 -- over the golden shape, each entry's battery, p, odds ratio and interval,
/// per-block counts and differences, reading and Holm (per comparison, over the gates) equal the
/// second engine within 1e-9; the weighted entry equals it in battery, blocks, per-block
/// differences and point estimate, and the quality family's Holm equals the exact adjustment of
/// the engine's binary p and the report's own weighted p.
/// Oracle: `oracle/golden_design/expected.toml` from `second_engine.py` (scipy hypergeometric,
/// statsmodels `StratifiedTable`, Mantel-Fleiss, Holm).
/// Bite: test the pooled table instead of the blocks, or turn the continuity correction on.
#[test]
fn gd_agrees_with_second_engine() {
    let report = golden_report();
    let comparisons = report["comparisons"].as_array().expect("comparisons");
    let rows = expected();
    assert_eq!(rows.len(), 36);
    let mut quality: BTreeMap<Pair, Vec<(String, f64, f64)>> = BTreeMap::new();
    for row in &rows {
        let comparison = comparisons
            .iter()
            .find(|c| c["factor"] == row.factor && same_pair(c, &row.a, &row.b))
            .unwrap_or_else(|| panic!("a comparison of {} {} {}", row.factor, row.a, row.b));
        // The report orders a pair as its store does; the expected rows read A to B in declared
        // order, so a reversed pair flips the sign of every difference and inverts the odds ratio.
        let reversed = label(&comparison["levels"][0]) == row.b;
        let sign = if reversed { -1.0 } else { 1.0 };
        let id = match row.expectation.as_str() {
            "w" => "Q2".to_owned(),
            other => other.to_uppercase(),
        };
        let entry = comparison["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .find(|e| e["id"] == id)
            .unwrap_or_else(|| panic!("an entry {id}: {}", comparison["entries"]));
        let what = format!("{} {} {} {}", row.factor, row.a, row.b, id);
        assert_eq!(entry["role"], row.role, "{what}");
        assert_eq!(entry["battery"], row.battery, "{what}");
        assert_eq!(entry["blocks"], row.blocks, "{what}");
        let blocks = entry["per_block"].as_array().expect("per_block");
        assert_eq!(blocks.len(), row.per_block.len(), "{what}");
        for wanted in &row.per_block {
            let held = blocks
                .iter()
                .find(|b| label(&b["key"][0]["level"]) == wanted.key)
                .unwrap_or_else(|| panic!("{what}: block {}", wanted.key));
            close(
                held["diff"].as_f64(),
                Some(sign * wanted.diff),
                &format!("{what} diff of {}", wanted.key),
            );
            if let (Some(ak), Some(an), Some(bk), Some(bn)) =
                (wanted.a_k, wanted.a_n, wanted.b_k, wanted.b_n)
            {
                let (first, second) = if reversed {
                    (json!({"k": bk, "n": bn}), json!({"k": ak, "n": an}))
                } else {
                    (json!({"k": ak, "n": an}), json!({"k": bk, "n": bn}))
                };
                assert_eq!(held["a"], first, "{what}");
                assert_eq!(held["b"], second, "{what}");
            }
        }
        if row.battery == "permutation" {
            close(
                entry["diff"].as_f64(),
                row.diff.map(|d| sign * d),
                &format!("{what} point estimate"),
            );
        } else {
            assert_eq!(
                Some(entry["fallback"].as_bool().expect("a flag")),
                row.fallback,
                "{what}"
            );
            close(entry["p"].as_f64(), row.p, &format!("{what} p"));
            let odds = |x: Option<f64>| x.map(|v| if reversed { 1.0 / v } else { v });
            let (lo, hi) = if reversed {
                (row.or_hi, row.or_lo)
            } else {
                (row.or_lo, row.or_hi)
            };
            close(
                entry["or_mh"].as_f64(),
                odds(row.or_mh),
                &format!("{what} or_mh"),
            );
            close(entry["or_lo"].as_f64(), odds(lo), &format!("{what} or_lo"));
            close(entry["or_hi"].as_f64(), odds(hi), &format!("{what} or_hi"));
            assert_eq!(
                Some(entry["interval_reading"].as_str().expect("a reading")),
                row.reading.as_deref(),
                "{what}"
            );
        }
        if row.role == "gate" {
            close(
                entry["p_holm"].as_f64(),
                row.p_holm,
                &format!("{what} holm"),
            );
        } else {
            let p = entry["p"].as_f64().expect("a p");
            let held_p = if row.battery == "permutation" {
                p
            } else {
                row.p.expect("a p")
            };
            let adjusted = entry["p_holm"].as_f64().expect("a p_holm");
            quality
                .entry((row.factor.clone(), row.a.clone(), row.b.clone()))
                .or_default()
                .push((id, held_p, adjusted));
        }
    }
    assert_eq!(quality.len(), 9);
    for (pair, family) in quality {
        let [(id_x, px, hx), (id_y, py, hy)] = family.as_slice() else {
            panic!("two quality entries: {pair:?}");
        };
        let [wx, wy] = holm_two([*px, *py]);
        assert!(
            (hx - wx).abs() <= TOLERANCE,
            "{pair:?} {id_x}: {hx} against {wx}"
        );
        assert!(
            (hy - wy).abs() <= TOLERANCE,
            "{pair:?} {id_y}: {hy} against {wy}"
        );
    }
}

/// Guards: A8 -- on gate G1 the planted pair (versions 0.4.0 and 0.4.1, 5/20 and 6/20 rising to
/// 17/20 and 16/20 in 2 of 3 models) reads "effect", and the null pair (versions 0.4.2 and
/// 0.4.3, equal counts everywhere) reads "inconclusive" with p = 1.
/// Oracle: the planted counts of `design.toml`, not the report.
/// Bite: blocks that mix the versions, which pool the planted effect away from the null pair, or a
/// `level_of` that does not read the typed treatment, which gives the versions no arms.
#[test]
fn gd_readings() {
    let report = golden_report();
    let comparisons = report["comparisons"].as_array().expect("comparisons");
    let g1 = |a: &str, b: &str| -> Value {
        comparisons
            .iter()
            .filter(|c| c["factor"] == "request.treatment")
            .find(|c| same_pair(c, a, b))
            .and_then(|c| c["entries"].as_array()?.iter().find(|e| e["id"] == "G1"))
            .expect("the G1 entry")
            .clone() // BORROW: the report owns the entry
    };
    assert_eq!(g1("0.4.0", "0.4.1")["interval_reading"], "effect");
    let null = g1("0.4.2", "0.4.3");
    assert_eq!(null["interval_reading"], "inconclusive");
    assert!((null["p"].as_f64().expect("a p") - 1.0).abs() <= TOLERANCE);
}
