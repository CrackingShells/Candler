#![cfg(test)]
//! Guards for the CMH test. Each reads `tests/vectors/stats/cmh.toml`, which `gen_stats.py` writes
//! from statsmodels' `StratifiedTable` (and, for the Mantel–Fleiss quantity, from exact
//! arithmetic on the margins); a vector row is never edited to make a guard pass.

use std::path::PathBuf;

use candler_core::analysis::blocking::Stratum;
use candler_core::analysis::cmh::{CmhResult, cmh};
use candler_core::analysis::error::StatsError;
use serde::Deserialize;

const LEVEL: f64 = 0.95;
const RELATIVE: f64 = 1e-12;

#[derive(Deserialize)]
struct Vectors {
    rows: Vec<Row>,
}

#[derive(Deserialize)]
struct Row {
    label: String,
    pa: Vec<u64>,
    na: Vec<u64>,
    pb: Vec<u64>,
    nb: Vec<u64>,
    informative: usize,
    mf: f64,
    mf_holds: bool,
    statistic: Option<f64>,
    p: Option<f64>,
    or_mh: Option<f64>,
    or_lo: Option<f64>,
    or_hi: Option<f64>,
    hom_p: Option<f64>,
}

fn rows() -> Vec<Row> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/stats/cmh.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let parsed: Vectors = toml::from_str(&text).unwrap_or_else(|e| panic!("cmh.toml: {e}"));
    assert!(parsed.rows.len() >= 20, "cmh.toml has too few rows");
    parsed.rows
}

fn strata(row: &Row) -> Vec<Stratum> {
    (0..row.pa.len())
        .map(|k| Stratum {
            key: Vec::new(),
            a: (row.pa[k], row.na[k]),
            b: (row.pb[k], row.nb[k]),
        })
        .collect()
}

fn run(row: &Row) -> CmhResult {
    cmh(&strata(row), LEVEL).unwrap_or_else(|e| panic!("{}: {e}", row.label))
}

fn close(label: &str, what: &str, got: Option<f64>, want: f64) {
    let got =
        got.unwrap_or_else(|| panic!("{label}: {what} is None, the second engine has {want:?}"));
    let slack = RELATIVE * want.abs().max(f64::MIN_POSITIVE);
    assert!(
        (got - want).abs() <= slack,
        "{label}: {what} is {got:?}, the second engine has {want:?} (relative {RELATIVE:e})"
    );
}

/// Guards: the χ² statistic and its p-value, without continuity correction, on every row that
/// has blocks of information: K = 2 to 6 blocks of 5 to 200 runs per arm, zero cells, dropped
/// zero-margin blocks.
/// Oracle: statsmodels `StratifiedTable.test_null_odds(correction=False)`.
#[test]
fn cmh_statistic_matches_statsmodels() {
    let mut seen = 0;
    for row in rows() {
        let got = run(&row);
        if let (Some(statistic), Some(p)) = (row.statistic, row.p) {
            close(&row.label, "statistic", got.statistic, statistic);
            close(&row.label, "p", got.p, p);
            seen += 1;
        }
    }
    assert!(seen >= 20, "only {seen} rows carry a statistic");
}

/// Guards: the Mantel–Haenszel odds ratio of B over A and the Robins–Breslow–Greenland 95 %
/// interval, on every row where the odds ratio is finite and positive; an odds ratio of 0
/// (arm B never passes) has no interval.
/// Oracle: statsmodels `oddsratio_pooled` and `oddsratio_pooled_confint(alpha=0.05)`.
#[test]
fn mh_odds_ratio_and_rbg_interval_match_statsmodels() {
    let mut intervals = 0;
    for row in rows() {
        let got = run(&row);
        if let Some(or_mh) = row.or_mh {
            close(&row.label, "or_mh", got.or_mh, or_mh);
        }
        if let (Some(lo), Some(hi)) = (row.or_lo, row.or_hi) {
            let (got_lo, got_hi) = got
                .or_ci
                .unwrap_or_else(|| panic!("{}: no interval, the second engine has one", row.label));
            close(&row.label, "or_lo", Some(got_lo), lo);
            close(&row.label, "or_hi", Some(got_hi), hi);
            intervals += 1;
        }
    }
    assert!(intervals >= 20, "only {intervals} rows carry an interval");
}

/// Guards: the Mantel–Fleiss quantity min(ΣE − ΣL, ΣU − ΣE) and its verdict against 5, including
/// a row exactly on 5 (which holds) and a row between 3 and 5 (which does not).
/// Oracle: a hand count of the lower and upper bounds of each block's first cell from its
/// margins, in exact rationals (`gen_stats.py`, `mantel_fleiss`).
#[test]
fn mantel_fleiss_matches_hand_count() {
    let (mut on_five, mut between, mut failing) = (0, 0, 0);
    for row in rows() {
        let got = run(&row);
        close(&row.label, "mantel_fleiss", Some(got.mantel_fleiss), row.mf);
        assert_eq!(
            got.mantel_fleiss_holds, row.mf_holds,
            "{}: verdict",
            row.label
        );
        on_five += usize::from((row.mf - 5.0).abs() < f64::EPSILON);
        between += usize::from((3.0..5.0).contains(&row.mf));
        failing += usize::from(!row.mf_holds);
    }
    assert!(
        on_five >= 1 && between >= 1 && failing >= 3,
        "the vectors lose their bite rows"
    );
}

/// Guards: the Breslow–Day homogeneity p-value at the Mantel–Haenszel odds ratio (no Tarone
/// adjustment), over the informative blocks only, with `K − 1` degrees of freedom; it is `None`
/// below two informative blocks.
/// Oracle: statsmodels `test_equal_odds(adjust=False)` statistic, with scipy's `chi2.sf`
/// (statsmodels writes `1 - cdf`).
#[test]
fn homogeneity_matches_statsmodels() {
    let mut seen = 0;
    for row in rows() {
        let got = run(&row);
        match row.hom_p {
            Some(hom_p) => {
                close(&row.label, "homogeneity_p", got.homogeneity_p, hom_p);
                seen += 1;
            }
            None => assert!(got.homogeneity_p.is_none(), "{}: homogeneity_p", row.label),
        }
    }
    assert!(seen >= 15, "only {seen} rows carry a homogeneity test");
}

/// Guards: the written convention on degenerate blocks. A block with a zero margin (no runs in an
/// arm, no passes, no fails) is dropped; no informative block leaves every quantity undefined
/// (the Mantel–Fleiss quantity 0, not holding); an odds ratio with Σ S = 0 is `None`, with Σ R = 0
/// it is 0 and has no interval; counts above their runs are refused.
/// Oracle: statsmodels where it returns a finite value (asserted in the generator); elsewhere the
/// convention in the module doc.
#[test]
fn degenerate_blocks_follow_the_convention() {
    let (mut all_degenerate, mut r_zero, mut s_zero) = (0, 0, 0);
    for row in rows() {
        let got = run(&row);
        assert_eq!(
            row.statistic.is_some(),
            row.informative > 0,
            "{}: vector",
            row.label
        );
        assert_eq!(
            got.statistic.is_some(),
            row.informative > 0,
            "{}: statistic",
            row.label
        );
        assert_eq!(got.p.is_some(), row.informative > 0, "{}: p", row.label);
        assert_eq!(
            got.or_mh.is_some(),
            row.or_mh.is_some(),
            "{}: or_mh",
            row.label
        );
        assert_eq!(
            got.or_ci.is_some(),
            row.or_lo.is_some(),
            "{}: or_ci",
            row.label
        );
        if row.informative == 0 {
            all_degenerate += 1;
            assert!(
                got.homogeneity_p.is_none() && !got.mantel_fleiss_holds,
                "{}",
                row.label
            );
            assert!(got.mantel_fleiss.abs() < f64::EPSILON, "{}", row.label);
        }
        r_zero += usize::from(row.or_mh == Some(0.0));
        s_zero += usize::from(row.informative > 0 && row.or_mh.is_none());
    }
    assert!(
        all_degenerate >= 2 && r_zero >= 1 && s_zero >= 1,
        "the vectors lose a degenerate case"
    );

    let over = Stratum {
        key: Vec::new(),
        a: (6, 5),
        b: (1, 5),
    };
    assert_eq!(
        cmh(&[over], LEVEL),
        Err(StatsError::CountAboveRuns { k: 6, n: 5 })
    );
    let fine = Stratum {
        key: Vec::new(),
        a: (1, 5),
        b: (3, 5),
    };
    assert!(cmh(&[fine], 1.0).is_err(), "a level of 1 has no interval");
    assert_eq!(cmh(&[], LEVEL).map(|r| r.statistic), Ok(None));
}
