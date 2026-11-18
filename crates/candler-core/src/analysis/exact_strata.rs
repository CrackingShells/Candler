//! The exact conditional test of a 2×2×K table: the convolution of the blocks' hypergeometrics.

use super::blocking::Stratum;
use super::error::StatsError;
use super::midp::{block_pmf, mid_p};
use super::wilson::Counts;

/// The distribution of the sum of two independent variables with these probabilities.
fn convolve(left: &[f64], right: &[f64]) -> Vec<f64> {
    let mut sum = vec![0.0; (left.len() + right.len()).saturating_sub(1)];
    for (i, &p) in left.iter().enumerate() {
        for (j, &q) in right.iter().enumerate() {
            if let Some(slot) = sum.get_mut(i + j) {
                *slot += p * q;
            }
        }
    }
    sum
}

/// The two-sided mid-p of the exact conditional test of no difference in any block.
///
/// Given every block's margins, A's passes in block `k` follow that block's hypergeometric
/// distribution, and the sum over blocks follows their convolution. The p-value applies the rule
/// of [`midp_two_sided`](super::midp::midp_two_sided) to that distribution at the observed sum.
/// A block with a zero margin is a point mass, so it moves the support and changes no
/// probability.
///
/// # Errors
/// Returns [`StatsError::NoRuns`] for no blocks, [`StatsError::CountAboveRuns`] or
/// [`StatsError::CountTooLarge`] for counts that are not a table, and
/// [`StatsError::Distribution`] if the distribution library refuses a block.
pub fn exact_conditional(strata: &[Stratum]) -> Result<f64, StatsError> {
    if strata.is_empty() {
        return Err(StatsError::NoRuns);
    }
    let mut first = 0_u64;
    let mut observed = 0_u64;
    let mut pmf = vec![1.0];
    for stratum in strata {
        let a = Counts::new(stratum.a.0, stratum.a.1)?;
        let b = Counts::new(stratum.b.0, stratum.b.1)?;
        let (block_first, block) = block_pmf(a, b)?;
        first += block_first;
        observed += a.k();
        pmf = convolve(&pmf, &block);
    }
    let index =
        usize::try_from(observed - first).map_err(|e| StatsError::Distribution(e.to_string()))?;
    Ok(mid_p(&pmf, index))
}
