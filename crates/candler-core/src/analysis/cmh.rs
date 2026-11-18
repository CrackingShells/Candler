//! The Cochran–Mantel–Haenszel test, the Mantel–Haenszel odds ratio and the Mantel–Fleiss criterion.
//! The CMH test and the Mantel–Haenszel odds ratio against statsmodels' `StratifiedTable`.
//!
//! Each block is the 2×2 table `[[a, b], [c, d]]`: arm B's passes and fails, then arm A's. The
//! odds ratio is the odds of passing in B over the odds in A, and the χ² statistic is the
//! Cochran–Mantel–Haenszel statistic without continuity correction, on 1 degree of freedom.
//!
//! # Zero-cell convention
//!
//! - **No shift.** A zero cell stays zero: nothing is added to it (statsmodels' default,
//!   `shift_zeros=False`). The statistic, the odds ratio and the interval are sums over blocks and
//!   take a zero cell as it is.
//! - **Informative blocks.** A block with a zero margin (no runs in an arm, no passes, or no fails)
//!   carries no information. It is dropped from every quantity, and counted in none of them,
//!   Breslow–Day's degrees of freedom included. The remaining blocks are the informative ones.
//! - **No informative block.** Every quantity is undefined: `statistic`, `p`, `or_mh`, `or_ci` and
//!   `homogeneity_p` are `None`, and the Mantel–Fleiss quantity is 0, which does not hold.
//!   statsmodels returns `NaN` for the statistic there.
//! - **Odds ratio.** With `Σ S = Σ b c / n = 0` the odds ratio is infinite and `or_mh` is `None`
//!   (statsmodels returns `inf`). With `Σ R = Σ a d / n = 0` and `Σ S > 0` it is 0 and has no
//!   interval, since the interval is on the log scale. Both sums are positive for the interval.
//! - **Homogeneity.** The Breslow–Day test needs two informative blocks and a positive, finite
//!   odds ratio; otherwise `homogeneity_p` is `None`. It is reported and never a gate.
//!
//! Wherever statsmodels returns a finite value, Candler's equals it to a relative `1e-12` (and `1e-15`
//! absolute on a p-value, the error of `1 - cdf`); the
//! only difference is the homogeneity p-value, where statsmodels writes `1 - cdf` and Candler
//! the survival function.
//!
//! # Sources
//!
//! Mantel & Haenszel (1959, JNCI 22:719) for the statistic and the pooled odds ratio; Robins,
//! Breslow & Greenland (1986, Biometrics 42:311) for the variance of the log odds ratio;
//! Mantel & Fleiss (1980, Am J Epidemiol 112:129) for the criterion; Breslow & Day (1980) for
//! the homogeneity test.

use statrs::distribution::{ChiSquared, ContinuousCDF as _, Normal};

use super::blocking::Stratum;
use super::error::StatsError;

/// The Mantel–Fleiss quantity at which the χ² approximation is taken as adequate.
const MANTEL_FLEISS_MINIMUM: f64 = 5.0;

/// The largest count a float holds exactly.
const EXACT_LIMIT: u64 = 1 << 53;

/// The CMH test of one comparison over its blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct CmhResult {
    /// The χ² statistic on 1 degree of freedom, without continuity correction.
    pub statistic: Option<f64>,
    /// The p-value of [`Self::statistic`].
    pub p: Option<f64>,
    /// The Mantel–Haenszel odds ratio of passing in arm B over arm A.
    pub or_mh: Option<f64>,
    /// The Robins–Breslow–Greenland interval of [`Self::or_mh`].
    pub or_ci: Option<(f64, f64)>,
    /// The Mantel–Fleiss quantity: the smaller distance from the expected count of arm B's
    /// passes to its lower and upper bounds, summed over the blocks.
    pub mantel_fleiss: f64,
    /// Whether [`Self::mantel_fleiss`] reaches 5.
    pub mantel_fleiss_holds: bool,
    /// The p-value of the Breslow–Day homogeneity test of the blocks' odds ratios.
    pub homogeneity_p: Option<f64>,
}

/// One informative block: arm B's passes and fails (`a`, `b`), then arm A's (`c`, `d`).
#[derive(Debug, Clone, Copy)]
struct Block {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
}

impl Block {
    fn runs(&self) -> f64 {
        self.a + self.b + self.c + self.d
    }

    /// Arm B's runs.
    fn row(&self) -> f64 {
        self.a + self.b
    }

    /// The passes of both arms.
    fn col(&self) -> f64 {
        self.a + self.c
    }
}

#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    reason = "CAST: u64 to f64 is exact below 2^53, which block_of enforces"
)]
const fn to_f64(count: u64) -> f64 {
    count as f64 // CAST: u64 → f64, exact because the count is at most 2^53
}

/// The block of a stratum, or `None` when a margin is zero.
fn block_of(stratum: &Stratum) -> Result<Option<Block>, StatsError> {
    for (k, n) in [stratum.a, stratum.b] {
        if n > EXACT_LIMIT {
            return Err(StatsError::CountTooLarge { n });
        }
        if k > n {
            return Err(StatsError::CountAboveRuns { k, n });
        }
    }
    let (a_pass, a_runs) = stratum.a;
    let (b_pass, b_runs) = stratum.b;
    let passes = a_pass + b_pass;
    if a_runs == 0 || b_runs == 0 || passes == 0 || passes == a_runs + b_runs {
        return Ok(None);
    }
    Ok(Some(Block {
        a: to_f64(b_pass),
        b: to_f64(b_runs - b_pass),
        c: to_f64(a_pass),
        d: to_f64(a_runs - a_pass),
    }))
}

/// The expected count of the first cell and its hypergeometric variance, given the margins.
fn expected_and_variance(block: &Block) -> (f64, f64) {
    let n = block.runs();
    let row_col = block.row() * block.col();
    let expected = row_col / n;
    let variance = row_col * (block.b + block.d) * (block.c + block.d) / (n * n * (n - 1.0));
    (expected, variance)
}

/// The Robins–Breslow–Greenland variance of the log of the Mantel–Haenszel odds ratio, given
/// `sum_r = Σ a d / n` and `sum_s = Σ b c / n`.
fn rbg_variance(blocks: &[Block], sum_r: f64, sum_s: f64) -> f64 {
    let (mut pr, mut cross, mut qs) = (0.0, 0.0, 0.0);
    for block in blocks {
        let n = block.runs();
        let agree = (block.a + block.d) / n;
        let differ = (block.b + block.c) / n;
        let r_k = block.a * block.d / n;
        let s_k = block.b * block.c / n;
        pr += agree * r_k;
        cross += agree * s_k + differ * r_k;
        qs += differ * s_k;
    }
    pr / (2.0 * sum_r * sum_r) + cross / (2.0 * sum_r * sum_s) + qs / (2.0 * sum_s * sum_s)
}

/// The Breslow–Day statistic at the common odds ratio `odds`, without Tarone's adjustment.
fn breslow_day(blocks: &[Block], odds: f64) -> f64 {
    let mut statistic = 0.0;
    for block in blocks {
        let (row, col) = (block.row(), block.col());
        let diff = block.d - block.a;
        let q_a = 1.0 - odds;
        let q_b = odds * (row + col) + diff;
        let q_c = -odds * row * col;
        let expected = if q_a.abs() < f64::EPSILON {
            -q_c / q_b
        } else {
            (-q_b + (q_b * q_b - 4.0 * q_a * q_c).sqrt()) / (2.0 * q_a)
        };
        let variance = 1.0
            / (1.0 / expected
                + 1.0 / (col - expected)
                + 1.0 / (row - expected)
                + 1.0 / (diff + expected));
        statistic += (block.a - expected).powi(2) / variance;
    }
    statistic
}

/// The Mantel–Fleiss quantity: the smaller of `ΣE − ΣL` and `ΣU − ΣE`, with `L` and `U` the bounds
/// of arm B's passes in each block given the margins.
fn mantel_fleiss(blocks: &[Block]) -> f64 {
    let (mut expected, mut lower, mut upper) = (0.0, 0.0, 0.0);
    for block in blocks {
        expected += block.row() * block.col() / block.runs();
        lower += (block.col() - (block.c + block.d)).max(0.0);
        upper += block.row().min(block.col());
    }
    (expected - lower).min(upper - expected)
}

fn distribution<E: std::fmt::Display>(error: E) -> StatsError {
    StatsError::Distribution(error.to_string())
}

/// The CMH test of the blocks `strata` of one comparison, with the Mantel–Haenszel odds ratio of
/// arm B over arm A and its Robins–Breslow–Greenland interval at `level`.
///
/// # Errors
/// Returns [`StatsError::CountAboveRuns`] or [`StatsError::CountTooLarge`] for a block whose
/// counts are not counts, and [`StatsError::Distribution`] when `level` is not in `(0, 1)`.
pub fn cmh(strata: &[Stratum], level: f64) -> Result<CmhResult, StatsError> {
    if level.is_nan() || level <= 0.0 || level >= 1.0 {
        return Err(StatsError::Distribution(format!(
            "the level {level} is not in (0, 1)"
        )));
    }
    let mut blocks = Vec::with_capacity(strata.len());
    for stratum in strata {
        blocks.extend(block_of(stratum)?);
    }
    let mut result = CmhResult {
        statistic: None,
        p: None,
        or_mh: None,
        or_ci: None,
        mantel_fleiss: 0.0,
        mantel_fleiss_holds: false,
        homogeneity_p: None,
    };
    if blocks.is_empty() {
        return Ok(result);
    }

    let (mut excess, mut variance) = (0.0, 0.0);
    for block in &blocks {
        let (expected, var) = expected_and_variance(block);
        excess += block.a - expected;
        variance += var;
    }
    let statistic = excess * excess / variance;
    result.statistic = Some(statistic);
    result.p = Some(ChiSquared::new(1.0).map_err(distribution)?.sf(statistic));

    let sum_r: f64 = blocks.iter().map(|b| b.a * b.d / b.runs()).sum();
    let sum_s: f64 = blocks.iter().map(|b| b.b * b.c / b.runs()).sum();
    if sum_s > 0.0 {
        result.or_mh = Some(sum_r / sum_s);
    }
    if sum_r > 0.0 && sum_s > 0.0 {
        let quantile = Normal::new(0.0, 1.0)
            .map_err(distribution)?
            .inverse_cdf(1.0 - (1.0 - level) / 2.0);
        let half = quantile * rbg_variance(&blocks, sum_r, sum_s).sqrt();
        let log_or = (sum_r / sum_s).ln();
        result.or_ci = Some(((log_or - half).exp(), (log_or + half).exp()));
        if blocks.len() >= 2 {
            let df = f64::from(u32::try_from(blocks.len() - 1).map_err(distribution)?);
            let stat = breslow_day(&blocks, sum_r / sum_s);
            result.homogeneity_p = Some(ChiSquared::new(df).map_err(distribution)?.sf(stat));
        }
    }

    let quantity = mantel_fleiss(&blocks);
    result.mantel_fleiss = quantity;
    result.mantel_fleiss_holds = quantity >= MANTEL_FLEISS_MINIMUM;
    Ok(result)
}
