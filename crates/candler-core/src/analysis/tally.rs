//! The rate algebra over a selection: per arm, every count a rate rule asks for.
//!
//! The functions here read neither the store nor the clock: the caller hands over the selection
//! and the validity of its runs.

use std::collections::BTreeMap;

use candler_schemas::execution::{Integrity, InvalidReason};
use candler_schemas::outcome::{Cause, OutcomeRecord, Role, Status};
use candler_schemas::primitives::{ExpectationId, NameVersion, RunKey};

use super::error::StatsError;
use super::wilson::{Counts, Interval, wilson};
use crate::evaluation::selection::rates::CellKey;
use crate::evaluation::selection::select::Selection;

/// Whether one run counts, as the run's manifest says: the caller reads the manifests.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RunFacts {
    /// The run.
    pub run: RunKey,
    /// Whether the run counts, and why not if it does not.
    pub integrity: Integrity,
}

impl RunFacts {
    /// The facts of one run.
    #[must_use]
    pub const fn new(run: RunKey, integrity: Integrity) -> Self {
        Self { run, integrity }
    }
}

/// Why a proportion or an interval is not given for an expectation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Omitted {
    /// The expectation's role is descriptive: it has no rate and no proportion.
    Descriptive,
    /// Some counted outcome carries a weight other than 1/1 or 0/1, so the outcomes are not plain
    /// passes and failures.
    Weighted,
    /// No run was graded (`n = 0`).
    NoRuns,
    /// The statistic could not be computed from counts the store holds.
    Unavailable,
}

impl Omitted {
    /// The reason as it is reported: `descriptive`, `weighted`, `no_runs` or `unavailable`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Descriptive => "descriptive",
            Self::Weighted => "weighted",
            Self::NoRuns => "no_runs",
            Self::Unavailable => "unavailable",
        }
    }
}

/// A rate: the sums of effective weights over the counted outcomes, as a pair of counts.
///
/// There is no constructor from a float: a rate is always its numerator and its denominator.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Rate {
    num: u64,
    den: u64,
}

impl Rate {
    /// The sum of the numerators.
    #[must_use]
    pub const fn num(&self) -> u64 {
        self.num
    }

    /// The sum of the denominators.
    #[must_use]
    pub const fn den(&self) -> u64 {
        self.den
    }

    fn add(&mut self, num: u64, den: u64) {
        self.num = self.num.saturating_add(num);
        self.den = self.den.saturating_add(den);
    }
}

/// Everything counted for one expectation in one arm.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExpectationTally {
    /// The role, copied from the outcomes.
    pub role: Role,
    /// Outcomes that passed.
    pub pass: u64,
    /// Outcomes that failed.
    pub fail: u64,
    /// Outcomes that could not be measured for want of an input; each counts as a failure.
    pub blocked: u64,
    /// Outcomes that were not measured, by cause (`<kind>:<value>`), left out of `n`.
    pub not_measured: BTreeMap<String, u64>,
    /// Outcomes that do not apply, by cause (`<kind>:<value>`), left out of `n`.
    pub not_applicable: BTreeMap<String, u64>,
    /// Passes whose weight is 0/0: left out of `n` and of every sum.
    pub zero_weight: u64,
    /// The sum of the effective weights' numerators over the counted outcomes.
    pub num: u64,
    /// The sum of the effective weights' denominators over the counted outcomes.
    pub den: u64,
    /// Whether every counted outcome weighs 1/1 or 0/1.
    pub binary: bool,
}

impl ExpectationTally {
    const fn new(role: Role) -> Self {
        Self {
            role,
            pass: 0,
            fail: 0,
            blocked: 0,
            not_measured: BTreeMap::new(),
            not_applicable: BTreeMap::new(),
            zero_weight: 0,
            num: 0,
            den: 0,
            binary: true,
        }
    }

    /// The counts of passes in graded runs, `k` out of `n = pass + fail + blocked`.
    ///
    /// # Errors
    /// Returns [`Omitted::Descriptive`] for a descriptive expectation, [`Omitted::Weighted`] when
    /// an outcome carries another weight than 1/1 or 0/1, and [`Omitted::NoRuns`] when `n` is 0.
    pub fn counts(&self) -> Result<Counts, Omitted> {
        match self.role {
            Role::Descriptive => return Err(Omitted::Descriptive),
            Role::Gate | Role::Quality => {}
        }
        if !self.binary {
            return Err(Omitted::Weighted);
        }
        let graded = self
            .pass
            .saturating_add(self.fail)
            .saturating_add(self.blocked);
        if graded == 0 {
            return Err(Omitted::NoRuns);
        }
        Counts::new(self.pass, graded).map_err(|_| Omitted::Unavailable)
    }

    /// The Wilson interval of [`counts`](Self::counts).
    ///
    /// # Errors
    /// The reasons of [`counts`](Self::counts), and [`Omitted::Unavailable`] if the interval
    /// cannot be computed.
    pub fn wilson(&self) -> Result<Interval, Omitted> {
        wilson(self.counts()?).map_err(|_| Omitted::Unavailable)
    }

    /// Counts the outcome of a valid run.
    fn count(&mut self, record: &OutcomeRecord) -> Result<(), StatsError> {
        match record.status() {
            Status::Pass | Status::Fail | Status::Blocked => {
                let Some(weight) = record.effective_weight() else {
                    return Ok(());
                };
                if weight.is_zero() {
                    self.zero_weight += 1;
                    return Ok(());
                }
                match record.status() {
                    Status::Pass => self.pass += 1,
                    Status::Fail => self.fail += 1,
                    Status::Blocked => self.blocked += 1,
                    Status::NotMeasured | Status::NotApplicable => {}
                }
                match self.role {
                    Role::Gate | Role::Quality => {
                        self.num = self.num.saturating_add(weight.num());
                        self.den = self.den.saturating_add(weight.den());
                        let plain = (weight.num(), weight.den());
                        self.binary &= plain == (1, 1) || plain == (0, 1);
                    }
                    Role::Descriptive => {}
                }
            }
            Status::NotMeasured => bump(&mut self.not_measured, cause_key(record.cause())?),
            Status::NotApplicable => bump(&mut self.not_applicable, cause_key(record.cause())?),
        }
        Ok(())
    }
}

/// What one arm holds: its runs by validity and its expectations by id.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct ArmTally {
    /// Listed runs that count.
    pub valid_runs: u64,
    /// Invalid runs by reason, for the reasons an arm can own.
    pub invalid: BTreeMap<String, u64>,
    /// Runs the experimenter ended (`budget_stop`, `killed`, `interrupted`), never charged to an
    /// arm and kept apart so that no reader sums them as failures.
    pub experimenter_ends: BTreeMap<String, u64>,
    /// The expectations of the arm's valid runs.
    pub expectations: BTreeMap<ExpectationId, ExpectationTally>,
}

impl ArmTally {
    /// The rate of a role: the sums of the numerators and denominators over the role's
    /// expectations. A descriptive role adds nothing, so its rate is 0/0.
    #[must_use]
    pub fn rate(&self, role: Role) -> Rate {
        let mut rate = Rate::default();
        for tally in self.expectations.values().filter(|t| t.role == role) {
            rate.add(tally.num, tally.den);
        }
        rate
    }
}

fn bump(map: &mut BTreeMap<String, u64>, key: String) {
    let slot = map.entry(key).or_default();
    *slot += 1;
}

/// The cause as `<kind>:<value>`, from the cause's serialised one-key object.
fn cause_key(cause: Option<&Cause>) -> Result<String, StatsError> {
    let Some(cause) = cause else {
        return Ok("none:".to_owned());
    };
    let value = serde_json::to_value(cause).map_err(|e| StatsError::Unreadable(e.to_string()))?;
    let serde_json::Value::Object(object) = value else {
        return Err(StatsError::Unreadable(
            "a cause is not an object".to_owned(),
        ));
    };
    let mut entries = object.into_iter();
    match (entries.next(), entries.next()) {
        (Some((kind, serde_json::Value::String(text))), None) => Ok(format!("{kind}:{text}")),
        (Some(_) | None, _) => Err(StatsError::Unreadable(
            "a cause is not one key with a string".to_owned(),
        )),
    }
}

/// The serde name of an invalid reason.
fn reason_name(reason: InvalidReason) -> Result<String, StatsError> {
    match serde_json::to_value(reason) {
        Ok(serde_json::Value::String(name)) => Ok(name),
        Ok(other) => Err(StatsError::Unreadable(format!(
            "a reason is not a string: {other}"
        ))),
        Err(e) => Err(StatsError::Unreadable(e.to_string())),
    }
}

/// Tallies the outcomes of `evaluator` ([`Selection::outcomes`]) per arm (the cell of the run
/// key): one record per run and expectation, whatever context or suite entry digests it was
/// graded under.
///
/// Every listed run is counted once by run key, whatever its records: a valid run in
/// `valid_runs`, an invalid one in `invalid` under its reason, except that a run the experimenter
/// ended (`budget_stop`, `killed`, `interrupted`) goes in `experimenter_ends`. A run listed twice
/// counts once, by its last listing. The records of an invalid run are in neither `k` nor `n`.
///
/// # Errors
/// Returns [`StatsError::RunNotListed`] for an outcome of `evaluator` whose run is not in
/// `runs`, and [`StatsError::Unreadable`] for a stored value of a shape this version cannot read.
pub fn tally(
    selection: &Selection,
    evaluator: &NameVersion,
    runs: &[RunFacts],
) -> Result<BTreeMap<CellKey, ArmTally>, StatsError> {
    let mut facts: BTreeMap<&RunKey, &Integrity> = BTreeMap::new();
    for fact in runs {
        facts.insert(&fact.run, &fact.integrity);
    }
    let mut arms: BTreeMap<CellKey, ArmTally> = BTreeMap::new();
    for (run, integrity) in &facts {
        // BORROW: the key owns the cell digest
        let arm = arms.entry(CellKey::new(run.cell().clone())).or_default();
        if matches!(integrity, Integrity::Valid) {
            arm.valid_runs += 1;
        } else if let Integrity::Invalid { reason, .. } = integrity {
            let experimenter = matches!(
                reason,
                InvalidReason::BudgetStop | InvalidReason::Killed | InvalidReason::Interrupted
            );
            let map = if experimenter {
                &mut arm.experimenter_ends
            } else {
                &mut arm.invalid
            };
            bump(map, reason_name(*reason)?);
        } else {
            return Err(StatsError::Unreadable(format!(
                "run {run} has an integrity this version does not know"
            )));
        }
    }
    for record in selection
        .outcomes()
        .filter(|record| record.evaluator() == evaluator)
    {
        let Some(integrity) = facts.get(record.run()) else {
            return Err(StatsError::RunNotListed {
                run: record.run().clone(),
            });
        };
        if !matches!(integrity, Integrity::Valid) {
            continue;
        }
        let cell = CellKey::new(record.run().cell().clone());
        let Some(arm) = arms.get_mut(&cell) else {
            continue;
        };
        // BORROW: the key owns the expectation id
        arm.expectations
            .entry(record.expectation_id().clone())
            .or_insert_with(|| ExpectationTally::new(record.role()))
            .count(record)?;
    }
    Ok(arms)
}
