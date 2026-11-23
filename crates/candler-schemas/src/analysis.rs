//! The record `candler analyze` prints: the report of one evaluator over a store.
//!
//! It holds the arms with what each spent, the pairs of arms compared with the entries tested or
//! left out, every run, and the summary of what was decided. The CLI prints it as JSON, or renders
//! its text and CSV forms from it; the core builds it from the statistics it computed.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::execution::Integrity;
use crate::generate::GeneratedSchema;
use crate::primitives::schema_tag;

/// The version marker of the analysis report.
pub const ANALYSIS_REPORT_SCHEMA: &str = "analysis-report/1";

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    vec![GeneratedSchema::of::<AnalysisReport>("analysis-report")]
}

schema_tag! {
    /// The constant `schema` field of the analysis report.
    pub AnalysisReportTag = ANALYSIS_REPORT_SCHEMA
}

/// The report of one evaluator over a store: what `candler analyze` prints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnalysisReport {
    /// The version marker of the document: always `analysis-report/1`.
    pub schema: AnalysisReportTag,
    /// The evaluator, as `name@version`.
    pub evaluator: String,
    /// The significance level of every test and where it comes from.
    pub alpha: Alpha,
    /// How many records of the evaluator break the rule that a run holds one current outcome per
    /// expectation.
    pub i4_violations: u64,
    /// The arms: one per cell within a comparability class.
    pub arms: Vec<Arm>,
    /// The pairs of arms that differ in exactly one declared factor.
    pub comparisons: Vec<Comparison>,
    /// The tests each Holm adjustment covers together, by role.
    pub families: Vec<Family>,
    /// The pairs of one comparability class that differ in no declared factor or in several, and
    /// are not compared.
    pub pairs_not_one_factor: u64,
    /// Every run, by run key.
    pub runs: Vec<RunRow>,
    /// One decision per tested entry and one line per entry left out.
    pub summary: Summary,
}

impl AnalysisReport {
    /// A report with its version marker.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the report is built once, from the nine parts it holds"
    )]
    pub const fn new(
        evaluator: String,
        alpha: Alpha,
        i4_violations: u64,
        arms: Vec<Arm>,
        comparisons: Vec<Comparison>,
        families: Vec<Family>,
        pairs_not_one_factor: u64,
        runs: Vec<RunRow>,
        summary: Summary,
    ) -> Self {
        Self {
            schema: AnalysisReportTag,
            evaluator,
            alpha,
            i4_violations,
            arms,
            comparisons,
            families,
            pairs_not_one_factor,
            runs,
            summary,
        }
    }
}

/// The significance level with the place it comes from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Alpha {
    /// The level.
    pub value: f64,
    /// The kind of source the level comes from.
    pub source: String,
}

/// A count and the total it is out of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Fraction {
    /// The numerator.
    pub num: u64,
    /// The denominator.
    pub den: u64,
}

/// Passes in graded runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    /// The passes.
    pub k: u64,
    /// The graded runs.
    pub n: u64,
}

/// An interval with its two limits.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Interval {
    /// The lower limit.
    pub lo: f64,
    /// The upper limit.
    pub hi: f64,
}

/// What the sealed runs of an arm recorded as cost.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Spent {
    /// The sum of the recorded costs, in dollars.
    pub cost_usd: f64,
    /// The runs whose cost is in the sum.
    pub runs_counted: u64,
    /// The runs that recorded no cost, which the sum does not cover.
    pub runs_without_total: u64,
}

/// The runs of an arm by what became of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArmRuns {
    /// The valid sealed runs.
    pub valid: u64,
    /// The runs that have no seal yet, counted in no rate and no verdict.
    pub unsealed: u64,
    /// The invalid runs, by cause.
    pub invalid: BTreeMap<String, u64>,
    /// The runs the experimenter ended, by cause.
    pub experimenter_ends: BTreeMap<String, u64>,
}

/// The gate rate and the quality rate of an arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rates {
    /// The weighted pass rate of the gate expectations.
    pub gate: Fraction,
    /// The weighted pass rate of the quality expectations.
    pub quality: Fraction,
}

/// The status totals of an arm over its gate and quality expectations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Status {
    /// Passes, with the passes of weight 0/0.
    pub pass: u64,
    /// Failures.
    pub fail: u64,
    /// Outcomes blocked for want of an input.
    pub blocked: u64,
    /// Outcomes not measured, whatever the cause.
    pub not_measured: u64,
    /// Outcomes that do not apply, whatever the cause.
    pub not_applicable: u64,
}

/// What an arm lost to the instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstrumentError {
    /// Valid runs with at least one outcome not measured because of the instrument.
    pub runs: u64,
    /// The outcomes not measured because of the instrument.
    pub outcomes: u64,
}

/// One expectation in one arm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArmExpectation {
    /// The expectation id.
    pub id: String,
    /// Its role: `gate`, `quality` or `descriptive`.
    pub role: String,
    /// Passes.
    pub pass: u64,
    /// Failures.
    pub fail: u64,
    /// Outcomes blocked for want of an input.
    pub blocked: u64,
    /// Outcomes not measured, by cause.
    pub not_measured: BTreeMap<String, u64>,
    /// Outcomes that do not apply, by cause.
    pub not_applicable: BTreeMap<String, u64>,
    /// Passes of weight 0/0.
    pub zero_weight: u64,
    /// The weighted passes over the weight.
    pub weight: Fraction,
    /// The Wilson interval of the pass proportion; null when there is none.
    pub wilson: Option<Interval>,
    /// Why there is no Wilson interval; null when there is one.
    pub omitted: Option<String>,
}

/// One arm: a cell within a comparability class, with its tally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Arm {
    /// The experiment digest of the comparability class the arm belongs to.
    pub experiment: String,
    /// The cell.
    pub cell: String,
    /// Its runs.
    pub runs: ArmRuns,
    /// Its gate and quality rates.
    pub rates: Rates,
    /// Its status totals.
    pub status: Status,
    /// What the instrument cost it.
    pub instrument_error: InstrumentError,
    /// Its expectations, by id.
    pub expectations: Vec<ArmExpectation>,
    /// The cost its sealed runs recorded.
    pub spent: Spent,
}

/// The effect of arm B over arm A in one block of a tested entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BlockEffect {
    /// The block's key: each blocking factor with its level, in the order of the declared factors.
    pub key: Vec<BlockLevel>,
    /// Arm A's weighted passes and runs in the block.
    pub a: Counts,
    /// Arm B's, in the same terms.
    pub b: Counts,
    /// The rate of arm B minus the rate of arm A in the block.
    pub diff: f64,
}

/// A blocking factor and the level a block holds it at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BlockLevel {
    /// The factor, as a field path.
    pub factor: String,
    /// The level.
    pub level: Value,
}

/// What the interval of a test says: never that there is no difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Reading {
    /// The interval excludes the null value: 0 for a difference, 1 for an odds ratio.
    Effect,
    /// The interval spans the null value, or the test gives none.
    Inconclusive,
}

/// An expectation tested between the two arms of a comparable pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Tested {
    /// The expectation id.
    pub id: String,
    /// Its role.
    pub role: String,
    /// Passes and graded runs of arm A.
    pub a: Counts,
    /// Passes and graded runs of arm B.
    pub b: Counts,
    /// The two-sided p-value of the test.
    pub p: f64,
    /// The Holm-adjusted p over the family of the role.
    pub p_holm: f64,
    /// Whether the adjusted p is below alpha.
    pub reject: bool,
    /// The difference of arm B's rate over arm A's; null when the test gives none.
    pub diff: Option<f64>,
    /// The lower limit of the interval of `diff`.
    pub diff_lo: Option<f64>,
    /// The upper limit of the interval of `diff`.
    pub diff_hi: Option<f64>,
    /// The test that produced `p`: `mid_p`, `cmh`, `exact_strata` or `permutation`; null for the
    /// one-factor Fisher test.
    pub battery: Option<String>,
    /// Whether the exact conditional test replaced a CMH test whose criterion failed.
    pub fallback: bool,
    /// The number of blocks the test ran over.
    pub blocks: u64,
    /// What the interval says: it never reads as "no difference".
    pub interval_reading: Reading,
    /// The Mantel-Haenszel odds ratio of arm B over arm A.
    pub or_mh: Option<f64>,
    /// The lower limit of the odds ratio's interval.
    pub or_lo: Option<f64>,
    /// The upper limit of the odds ratio's interval.
    pub or_hi: Option<f64>,
    /// The p-value of the Breslow-Day homogeneity test: shown, never used to choose.
    pub homogeneity_p: Option<f64>,
    /// The effect of arm B over arm A in each block.
    pub per_block: Vec<BlockEffect>,
}

/// An expectation of a comparable pair that was not tested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OmittedEntry {
    /// The expectation id.
    pub id: String,
    /// Its role.
    pub role: String,
    /// Why no test was made.
    pub omitted: String,
}

/// An expectation of a pair, tested or left out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[non_exhaustive]
pub enum Entry {
    /// Both arms hold plain counts: the expectation was tested.
    Tested(Box<Tested>),
    /// No test was made.
    Omitted(OmittedEntry),
}

/// What the verdict said of the two arms of a pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComparabilityClass {
    /// `comparable`, `refused` or `malformed`.
    pub verdict: String,
    /// The paths or reasons that keep the pair from a verdict; empty for a comparable pair.
    pub causes: Vec<String>,
}

/// One pair of arms that differ in exactly one declared factor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Comparison {
    /// The experiment digest of the comparability class the pair is compared in.
    pub experiment: String,
    /// The first arm's cell.
    pub a: String,
    /// The second arm's cell.
    pub b: String,
    /// The declared factor the arms differ in.
    pub factor: String,
    /// The levels of the factor the two arms are at; null when the comparison does not state them.
    pub levels: Option<(Value, Value)>,
    /// What the verdict said of the pair.
    pub comparability_class: ComparabilityClass,
    /// The expectations graded in both arms; empty unless the pair is comparable.
    pub entries: Vec<Entry>,
}

/// The tests of one role that Holm's adjustment covers together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Family {
    /// The role.
    pub role: String,
    /// The number of tests in the family.
    pub m: u64,
}

/// One run: the values it recorded, by path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunRow {
    /// The run key.
    pub run: String,
    /// Its cell.
    pub cell: String,
    /// Whether it counts, and why not.
    pub integrity: Integrity,
    /// Every recorded value of the cell, by path.
    pub comparable: BTreeMap<String, Value>,
    /// Every measured value, by path.
    pub measured: BTreeMap<String, Value>,
}

/// One expectation of a comparison that was tested, with what it came to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    /// The experiment digest of the comparability class of the comparison.
    pub experiment: String,
    /// The declared factor the arms differ in.
    pub factor: String,
    /// The levels that label arm A and arm B; null when the comparison does not state them.
    pub levels: Option<(Value, Value)>,
    /// The expectation id.
    pub id: String,
    /// Its role.
    pub role: String,
    /// The difference, arm B minus arm A; null when the test gives none.
    pub diff: Option<f64>,
    /// The lower limit of the interval of `diff`.
    pub diff_lo: Option<f64>,
    /// The upper limit of the interval of `diff`.
    pub diff_hi: Option<f64>,
    /// The Holm-adjusted p over the family of the role.
    pub p_holm: f64,
    /// Whether the adjusted p is below alpha.
    pub reject: bool,
    /// The number of tests in the Holm family.
    pub family: u64,
}

/// One expectation of a comparison that was left out of the tests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Omitted {
    /// The experiment digest of the comparability class of the comparison.
    pub experiment: String,
    /// The declared factor the arms differ in.
    pub factor: String,
    /// The levels that label arm A and arm B; null when the comparison does not state them.
    pub levels: Option<(Value, Value)>,
    /// The expectation id.
    pub id: String,
    /// Its role.
    pub role: String,
    /// Why it was left out.
    pub reason: String,
}

/// What a report decided.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    /// One decision per tested entry, in the order of the comparisons.
    pub decisions: Vec<Decision>,
    /// The entries left out, with their reasons.
    pub omitted: Vec<Omitted>,
}
