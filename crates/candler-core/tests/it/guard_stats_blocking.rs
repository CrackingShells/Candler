//! The choice of blocks and of the test each comparison runs.
//!
//! Runs are fabricated manifests; every block count is a hand count. The p-values and intervals
//! are the ones of the engines that the vector files already pin (`midp`, `cmh`, `exact_strata`,
//! `permutation`, `bca`), so no expected number comes from `blocking.rs`.

use candler_core::analysis::bca::stratified_bca;
use candler_core::analysis::blocking::{
    Battery, Design, OutcomeKind, Stratum, blocks_of, choose, holm_groups, test_blocked,
    unit_blocks_of,
};
use candler_core::analysis::cmh::cmh;
use candler_core::analysis::exact_strata::exact_conditional;
use candler_core::analysis::midp::midp_two_sided;
use candler_core::analysis::permutation::{UnitBlock, stratified_permutation};
use candler_core::analysis::report::RunView;
use candler_core::analysis::tally::RunFacts;
use candler_schemas::execution::RunManifest;
use candler_schemas::outcome::Role;
use candler_schemas::primitives::{FieldPath, RunKey};
use serde_json::{Value, json};

const MODEL: &str = "plan.model";
const TREATMENT: &str = "plan.treatment";
const PASS: (u64, u64) = (1, 1);
const FAIL: (u64, u64) = (0, 1);
const SEED: u64 = 7;

fn path(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

/// The manifest of one run of the cell `(model, treatment)`.
fn manifest(model: &str, treatment: &str, replicate: u32) -> RunManifest {
    let values = vec![
        json!({"path": MODEL, "value": model}),
        json!({"path": TREATMENT, "value": treatment}),
    ];
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
    .map(|(key, value)| json!({"path": key, "value": value, "source": {"kind": "experiment_file"}}))
    .collect();
    let body = |cell: &str| {
        json!({
            "schema": "run-manifest/1",
            "run": {"cell": cell, "replicate": replicate, "attempt": 1},
            "experiment": digest_text('0'),
            "factors": [MODEL, TREATMENT],
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
            "integrity": {"kind": "valid"}
        })
    };
    let probe: RunManifest = serde_json::from_value(body(&digest_text('f'))).expect("a manifest");
    let cell = probe.cell_digest().to_string();
    serde_json::from_value(body(cell.as_str())).expect("a manifest") // BORROW: explicit .as_str()
}

/// One cell of a fabricated store: its levels and the `(num, den)` of each run's outcome.
type Cell = (&'static str, &'static str, Vec<(u64, u64)>);

/// The runs of every cell and the per-run units of the one expectation.
fn store(cells: &[Cell]) -> (Vec<RunView>, Vec<(RunKey, u64, u64)>) {
    let mut views = Vec::new();
    let mut units = Vec::new();
    for (model, treatment, outcomes) in cells {
        for (at, (num, den)) in outcomes.iter().enumerate() {
            let manifest = manifest(model, treatment, u32::try_from(at).expect("a small index"));
            units.push((manifest.run().clone(), *num, *den));
            views.push(RunView {
                facts: RunFacts::new(manifest.run().clone(), manifest.integrity().clone()),
                manifest,
            });
        }
    }
    (views, units)
}

fn declared() -> Vec<FieldPath> {
    vec![path(MODEL), path(TREATMENT)]
}

fn key(factor: &str, level: &str) -> Vec<(FieldPath, Value)> {
    vec![(path(factor), json!(level))]
}

fn stratum(at: &str, a: (u64, u64), b: (u64, u64)) -> Stratum {
    Stratum {
        key: key(MODEL, at),
        a,
        b,
    }
}

/// Guards: the blocks of a comparison are the level combinations of the declared factors other
/// than the compared one: a missed declared factor or the compared factor in the key.
/// Oracle: hand counts of a 2×2 store with two runs per cell.
#[test]
fn blocks_exclude_the_compared_factor() {
    let (views, units) = store(&[
        ("m1", "t1", vec![PASS, PASS]),
        ("m1", "t2", vec![PASS, FAIL]),
        ("m2", "t1", vec![FAIL, FAIL]),
        ("m2", "t2", vec![PASS, PASS]),
    ]);
    let by_treatment = blocks_of(
        &views,
        &units,
        &path(TREATMENT),
        &json!("t1"),
        &json!("t2"),
        &declared(),
    );
    assert_eq!(
        by_treatment,
        vec![
            stratum("m1", (2, 2), (1, 2)),
            stratum("m2", (0, 2), (2, 2)),
        ]
    );
    let by_model = blocks_of(
        &views,
        &units,
        &path(MODEL),
        &json!("m1"),
        &json!("m2"),
        &declared(),
    );
    assert_eq!(
        by_model,
        vec![
            Stratum {
                key: key(TREATMENT, "t1"),
                a: (2, 2),
                b: (0, 2),
            },
            Stratum {
                key: key(TREATMENT, "t2"),
                a: (1, 2),
                b: (2, 2),
            },
        ]
    );
}

/// Guards: a design with one block gets the mid-p and the difference B minus A, and the choice
/// reads the design alone.
/// Oracle: the mid-p engine's own value; the difference is 1/4 − 3/4 by hand.
#[test]
fn one_block_uses_midp() {
    assert_eq!(choose(OutcomeKind::Binary, 1), Battery::MidP);
    assert_eq!(choose(OutcomeKind::Binary, 2), Battery::Cmh);
    assert_eq!(choose(OutcomeKind::Binary, 9), Battery::Cmh);
    assert_eq!(choose(OutcomeKind::Weighted, 1), Battery::Permutation);
    assert_eq!(choose(OutcomeKind::Weighted, 3), Battery::Permutation);
    let blocks = vec![stratum("m1", (3, 4), (1, 4))];
    let tested = test_blocked(&Design::Binary(&blocks), SEED).expect("a test");
    assert_eq!(tested.battery, Battery::MidP);
    assert_eq!(tested.blocks, 1);
    assert!(!tested.fallback);
    let expected = midp_two_sided(&blocks[0]).expect("a mid-p");
    assert!((tested.p - expected).abs() < 1e-12, "{} vs {expected}", tested.p);
    assert!((tested.diff.expect("a difference") + 0.5).abs() < 1e-12);
    assert_eq!(tested.per_block.len(), 1);
}

/// Guards: two blocks or more run the CMH test, not a pooled table.
/// Oracle: the CMH engine's own p; per-block differences are 10/25 − 20/25 and 8/25 − 18/25.
#[test]
fn many_blocks_use_cmh() {
    let blocks = vec![
        stratum("m1", (20, 25), (10, 25)),
        stratum("m2", (18, 25), (8, 25)),
    ];
    let engine = cmh(&blocks, 0.95).expect("a CMH test");
    assert!(engine.mantel_fleiss_holds, "the row needs a large design");
    let tested = test_blocked(&Design::Binary(&blocks), SEED).expect("a test");
    assert_eq!(tested.battery, Battery::Cmh);
    assert_eq!(tested.blocks, 2);
    assert!(!tested.fallback);
    let expected = engine.p.expect("a p");
    assert!((tested.p - expected).abs() < 1e-12);
    assert_eq!(tested.or_mh, engine.or_mh);
    assert_eq!(tested.per_block.len(), 2);
    for block in &tested.per_block {
        assert!((block.diff + 0.4).abs() < 1e-12, "{}", block.diff);
    }
}

/// Guards: when the Mantel–Fleiss criterion fails the exact conditional mid-p is reported and the
/// fallback is recorded; a silent CMH χ² on a thin design is the bug.
/// Oracle: the exact conditional engine's own p.
#[test]
fn cmh_falls_back_to_exact_when_mantel_fleiss_fails() {
    let blocks = vec![stratum("m1", (1, 2), (0, 2)), stratum("m2", (1, 2), (0, 2))];
    let engine = cmh(&blocks, 0.95).expect("a CMH test");
    assert!(!engine.mantel_fleiss_holds, "the row needs a thin design");
    let tested = test_blocked(&Design::Binary(&blocks), SEED).expect("a test");
    assert_eq!(tested.battery, Battery::ExactStrata);
    assert!(tested.fallback);
    let expected = exact_conditional(&blocks).expect("an exact p");
    assert!((tested.p - expected).abs() < 1e-12);
}

/// Guards: a weighted expectation gets the stratified permutation test and BCa interval, with the
/// per-run units as exact fractions.
/// Oracle: the unit blocks and the difference (2/8 − 7/8) by hand; p and interval from the
/// resampling engines.
#[test]
fn weighted_uses_permutation_and_bca() {
    let (views, units) = store(&[
        ("m1", "t1", vec![(1, 2), (2, 2)]),
        ("m1", "t2", vec![(0, 2), (1, 2)]),
        ("m2", "t1", vec![(2, 2), (2, 2)]),
        ("m2", "t2", vec![(1, 2), (0, 2)]),
    ]);
    let blocks = unit_blocks_of(
        &views,
        &units,
        &path(TREATMENT),
        &json!("t1"),
        &json!("t2"),
        &declared(),
    );
    let expected_blocks = vec![
        UnitBlock {
            a: vec![(1.0, 2.0), (2.0, 2.0)],
            b: vec![(0.0, 2.0), (1.0, 2.0)],
        },
        UnitBlock {
            a: vec![(2.0, 2.0), (2.0, 2.0)],
            b: vec![(1.0, 2.0), (0.0, 2.0)],
        },
    ];
    assert_eq!(blocks, expected_blocks);
    let tested = test_blocked(&Design::Weighted(&blocks), SEED).expect("a test");
    assert_eq!(tested.battery, Battery::Permutation);
    assert!(!tested.fallback);
    let perm = stratified_permutation(&blocks, 10_000, SEED).expect("a permutation test");
    assert!((tested.p - perm.p).abs() < 1e-12);
    assert!((tested.diff.expect("a difference") + 0.625).abs() < 1e-12);
    let interval = stratified_bca(&blocks, 10_000, 0.95, SEED).expect("an interval");
    assert_eq!(tested.interval, Some(interval));
}

/// Guards: Holm families are one comparison's gate expectations, and apart its quality
/// expectations; two comparisons never share one.
/// Oracle: Holm by hand: gates (0.01, 0.04) give (0.02, 0.04); a lone p stays as it is.
#[test]
fn holm_family_is_per_comparison_and_role() {
    let keys = [
        (0, Role::Gate),
        (0, Role::Gate),
        (0, Role::Quality),
        (1, Role::Gate),
    ];
    let adjusted = holm_groups(&keys, &[0.01, 0.04, 0.01, 0.03]).expect("an adjustment");
    let expected = [0.02, 0.04, 0.01, 0.03];
    assert_eq!(adjusted.len(), expected.len());
    for (got, want) in adjusted.iter().zip(expected) {
        assert!((got - want).abs() < 1e-12, "{adjusted:?}");
    }
}
