//! The choice of blocks and of the test each comparison runs.
//!
//! Runs are fabricated manifests; every block count is a hand count. The p-values and intervals
//! are the ones of the engines that the vector files already pin (`midp`, `cmh`, `exact_strata`,
//! `permutation`, `bca`), so no expected number comes from `blocking.rs`.

use candler_core::analysis::blocking::{
    Battery, Design, OutcomeKind, Stratum, choose, holm_groups, holm_rejects, test_blocked,
};
use candler_core::analysis::cmh::cmh;
use candler_core::analysis::exact_strata::exact_conditional;
use candler_core::analysis::midp::midp_two_sided;
use candler_schemas::outcome::Role;
use candler_schemas::primitives::FieldPath;
use serde_json::{Value, json};

const MODEL: &str = "plan.model";
const SEED: u64 = 7;

fn path(text: &str) -> FieldPath {
    text.parse().expect("a field path")
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
    assert!(
        (tested.p - expected).abs() < 1e-12,
        "{} vs {expected}",
        tested.p
    );
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

/// Guards: a descriptive entry is never rejected, whatever its raw p.
/// Oracle: by hand, p 0.01 is below alpha for a gate and is not rejected as descriptive.
#[test]
fn descriptive_is_never_rejected() {
    let keys = [(0, Role::Gate), (0, Role::Descriptive), (0, Role::Quality)];
    let adjusted = holm_groups(&keys, &[0.01, 0.01, 0.2]).expect("an adjustment");
    assert_eq!(holm_rejects(&keys, &adjusted), vec![true, false, false]);
}
