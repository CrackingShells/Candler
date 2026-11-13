#![cfg(test)]
//! Guards for the oracle over the vendored mini-corpus. Every count below is a literal written by
//! hand after counting the raw `eval-K-grading.json` files with `jq` and `python3 -I`.

use crate::oracle;

use std::path::PathBuf;

use oracle::{ExpectedIntegrity, ExpectedRow, expected_rows, expected_runs};

const V3: &str = "20260806-035940_v0.2.3_claude-haiku-4-5_model-claude-haiku-4-5_skill_version-htn-creator-v0.2.3";
const V6: &str = "20260806-035940_v0.2.6_claude-haiku-4-5_model-claude-haiku-4-5_skill_version-htn-creator-0.2.6";

fn mini() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini-corpus")
}

fn find<'r>(rows: &'r [ExpectedRow], cell: &str, run_n: u32, id: &str) -> &'r ExpectedRow {
    rows.iter()
        .find(|r| r.cell == cell && r.run_n == run_n && r.expectation_id == id)
        .unwrap_or_else(|| panic!("no row {cell} run-{run_n} {id}"))
}

fn count(rows: &[ExpectedRow], keep: impl Fn(&ExpectedRow) -> bool) -> usize {
    rows.iter().filter(|r| keep(r)).count()
}

/// Guards: D6 — the mini-corpus yields exactly its valid rows: 136 in total, split by status and
/// by run, with the contaminated synthetic run contributing none. Oracle: counts made by hand
/// with `jq` over the raw grading files (138 entries, 2 of them in the contaminated run).
#[test]
fn the_mini_corpus_yields_the_hand_counted_rows() {
    let rows = expected_rows(&mini());
    assert_eq!(rows.len(), 136);
    let by = |s: &str| count(&rows, |r| r.status == s);
    assert_eq!(by("pass"), 105);
    assert_eq!(by("fail"), 7);
    assert_eq!(by("blocked"), 13);
    assert_eq!(by("not_measured"), 10);
    assert_eq!(by("not_applicable"), 1);
    let per_run = |cell: &str, n: u32| count(&rows, |r| r.cell == cell && r.run_n == n);
    assert_eq!(per_run(V3, 6), 43);
    assert_eq!(per_run(V3, 17), 43);
    assert_eq!(per_run(V6, 12), 43);
    assert_eq!(per_run("synthetic_cell", 1), 0);
    assert_eq!(per_run("synthetic_cell", 2), 5);
    assert_eq!(per_run("synthetic_cell", 3), 2);
    assert_eq!(count(&rows, |r| r.role == "gate"), 63);
    assert_eq!(count(&rows, |r| r.role == "quality"), 21);
    assert_eq!(count(&rows, |r| r.role == "descriptive"), 52);
}

/// Guards: D6 — a weight is kept only when it differs from the status default, and a pass 0/0
/// carries the cause `legacy: zero_weight`. Oracle: the score and `max_score` pairs listed by hand
/// from the raw files (19 non-default weights, 6 zero-weight passes).
#[test]
fn weights_follow_the_status_defaults() {
    let rows = expected_rows(&mini());
    assert_eq!(count(&rows, |r| r.weight.is_some()), 25);
    assert_eq!(
        count(&rows, |r| r.cause.as_deref() == Some("legacy: zero_weight")),
        6
    );
    assert_eq!(count(&rows, |r| r.weight.as_deref() == Some("0/0")), 6);
    assert!(
        rows.iter()
            .filter(|r| r.status == "blocked")
            .all(|r| r.weight.is_none())
    );
    assert!(
        rows.iter()
            .filter(|r| r.status == "not_measured")
            .all(|r| r.weight.is_none())
    );
    let a5 = find(&rows, V3, 17, "A5");
    // BORROW: `.as_str()` views the owned status as a `&str` for the comparison
    assert_eq!(
        (a5.status.as_str(), a5.weight.as_deref()),
        ("pass", Some("35/35"))
    );
    let x3 = find(&rows, V3, 6, "C-X3");
    // BORROW: `.as_str()` views the owned status as a `&str` for the comparison
    assert_eq!(
        (x3.status.as_str(), x3.weight.as_deref()),
        ("fail", Some("1/2"))
    );
    let d1 = find(&rows, V6, 12, "D1");
    // BORROW: `.as_str()` views the owned status as a `&str` for the comparison
    assert_eq!(
        (d1.status.as_str(), d1.weight.as_deref()),
        ("fail", Some("0/3"))
    );
    assert_eq!(find(&rows, V3, 6, "A1").weight, None);
}

/// Guards: D6 — a blocked row names its artifact, a replay-less `not_measured` row names its
/// legacy code, and a legacy `not_applicable` row maps to `not_applicable_to_arm`. Oracle: the
/// `blocked_by` fields and `TIER_D` evidence strings counted by hand in the raw files.
#[test]
fn causes_follow_the_legacy_fields() {
    let rows = expected_rows(&mini());
    assert_eq!(
        count(&rows, |r| r.cause.as_deref()
            == Some("artifact: cont-train.json")),
        13
    );
    assert_eq!(
        count(&rows, |r| r.cause.as_deref()
            == Some("legacy: no_replay_target")),
        4
    );
    assert_eq!(
        count(&rows, |r| r.cause.as_deref()
            == Some("legacy: zero_mappings")),
        6
    );
    assert_eq!(
        count(&rows, |r| r.cause.as_deref() == Some("legacy: unrecorded")),
        0
    );
    assert_eq!(
        count(&rows, |r| r.cause.as_deref()
            == Some("reason: not_applicable_to_arm")),
        1
    );
    assert_eq!(count(&rows, |r| r.cause.is_some()), 13 + 4 + 6 + 1 + 6);
    assert_eq!(
        find(&rows, V3, 6, "D-E1").cause.as_deref(),
        Some("legacy: no_replay_target")
    );
    assert_eq!(
        find(&rows, V6, 12, "D-E3").cause.as_deref(),
        Some("legacy: zero_mappings")
    );
    assert_eq!(
        find(&rows, V3, 17, "C-M1").cause.as_deref(),
        Some("artifact: cont-train.json")
    );
    assert_eq!(
        find(&rows, "synthetic_cell", 3, "A2").status,
        "not_applicable"
    );
}

/// Guards: D6 — the number token decides the value kind: an integer literal is a count, a literal
/// with a point or an exponent is a number, a string is text. Oracle: the raw tokens, typed by
/// python's `json` as 25 int, 8 float and 1 str among the valid rows (102 rows have no value).
#[test]
fn the_number_token_decides_the_value_kind() {
    let rows = expected_rows(&mini());
    assert_eq!(count(&rows, |r| r.value.is_none()), 102);
    assert_eq!(
        count(&rows, |r| r
            .value
            .as_deref()
            .is_some_and(|v| v.starts_with("count:"))),
        25
    );
    assert_eq!(
        count(&rows, |r| r
            .value
            .as_deref()
            .is_some_and(|v| v.starts_with("number:"))),
        8
    );
    assert_eq!(
        count(&rows, |r| r
            .value
            .as_deref()
            .is_some_and(|v| v.starts_with("text:"))),
        1
    );
    let value = |id: &str| find(&rows, "synthetic_cell", 2, id).value.as_deref();
    assert_eq!(value("C1"), Some("text:three actions, two methods"));
    assert_eq!(value("C2"), Some("count:14"));
    assert_eq!(value("C3"), Some("number:2.5"));
    assert_eq!(value("C4"), Some("number:100.0"));
    assert_eq!(
        find(&rows, V3, 6, "C6").value.as_deref(),
        Some("number:3.29")
    );
    assert_eq!(
        find(&rows, V3, 6, "D1").value.as_deref(),
        Some("number:1.0")
    );
}

/// Guards: I10 — a legacy `contaminated` verdict is an invalid run (`input_leak`), `clean` and
/// `incomplete` are valid, and the harness version and the measured provenance leaves come from
/// the stream and `eval_metadata.json`. Oracle: `PROVENANCE.json` verdicts, the init lines and
/// the provenance blocks read by hand with `jq`.
#[test]
fn run_facts_come_from_the_verdicts_streams_and_metadata() {
    let runs = expected_runs(&mini());
    assert_eq!(runs.len(), 6);
    let run = |cell: &str, n: u32| {
        runs.iter()
            .find(|r| r.cell == cell && r.run_n == n)
            .unwrap_or_else(|| panic!("no run {cell} run-{n}"))
    };
    // BORROW: explicit `.to_owned()` of a literal for the owned field
    assert_eq!(
        run("synthetic_cell", 1).integrity,
        ExpectedIntegrity::Invalid("input_leak".to_owned())
    );
    for (cell, n) in [
        (V3, 6),
        (V3, 17),
        (V6, 12),
        ("synthetic_cell", 2),
        ("synthetic_cell", 3),
    ] {
        assert_eq!(run(cell, n).integrity, ExpectedIntegrity::Valid);
    }
    assert!(runs.iter().all(|r| r.harness_version == "2.1.220"));
    let sandbox = |d: &str, v: &str| {
        vec![
            // BORROW: explicit `.to_owned()` of a literal for the owned field
            (
                "provenance.planner.sandbox.distribution".to_owned(),
                format!("\"{d}\""),
            ),
            // BORROW: explicit `.to_owned()` of a literal for the owned field
            (
                "provenance.planner.sandbox.version".to_owned(),
                format!("\"{v}\""),
            ),
        ]
    };
    assert_eq!(run(V3, 6).measured, sandbox("gtpyhop", "1.9.7"));
    assert_eq!(run(V3, 17).measured, sandbox("gtpyhop", "1.9.7"));
    assert_eq!(run(V6, 12).measured, sandbox("gtpyhop-core", "2.0.1"));
}
