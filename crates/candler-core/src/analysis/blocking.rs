//! The blocks of a comparison: the level combinations of the declared factors other than the
//! compared one, and the test each design gets.

use std::collections::BTreeMap;

use candler_schemas::execution::Integrity;
use candler_schemas::execution::paths::{REQUEST_TREATMENT, TREATMENT};
use candler_schemas::outcome::Role;
use candler_schemas::primitives::{FieldPath, RunKey};
use serde_json::Value;

use super::bca::stratified_bca;
use super::cmh::cmh;
use super::error::StatsError;
use super::exact_strata::exact_conditional;
use super::holm::{holm, rejected};
use super::midp::midp_two_sided;
use super::newcombe::newcombe_difference;
use super::permutation::{UnitBlock, stratified_permutation};
use super::report::{RunView, Tested};
use super::wilson::Counts;

/// The permutation replicates of a weighted comparison.
pub const REPLICATES: u32 = 10_000;

/// The confidence level of the intervals of a blocked comparison.
pub const LEVEL: f64 = 0.95;

/// One block of a comparison between arm A and arm B.
#[derive(Debug, Clone, PartialEq)]
pub struct Stratum {
    /// The level of each declared factor other than the compared one, which names the block.
    pub key: Vec<(FieldPath, Value)>,
    /// Arm A's passes and runs in this block.
    pub a: (u64, u64),
    /// Arm B's passes and runs in this block.
    pub b: (u64, u64),
}

/// Whether an expectation's outcomes are plain passes and fails or carry weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutcomeKind {
    /// Every outcome weighs 1/1 or 0/1.
    Binary,
    /// Some outcome carries another weight.
    Weighted,
}

/// The test a design gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Battery {
    /// The two-sided mid-p of one 2×2 table.
    MidP,
    /// The Cochran–Mantel–Haenszel test over several blocks.
    Cmh,
    /// The exact conditional test over several blocks, the fallback of [`Battery::Cmh`].
    ExactStrata,
    /// The stratified permutation test with a stratified `BCa` interval.
    Permutation,
}

/// What a blocked comparison is tested on.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum Design<'a> {
    /// The blocks of a binary expectation.
    Binary(&'a [Stratum]),
    /// The blocks of a weighted expectation.
    Weighted(&'a [UnitBlock]),
}

impl Design<'_> {
    /// The kind of outcome of the design.
    #[must_use]
    pub const fn kind(&self) -> OutcomeKind {
        match self {
            Self::Binary(_) => OutcomeKind::Binary,
            Self::Weighted(_) => OutcomeKind::Weighted,
        }
    }

    /// The number of blocks.
    #[must_use]
    pub const fn blocks(&self) -> usize {
        match self {
            Self::Binary(blocks) => blocks.len(),
            Self::Weighted(blocks) => blocks.len(),
        }
    }
}

/// The effect of B over A in one block.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BlockEffect {
    /// The block's key.
    pub key: Vec<(FieldPath, Value)>,
    /// Arm A's weighted passes and runs: `(num, den)` summed for a weighted block.
    pub a: (u64, u64),
    /// Arm B's, in the same terms.
    pub b: (u64, u64),
    /// The rate of arm B minus the rate of arm A in the block.
    pub diff: f64,
}

/// The result of a blocked comparison of one expectation.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Blocked {
    /// The test that produced [`Self::p`].
    pub battery: Battery,
    /// The number of blocks.
    pub blocks: usize,
    /// Whether the exact conditional test replaced a CMH test whose criterion failed.
    pub fallback: bool,
    /// The two-sided p-value.
    pub p: f64,
    /// The difference B minus A, when the test gives one.
    pub diff: Option<f64>,
    /// The interval of [`Self::diff`].
    pub interval: Option<(f64, f64)>,
    /// The Mantel–Haenszel odds ratio of B over A, over several blocks of a binary expectation.
    pub or_mh: Option<f64>,
    /// The interval of [`Self::or_mh`].
    pub or_ci: Option<(f64, f64)>,
    /// The p-value of the Breslow–Day homogeneity test, shown and never used to choose.
    pub homogeneity_p: Option<f64>,
    /// The effect in each block, in block order.
    pub per_block: Vec<BlockEffect>,
}

/// The test a design gets, a function of the design alone.
#[must_use]
pub const fn choose(kind: OutcomeKind, blocks: usize) -> Battery {
    match kind {
        OutcomeKind::Weighted => Battery::Permutation,
        OutcomeKind::Binary if blocks <= 1 => Battery::MidP,
        OutcomeKind::Binary => Battery::Cmh,
    }
}

/// The units of the runs of one block, per arm.
struct Gathered {
    key: Vec<(FieldPath, Value)>,
    a: Vec<(u64, u64)>,
    b: Vec<(u64, u64)>,
}

/// The level a run's manifest records for a factor: for the treatment, the typed reference
/// `{name, tag, commit}` of the cell's comparable, which `values` never holds; for another
/// factor, the value at its path, `null` when it records none. A declared factor is never a
/// strict prefix of a path where cells differ: `check_factors` refuses that before any level is
/// read.
pub(super) fn level_of(view: &RunView, factor: &FieldPath) -> Value {
    let comparable = view.manifest.comparable();
    if matches!(factor.as_str(), REQUEST_TREATMENT | TREATMENT) {
        return serde_json::to_value(comparable.treatment()).unwrap_or(Value::Null);
    }
    if let Some(held) = comparable
        .values()
        .iter()
        .find(|held| held.path() == factor)
    {
        return held
            .value()
            .map_or(Value::Null, |value| value.as_value().clone()); // BORROW: the key owns the level
    }
    Value::Null
}

/// The valid runs at level `a` or `b` of the compared factor, grouped by the levels of the other
/// declared factors in key order; a block with an empty arm is left out.
fn gather(
    runs: &[RunView],
    units: &[(RunKey, u64, u64)],
    compared: &FieldPath,
    a: &Value,
    b: &Value,
    declared: &[FieldPath],
) -> Vec<Gathered> {
    let mut by_run: BTreeMap<&RunKey, Vec<(u64, u64)>> = BTreeMap::new();
    for (run, num, den) in units {
        by_run.entry(run).or_default().push((*num, *den));
    }
    let mut blocks: BTreeMap<String, Gathered> = BTreeMap::new();
    for view in runs
        .iter()
        .filter(|view| matches!(view.facts.integrity, Integrity::Valid))
    {
        let level = level_of(view, compared);
        let Some(held) = by_run.get(&view.facts.run) else {
            continue;
        };
        let key: Vec<(FieldPath, Value)> = declared
            .iter()
            .filter(|factor| *factor != compared)
            .map(|factor| (factor.clone(), level_of(view, factor))) // BORROW: the key owns the path
            .collect();
        let name = key
            .iter()
            .map(|(factor, value)| format!("{factor}={value}"))
            .collect::<Vec<_>>()
            .join("\u{1f}");
        let block = blocks.entry(name).or_insert_with(|| Gathered {
            key,
            a: Vec::new(),
            b: Vec::new(),
        });
        if level == *a {
            block.a.extend(held.iter().copied());
        } else if level == *b {
            block.b.extend(held.iter().copied());
        }
    }
    blocks
        .into_values()
        .filter(|block| !block.a.is_empty() && !block.b.is_empty())
        .map(|mut block| {
            block.a.sort_unstable();
            block.b.sort_unstable();
            block
        })
        .collect()
}

/// Passes and runs of binary units: the numerators sum to the passes.
fn counts_of(units: &[(u64, u64)]) -> (u64, u64) {
    let passes = units
        .iter()
        .fold(0_u64, |sum, (num, _)| sum.saturating_add(*num));
    (passes, u64::try_from(units.len()).unwrap_or(u64::MAX))
}

/// The blocks of a binary expectation, one per level combination of the declared factors other
/// than `compared`, in key order. `units` are the per-run `(run, num, den)` of the expectation;
/// the arms are the runs at level `a` and level `b` of `compared`, and a block with an empty arm
/// is left out.
#[must_use]
pub fn blocks_of(
    runs: &[RunView],
    units: &[(RunKey, u64, u64)],
    compared: &FieldPath,
    a: &Value,
    b: &Value,
    declared: &[FieldPath],
) -> Vec<Stratum> {
    gather(runs, units, compared, a, b, declared)
        .into_iter()
        .map(|block| Stratum {
            key: block.key,
            a: counts_of(block.a.as_slice()),
            b: counts_of(block.b.as_slice()),
        })
        .collect()
}

/// The blocks of a weighted expectation, in the order and with the arms of [`blocks_of`].
#[must_use]
pub fn unit_blocks_of(
    runs: &[RunView],
    units: &[(RunKey, u64, u64)],
    compared: &FieldPath,
    a: &Value,
    b: &Value,
    declared: &[FieldPath],
) -> Vec<UnitBlock> {
    let floats = |arm: Vec<(u64, u64)>| -> Vec<(f64, f64)> {
        arm.into_iter()
            .map(|(num, den)| (to_f64(num), to_f64(den)))
            .collect()
    };
    gather(runs, units, compared, a, b, declared)
        .into_iter()
        .map(|block| UnitBlock {
            a: floats(block.a),
            b: floats(block.b),
        })
        .collect()
}

/// The keys of the blocks of [`unit_blocks_of`], in the same order.
#[must_use]
pub fn unit_block_keys(
    runs: &[RunView],
    units: &[(RunKey, u64, u64)],
    compared: &FieldPath,
    a: &Value,
    b: &Value,
    declared: &[FieldPath],
) -> Vec<Vec<(FieldPath, Value)>> {
    gather(runs, units, compared, a, b, declared)
        .into_iter()
        .map(|block| block.key)
        .collect()
}

impl Blocked {
    /// Names the per-block effects by the keys of their blocks, in block order; a weighted
    /// block carries no key of its own.
    #[must_use]
    pub fn keyed(mut self, keys: &[Vec<(FieldPath, Value)>]) -> Self {
        for (effect, key) in self.per_block.iter_mut().zip(keys) {
            effect.key.clone_from(key);
        }
        self
    }
}

/// A count as a float.
#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    reason = "CAST: a sum of weights is far below 2^53"
)]
const fn to_f64(value: u64) -> f64 {
    value as f64 // CAST: u64 → f64, a sum of weights is far below 2^53
}

/// The rate `k / n`, the caller having checked `n > 0`.
fn rate(k: u64, n: u64) -> f64 {
    to_f64(k) / to_f64(n)
}

/// Runs the test the design gets: the mid-p for one block, the CMH test for several, the exact
/// conditional test when the Mantel–Fleiss criterion fails, the stratified permutation test with
/// a `BCa` interval for a weighted expectation. The choice reads the design, never a p-value.
///
/// # Errors
/// [`StatsError::NoRuns`] if there is no block or a block has an empty arm; otherwise the
/// [`StatsError`] of the method.
pub fn test_blocked(design: &Design<'_>, seed: u64) -> Result<Blocked, StatsError> {
    let battery = choose(design.kind(), design.blocks());
    match design {
        Design::Binary(blocks) => binary(battery, blocks),
        Design::Weighted(blocks) => weighted(blocks, seed),
    }
}

fn binary(battery: Battery, blocks: &[Stratum]) -> Result<Blocked, StatsError> {
    if blocks.is_empty() || blocks.iter().any(|block| block.a.1 == 0 || block.b.1 == 0) {
        return Err(StatsError::NoRuns);
    }
    let per_block: Vec<BlockEffect> = blocks
        .iter()
        .map(|block| BlockEffect {
            key: block.key.clone(), // BORROW: the effect owns the key
            a: block.a,
            b: block.b,
            diff: rate(block.b.0, block.b.1) - rate(block.a.0, block.a.1),
        })
        .collect();
    let mut blocked = Blocked {
        battery,
        blocks: blocks.len(),
        fallback: false,
        p: 1.0,
        diff: None,
        interval: None,
        or_mh: None,
        or_ci: None,
        homogeneity_p: None,
        per_block,
    };
    if let (Battery::MidP, [block]) = (battery, blocks) {
        blocked.p = midp_two_sided(block)?;
        let difference = newcombe_difference(
            Counts::new(block.b.0, block.b.1)?,
            Counts::new(block.a.0, block.a.1)?,
        )?;
        blocked.diff = Some(difference.diff());
        blocked.interval = Some((difference.lo(), difference.hi()));
    } else {
        let engine = cmh(blocks, LEVEL)?;
        blocked.or_mh = engine.or_mh;
        blocked.or_ci = engine.or_ci;
        blocked.homogeneity_p = engine.homogeneity_p;
        match engine.p {
            Some(p) if engine.mantel_fleiss_holds => {
                blocked.battery = Battery::Cmh;
                blocked.p = p;
            }
            _ => {
                blocked.battery = Battery::ExactStrata;
                blocked.fallback = true;
                blocked.p = exact_conditional(blocks)?;
            }
        }
    }
    Ok(blocked)
}

fn weighted(blocks: &[UnitBlock], seed: u64) -> Result<Blocked, StatsError> {
    if blocks.is_empty() {
        return Err(StatsError::NoRuns);
    }
    let mut per_block = Vec::with_capacity(blocks.len());
    for block in blocks {
        let (a, b) = (sums(block.a.as_slice()), sums(block.b.as_slice()));
        if a.1 == 0 || b.1 == 0 {
            return Err(StatsError::NoRuns);
        }
        per_block.push(BlockEffect {
            key: Vec::new(),
            a,
            b,
            diff: rate(b.0, b.1) - rate(a.0, a.1),
        });
    }
    let perm = stratified_permutation(blocks, REPLICATES, seed)?;
    let interval = stratified_bca(blocks, REPLICATES, LEVEL, seed)?;
    Ok(Blocked {
        battery: Battery::Permutation,
        blocks: blocks.len(),
        fallback: false,
        p: perm.p,
        diff: Some(perm.statistic),
        interval: Some(interval),
        or_mh: None,
        or_ci: None,
        homogeneity_p: None,
        per_block,
    })
}

/// The summed numerators and denominators of a block's arm, as whole numbers.
fn sums(arm: &[(f64, f64)]) -> (u64, u64) {
    let total = |pick: fn(&(f64, f64)) -> f64| -> u64 {
        let sum: f64 = arm.iter().map(pick).sum();
        from_f64(sum)
    };
    (total(|run| run.0), total(|run| run.1))
}

/// A whole-number float as a count; the units were whole numbers when they became floats.
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "CAST: a sum of whole non-negative weights below 2^53"
)]
const fn from_f64(value: f64) -> u64 {
    value as u64 // CAST: f64 → u64, a sum of whole non-negative weights
}

/// The Holm-adjusted p-values of tests that belong to comparisons, in the input order: one
/// family per comparison and role, over the gate expectations and, apart, the quality
/// expectations; a descriptive entry keeps its p. `keys` holds each test's `(comparison, role)`.
///
/// # Errors
/// [`StatsError::Unreadable`] if `keys` and `p` differ in length; [`StatsError::NotAProbability`]
/// for a p that is not a probability.
pub fn holm_groups(keys: &[(usize, Role)], p: &[f64]) -> Result<Vec<f64>, StatsError> {
    if keys.len() != p.len() {
        return Err(StatsError::Unreadable(
            "each p-value needs one comparison and role".to_owned(),
        ));
    }
    let mut families: BTreeMap<(usize, bool), Vec<usize>> = BTreeMap::new();
    for (at, (comparison, role)) in keys.iter().enumerate() {
        match role {
            Role::Gate => families.entry((*comparison, true)).or_default().push(at),
            Role::Quality => families.entry((*comparison, false)).or_default().push(at),
            Role::Descriptive => {}
        }
    }
    let mut adjusted = p.to_vec();
    for members in families.values() {
        let own: Vec<f64> = members
            .iter()
            .filter_map(|at| p.get(*at).copied())
            .collect();
        for (at, value) in members.iter().zip(holm(own.as_slice())?) {
            if let Some(slot) = adjusted.get_mut(*at) {
                *slot = value;
            }
        }
    }
    Ok(adjusted)
}

/// Which adjusted values reject the null: those of a gate or quality test below alpha; a
/// descriptive entry is never tested.
#[must_use]
pub fn holm_rejects(keys: &[(usize, Role)], adjusted: &[f64]) -> Vec<bool> {
    keys.iter()
        .zip(rejected(adjusted))
        .map(|((_, role), reject)| reject && !matches!(role, Role::Descriptive))
        .collect()
}

/// Adjusts the tested entries of each comparison in place: Holm over its gate expectations and,
/// apart, over its quality expectations, setting `p_holm` and `reject`.
///
/// # Errors
/// The [`StatsError`] of [`holm_groups`].
pub fn holm_families(comparisons: &mut [Vec<Tested>]) -> Result<(), StatsError> {
    let mut keys = Vec::new();
    let mut ps = Vec::new();
    for (comparison, tests) in comparisons.iter().enumerate() {
        for tested in tests {
            keys.push((comparison, tested.role));
            ps.push(tested.p);
        }
    }
    let adjusted = holm_groups(keys.as_slice(), ps.as_slice())?;
    let rejects = holm_rejects(keys.as_slice(), adjusted.as_slice());
    for ((tested, value), reject) in comparisons.iter_mut().flatten().zip(adjusted).zip(rejects) {
        tested.p_holm = value;
        tested.reject = reject;
    }
    Ok(())
}
