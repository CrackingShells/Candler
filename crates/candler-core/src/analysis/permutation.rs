//! The stratified permutation test of the difference in weighted pass rate.
//!
//! The statistic is `T = sum_k w_k (r_Bk - r_Ak)`, where `r` is a block's `sum(num) / sum(den)`
//! for one arm and `w_k = n_k / N` is the block's share of all runs. Labels are shuffled within
//! each block only, so each block keeps its arm sizes: under the null of no difference in any
//! block, runs are exchangeable within a block (Phipson & Smyth 2010). A permuted `T*` is as
//! extreme as the observed `T` when `|T*| >= |T| - 1e-12 * max(1, |T|)`, and
//! `p = (b + 1) / (B + 1)` for `b` extreme replicates in `B`, so the observed labelling counts
//! once and `p` is never 0.
//!
//! Every replicate shuffles one identity order per block, in block order, from one
//! [`SplitMix64`] seeded by the caller; the first `n_A` positions of the shuffled order form
//! arm A. A run's `den` must be finite and positive and its `num` finite.

use super::error::StatsError;
use super::splitmix::SplitMix64;

/// The runs of one block, each as `(num, den)`, per arm.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitBlock {
    /// Arm A's runs.
    pub a: Vec<(f64, f64)>,
    /// Arm B's runs.
    pub b: Vec<(f64, f64)>,
}

/// The result of a stratified permutation test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PermResult {
    /// The observed `T`.
    pub statistic: f64,
    /// The two-sided p-value, `(extreme + 1) / (B + 1)`.
    pub p: f64,
    /// The permuted replicates at least as extreme as the observed `T`.
    pub extreme: u32,
}

/// One block reduced to its arm sums and run count.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Summary {
    /// `(sum num, sum den)` of arm A.
    pub a: (f64, f64),
    /// `(sum num, sum den)` of arm B.
    pub b: (f64, f64),
    /// The runs in the block.
    pub n: usize,
}

/// A run count as a float; exact below 2^53.
#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    reason = "a run count is far below 2^53, where usize converts exactly"
)]
pub(crate) const fn count_f64(n: usize) -> f64 {
    n as f64
}

/// The `(sum num, sum den)` of runs, added in order.
pub(crate) fn arm_sums<'a>(runs: impl Iterator<Item = &'a (f64, f64)>) -> (f64, f64) {
    runs.fold((0.0, 0.0), |(num, den), &(n, d)| (num + n, den + d))
}

/// `T` of block summaries: `sum_k (n_k / N) (r_Bk - r_Ak)`.
pub(crate) fn statistic_of(summaries: &[Summary]) -> f64 {
    let total = count_f64(summaries.iter().map(|s| s.n).sum());
    summaries.iter().fold(0.0, |t, s| {
        let weight = count_f64(s.n) / total;
        t + weight * (s.b.0 / s.b.1 - s.a.0 / s.a.1)
    })
}

/// The summaries of the blocks as observed.
pub(crate) fn summarise(blocks: &[UnitBlock]) -> Vec<Summary> {
    blocks
        .iter()
        .map(|block| Summary {
            a: arm_sums(block.a.iter()),
            b: arm_sums(block.b.iter()),
            n: block.a.len() + block.b.len(),
        })
        .collect()
}

/// Refuses blocks that have no runs, an empty arm, or a run that is not a finite `(num, den)`
/// with `den > 0`.
pub(crate) fn validate(blocks: &[UnitBlock]) -> Result<(), StatsError> {
    if blocks.is_empty() {
        return Err(StatsError::NoRuns);
    }
    for block in blocks {
        if block.a.is_empty() || block.b.is_empty() {
            return Err(StatsError::NoRuns);
        }
        for &(num, den) in block.a.iter().chain(&block.b) {
            if !num.is_finite() || !den.is_finite() || den <= 0.0 {
                return Err(StatsError::Unreadable(format!(
                    "a run needs a finite num and a finite den above 0, got ({num}, {den})"
                )));
            }
        }
    }
    Ok(())
}

/// The two-sided stratified permutation test of the difference in weighted pass rate, B minus A.
///
/// `b` replicates shuffle the arm labels within each block only, drawing from a
/// [`SplitMix64`] seeded with `seed`.
///
/// # Errors
/// [`StatsError::NoRuns`] if there is no block or a block has an empty arm;
/// [`StatsError::Unreadable`] if a run is not a finite `(num, den)` with `den > 0`, or if `b` is 0.
pub fn stratified_permutation(
    blocks: &[UnitBlock],
    b: u32,
    seed: u64,
) -> Result<PermResult, StatsError> {
    validate(blocks)?;
    if b == 0 {
        return Err(StatsError::Unreadable(
            "a permutation test needs at least one replicate".to_owned(),
        ));
    }
    let observed = statistic_of(&summarise(blocks));
    let cutoff = observed.abs() - 1e-12 * observed.abs().max(1.0);
    let pools: Vec<(usize, Vec<(f64, f64)>)> = blocks
        .iter()
        .map(|block| {
            (
                block.a.len(),
                block.a.iter().chain(&block.b).copied().collect(),
            )
        })
        .collect();
    let mut rng = SplitMix64::new(seed);
    let mut extreme: u32 = 0;
    let mut summaries: Vec<Summary> = Vec::with_capacity(pools.len());
    for _ in 0..b {
        summaries.clear();
        for (n_a, runs) in &pools {
            let mut order: Vec<usize> = (0..runs.len()).collect();
            rng.shuffle_within(&mut order);
            let in_a = order.iter().take(*n_a).filter_map(|&i| runs.get(i));
            let in_b = order.iter().skip(*n_a).filter_map(|&i| runs.get(i));
            summaries.push(Summary {
                a: arm_sums(in_a),
                b: arm_sums(in_b),
                n: runs.len(),
            });
        }
        if statistic_of(&summaries).abs() >= cutoff {
            extreme += 1;
        }
    }
    Ok(PermResult {
        statistic: observed,
        p: (f64::from(extreme) + 1.0) / (f64::from(b) + 1.0),
        extreme,
    })
}
