#![cfg(test)]
//! Guards for the report of `analyze`, over a fabricated store that lives in memory: a `Selection`
//! of hand-written records, and run manifests built from JSON. Every count is a hand count, and
//! every interval, p-value and difference is a row of the vector files `tests/vectors/stats/*.toml`
//! that scipy and statsmodels wrote, so no expected number comes from the code under test.

use std::collections::BTreeMap;
use std::path::PathBuf;

use candler_core::analysis::refusal::ReportError;
use candler_core::analysis::report::{
    Entry, OmitReason, PairVerdict, Report, ReportInput, RunView, Unsealed, check_factors,
    choose_evaluator,
};
use candler_core::analysis::tally::{Omitted, RunFacts};
use candler_core::evaluation::selection::select::{Selection, select};
use candler_schemas::execution::RunManifest;
use candler_schemas::outcome::{OutcomeRecord, Role};
use candler_schemas::primitives::{FieldPath, NameVersion};
use candler_schemas::refusal::Refusal as _;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

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

fn view(spec: &Spec) -> RunView {
    let manifest = manifest(spec);
    RunView {
        facts: RunFacts::new(manifest.run().clone(), manifest.integrity().clone()),
        manifest,
    }
}

/// One outcome record. `weight` is `(num, den)`, absent for the default of the status; `cause` is
/// the reason code of a `blocked`, `not_measured` or `not_applicable`.
fn record(
    run: &RunView,
    n: usize,
    expectation: &str,
    role: &str,
    status: &str,
    weight: Option<(u32, u32)>,
    cause: Option<&str>,
) -> OutcomeRecord {
    record_by(EVALUATOR, run, n, expectation, role, status, weight, cause)
}

/// One outcome record of `evaluator`.
#[expect(clippy::too_many_arguments, reason = "a record is its fields")]
fn record_by(
    evaluator: &str,
    run: &RunView,
    n: usize,
    expectation: &str,
    role: &str,
    status: &str,
    weight: Option<(u32, u32)>,
    cause: Option<&str>,
) -> OutcomeRecord {
    let key = run.manifest.run();
    let mut doc = json!({
        "run": {"cell": key.cell().to_string(), "replicate": key.replicate(), "attempt": key.attempt()},
        "expectation_id": expectation,
        "role": role,
        "evaluator": evaluator,
        "instruments": [],
        "suite_entry_digest": digest_text('1'),
        "context_digest": digest_text('2'),
        "request_id": format!("01926b3a-7c3e-7a1b-8c4d-{n:012x}"),
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

/// The rows of one cell for `E1`, `E2` and `E3` over runs 1 to 10, as the layout of the leaf lists
/// them.
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

/// The fabricated store of the leaf: three cells of ten valid runs, and for `alpha` two invalid
/// runs that each carry a pass on `E1` and `E2`.
fn layout(with_invalid: bool) -> (Selection, Vec<RunView>) {
    let mut runs: Vec<RunView> = Vec::new();
    let mut records: Vec<OutcomeRecord> = Vec::new();
    let mut n = 0_usize;
    let mut next = || {
        n += 1;
        n
    };
    for model in ["alpha", "beta", "gamma"] {
        let (e1, e2) = rows_of(model);
        for replicate in 1..=10_u32 {
            let run = view(&Spec::new(model, replicate));
            let at = usize::try_from(replicate).expect("small") - 1;
            let (s1, c1) = e1[at];
            let (s2, c2) = e2[at];
            records.push(record(&run, next(), "E1", "gate", s1, None, c1));
            records.push(record(&run, next(), "E2", "gate", s2, None, c2));
            let (status, weight) = match (model, replicate) {
                ("alpha", 9 | 10) => ("fail", (2, 5)),
                _ => ("pass", (5, 5)),
            };
            records.push(record(
                &run,
                next(),
                "E3",
                "quality",
                status,
                Some(weight),
                None,
            ));
            runs.push(run);
        }
    }
    if with_invalid {
        for (replicate, reason) in [(11, "gateway"), (12, "killed")] {
            let mut spec = Spec::new("alpha", replicate);
            spec.integrity = json!({"kind": "invalid", "reason": reason});
            let run = view(&spec);
            records.push(record(&run, next(), "E1", "gate", "pass", None, None));
            records.push(record(&run, next(), "E2", "gate", "pass", None, None));
            runs.push(run);
        }
    }
    (select(records), runs)
}

fn evaluator() -> NameVersion {
    EVALUATOR.parse().expect("name@version")
}

fn cell_of(runs: &[RunView], model: &str) -> String {
    runs.iter()
        .find(|v| {
            v.manifest
                .comparable()
                .values()
                .iter()
                .any(|x| x.value().is_some_and(|j| j.as_value() == &json!(model)))
        })
        .map(|v| v.manifest.run().cell().hex().to_owned())
        .expect("a run of the model")
}

fn paths(list: &[&str]) -> Vec<FieldPath> {
    list.iter()
        .map(|p| p.parse().expect("a field path"))
        .collect()
}

fn report(selection: &Selection, runs: Vec<RunView>) -> Report {
    Report::build(ReportInput {
        selection: selection.clone(),
        evaluator: evaluator(),
        runs,
        unsealed: Vec::new(),
    })
    .expect("the report builds")
}

#[derive(Deserialize)]
struct Vectors<R> {
    rows: Vec<R>,
}

fn vector<R: DeserializeOwned>(file: &str) -> Vec<R> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/vectors/stats")
        .join(file);
    let text = std::fs::read_to_string(&path).expect("the vector file reads");
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

fn fisher_p(a: (u64, u64), b: (u64, u64)) -> f64 {
    vector::<FisherRow>("fisher.toml")
        .into_iter()
        .find(|r| (r.k1, r.n1, r.k2, r.n2) == (a.0, a.1, b.0, b.1))
        .map(|r| r.p)
        .expect("a fisher vector row")
}

/// The hand counts of one arm: model, gate, quality, `E1` and `E2` as `k` and `n`, and the status
/// totals.
type Want = (
    &'static str,
    (u64, u64),
    (u64, u64),
    (u64, u64),
    (u64, u64),
    [u64; 5],
);

/// Guards: A1 — every arm's gate and quality sums, status totals, `k` and `n` and Wilson interval
/// equal the hand counts of the layout, and E3 (weights 5/5 and 2/5) is `weighted` in every arm.
/// Oracle: the hand counts written in the leaf (gate A 9/15, B 2/15, C 10/15; quality A 44/50, B
/// and C 50/50) and the rows of `wilson.toml`.
/// Bite: tally the invalid runs, or leave `blocked` out of `n`, and a count turns red.
#[test]
fn a1_arms_equal_the_hand_counts() {
    let (selection, runs) = layout(true);
    let cells: BTreeMap<&str, String> = ["alpha", "beta", "gamma"]
        .into_iter()
        .map(|m| (m, cell_of(&runs, m)))
        .collect();
    let report = report(&selection, runs);
    assert_eq!(report.arms.len(), 3);
    // (model, gate, quality, E1 k/n, E2 k/n, status pass/fail/blocked/not_measured/not_applicable
    // over gate and quality)
    let want: [Want; 3] = [
        (
            "alpha",
            (9, 15),
            (44, 50),
            (6, 10),
            (3, 5),
            [17, 6, 2, 5, 0],
        ),
        (
            "beta",
            (2, 15),
            (50, 50),
            (2, 10),
            (0, 5),
            [12, 13, 0, 0, 5],
        ),
        (
            "gamma",
            (10, 15),
            (50, 50),
            (5, 10),
            (5, 5),
            [20, 5, 0, 5, 0],
        ),
    ];
    for (model, gate, quality, e1, e2, status) in want {
        let arm = report
            .arms
            .iter()
            .find(|a| a.cell.to_string() == cells[model])
            .expect("the arm");
        assert_eq!((arm.gate.num(), arm.gate.den()), gate, "{model} gate");
        assert_eq!(
            (arm.quality.num(), arm.quality.den()),
            quality,
            "{model} quality"
        );
        let by_id: BTreeMap<String, _> = arm
            .expectations
            .iter()
            .map(|e| (e.id.to_string(), e))
            .collect();
        for (id, (k, n)) in [("E1", e1), ("E2", e2)] {
            let counts = by_id[id].tally.counts().expect("binary");
            assert_eq!((counts.k(), counts.n()), (k, n), "{model} {id}");
            let row = vector::<WilsonRow>("wilson.toml")
                .into_iter()
                .find(|r| (r.k, r.n) == (k, n))
                .expect("a wilson row");
            let interval = by_id[id].wilson.expect("an interval");
            close("wilson lo", interval.lo(), row.lo);
            close("wilson hi", interval.hi(), row.hi);
        }
        assert_eq!(
            by_id["E3"].tally.counts(),
            Err(Omitted::Weighted),
            "{model} E3"
        );
        assert_eq!(by_id["E3"].wilson, Err(Omitted::Weighted));
        let totals = arm.status;
        assert_eq!(
            [
                totals.pass,
                totals.fail,
                totals.blocked,
                totals.not_measured,
                totals.not_applicable
            ],
            status,
            "{model} status"
        );
    }
    let alpha = report
        .arms
        .iter()
        .find(|a| a.cell.to_string() == cells["alpha"])
        .expect("the arm");
    assert_eq!(alpha.tally.valid_runs, 10);
    assert_eq!(
        alpha.tally.invalid,
        BTreeMap::from([("gateway".to_owned(), 1)])
    );
    assert_eq!(
        alpha.tally.experimenter_ends,
        BTreeMap::from([("killed".to_owned(), 1)])
    );
    assert_eq!(report.i4_violations, 0);
}

/// Guards: A1, I20 — the passes of an invalid run and of a run the experimenter ended count
/// nowhere: the report over the layout with the two extra runs equals the one without them in
/// every count, the experimenter's end sits apart from the invalid reasons, and the two runs
/// still have their rows.
/// Oracle: the layout's hand counts and its twin without the two runs.
/// Bite: count the records of invalid runs in `k` and `n`, or list `killed` under `invalid`, and
/// this turns red.
#[test]
fn a1_invalid_and_experimenter_runs_are_out_of_k_and_n() {
    let (with, runs_with) = layout(true);
    let (without, runs_without) = layout(false);
    let with = report(&with, runs_with);
    let without = report(&without, runs_without);
    for (a, b) in with.arms.iter().zip(&without.arms) {
        assert_eq!(a.gate, b.gate);
        assert_eq!(a.quality, b.quality);
        assert_eq!(a.status, b.status);
        assert_eq!(a.tally.expectations, b.tally.expectations);
    }
    let alpha = with
        .arms
        .iter()
        .find(|a| a.tally.invalid.contains_key("gateway"))
        .expect("the arm with an invalid run");
    assert!(!alpha.tally.invalid.contains_key("killed"));
    assert_eq!(alpha.tally.experimenter_ends.get("killed"), Some(&1));
    assert_eq!(with.runs.len(), 32);
    assert_eq!(without.runs.len(), 30);
}

/// Guards: A1 — a run's row is an open map: a path no code names (a future `session.*` value, the
/// measured `census.init_tools`) appears in `measured` with no change, a value recorded for the
/// cell appears in `comparable`, and a value that was not recorded is left out.
/// Oracle: the paths written into the manifest by the test.
/// Bite: filter the rows to an allowlist of paths and this turns red.
#[test]
fn a1_runs_form_an_open_map() {
    let mut spec = Spec::new("alpha", 1);
    spec.measured = vec![
        ("session.turns", json!(3)),
        ("census.init_tools", json!(["Read"])),
        ("zz.future.path", json!({"nested": true})),
    ];
    spec.extra = vec![("plan.effort", json!("high"))];
    spec.unrecorded = vec!["plan.absent"];
    let run = view(&spec);
    let selection = select([record(&run, 1, "E1", "gate", "pass", None, None)]);
    let report = report(&selection, vec![run]);
    let row = report.runs.first().expect("one row");
    assert_eq!(row.measured.get("session.turns"), Some(&json!(3)));
    assert_eq!(
        row.measured.get("census.init_tools"),
        Some(&json!(["Read"]))
    );
    assert_eq!(
        row.measured.get("zz.future.path"),
        Some(&json!({"nested": true}))
    );
    assert_eq!(
        row.measured.get("provenance.eval_infra.dirty"),
        Some(&json!(false))
    );
    assert_eq!(row.comparable.get("plan.model"), Some(&json!("alpha")));
    assert_eq!(row.comparable.get("plan.effort"), Some(&json!("high")));
    assert!(
        !row.comparable.contains_key("plan.absent"),
        "an unrecorded value is left out"
    );
    assert_eq!(row.comparable.len(), 2);
}

/// The three pairs of the layout, by model, with `plan.model` declared.
fn holm_report() -> (Report, BTreeMap<&'static str, String>) {
    let (selection, runs) = layout(true);
    let cells: BTreeMap<&str, String> = ["alpha", "beta", "gamma"]
        .into_iter()
        .map(|m| (m, cell_of(&runs, m)))
        .collect();
    (report(&selection, runs), cells)
}

/// Guards: A1 — the Holm family is one per role per call: the three comparable pairs give six
/// tested gate entries in one family `gate, 6`, E3 is `weighted` in every arm so the quality
/// family is empty, and each `p_holm` equals the step-down adjustment of the six raw p-values,
/// the p-values and differences being rows of the second engine.
/// Oracle: `fisher.toml` and `newcombe.toml`, and the test's own few lines of Holm.
/// Bite: adjust each pair on its own (family of 2), or leave Holm out, and this turns red.
#[test]
fn a1_holm_family_is_one_per_role_per_call() {
    let (report, cells) = holm_report();
    let comparisons = &report.comparisons;
    assert_eq!(comparisons.len(), 3);
    assert_eq!(report.pairs_not_one_factor, 0);
    assert_eq!(report.families.len(), 1);
    assert_eq!(report.families[0].role, Role::Gate);
    assert_eq!(report.families[0].m, 6);
    let counts: BTreeMap<(&str, &str), (u64, u64)> = BTreeMap::from([
        (("alpha", "E1"), (6, 10)),
        (("alpha", "E2"), (3, 5)),
        (("beta", "E1"), (2, 10)),
        (("beta", "E2"), (0, 5)),
        (("gamma", "E1"), (5, 10)),
        (("gamma", "E2"), (5, 5)),
    ]);
    let model_of = |cell: &candler_core::evaluation::selection::rates::CellKey| {
        *cells
            .iter()
            .find(|(_, hex)| **hex == cell.to_string())
            .expect("a cell")
            .0
    };
    let mut raw: Vec<f64> = Vec::new();
    let mut seen = Vec::new();
    for comparison in comparisons {
        assert_eq!(comparison.verdict, PairVerdict::Comparable);
        assert_eq!(comparison.factor.to_string(), "plan.model");
        let (a, b) = (model_of(&comparison.a), model_of(&comparison.b));
        assert_eq!(
            comparison.entries.len(),
            3,
            "E1, E2 and E3 are graded in both arms"
        );
        for entry in &comparison.entries {
            if let Entry::Tested(tested) = entry {
                let id = tested.id.to_string();
                let (ca, cb) = (counts[&(a, id.as_str())], counts[&(b, id.as_str())]);
                assert_eq!((tested.a.k(), tested.a.n()), ca);
                assert_eq!((tested.b.k(), tested.b.n()), cb);
                close("p", tested.p, fisher_p(ca, cb));
                let row = vector::<NewcombeRow>("newcombe.toml")
                    .into_iter()
                    .find(|r| (r.k1, r.n1, r.k2, r.n2) == (ca.0, ca.1, cb.0, cb.1))
                    .expect("a newcombe row");
                close("diff", tested.diff, row.diff);
                close("diff_lo", tested.diff_lo, row.lo);
                close("diff_hi", tested.diff_hi, row.hi);
                raw.push(tested.p);
                seen.push(tested.clone());
            } else {
                assert_eq!(entry.id().to_string(), "E3");
                assert_eq!(
                    entry,
                    &Entry::Omitted {
                        id: entry.id().clone(),
                        role: Role::Quality,
                        reason: OmitReason::Counts(Omitted::Weighted)
                    }
                );
            }
        }
    }
    assert_eq!(raw.len(), 6);
    // The step-down adjustment, written out: sort ascending, multiply by (m - rank), keep the
    // running maximum, cap at 1.
    let m = raw.len();
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&i, &j| raw[i].total_cmp(&raw[j]).then(i.cmp(&j)));
    let mut adjusted = vec![0.0_f64; m];
    let mut running = 0.0_f64;
    for (rank, &at) in order.iter().enumerate() {
        running = running.max(f64::from(u32::try_from(m - rank).expect("small")) * raw[at]);
        adjusted[at] = running.min(1.0);
    }
    for (at, tested) in seen.iter().enumerate() {
        close("p_holm", tested.p_holm, adjusted[at]);
        assert_eq!(tested.reject, adjusted[at] < 0.05, "reject at {at}");
    }
    assert!(
        seen.iter().any(|t| t.p_holm > t.p),
        "a family of six moves some p-value up"
    );
}

/// A selection of one pass per evaluator on one run.
fn selection_of(evaluators: &[&str]) -> (Selection, Vec<RunView>) {
    let run = view(&Spec::new("alpha", 1));
    let records: Vec<OutcomeRecord> = evaluators
        .iter()
        .enumerate()
        .map(|(n, evaluator)| record_by(evaluator, &run, n + 1, "E1", "gate", "pass", None, None))
        .collect();
    (select(records), vec![run])
}

/// Guards: A3 — one evaluator version is chosen: a store with one evaluator derives it, a store
/// with two refuses and lists both, a named `name@version` is accepted when it is there and
/// refused when it is not, an empty store is refused, and the report counts the chosen
/// evaluator's records alone.
/// Oracle: the evaluators written into the records, and the codes of the `[analysis]` section of
/// `docs/contract/codes.toml`.
/// Bite: return the first evaluator when several exist, and the mix is silently analysed, so this
/// turns red.
#[test]
fn a3_one_evaluator_version_is_chosen() {
    let (one, _) = selection_of(&["suite-eval@1.0.0"]);
    assert_eq!(
        choose_evaluator(&one, None).expect("derived").to_string(),
        "suite-eval@1.0.0"
    );
    let (two, runs) = selection_of(&["suite-eval@1.0.0", "suite-eval@2.0.0"]);
    let mixed = choose_evaluator(&two, None).expect_err("a mix is refused");
    assert_eq!(
        mixed,
        ReportError::MixedEvaluatorVersions {
            found: vec!["suite-eval@1.0.0".to_owned(), "suite-eval@2.0.0".to_owned()]
        }
    );
    assert_eq!(mixed.code().as_str(), "analysis.mixed_evaluator_versions");
    assert!(mixed.to_string().contains("suite-eval@2.0.0"), "{mixed}");
    assert_eq!(
        choose_evaluator(&two, Some("suite-eval@2.0.0"))
            .expect("named")
            .to_string(),
        "suite-eval@2.0.0"
    );
    let unknown = choose_evaluator(&two, Some("suite-eval@3.0.0")).expect_err("not there");
    assert_eq!(unknown.code().as_str(), "analysis.unknown_evaluator");
    let none = choose_evaluator(&Selection::default(), None).expect_err("nothing to analyse");
    assert_eq!(none, ReportError::NoOutcomes);
    assert_eq!(none.code().as_str(), "analysis.no_outcomes");
    // The report holds the records of the chosen evaluator and no other.
    let chosen = choose_evaluator(&two, Some("suite-eval@2.0.0")).expect("named");
    let built = Report::build(ReportInput {
        selection: two,
        evaluator: chosen,
        runs,
        unsealed: Vec::new(),
    })
    .expect("the report builds");
    let arm = built.arms.first().expect("one arm");
    assert_eq!((arm.gate.num(), arm.gate.den()), (1, 1));
}

/// Guards: A3 — a bare version selects the evaluators that hold it: exactly one is accepted, none
/// is unknown, several are ambiguous and name each; a text that is neither `name@version` nor a
/// version is unknown.
/// Oracle: the evaluators written into the records.
/// Bite: accept a bare version when two names hold it (take the first), and this turns red.
#[test]
fn a3_a_bare_version_needs_a_unique_name() {
    let (sel, _) = selection_of(&["a-eval@1.0.0", "b-eval@2.0.0"]);
    assert_eq!(
        choose_evaluator(&sel, Some("2.0.0"))
            .expect("one holds it")
            .to_string(),
        "b-eval@2.0.0"
    );
    assert_eq!(
        choose_evaluator(&sel, Some("1.0.0"))
            .expect("one holds it")
            .to_string(),
        "a-eval@1.0.0"
    );
    let none = choose_evaluator(&sel, Some("3.0.0")).expect_err("none holds it");
    assert_eq!(none.code().as_str(), "analysis.unknown_evaluator");
    let text = choose_evaluator(&sel, Some("banana")).expect_err("not a version");
    assert_eq!(text.code().as_str(), "analysis.unknown_evaluator");
    let (many, _) = selection_of(&["a-eval@1.0.0", "b-eval@2.0.0", "c-eval@2.0.0"]);
    let ambiguous = choose_evaluator(&many, Some("2.0.0")).expect_err("two hold it");
    assert_eq!(
        ambiguous,
        ReportError::AmbiguousEvaluatorName {
            version: "2.0.0".to_owned(),
            names: vec!["b-eval@2.0.0".to_owned(), "c-eval@2.0.0".to_owned()]
        }
    );
    assert_eq!(
        ambiguous.code().as_str(),
        "analysis.ambiguous_evaluator_name"
    );
    assert_eq!(
        choose_evaluator(&many, Some("c-eval@2.0.0"))
            .expect("named")
            .to_string(),
        "c-eval@2.0.0"
    );
}

/// `count` runs of `model`, each shaped by `shape`.
fn runs_of(model: &'static str, count: u32, shape: impl Fn(&mut Spec)) -> Vec<Spec> {
    (1..=count)
        .map(|replicate| {
            let mut spec = Spec::new(model, replicate);
            shape(&mut spec);
            spec
        })
        .collect()
}

/// The report of two arms of five runs, whose `E1` is 4 of 5 for the first and 1 of 5 for the
/// second, with `declared` the factors of the experiment file.
fn duo(a: Vec<Spec>, b: Vec<Spec>, declared: &[&'static str]) -> (Report, RunView, RunView) {
    let (a, b): (Vec<Spec>, Vec<Spec>) = (
        a.into_iter()
            .map(|s| Spec {
                factors: declared.to_vec(),
                ..s
            })
            .collect(),
        b.into_iter()
            .map(|s| Spec {
                factors: declared.to_vec(),
                ..s
            })
            .collect(),
    );
    let mut runs: Vec<RunView> = Vec::new();
    let mut records = Vec::new();
    for (side, specs) in [a, b].into_iter().enumerate() {
        for spec in specs {
            let run = view(&spec);
            let pass = if side == 0 {
                spec.replicate <= 4
            } else {
                spec.replicate == 1
            };
            let status = if pass { "pass" } else { "fail" };
            records.push(record(
                &run,
                records.len() + 1,
                "E1",
                "gate",
                status,
                None,
                None,
            ));
            runs.push(run);
        }
    }
    let first = runs.first().expect("a run").clone();
    let last = runs.last().expect("a run").clone();
    (report(&select(records), runs), first, last)
}

fn only_comparison(report: &Report) -> &candler_core::analysis::report::Comparison {
    let comparisons = &report.comparisons;
    assert_eq!(comparisons.len(), 1);
    comparisons.first().expect("one comparison")
}

/// Guards: A2 — a refused pair keeps both arms' rates and the cause of the refusal, and makes no
/// test: a different `eval_infra.commit`, a value no factor declares (`plan.other`), a dirty
/// build, and a dirty build found in the second run of one arm only (the verdict reads every
/// valid run of both arms).
/// Oracle: the paths `fixtures/comparability/verdicts.json` expects for its rows C48, C03 and
/// C45, which these manifests are built as.
/// Bite: compare across the refusal, drop the cause, or call the verdict over the first run of
/// each arm, and this turns red.
#[test]
fn a2_a_refused_pair_keeps_its_rates_and_cause() {
    let cases: [(&str, Vec<Spec>, Vec<Spec>, &str); 4] = [
        (
            "C48 a different build",
            runs_of("alpha", 5, |_| {}),
            runs_of("beta", 5, |s| {
                s.provenance = vec![("eval_infra.commit", json!("def5678"))];
            }),
            "provenance.eval_infra.commit",
        ),
        (
            "C03 a value no factor declares",
            runs_of("alpha", 5, |_| {}),
            runs_of("beta", 5, |s| s.extra = vec![("plan.other", json!("x"))]),
            "plan.other",
        ),
        (
            "C45 a dirty build",
            runs_of("alpha", 5, |s| {
                s.provenance = vec![("eval_infra.dirty", json!(true))];
            }),
            runs_of("beta", 5, |s| {
                s.provenance = vec![("eval_infra.dirty", json!(true))];
            }),
            "provenance.eval_infra.dirty",
        ),
        (
            "a dirty build in the second run only",
            runs_of("alpha", 5, |_| {}),
            runs_of("beta", 5, |s| {
                if s.replicate == 2 {
                    s.provenance = vec![("eval_infra.dirty", json!(true))];
                }
            }),
            "provenance.eval_infra.dirty",
        ),
    ];
    for (label, a, b, cause) in cases {
        let (report, _, _) = duo(a, b, &["plan.model"]);
        let comparison = only_comparison(&report);
        assert_eq!(
            comparison.verdict,
            PairVerdict::Refused(vec![cause.to_owned()]),
            "{label}"
        );
        assert!(
            comparison.entries.is_empty(),
            "{label}: a refused pair has no test"
        );
        assert_eq!(report.families.len(), 0, "{label}");
        assert_eq!(report.arms.len(), 2, "{label}");
        let rates: Vec<(u64, u64)> = report
            .arms
            .iter()
            .map(|arm| (arm.gate.num(), arm.gate.den()))
            .collect();
        assert!(
            rates.contains(&(4, 5)) && rates.contains(&(1, 5)),
            "{label}: {rates:?}"
        );
    }
}

/// Guards: A2 — a pair that differs in one declared factor and nothing else, with the clean
/// provenance block, is comparable and tested (the positive control, as C52), and an invalid run
/// that carries a dirty build is not in the verdict.
/// Oracle: the Fisher row of `fisher.toml` for 4 of 5 against 1 of 5, and `verdicts.json` row
/// C52.
/// Bite: put invalid runs into the verdict, and the control is refused; skip the verdict, and the
/// refused rows of its twin turn red.
#[test]
fn a2_a_comparable_pair_is_tested() {
    let (report, _, _) = duo(
        runs_of("alpha", 5, |_| {}),
        runs_of("beta", 5, |_| {}),
        &["plan.model"],
    );
    let comparison = only_comparison(&report);
    assert_eq!(comparison.verdict, PairVerdict::Comparable);
    let [Entry::Tested(tested)] = comparison.entries.as_slice() else {
        panic!("one tested entry: {:?}", comparison.entries);
    };
    assert_eq!(tested.id.to_string(), "E1");
    close("p", tested.p, fisher_p((4, 5), (1, 5)));
    // An invalid run with a dirty build sits in the second arm: it is not in the verdict.
    let mut b = runs_of("beta", 5, |_| {});
    let mut dirty = Spec::new("beta", 6);
    dirty.integrity = json!({"kind": "invalid", "reason": "gateway"});
    dirty.provenance = vec![("eval_infra.dirty", json!(true))];
    b.push(dirty);
    let (report, _, _) = duo(runs_of("alpha", 5, |_| {}), b, &["plan.model"]);
    assert_eq!(only_comparison(&report).verdict, PairVerdict::Comparable);
}

/// Guards: A2 — only pairs that differ in exactly one declared factor are compared: two arms that
/// differ in two declared factors give no comparison, and two arms that differ in an undeclared
/// path alone give none either; each is counted in `pairs_not_one_factor`.
/// Oracle: the factors declared in the call and the paths the manifests differ in, written by
/// hand.
/// Bite: compare every pair, or read the manifests' own `factors` instead of the declared ones,
/// and this turns red.
#[test]
fn a2_only_one_factor_pairs_are_compared() {
    let two = |s: &mut Spec| s.extra = vec![("plan.effort", json!("high"))];
    let (report, _, _) = duo(
        runs_of("alpha", 5, |_| {}),
        runs_of("beta", 5, two),
        &["plan.model", "plan.effort"],
    );
    assert_eq!(report.comparisons.len(), 0);
    assert_eq!(report.pairs_not_one_factor, 1);
    let undeclared = |s: &mut Spec| s.extra = vec![("plan.other", json!("x"))];
    let (report, _, _) = duo(
        runs_of("alpha", 5, |_| {}),
        runs_of("alpha", 5, undeclared),
        &["plan.model"],
    );
    assert_eq!(report.comparisons.len(), 0);
    assert_eq!(report.pairs_not_one_factor, 1);
}

/// Guards: A2 — a cell that sits in two strata is two arms: alpha and beta each have three runs
/// under one experiment digest and three under another, so the report holds four arms of three
/// runs each, one comparison per stratum on that stratum's own runs (3 of 3 against 0 of 3), and
/// one Holm family of exactly two tests.
/// Oracle: the Fisher row of `fisher.toml` for 3 of 3 against 0 of 3, and Holm's value for two
/// equal p-values, `2p`, from `holm.toml`'s rule.
/// Bite: key the tally by cell alone, and each arm pools six runs; compare each pair once on the
/// pooled arms, and the count of comparisons or the p-value turns red.
#[test]
fn a2_a_cell_in_two_strata_is_tallied_per_stratum() {
    let mut runs: Vec<RunView> = Vec::new();
    let mut records = Vec::new();
    for (at, experiment) in ['0', '1'].into_iter().enumerate() {
        for (model, status) in [("alpha", "pass"), ("beta", "fail")] {
            for replicate in 1..=3_u32 {
                let mut spec = Spec::new(model, 3 * u32::try_from(at).expect("small") + replicate);
                spec.experiment = experiment;
                let run = view(&spec);
                records.push(record(
                    &run,
                    records.len() + 1,
                    "E1",
                    "gate",
                    status,
                    None,
                    None,
                ));
                runs.push(run);
            }
        }
    }
    let report = report(&select(records), runs);
    assert_eq!(report.arms.len(), 4, "a cell is an arm in each stratum");
    for arm in &report.arms {
        assert_eq!(arm.tally.valid_runs, 3, "{}", arm.experiment);
        let counts = arm.expectations[0].tally.counts().expect("binary");
        let alpha = counts.k() == 3;
        assert_eq!((counts.k(), counts.n()), (if alpha { 3 } else { 0 }, 3));
    }
    let strata: Vec<&str> = report.arms.iter().map(|a| a.experiment.as_str()).collect();
    assert_eq!(strata.iter().filter(|s| **s == digest_text('0')).count(), 2);
    assert_eq!(strata.iter().filter(|s| **s == digest_text('1')).count(), 2);
    assert_eq!(report.comparisons.len(), 2, "one comparison per stratum");
    assert_eq!(
        report
            .comparisons
            .iter()
            .map(|c| c.experiment.clone())
            .collect::<Vec<_>>(),
        vec![digest_text('0'), digest_text('1')]
    );
    assert_eq!(report.families.len(), 1);
    assert_eq!(report.families[0].m, 2, "two tests, not four and not one");
    let p = fisher_p((3, 3), (0, 3));
    for comparison in &report.comparisons {
        let [Entry::Tested(tested)] = comparison.entries.as_slice() else {
            panic!("one tested entry: {:?}", comparison.entries);
        };
        let (alpha_side, beta_side) = if tested.a.k() == 3 {
            (tested.a, tested.b)
        } else {
            (tested.b, tested.a)
        };
        assert_eq!(
            (alpha_side.k(), alpha_side.n(), beta_side.k(), beta_side.n()),
            (3, 3, 0, 3)
        );
        close("p", tested.p, p);
        close("p_holm", tested.p_holm, (2.0 * p).min(1.0));
    }
}

/// The cells of a 2 by 2 store, `plan.model` by `plan.other`, one run each, with the factors the
/// manifests record.
fn two_by_two(factors: &[&'static str]) -> (Selection, Vec<RunView>) {
    let mut runs: Vec<RunView> = Vec::new();
    let mut records = Vec::new();
    for (model, other) in [("alpha", "x"), ("alpha", "y"), ("beta", "x"), ("beta", "y")] {
        let mut spec = Spec::new(model, 1);
        spec.extra = vec![("plan.other", json!(other))];
        spec.factors = factors.to_vec();
        let run = view(&spec);
        records.push(record(
            &run,
            records.len() + 1,
            "E1",
            "gate",
            "pass",
            None,
            None,
        ));
        runs.push(run);
    }
    (select(records), runs)
}

/// Guards: A2 — the factors of a stratum come from its manifests, and an experiment file is a
/// cross-check: on a 2 by 2 store whose manifests record `plan.model` and `plan.other`, four
/// pairs differ in one factor and are compared and two differ in both and are not; a file that
/// declares the parent path `plan` or an unrelated path is refused as
/// `analysis.experiment_disagrees`, naming both sets; a file with the recorded factors is
/// accepted; and manifests that record the parent `plan` are refused as
/// `analysis.factor_broader_than_variation`.
/// Oracle: the factors written into the manifests and the files by the test, and the codes of
/// `codes.toml`.
/// Bite: skip the cross-check, and the parent-path and unrelated-path files are accepted; skip
/// the prefix check, and the broad record is accepted.
#[test]
fn a2_a_wrong_experiment_is_refused() {
    let (selection, runs) = two_by_two(&["plan.model", "plan.other"]);
    let report = report(&selection, runs.clone());
    assert_eq!(report.comparisons.len(), 4);
    assert_eq!(report.pairs_not_one_factor, 2);
    assert_eq!(check_factors(&runs, None), Ok(()));
    assert_eq!(
        check_factors(&runs, Some(&paths(&["plan.model", "plan.other"]))),
        Ok(())
    );
    for wrong in [&["plan"][..], &["plan.zzz"], &["plan.model"]] {
        let refusal = check_factors(&runs, Some(&paths(wrong))).expect_err("a wrong file");
        assert_eq!(
            refusal.code().as_str(),
            "analysis.experiment_disagrees",
            "{wrong:?}"
        );
        let text = refusal.to_string();
        assert!(
            text.contains("plan.model, plan.other"),
            "the manifests' factors: {text}"
        );
        assert!(
            text.contains(&wrong.join(", ")),
            "the file's factors: {text}"
        );
    }
    let (_, broad) = two_by_two(&["plan"]);
    let refusal = check_factors(&broad, None).expect_err("a record broader than the variation");
    assert_eq!(
        refusal.code().as_str(),
        "analysis.factor_broader_than_variation"
    );
}

/// Guards: A1 — a run without a seal is counted apart: with the layout and two unsealed runs, one
/// whose manifest names the experiment of an arm and one whose manifest is unreadable, the call
/// succeeds, the arm shows `unsealed` 1, a stratum `unknown` holds the other, and every count,
/// rate and verdict equals the report without them although the unsealed run has a pass on the
/// store.
/// Oracle: the layout's hand counts.
/// Bite: count the records of an unsealed run, or leave the unsealed runs out of the listed ones
/// (the store then refuses the call with a run that has records and no listing), and this turns
/// red.
#[test]
fn a1_an_unsealed_run_is_counted_apart() {
    let (selection, runs) = layout(false);
    let baseline = report(&selection, runs.clone());
    let open = view(&Spec::new("alpha", 99));
    let mut records: Vec<OutcomeRecord> = selection.current().cloned().collect();
    records.push(record(&open, 9_999, "E1", "gate", "pass", None, None));
    let held = Report::build(ReportInput {
        selection: select(records),
        evaluator: evaluator(),
        runs,
        unsealed: vec![
            Unsealed {
                run: open.manifest.run().clone(),
                experiment: Some(digest_text('0').parse().expect("a digest")),
            },
            Unsealed {
                run: view(&Spec::new("gamma", 98)).manifest.run().clone(),
                experiment: None,
            },
        ],
    })
    .expect("an unsealed run does not fail the call");
    assert_eq!(held.arms.len(), baseline.arms.len() + 1);
    for (a, b) in baseline
        .arms
        .iter()
        .zip(held.arms.iter().filter(|a| a.experiment != "unknown"))
    {
        assert_eq!((a.gate, a.quality, a.status), (b.gate, b.quality, b.status));
        assert_eq!(a.tally.expectations, b.tally.expectations);
    }
    let sealed: u64 = held
        .arms
        .iter()
        .filter(|a| a.experiment != "unknown")
        .map(|a| a.unsealed)
        .sum();
    assert_eq!(sealed, 1);
    let unknown = held
        .arms
        .iter()
        .find(|a| a.experiment == "unknown")
        .expect("the unknown stratum");
    assert_eq!((unknown.unsealed, unknown.tally.valid_runs), (1, 0));
    assert_eq!(held.comparisons, baseline.comparisons);
}

/// Guards: A2 — an expectation graded in one arm of a comparable pair is reported as omitted with
/// the reason `one_arm`, and is not dropped.
/// Oracle: the expectations written into the records, `E1` in both arms and `E9` in the first.
/// Bite: skip the expectations of one arm, and the `E9` entry is missing.
#[test]
fn a2_an_expectation_of_one_arm_is_reported_as_omitted() {
    let mut runs: Vec<RunView> = Vec::new();
    let mut records = Vec::new();
    for (model, status) in [("alpha", "pass"), ("beta", "fail")] {
        for replicate in 1..=3_u32 {
            let run = view(&Spec::new(model, replicate));
            records.push(record(
                &run,
                records.len() + 1,
                "E1",
                "gate",
                status,
                None,
                None,
            ));
            if model == "alpha" {
                records.push(record(
                    &run,
                    records.len() + 1,
                    "E9",
                    "quality",
                    "pass",
                    None,
                    None,
                ));
            }
            runs.push(run);
        }
    }
    let report = report(&select(records), runs);
    let comparison = report.comparisons.first().expect("one comparison");
    let ids: Vec<String> = comparison
        .entries
        .iter()
        .map(|e| e.id().to_string())
        .collect();
    assert_eq!(ids, ["E1", "E9"]);
    assert_eq!(
        comparison.entries[1],
        Entry::Omitted {
            id: "E9".parse().expect("an id"),
            role: Role::Quality,
            reason: OmitReason::OneArm
        }
    );
    assert_eq!(OmitReason::OneArm.as_str(), "one_arm");
}
