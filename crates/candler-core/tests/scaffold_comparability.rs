#![cfg(test)]
//! Development checks of the diff; deleted by the last commit of the leaf.

use candler_core::manifest::compare::{compare, covers};
use candler_schemas::execution::Comparable;
use serde_json::{Value, json};

fn cell(model: Value, idx: Value) -> Comparable {
    serde_json::from_value(json!({
        "values": [
            {"path": "model.id", "value": model},
            {"path": "model.idx", "value": idx}
        ],
        "harness": {"name": "h", "version": "1.0.0"},
        "treatment": {"name": "t"},
        "snapshots": [], "inputs": [], "environments": []
    }))
    .expect("a cell")
}

#[test]
fn identical_arms_have_an_empty_diff() {
    assert!(compare(&cell(json!("a"), json!("x")), &cell(json!("a"), json!("x"))).is_empty());
}

#[test]
fn one_differing_leaf_is_named() {
    let diff = compare(&cell(json!("a"), json!("x")), &cell(json!("b"), json!("x")));
    let paths: Vec<String> = diff.iter().map(|d| d.path.to_string()).collect();
    assert_eq!(paths, ["model.id"]);
}

#[test]
fn segments_never_match_by_characters() {
    assert!(covers(&["model", "id"], &["model", "id", "x"]));
    assert!(!covers(&["model", "id"], &["model", "idx"]));
}

#[test]
fn an_empty_stratum_has_no_verdict() {
    use candler_core::manifest::compare::{NUISANCE_PATHS, Verdict, stratum_verdict};
    assert!(matches!(stratum_verdict(&[], &[]), Verdict::Malformed(_)));
}

#[test]
fn no_comparable_path_is_nuisance() {
    use candler_core::manifest::compare::NUISANCE_PATHS;
    assert!(NUISANCE_PATHS.iter().all(|p| !p.starts_with("comparable")));
}
