//! The stratified `BCa` bootstrap interval of the difference in weighted pass rate.
//!
//! Replicates resample runs with replacement within each arm within each block, so every block
//! keeps its arm sizes and its weight `n_k / N`; `T*` is the statistic of [`super::permutation`]
//! on the resampled runs. The bias correction is `z0 = Phi^-1(share of T* below T)` (Efron 1987),
//! the share clamped to `[1/B, (B-1)/B]` so `z0` is finite. The acceleration `a` comes from the
//! leave-one-run-out jackknife over all runs ([`jackknife_acceleration`]). The endpoints are the
//! linearly interpolated quantiles of the sorted replicates at
//! `Phi(z0 + (z0 + z) / (1 - a (z0 + z)))` for `z` the normal quantiles of `(1 - level) / 2` and
//! `1 - (1 - level) / 2`.
//!
//! A degenerate block (every run in it equal) adds no spread. If every replicate equals the
//! observed statistic, the interval is the point `(T, T)`; if the jackknife values are all equal,
//! `a = 0`. A block needs at least two runs in each arm, so that each run can be left out.

use statrs::distribution::{ContinuousCDF as _, Normal};

use super::error::StatsError;
use super::permutation::{Summary, UnitBlock, arm_sums, statistic_of, summarise, validate};
use super::splitmix::SplitMix64;
use core::num::NonZeroUsize;

/// Refuses what [`validate`] refuses, and an arm with fewer than two runs.
fn validate_for_bca(blocks: &[UnitBlock]) -> Result<(), StatsError> {
    validate(blocks)?;
    if blocks
        .iter()
        .any(|block| block.a.len() < 2 || block.b.len() < 2)
    {
        return Err(StatsError::Unreadable(
            "a bootstrap needs at least two runs in each arm of each block".to_owned(),
        ));
    }
    Ok(())
}

/// The acceleration `a = sum(d^3) / (6 (sum(d^2))^(3/2))`, with `d = mean - theta_i` over the
/// statistic `theta_i` of the data with run `i` left out (weights taken from the remaining
/// runs); 0 if every `theta_i` is equal.
///
/// # Errors
/// As [`stratified_bca`] for the blocks.
pub fn jackknife_acceleration(blocks: &[UnitBlock]) -> Result<f64, StatsError> {
    validate_for_bca(blocks)?;
    let base = summarise(blocks);
    let mut thetas: Vec<f64> = Vec::new();
    for (k, block) in blocks.iter().enumerate() {
        for (is_b, runs) in [(false, &block.a), (true, &block.b)] {
            for &(num, den) in runs {
                let mut left_out = base.clone();
                if let Some(summary) = left_out.get_mut(k) {
                    summary.n -= 1;
                    let arm = if is_b { &mut summary.b } else { &mut summary.a };
                    *arm = (arm.0 - num, arm.1 - den);
                }
                thetas.push(statistic_of(&left_out));
            }
        }
    }
    let first = thetas.first().copied().unwrap_or(0.0);
    if thetas.iter().all(|t| t.total_cmp(&first).is_eq()) {
        return Ok(0.0);
    }
    let mean = thetas.iter().sum::<f64>() / super::permutation::count_f64(thetas.len());
    let (cubes, squares) = thetas.iter().fold((0.0, 0.0), |(c, s), &t| {
        let d = mean - t;
        (c + d * d * d, s + d * d)
    });
    Ok(cubes / (6.0 * squares * squares.sqrt()))
}

/// The sums of `runs` resampled with replacement, one draw per run.
fn resampled_sums(rng: &mut SplitMix64, runs: &[(f64, f64)], bound: NonZeroUsize) -> (f64, f64) {
    let mut drawn: Vec<(f64, f64)> = Vec::with_capacity(runs.len());
    for _ in 0..runs.len() {
        if let Some(&run) = runs.get(rng.index_below(bound)) {
            drawn.push(run);
        }
    }
    arm_sums(drawn.iter())
}

/// The standard normal distribution function, from the non-alternating series
/// `erf(t) = 2/sqrt(pi) e^(-t^2) sum 2^n t^(2n+1) / (2n+1)!!`, accurate to about 1e-16 where
/// `statrs`'s own distribution function is off by up to 1e-11. Beyond `t = 6` the tail is below
/// 1e-17 and is taken as 0.
fn normal_cdf(x: f64) -> f64 {
    let t = x.abs() / core::f64::consts::SQRT_2;
    if t > 6.0 {
        return if x < 0.0 { 0.0 } else { 1.0 };
    }
    let squared = t * t;
    let mut term = t;
    let mut sum = t;
    let mut odd = 1.0_f64;
    for _ in 0..500 {
        odd += 2.0;
        term *= 2.0 * squared / odd;
        sum += term;
        if term < 1e-17 * sum {
            break;
        }
    }
    let erf = 2.0 / core::f64::consts::PI.sqrt() * (-squared).exp() * sum;
    f64::midpoint(1.0, if x < 0.0 { -erf } else { erf })
}

/// The linearly interpolated quantile `q` of sorted values.
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the position is in [0, len - 1], so its floor is a valid index"
)]
fn quantile(sorted: &[f64], q: f64) -> f64 {
    let position =
        q.clamp(0.0, 1.0) * super::permutation::count_f64(sorted.len().saturating_sub(1));
    let lower = position.floor();
    let fraction = position - lower;
    let at = lower as usize;
    let low = sorted.get(at).copied().unwrap_or(f64::NAN);
    match sorted.get(at + 1) {
        Some(&high) => low + fraction * (high - low),
        None => low,
    }
}

/// The two-sided stratified `BCa` interval of the difference in weighted pass rate, B minus A, at
/// confidence `level`, from `b` bootstrap replicates drawn from a [`SplitMix64`] seeded with
/// `seed`.
///
/// # Errors
/// [`StatsError::NoRuns`] if there is no block or a block has an empty arm;
/// [`StatsError::Unreadable`] if a run is not a finite `(num, den)` with `den > 0`, an arm has
/// fewer than two runs, `b < 2`, or `level` is not in `(0, 1)`;
/// [`StatsError::Distribution`] if the acceleration leaves the interval undefined.
pub fn stratified_bca(
    blocks: &[UnitBlock],
    b: u32,
    level: f64,
    seed: u64,
) -> Result<(f64, f64), StatsError> {
    validate_for_bca(blocks)?;
    if b < 2 || !(level > 0.0 && level < 1.0) {
        return Err(StatsError::Unreadable(
            "a BCa interval needs b >= 2 and a level in (0, 1)".to_owned(),
        ));
    }
    let base = summarise(blocks);
    let observed = statistic_of(&base);
    let mut rng = SplitMix64::new(seed);
    let mut thetas: Vec<f64> = Vec::new();
    let mut summaries: Vec<Summary> = Vec::with_capacity(blocks.len());
    for _ in 0..b {
        summaries.clear();
        for block in blocks {
            let (Some(bound_a), Some(bound_b)) = (
                NonZeroUsize::new(block.a.len()),
                NonZeroUsize::new(block.b.len()),
            ) else {
                continue;
            };
            let a = resampled_sums(&mut rng, &block.a, bound_a);
            let b_sums = resampled_sums(&mut rng, &block.b, bound_b);
            summaries.push(Summary {
                a,
                b: b_sums,
                n: block.a.len() + block.b.len(),
            });
        }
        thetas.push(statistic_of(&summaries));
    }
    thetas.sort_by(f64::total_cmp);
    if thetas.first() == thetas.last() && thetas.first() == Some(&observed) {
        return Ok((observed, observed));
    }
    let below = thetas.iter().filter(|&&t| t < observed).count();
    let below = below.max(1).min(thetas.len() - 1);
    let normal = Normal::new(0.0, 1.0).map_err(|e| StatsError::Distribution(e.to_string()))?;
    let z0 = normal.inverse_cdf(super::permutation::count_f64(below) / f64::from(b));
    let accel = jackknife_acceleration(blocks)?;
    let alpha = 1.0 - level;
    let mut ends = [0.0_f64; 2];
    for (end, tail) in ends.iter_mut().zip([alpha / 2.0, 1.0 - alpha / 2.0]) {
        let z = z0 + normal.inverse_cdf(tail);
        let denominator = 1.0 - accel * z;
        if !denominator.is_finite() || denominator <= 0.0 {
            return Err(StatsError::Distribution(
                "the acceleration leaves the BCa quantile undefined".to_owned(),
            ));
        }
        *end = quantile(&thetas, normal_cdf(z0 + z / denominator));
    }
    let [low, high] = ends;
    Ok((low, high))
}
