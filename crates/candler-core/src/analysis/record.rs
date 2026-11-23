//! The analysis report record: the statistics of a [`Report`] written as the versioned record
//! `candler analyze` prints. Nothing is computed here; every value is the report's own.

use candler_schemas::analysis::{
    Alpha, AnalysisReport, Arm, ArmExpectation, ArmRuns, BlockEffect, BlockLevel,
    ComparabilityClass, Comparison, Counts, Decision, Entry, Family, Fraction, InstrumentError,
    Interval, Omitted, OmittedEntry, Rates, Reading, RunRow, Spent, Status, Summary, Tested,
};
use serde::de::Error as _;

use super::blocking::Battery;
use super::report::{self, ArmReport, ExpectationReport, Report, role_name};
use super::spend::Spend;
use super::summary;
use super::tally::Rate;
use super::{ALPHA, alpha_source};

/// A count as the record's 64-bit number.
fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// The test that produced a p-value, by name.
const fn battery_name(battery: Battery) -> &'static str {
    match battery {
        Battery::MidP => "mid_p",
        Battery::Cmh => "cmh",
        Battery::ExactStrata => "exact_strata",
        Battery::Permutation => "permutation",
    }
}

fn fraction(rate: Rate) -> Fraction {
    Fraction {
        num: rate.num(),
        den: rate.den(),
    }
}

/// A spend as the record states it.
#[must_use]
pub const fn spent(spend: &Spend) -> Spent {
    Spent {
        cost_usd: spend.cost_usd,
        runs_counted: spend.runs_counted,
        runs_without_total: spend.runs_without_total,
    }
}

fn levels(
    levels: Option<&(serde_json::Value, serde_json::Value)>,
) -> Option<(serde_json::Value, serde_json::Value)> {
    levels.cloned() // BORROW: the record owns its levels
}

fn expectation(held: &ExpectationReport) -> ArmExpectation {
    let tally = &held.tally;
    let (wilson, omitted) = match &held.wilson {
        Ok(interval) => (
            Some(Interval {
                lo: interval.lo(),
                hi: interval.hi(),
            }),
            None,
        ),
        Err(reason) => (None, Some(reason.as_str().to_owned())),
    };
    ArmExpectation {
        id: held.id.to_string(),
        role: role_name(tally.role).to_owned(),
        pass: tally.pass,
        fail: tally.fail,
        blocked: tally.blocked,
        not_measured: tally.not_measured.clone(), // BORROW: the record owns its map
        not_applicable: tally.not_applicable.clone(), // BORROW: the record owns its map
        zero_weight: tally.zero_weight,
        weight: Fraction {
            num: tally.num,
            den: tally.den,
        },
        wilson,
        omitted,
    }
}

fn arm(arm: &ArmReport) -> Arm {
    Arm {
        experiment: arm.experiment.clone(), // BORROW: the record owns its text
        cell: arm.cell.to_string(),
        runs: ArmRuns {
            valid: arm.tally.valid_runs,
            unsealed: arm.unsealed,
            invalid: arm.tally.invalid.clone(), // BORROW: the record owns its map
            experimenter_ends: arm.tally.experimenter_ends.clone(), // BORROW: the record owns its map
        },
        rates: Rates {
            gate: fraction(arm.gate),
            quality: fraction(arm.quality),
        },
        status: Status {
            pass: arm.status.pass,
            fail: arm.status.fail,
            blocked: arm.status.blocked,
            not_measured: arm.status.not_measured,
            not_applicable: arm.status.not_applicable,
        },
        instrument_error: InstrumentError {
            runs: arm.instrument_error.runs,
            outcomes: arm.instrument_error.outcomes,
        },
        expectations: arm.expectations.iter().map(expectation).collect(),
        spent: spent(&arm.spent),
    }
}

fn tested(tested: &report::Tested) -> Tested {
    Tested {
        id: tested.id.to_string(),
        role: role_name(tested.role).to_owned(),
        a: Counts {
            k: tested.a.k(),
            n: tested.a.n(),
        },
        b: Counts {
            k: tested.b.k(),
            n: tested.b.n(),
        },
        p: tested.p,
        p_holm: tested.p_holm,
        reject: tested.reject,
        diff: tested.diff,
        diff_lo: tested.diff_lo,
        diff_hi: tested.diff_hi,
        battery: tested.battery.map(|held| battery_name(held).to_owned()),
        fallback: tested.fallback,
        blocks: count(tested.blocks),
        interval_reading: match tested.reading {
            report::Reading::Effect => Reading::Effect,
            report::Reading::Inconclusive => Reading::Inconclusive,
        },
        or_mh: tested.or_mh,
        or_lo: tested.or_ci.map(|(lo, _)| lo),
        or_hi: tested.or_ci.map(|(_, hi)| hi),
        homogeneity_p: tested.homogeneity_p,
        per_block: tested
            .per_block
            .iter()
            .map(|effect| BlockEffect {
                key: effect
                    .key
                    .iter()
                    .map(|(factor, level)| BlockLevel {
                        factor: factor.to_string(),
                        level: level.clone(), // BORROW: the record owns its level
                    })
                    .collect(),
                a: Counts {
                    k: effect.a.0,
                    n: effect.a.1,
                },
                b: Counts {
                    k: effect.b.0,
                    n: effect.b.1,
                },
                diff: effect.diff,
            })
            .collect(),
    }
}

fn entry(entry: &report::Entry) -> Entry {
    match entry {
        report::Entry::Tested(held) => Entry::Tested(Box::new(tested(held))),
        report::Entry::Omitted { id, role, reason } => Entry::Omitted(OmittedEntry {
            id: id.to_string(),
            role: role_name(*role).to_owned(),
            omitted: reason.as_str().to_owned(),
        }),
    }
}

fn comparison(comparison: &report::Comparison) -> Comparison {
    Comparison {
        experiment: comparison.experiment.clone(), // BORROW: the record owns its text
        a: comparison.a.to_string(),
        b: comparison.b.to_string(),
        factor: comparison.factor.to_string(),
        levels: levels(comparison.levels.as_ref()),
        comparability_class: ComparabilityClass {
            verdict: comparison.verdict.as_str().to_owned(),
            causes: comparison.verdict.causes().to_vec(),
        },
        entries: comparison.entries.iter().map(entry).collect(),
    }
}

fn decision(decision: &summary::Decision) -> Decision {
    Decision {
        experiment: decision.experiment.clone(), // BORROW: the record owns its text
        factor: decision.factor.to_string(),
        levels: levels(decision.levels.as_ref()),
        id: decision.id.to_string(),
        role: role_name(decision.role).to_owned(),
        diff: decision.diff,
        diff_lo: decision.diff_lo,
        diff_hi: decision.diff_hi,
        p_holm: decision.p_holm,
        reject: decision.reject,
        family: count(decision.family),
    }
}

fn omitted(held: &summary::Omitted) -> Omitted {
    Omitted {
        experiment: held.experiment.clone(), // BORROW: the record owns its text
        factor: held.factor.to_string(),
        levels: levels(held.levels.as_ref()),
        id: held.id.to_string(),
        role: role_name(held.role).to_owned(),
        reason: held.reason.as_str().to_owned(),
    }
}

fn alpha() -> Result<Alpha, serde_json::Error> {
    let source = serde_json::to_value(alpha_source())?;
    let kind = source
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| serde_json::Error::custom("the source of alpha has no kind"))?;
    Ok(Alpha {
        value: ALPHA,
        source: kind.to_owned(),
    })
}

/// The report as the record `analyze` prints.
///
/// # Errors
/// Returns the serialiser's error when the source of alpha cannot be named.
pub fn analysis_report(report: &Report) -> Result<AnalysisReport, serde_json::Error> {
    Ok(AnalysisReport::new(
        report.evaluator.to_string(),
        alpha()?,
        count(report.i4_violations),
        report.arms.iter().map(arm).collect(),
        report.comparisons.iter().map(comparison).collect(),
        report
            .families
            .iter()
            .map(|family| Family {
                role: role_name(family.role).to_owned(),
                m: count(family.m),
            })
            .collect(),
        count(report.pairs_not_one_factor),
        report
            .runs
            .iter()
            .map(|row| RunRow {
                run: row.run.to_string(),
                cell: row.cell.to_string(),
                integrity: row.integrity.clone(), // BORROW: the record owns its integrity
                comparable: row.comparable.clone(), // BORROW: the record owns its map
                measured: row.measured.clone(),   // BORROW: the record owns its map
            })
            .collect(),
        Summary {
            decisions: report.summary.decisions.iter().map(decision).collect(),
            omitted: report.summary.omitted.iter().map(omitted).collect(),
        },
    ))
}
