#![cfg(test)]
//! Guards for what the core derives from a report: the decision summary and the spend. The store
//! lives in memory, as in the report guards, and every expected value is counted in the test.

use std::collections::BTreeSet;

use candler_core::analysis::report::{Comparison, Entry, Report, ReportInput, RunView, Unsealed};
use candler_core::evaluation::selection::select::select;
use candler_schemas::outcome::OutcomeRecord;
use serde_json::json;

use super::guard_analysis_report::{Spec, evaluator, record, view};

/// The key that names one expectation of one comparison: its stratum, factor, levels and id.
fn key_of(comparison: &Comparison, id: &str) -> String {
    format!(
        "{}|{}|{:?}|{id}",
        comparison.experiment, comparison.factor, comparison.levels
    )
}

/// Three models by two settings of another factor, four runs a cell. `E1` and `E2` are gates and
/// `E3` is a quality expectation. `gamma` has no `E2` measured at all, so every pair that holds a
/// `gamma` arm leaves `E2` out.
fn store() -> (
    candler_core::evaluation::selection::select::Selection,
    Vec<RunView>,
) {
    let passes = |model: &str, other: &str, expectation: &str| -> u32 {
        match (model, other, expectation) {
            ("alpha", "x", "E1") => 4,
            ("alpha", "y", "E1") => 3,
            ("beta", "x", "E1") => 1,
            ("beta", "y", "E1") => 0,
            ("alpha", _, "E2") => 3,
            ("beta", _, "E2") => 1,
            ("alpha", _, "E3") => 4,
            ("beta", _, "E3") => 2,
            _ => 2,
        }
    };
    let mut runs: Vec<RunView> = Vec::new();
    let mut records: Vec<OutcomeRecord> = Vec::new();
    for model in ["alpha", "beta", "gamma"] {
        for other in ["x", "y"] {
            for replicate in 1..=4_u32 {
                let mut spec = Spec::new(model, replicate);
                spec.extra = vec![("plan.other", json!(other))];
                spec.factors = vec!["plan.model", "plan.other"];
                let run = view(&spec);
                for (expectation, role) in [("E1", "gate"), ("E2", "gate"), ("E3", "quality")] {
                    let (status, cause) = if model == "gamma" && expectation == "E2" {
                        ("not_measured", Some("instrument_error"))
                    } else if replicate <= passes(model, other, expectation) {
                        ("pass", None)
                    } else {
                        ("fail", None)
                    };
                    let n = records.len() + 1;
                    records.push(record(&run, n, expectation, role, status, None, cause));
                }
                runs.push(run);
            }
        }
    }
    (select(records), runs)
}

/// Guards: the decision summary cannot disagree with the entries it summarises. Over a store of
/// two factors, three models and three expectations, every tested entry of every comparison has
/// exactly one decision with its levels, its role, its `reject` and a family size equal to the
/// tested entries of that comparison and role; every omitted entry is listed once with its
/// reason and has no decision; nothing is listed that no entry holds.
/// Oracle: the relations, counted in the test from `Report.comparisons`; no statistic is a
/// literal.
/// Bite: drop an omitted entry, flip one decision's `reject` or take the levels from another
/// pair, and this turns red.
#[test]
fn summary_agrees_with_entries() {
    let (selection, runs) = store();
    let report = Report::build(ReportInput {
        selection,
        evaluator: evaluator(),
        runs,
        unsealed: Vec::new(),
    })
    .expect("the report builds");
    let summary = &report.summary;
    let mut tested_total = 0_usize;
    let mut omitted_total = 0_usize;
    for comparison in &report.comparisons {
        for entry in &comparison.entries {
            let key = key_of(comparison, entry.id().to_string().as_str());
            let decisions: Vec<_> = summary
                .decisions
                .iter()
                .filter(|d| format!("{}|{}|{:?}|{}", d.experiment, d.factor, d.levels, d.id) == key)
                .collect();
            let omitted: Vec<_> = summary
                .omitted
                .iter()
                .filter(|o| format!("{}|{}|{:?}|{}", o.experiment, o.factor, o.levels, o.id) == key)
                .collect();
            match entry {
                Entry::Tested(tested) => {
                    tested_total += 1;
                    assert!(
                        omitted.is_empty(),
                        "{key}: a tested entry listed as omitted"
                    );
                    let [decision] = decisions.as_slice() else {
                        panic!("{key}: {} decisions for one tested entry", decisions.len());
                    };
                    assert_eq!(decision.levels, comparison.levels, "{key}");
                    assert_eq!(decision.role, tested.role, "{key}");
                    assert_eq!(decision.reject, tested.reject, "{key}");
                    let family = comparison
                        .entries
                        .iter()
                        .filter(|e| matches!(e, Entry::Tested(t) if t.role == tested.role))
                        .count();
                    assert_eq!(decision.family, family, "{key}");
                }
                Entry::Omitted { reason, role, .. } => {
                    omitted_total += 1;
                    assert!(
                        decisions.is_empty(),
                        "{key}: an omitted entry has a decision"
                    );
                    let [held] = omitted.as_slice() else {
                        panic!("{key}: {} listings of one omitted entry", omitted.len());
                    };
                    assert_eq!(held.reason.as_str(), reason.as_str(), "{key}");
                    assert_eq!(held.role, *role, "{key}");
                }
                _ => panic!("{key}: an entry kind this guard does not know"),
            }
        }
    }
    assert!(tested_total > 0, "the store tests something");
    assert!(omitted_total > 0, "the store leaves something out");
    assert_eq!(summary.decisions.len(), tested_total);
    assert_eq!(summary.omitted.len(), omitted_total);
}

/// Guards: the spend of an arm is the sum of the costs its sealed runs recorded, the runs that
/// recorded one are counted, and a sealed run without one is counted apart, never as zero; a run
/// without a seal is in neither count.
/// Oracle: three sealed runs of one cell record 0.25 and 0.5 dollars and a third records none, and
/// a fourth run is unsealed: the sum is 0.25 + 0.5 = 0.75, 2 runs are counted and 1 has no total.
/// Bite: count an unsealed run, or treat a missing cost as 0 and count it, and this turns red.
#[test]
fn spend_sums_recorded_costs_and_counts_the_rest() {
    let mut records: Vec<OutcomeRecord> = Vec::new();
    let mut runs: Vec<RunView> = Vec::new();
    for (replicate, cost) in [(1_u32, Some(0.25)), (2, Some(0.5)), (3, None)] {
        let mut spec = Spec::new("alpha", replicate);
        if let Some(dollars) = cost {
            spec.measured = vec![("run.cost_usd", json!(dollars))];
        }
        let run = view(&spec);
        let n = records.len() + 1;
        records.push(record(&run, n, "E1", "gate", "pass", None, None));
        runs.push(run);
    }
    let unsealed = view(&Spec::new("alpha", 4));
    let report = Report::build(ReportInput {
        selection: select(records),
        evaluator: evaluator(),
        runs,
        unsealed: vec![Unsealed {
            run: unsealed.manifest.run().clone(),
            experiment: Some(unsealed.manifest.experiment().clone()),
        }],
    })
    .expect("the report builds");
    let arms: BTreeSet<u64> = report.arms.iter().map(|arm| arm.unsealed).collect();
    assert_eq!(arms, BTreeSet::from([1]), "one arm holds the unsealed run");
    let [arm] = report.arms.as_slice() else {
        panic!("one arm");
    };
    assert!(
        (arm.spent.cost_usd - 0.75).abs() <= 1e-12,
        "{:?}",
        arm.spent
    );
    assert_eq!(arm.spent.runs_counted, 2);
    assert_eq!(arm.spent.runs_without_total, 1);
}
