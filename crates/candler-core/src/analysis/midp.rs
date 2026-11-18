//! Fisher's two-sided mid-p for one 2×2 table, and the rule both exact tests share.

use statrs::distribution::{Discrete as _, Hypergeometric};

use super::blocking::Stratum;
use super::error::StatsError;
use super::fisher::TIE_TOLERANCE;
use super::wilson::Counts;

/// The null distribution of A's passes in one block, as the first value of the support and the
/// probability of each value from there.
///
/// With `N` runs, `K` passes and `n` runs of arm A, A's passes follow `Hypergeometric(N, K, n)`.
/// A block with a zero margin (an arm with no runs, no passes, or only passes) has one possible
/// value, so it is a point mass: its support is that value and its probability is 1.
pub(crate) fn block_pmf(a: Counts, b: Counts) -> Result<(u64, Vec<f64>), StatsError> {
    let total = a.n() + b.n();
    let passes = a.k() + b.k();
    let first = passes.saturating_sub(b.n());
    let last = a.n().min(passes);
    if first == last {
        return Ok((first, vec![1.0]));
    }
    let distribution = Hypergeometric::new(total, passes, a.n())
        .map_err(|e| StatsError::Distribution(e.to_string()))?;
    Ok((first, (first..=last).map(|x| distribution.pmf(x)).collect()))
}

/// The two-sided mid-p of observing `pmf[index]`: the probability of the values less likely than
/// it plus half the probability of the values as likely as it.
///
/// Two probabilities are as likely when they differ by at most [`TIE_TOLERANCE`] of the observed
/// one, the tolerance of [`fisher_two_sided`](super::fisher::fisher_two_sided).
pub(crate) fn mid_p(pmf: &[f64], index: usize) -> f64 {
    let observed = pmf.get(index).copied().unwrap_or(0.0);
    let margin = observed * TIE_TOLERANCE;
    let mut less = 0.0;
    let mut equal = 0.0;
    for &probability in pmf {
        if (probability - observed).abs() <= margin {
            equal += probability;
        } else if probability < observed {
            less += probability;
        }
    }
    (less + equal / 2.0).min(1.0)
}

/// The two-sided mid-p of the 2×2 table of one block.
///
/// With `f` the null pmf of A's passes, the p-value is the sum of `f(x)` over the values less
/// likely than the observed one plus half the sum over the values as likely. A table whose null
/// has one value (a zero margin) is its own and only tie, so its mid-p is 1/2.
///
/// # Errors
/// Returns [`StatsError::CountAboveRuns`] or [`StatsError::CountTooLarge`] for counts that are
/// not a table, and [`StatsError::Distribution`] if the distribution library refuses it.
pub fn midp_two_sided(stratum: &Stratum) -> Result<f64, StatsError> {
    let a = Counts::new(stratum.a.0, stratum.a.1)?;
    let b = Counts::new(stratum.b.0, stratum.b.1)?;
    let (first, pmf) = block_pmf(a, b)?;
    let index =
        usize::try_from(a.k() - first).map_err(|e| StatsError::Distribution(e.to_string()))?;
    Ok(mid_p(&pmf, index))
}
