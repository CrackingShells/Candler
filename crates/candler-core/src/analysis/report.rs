//! The report of `analyze`: one evaluator version, the tally of every arm and, for each pair of
//! arms that differ in exactly one declared factor, the tests of the pair.
//!
//! Nothing here reads the store or the clock: the caller hands over the selection, the facts of
//! every run and the manifests. Counts come from [`tally`], methods from the
//! sibling modules, and whether two arms may be compared is answered by
//! [`stratum_verdict`] alone.

use std::collections::{BTreeMap, BTreeSet};

use candler_schemas::execution::{Integrity, RunManifest};
use candler_schemas::outcome::Role;
use candler_schemas::primitives::{Digest, ExpectationId, FieldPath, NameVersion, RunKey};
use serde_json::Value;

use super::error::StatsError;
use super::fisher::fisher_two_sided;
use super::holm::{holm, rejected};
use super::newcombe::newcombe_difference;
use super::refusal::ReportError;
use super::tally::{ArmTally, ExpectationTally, Omitted, Rate, RunFacts, tally};
use super::wilson::{Counts, Interval};
use crate::evaluation::selection::rates::CellKey;
use crate::evaluation::selection::select::{Selection, select};
use crate::manifest::compare::{ArmView, Verdict, compare, covers, stratum_verdict};

/// The role as it is reported: `gate`, `quality` or `descriptive`.
#[must_use]
pub const fn role_name(role: Role) -> &'static str {
    match role {
        Role::Gate => "gate",
        Role::Quality => "quality",
        Role::Descriptive => "descriptive",
    }
}

/// The evaluator whose outcomes a report covers.
///
/// Without a request there must be exactly one evaluator among the current records: the choice is
/// derived, and a store holding several is refused, because no option asks for a mix. With a
/// request, `name@version` must be one of them; a bare `SemVer` version selects the evaluators that
/// hold it, and exactly one must.
///
/// # Errors
/// [`ReportError::NoOutcomes`] when the selection is empty, [`ReportError::MixedEvaluatorVersions`]
/// for several evaluators and no request, [`ReportError::UnknownEvaluator`] when the request
/// matches none, and [`ReportError::AmbiguousEvaluatorName`] when a bare version matches several.
pub fn choose_evaluator(
    selection: &Selection,
    requested: Option<&str>,
) -> Result<NameVersion, ReportError> {
    let mut found: BTreeMap<String, NameVersion> = BTreeMap::new();
    for record in selection.current() {
        found
            .entry(record.evaluator().to_string())
            // BORROW: the map owns the evaluator
            .or_insert_with(|| record.evaluator().clone());
    }
    let listed = || found.keys().cloned().collect::<Vec<String>>();
    let mut all = found.values();
    let (Some(first), second) = (all.next(), all.next()) else {
        return Err(ReportError::NoOutcomes);
    };
    let Some(text) = requested else {
        return match second {
            None => Ok(first.clone()), // BORROW: the caller owns the choice
            Some(_) => Err(ReportError::MixedEvaluatorVersions { found: listed() }),
        };
    };
    let unknown = || ReportError::UnknownEvaluator {
        requested: text.to_owned(), // BORROW: the error owns the text
        found: listed(),
    };
    if text.contains('@') {
        return found.get(text).cloned().ok_or_else(unknown);
    }
    let Ok(version) = semver::Version::parse(text) else {
        return Err(unknown());
    };
    let holders: Vec<&NameVersion> = found
        .values()
        .filter(|held| *held.version() == version)
        .collect();
    match holders.as_slice() {
        [] => Err(unknown()),
        [only] => Ok((*only).clone()), // BORROW: the caller owns the choice
        several => Err(ReportError::AmbiguousEvaluatorName {
            version: text.to_owned(), // BORROW: the error owns the text
            names: several.iter().map(ToString::to_string).collect(),
        }),
    }
}

/// One run as the report reads it: whether it counts, and its manifest.
#[derive(Debug, Clone)]
pub struct RunView {
    /// Whether the run counts, from its manifest.
    pub facts: RunFacts,
    /// The manifest of the run.
    pub manifest: RunManifest,
}

/// What a report is built from.
#[derive(Debug, Clone)]
pub struct ReportInput {
    /// The current outcome records of the store.
    pub selection: Selection,
    /// The evaluator the report covers.
    pub evaluator: NameVersion,
    /// Every sealed run of the store.
    pub runs: Vec<RunView>,
    /// The runs without a seal: counted apart, in no `k`, no `n` and no verdict.
    pub unsealed: Vec<Unsealed>,
}

/// A run that has no seal yet.
#[derive(Debug, Clone)]
pub struct Unsealed {
    /// The run.
    pub run: RunKey,
    /// The experiment its manifest names, or `None` when the manifest cannot be read.
    pub experiment: Option<Digest>,
}

/// The stratum of an unsealed run whose manifest cannot be read.
pub const UNKNOWN_STRATUM: &str = "unknown";

/// The status totals of one arm over its gate and quality expectations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct StatusTotals {
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

/// One expectation in one arm: its tally and its Wilson interval or the reason it has none.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ExpectationReport {
    /// The expectation.
    pub id: ExpectationId,
    /// What was counted.
    pub tally: ExpectationTally,
    /// The Wilson interval of the pass proportion, or why there is none.
    pub wilson: Result<Interval, Omitted>,
}

/// One arm: a cell, with its tally.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ArmReport {
    /// The experiment digest of the stratum the arm belongs to: an arm is a cell within a
    /// stratum, and the runs of another stratum are not in it.
    pub experiment: String,
    /// The cell.
    pub cell: CellKey,
    /// The runs that have no seal yet, counted in no `k`, no `n` and no verdict.
    pub unsealed: u64,
    /// Its runs and expectations as counted.
    pub tally: ArmTally,
    /// The gate rate.
    pub gate: Rate,
    /// The quality rate.
    pub quality: Rate,
    /// The status totals over gate and quality.
    pub status: StatusTotals,
    /// The expectations, by id.
    pub expectations: Vec<ExpectationReport>,
}

impl ArmReport {
    fn new(experiment: String, cell: CellKey, tally: ArmTally) -> Self {
        let mut status = StatusTotals::default();
        let mut expectations = Vec::new();
        for (id, held) in &tally.expectations {
            if matches!(held.role, Role::Gate | Role::Quality) {
                status.pass = status.pass.saturating_add(held.pass + held.zero_weight);
                status.fail = status.fail.saturating_add(held.fail);
                status.blocked = status.blocked.saturating_add(held.blocked);
                status.not_measured = status
                    .not_measured
                    .saturating_add(held.not_measured.values().sum());
                status.not_applicable = status
                    .not_applicable
                    .saturating_add(held.not_applicable.values().sum());
            }
            expectations.push(ExpectationReport {
                id: id.clone(),      // BORROW: the report owns the id
                tally: held.clone(), // BORROW: the report owns the tally
                wilson: held.wilson(),
            });
        }
        Self {
            gate: tally.rate(Role::Gate),
            quality: tally.rate(Role::Quality),
            experiment,
            cell,
            unsealed: 0,
            tally,
            status,
            expectations,
        }
    }
}

/// One run row: the recorded fields of the run, an open map that holds whatever path a manifest
/// records.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RunRow {
    /// The run.
    pub run: RunKey,
    /// Its cell.
    pub cell: CellKey,
    /// Whether it counts.
    pub integrity: Integrity,
    /// Every recorded value of the cell, by path.
    pub comparable: BTreeMap<String, Value>,
    /// Every measured value, by path.
    pub measured: BTreeMap<String, Value>,
}

impl RunRow {
    fn of(view: &RunView) -> Self {
        let mut comparable = BTreeMap::new();
        for value in view.manifest.comparable().values() {
            if let Some(held) = value.value() {
                // BORROW: the row owns its path and value
                comparable.insert(value.path().to_string(), held.as_value().clone());
            }
        }
        let mut measured = BTreeMap::new();
        for entry in view.manifest.measured() {
            // BORROW: the row owns its path and value
            measured.insert(entry.path().to_string(), entry.value().as_value().clone());
        }
        Self {
            run: view.facts.run.clone(), // BORROW: the row owns the key
            cell: CellKey::new(view.facts.run.cell().clone()), // BORROW: the row owns the cell
            integrity: view.facts.integrity.clone(), // BORROW: the row owns the integrity
            comparable,
            measured,
        }
    }
}

/// What the verdict said of the two arms of a pair.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PairVerdict {
    /// The arms differ only in the declared factor.
    Comparable,
    /// The arms differ elsewhere, at these paths.
    Refused(Vec<String>),
    /// An arm could not be read, for these reasons.
    Malformed(Vec<String>),
}

impl PairVerdict {
    /// The verdict as it is reported: `comparable`, `refused` or `malformed`.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Comparable => "comparable",
            Self::Refused(_) => "refused",
            Self::Malformed(_) => "malformed",
        }
    }

    /// The causes of a refusal or a malformed arm; empty for a comparable pair.
    #[must_use]
    pub fn causes<'a>(&'a self) -> &'a [String] {
        match self {
            Self::Comparable => &[],
            // BORROW: explicit .as_slice() instead of Deref coercion
            Self::Refused(causes) | Self::Malformed(causes) => causes.as_slice(),
        }
    }
}

/// An expectation tested between the two arms of a comparable pair.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Tested {
    /// The expectation.
    pub id: ExpectationId,
    /// Its role.
    pub role: Role,
    /// Passes and graded runs of the first arm.
    pub a: Counts,
    /// Passes and graded runs of the second arm.
    pub b: Counts,
    /// Fisher's two-sided p.
    pub p: f64,
    /// The Holm-adjusted p over the family of the role.
    pub p_holm: f64,
    /// Whether the adjusted p is below alpha.
    pub reject: bool,
    /// The first arm's proportion minus the second's.
    pub diff: f64,
    /// The lower limit of Newcombe's interval of the difference.
    pub diff_lo: f64,
    /// The upper limit of Newcombe's interval of the difference.
    pub diff_hi: f64,
}

/// Why an expectation of a pair is not tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OmitReason {
    /// An arm holds no plain counts, for the reason of its tally.
    Counts(Omitted),
    /// Only one of the two arms graded the expectation.
    OneArm,
}

impl OmitReason {
    /// The reason as it is reported: the tally's, or `one_arm`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Counts(reason) => reason.as_str(),
            Self::OneArm => "one_arm",
        }
    }
}

/// An expectation of a pair, tested or left out.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Entry {
    /// Both arms hold plain counts: the expectation was tested.
    Tested(Tested),
    /// No test was made.
    Omitted {
        /// The expectation.
        id: ExpectationId,
        /// Its role.
        role: Role,
        /// Why.
        reason: OmitReason,
    },
}

impl Entry {
    /// The expectation.
    #[must_use]
    pub const fn id<'a>(&'a self) -> &'a ExpectationId {
        match self {
            Self::Tested(tested) => &tested.id,
            Self::Omitted { id, .. } => id,
        }
    }

    /// The role of the expectation.
    #[must_use]
    pub const fn role(&self) -> Role {
        match self {
            Self::Tested(tested) => tested.role,
            Self::Omitted { role, .. } => *role,
        }
    }
}

/// One pair of arms that differ in exactly one declared factor.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Comparison {
    /// The experiment digest of the stratum the pair is compared in.
    pub experiment: String,
    /// The first arm, by cell.
    pub a: CellKey,
    /// The second arm.
    pub b: CellKey,
    /// The declared factor the arms differ in.
    pub factor: FieldPath,
    /// What the verdict said.
    pub verdict: PairVerdict,
    /// The expectations graded in both arms; empty unless the pair is comparable.
    pub entries: Vec<Entry>,
}

/// The tests of one role that Holm's adjustment covers together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Family {
    /// The role.
    pub role: Role,
    /// The number of tests in the family.
    pub m: usize,
}

/// The report of one evaluator over a store.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Report {
    /// The evaluator.
    pub evaluator: NameVersion,
    /// The I4 violations among the records of the evaluator.
    pub i4_violations: usize,
    /// The arms, by stratum and cell.
    pub arms: Vec<ArmReport>,
    /// The compared pairs, by stratum and cells.
    pub comparisons: Vec<Comparison>,
    /// The Holm families, by role, for the roles that have a test.
    pub families: Vec<Family>,
    /// The pairs of one stratum that differ in no declared factor or in several, which are not
    /// compared.
    pub pairs_not_one_factor: usize,
    /// Every run, by run key.
    pub runs: Vec<RunRow>,
}

impl Report {
    /// Builds the report.
    ///
    /// # Errors
    /// The [`StatsError`] of the tally or of a method: none is an author's to fix.
    pub fn build(input: ReportInput) -> Result<Self, StatsError> {
        let ReportInput {
            selection,
            evaluator,
            mut runs,
            unsealed,
        } = input;
        runs.sort_by(|x, y| x.facts.run.cmp(&y.facts.run));
        let listed: BTreeSet<&RunKey> = runs
            .iter()
            .map(|view| &view.facts.run)
            .chain(unsealed.iter().map(|held| &held.run))
            .collect();
        for record in selection
            .current()
            .filter(|record| record.evaluator() == &evaluator)
        {
            if !listed.contains(record.run()) {
                return Err(StatsError::RunNotListed {
                    run: record.run().clone(), // BORROW: the error owns the run
                });
            }
        }
        // An arm is a cell within a stratum: each stratum is tallied over its own runs.
        let mut strata: BTreeMap<String, Vec<&RunView>> = BTreeMap::new();
        for view in &runs {
            strata
                .entry(view.manifest.experiment().to_string())
                .or_default()
                .push(view);
        }
        let mut arms: BTreeMap<(String, CellKey), ArmReport> = BTreeMap::new();
        for (stratum, views) in &strata {
            let members: BTreeSet<&RunKey> = views.iter().map(|view| &view.facts.run).collect();
            let own = select(
                selection
                    .current()
                    .filter(|record| members.contains(record.run()))
                    .cloned(), // BORROW: the stratum's selection owns its records
            );
            let facts: Vec<RunFacts> = views.iter().map(|view| view.facts.clone()).collect(); // BORROW: the tally reads its own copy
            for (cell, held) in tally(&own, &evaluator, facts.as_slice())? {
                // BORROW: the key and the arm own the stratum text
                arms.insert(
                    (stratum.clone(), cell.clone()),
                    ArmReport::new(stratum.clone(), cell, held),
                );
            }
        }
        for held in &unsealed {
            let stratum = held
                .experiment
                .as_ref()
                .map_or_else(|| UNKNOWN_STRATUM.to_owned(), ToString::to_string);
            let cell = CellKey::new(held.run.cell().clone()); // BORROW: the key owns the cell
            arms.entry((stratum.clone(), cell.clone()))
                .or_insert_with(|| ArmReport::new(stratum, cell, ArmTally::default()))
                .unsealed += 1;
        }
        let arms: Vec<ArmReport> = arms.into_values().collect();
        let i4_violations = selection
            .violations()
            .iter()
            .filter(|violation| violation.key.evaluator == evaluator)
            .count();
        let pairs = compare_pairs(runs.as_slice(), arms.as_slice())?;
        Ok(Self {
            evaluator,
            i4_violations,
            comparisons: pairs.comparisons,
            families: pairs.families,
            pairs_not_one_factor: pairs.not_one_factor,
            runs: runs.iter().map(RunRow::of).collect(),
            arms,
        })
    }
}

/// What comparing the pairs gave.
#[derive(Debug)]
pub struct Pairs {
    /// The compared pairs, by `(a, b)`.
    pub comparisons: Vec<Comparison>,
    /// The Holm families.
    pub families: Vec<Family>,
    /// The pairs that are not one-factor pairs.
    pub not_one_factor: usize,
}

/// The valid runs of each cell, by experiment digest: a stratum is the cells whose manifests name
/// one experiment.
type Strata<'a> = BTreeMap<String, BTreeMap<CellKey, Vec<&'a RunView>>>;

fn strata_of(runs: &[RunView]) -> Strata<'_> {
    let mut strata: Strata<'_> = BTreeMap::new();
    for view in runs
        .iter()
        .filter(|view| matches!(view.facts.integrity, Integrity::Valid))
    {
        strata
            .entry(view.manifest.experiment().to_string())
            .or_default()
            .entry(CellKey::new(view.facts.run.cell().clone())) // BORROW: the key owns the cell
            .or_default()
            .push(view);
    }
    strata
}

/// The declared factors that cover at least one path where the two arms differ.
fn factors_between<'a>(declared: &'a [FieldPath], a: &RunView, b: &RunView) -> Vec<&'a FieldPath> {
    let paths: Vec<FieldPath> = compare(a.manifest.comparable(), b.manifest.comparable())
        .into_iter()
        .map(|deviation| deviation.path)
        .collect();
    declared
        .iter()
        .filter(|factor| {
            let prefix: Vec<&str> = factor.segments().collect();
            paths.iter().any(|path| {
                let own: Vec<&str> = path.segments().collect();
                covers(prefix.as_slice(), own.as_slice())
            })
        })
        .collect()
}

/// The verdict over every valid run of both arms, in key order.
fn verdict_of(a: &[&RunView], b: &[&RunView], declared: &[FieldPath]) -> PairVerdict {
    let views: Vec<_> = a
        .iter()
        .chain(b.iter())
        .map(|view| ArmView::from_manifest(&view.manifest))
        .collect();
    match stratum_verdict(views.as_slice(), declared) {
        Verdict::Comparable => PairVerdict::Comparable,
        Verdict::Refused(paths) => {
            PairVerdict::Refused(paths.iter().map(ToString::to_string).collect())
        }
        Verdict::Malformed(errors) => {
            PairVerdict::Malformed(errors.iter().map(ToString::to_string).collect())
        }
    }
}

/// The entries of a comparable pair: each expectation graded in either arm, tested when both hold
/// plain counts and left out, with its reason, when not.
fn entries_of(a: &ArmReport, b: &ArmReport) -> Result<Vec<Entry>, StatsError> {
    let ids: BTreeSet<&ExpectationId> = a
        .tally
        .expectations
        .keys()
        .chain(b.tally.expectations.keys())
        .collect();
    let mut entries = Vec::new();
    for id in ids {
        let id = id.clone(); // BORROW: the entry owns the id
        let (held_a, held_b) = (a.tally.expectations.get(&id), b.tally.expectations.get(&id));
        let (Some(held_a), Some(held_b)) = (held_a, held_b) else {
            let role = held_a
                .or(held_b)
                .map_or(Role::Descriptive, |held| held.role);
            entries.push(Entry::Omitted {
                id,
                role,
                reason: OmitReason::OneArm,
            });
            continue;
        };
        let role = held_a.role;
        entries.push(match (held_a.counts(), held_b.counts()) {
            (Ok(counts_a), Ok(counts_b)) => {
                let difference = newcombe_difference(counts_a, counts_b)?;
                let p = fisher_two_sided(counts_a, counts_b)?;
                Entry::Tested(Tested {
                    id,
                    role,
                    a: counts_a,
                    b: counts_b,
                    p,
                    p_holm: p,
                    reject: false,
                    diff: difference.diff(),
                    diff_lo: difference.lo(),
                    diff_hi: difference.hi(),
                })
            }
            (Err(reason), _) | (Ok(_), Err(reason)) => Entry::Omitted {
                id,
                role,
                reason: OmitReason::Counts(reason),
            },
        });
    }
    Ok(entries)
}

/// The factors a stratum's manifests record: the paths that vary across the experiment's cells.
fn recorded_factors(cells: &BTreeMap<CellKey, Vec<&RunView>>) -> Vec<FieldPath> {
    let all: BTreeSet<&FieldPath> = cells
        .values()
        .flatten()
        .flat_map(|view| view.manifest.factors())
        .collect();
    all.into_iter().cloned().collect() // BORROW: the list owns its paths
}

/// The paths where two arms of a stratum differ, over the first run of each cell.
fn differing(a: &RunView, b: &RunView) -> Vec<FieldPath> {
    compare(a.manifest.comparable(), b.manifest.comparable())
        .into_iter()
        .map(|deviation| deviation.path)
        .collect()
}

/// Checks the factors a stratum's manifests record, and the factors of an experiment file when one
/// is given, before any pair is compared.
///
/// A recorded factor must not be broader than the variation it names: a factor that is a strict
/// prefix of a path where two cells of the stratum differ would make every difference under it
/// look declared. When an experiment file is given, its factors must be the ones the manifests of
/// every stratum record.
///
/// # Errors
/// [`ReportError::FactorBroaderThanVariation`] and [`ReportError::ExperimentDisagrees`].
pub fn check_factors(
    runs: &[RunView],
    experiment: Option<&[FieldPath]>,
) -> Result<(), ReportError> {
    for (stratum, cells) in &strata_of(runs) {
        let recorded = recorded_factors(cells);
        let firsts: Vec<&RunView> = cells
            .values()
            .filter_map(|views| views.first().copied())
            .collect();
        for (at, one) in firsts.iter().enumerate() {
            for other in firsts.iter().skip(at + 1) {
                for path in differing(one, other) {
                    let own: Vec<&str> = path.segments().collect();
                    for factor in &recorded {
                        let prefix: Vec<&str> = factor.segments().collect();
                        if prefix.len() < own.len() && covers(prefix.as_slice(), own.as_slice()) {
                            return Err(ReportError::FactorBroaderThanVariation {
                                experiment: stratum.clone(), // BORROW: the error owns the text
                                factor: factor.to_string(),
                                path: path.to_string(),
                            });
                        }
                    }
                }
            }
        }
        if let Some(file) = experiment {
            let wanted: BTreeSet<&FieldPath> = file.iter().collect();
            let held: BTreeSet<&FieldPath> = recorded.iter().collect();
            if wanted != held {
                return Err(ReportError::ExperimentDisagrees {
                    experiment: stratum.clone(), // BORROW: the error owns the text
                    file: wanted.iter().map(ToString::to_string).collect(),
                    manifests: held.iter().map(ToString::to_string).collect(),
                });
            }
        }
    }
    Ok(())
}

/// Compares every pair of arms of one stratum that differ in exactly one declared factor.
///
/// The verdict of [`stratum_verdict`] over every valid run of both arms says whether the pair is
/// comparable; a refused pair keeps its cause and has no test. Holm's adjustment runs once per
/// role over every tested entry of every comparable pair.
///
/// # Errors
/// The [`StatsError`] of a method.
pub fn compare_pairs(runs: &[RunView], arms: &[ArmReport]) -> Result<Pairs, StatsError> {
    let by_arm: BTreeMap<(&str, &CellKey), &ArmReport> = arms
        .iter()
        .map(|arm| ((arm.experiment.as_str(), &arm.cell), arm))
        .collect();
    let mut comparisons = Vec::new();
    let mut not_one_factor = 0_usize;
    for (stratum, cells) in &strata_of(runs) {
        let declared = recorded_factors(cells);
        let listed: Vec<(&CellKey, &Vec<&RunView>)> = cells.iter().collect();
        for (at, (cell_a, runs_a)) in listed.iter().enumerate() {
            for (cell_b, runs_b) in listed.iter().skip(at + 1) {
                let (Some(first_a), Some(first_b)) = (runs_a.first(), runs_b.first()) else {
                    continue;
                };
                let factors = factors_between(declared.as_slice(), first_a, first_b);
                let [factor] = factors.as_slice() else {
                    not_one_factor += 1;
                    continue;
                };
                let verdict = verdict_of(runs_a.as_slice(), runs_b.as_slice(), declared.as_slice());
                let arm_a = by_arm.get(&(stratum.as_str(), *cell_a));
                let arm_b = by_arm.get(&(stratum.as_str(), *cell_b));
                let entries = match (&verdict, arm_a, arm_b) {
                    (PairVerdict::Comparable, Some(arm_a), Some(arm_b)) => {
                        entries_of(arm_a, arm_b)?
                    }
                    _ => Vec::new(),
                };
                comparisons.push(Comparison {
                    experiment: stratum.clone(), // BORROW: the comparison owns the text
                    a: (*cell_a).clone(),        // BORROW: the comparison owns the cells
                    b: (*cell_b).clone(),
                    factor: (*factor).clone(), // BORROW: the comparison owns the factor
                    verdict,
                    entries,
                });
            }
        }
    }
    comparisons.sort_by(|x, y| (&x.experiment, &x.a, &x.b).cmp(&(&y.experiment, &y.a, &y.b)));
    for comparison in &mut comparisons {
        comparison.entries.sort_by(|x, y| x.id().cmp(y.id()));
    }
    let families = adjust(comparisons.as_mut_slice())?;
    Ok(Pairs {
        comparisons,
        families,
        not_one_factor,
    })
}

/// Fills `p_holm` and `reject` of every tested entry: one family per role, over the whole call.
fn adjust(comparisons: &mut [Comparison]) -> Result<Vec<Family>, StatsError> {
    let mut families = Vec::new();
    for role in [Role::Gate, Role::Quality] {
        let mut family: Vec<&mut Tested> = comparisons
            .iter_mut()
            .flat_map(|comparison| comparison.entries.iter_mut())
            .filter_map(|entry| match entry {
                Entry::Tested(tested) if tested.role == role => Some(tested),
                Entry::Tested(_) | Entry::Omitted { .. } => None,
            })
            .collect();
        if family.is_empty() {
            continue;
        }
        let raw: Vec<f64> = family.iter().map(|tested| tested.p).collect();
        let adjusted = holm(raw.as_slice())?;
        let rejects = rejected(adjusted.as_slice());
        for ((tested, value), reject) in family.iter_mut().zip(adjusted).zip(rejects) {
            tested.p_holm = value;
            tested.reject = reject;
        }
        families.push(Family { role, m: raw.len() });
    }
    Ok(families)
}
