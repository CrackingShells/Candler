#![cfg(test)]
//! The oracle over the whole golden corpus, checked against the totals the legacy tooling and
//! the design document state. Ignored: it needs `CANDLER_GOLDEN_CORPUS`.

#[path = "oracle/mod.rs"]
mod oracle;

use std::collections::BTreeSet;
use std::path::PathBuf;

use oracle::{ExpectedIntegrity, expected_rows, expected_runs};

/// Guards: D6 — the oracle's rows of the golden corpus add up to the published totals: 240 runs
/// and 10,320 rows first, then the per-role score sums of `_outcome.py` and the status counts of
/// the design document. Oracle: `htn_creator_eval/_outcome.py` at `htn-creator-eval@0.4.2`
/// (gate 10420/11645, quality 5080/5189, descriptive 3281/3472) and R01 §Golden corpus (pass
/// 8,291, blocked 802, `not_measured` 742, fail 485).
#[test]
#[ignore = "needs CANDLER_GOLDEN_CORPUS, the extracted campaign/20260806-cont-train folder"]
fn oracle_aggregates() {
    let root = std::env::var("CANDLER_GOLDEN_CORPUS")
        .expect("CANDLER_GOLDEN_CORPUS must name the extracted campaign/20260806-cont-train");
    let corpus = PathBuf::from(root);
    let runs = expected_runs(&corpus);
    let rows = expected_rows(&corpus);
    assert_eq!(runs.len(), 240);
    assert_eq!(rows.len(), 10_320);
    // BORROW: `.as_str()` views the owned status as a `&str` for the comparison
    let cells: BTreeSet<&str> = runs.iter().map(|r| r.cell.as_str()).collect();
    assert_eq!(cells.len(), 12);
    assert!(runs.iter().all(|r| r.integrity == ExpectedIntegrity::Valid));

    let sums = |role: &str| {
        let mut num = 0_i64;
        let mut den = 0_i64;
        for row in rows.iter().filter(|r| r.role == role) {
            let (n, d) = score_of(row);
            num += n;
            den += d;
        }
        (num, den)
    };
    assert_eq!(sums("gate"), (10_420, 11_645));
    assert_eq!(sums("quality"), (5_080, 5_189));
    assert_eq!(sums("descriptive"), (3_281, 3_472));

    let by = |s: &str| rows.iter().filter(|r| r.status == s).count();
    assert_eq!(by("pass"), 8_291);
    assert_eq!(by("blocked"), 802);
    assert_eq!(by("not_measured"), 742);
    assert_eq!(by("fail"), 485);
}

/// The `(score, max_score)` of a row with its status default restored.
fn score_of(row: &oracle::ExpectedRow) -> (i64, i64) {
    if let Some(w) = &row.weight {
        let (n, d) = w.split_once('/').expect("weight is num/den");
        return (n.parse().expect("num"), d.parse().expect("den"));
    }
    // BORROW: `.as_str()` views the owned status as a `&str` for the comparison
    match row.status.as_str() {
        "pass" => (1, 1),
        "fail" | "blocked" => (0, 1),
        "not_measured" | "not_applicable" => (0, 0),
        other => panic!("unknown status {other}"),
    }
}
