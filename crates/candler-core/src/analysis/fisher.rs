//! Fisher's exact test, two-sided.

use statrs::distribution::{Discrete as _, Hypergeometric};

use super::error::StatsError;
use super::wilson::Counts;

/// The relative tolerance on the probabilities of two tables that are equally likely.
///
/// It is the tolerance of R's `fisher.test` (`relErr = 1 + 10^-7`). scipy uses 1e-14 with a more
/// exact probability function, but the hypergeometric probabilities of the distribution library
/// carry a relative error of up to about 7e-14, so a tolerance of 1e-14 would split tables of the
/// same probability and give a wrong p-value (1/26 against 2/26 would give 0.6176 where 1 is
/// right). 1e-7 absorbs that error by a wide margin, and no pair of tables the vectors compare
/// has probabilities that differ by more than 1e-14 and less than 1e-7, so the choice between the
/// two changes no vector value.
pub const TIE_TOLERANCE: f64 = 1e-7;

/// The two-sided p-value of Fisher's exact test on `[[a.k, a.n − a.k], [b.k, b.n − b.k]]`.
///
/// With `N = a.n + b.n` and `K = a.k + b.k`, the first cell follows the hypergeometric
/// distribution `Hypergeometric(N, K, a.n)` under the null. The p-value is the sum of the
/// probabilities of every table that is no more likely than the observed one, that is, with
/// `pmf(x) <= pmf(a.k) * (1 + TIE_TOLERANCE)`, capped at 1. It is neither twice the smaller tail
/// nor the sum of the tails by the observed count alone.
///
/// # Errors
/// Returns [`StatsError::NoRuns`] if either arm has no runs, and [`StatsError::Distribution`] if
/// the distribution library refuses the table.
pub fn fisher_two_sided(a: Counts, b: Counts) -> Result<f64, StatsError> {
    if a.n() == 0 || b.n() == 0 {
        return Err(StatsError::NoRuns);
    }
    let total = a.n() + b.n();
    let successes = a.k() + b.k();
    let distribution = Hypergeometric::new(total, successes, a.n())
        .map_err(|e| StatsError::Distribution(e.to_string()))?;
    let first = (a.n() + successes).saturating_sub(total);
    let last = a.n().min(successes);
    let threshold = distribution.pmf(a.k()) * (1.0 + TIE_TOLERANCE);
    let p: f64 = (first..=last)
        .map(|x| distribution.pmf(x))
        .filter(|probability| *probability <= threshold)
        .sum();
    Ok(p.min(1.0))
}
