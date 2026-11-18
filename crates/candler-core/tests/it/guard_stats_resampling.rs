//! `SplitMix64`, the stratified permutation test and the `BCa` interval against their Python twin.
//! The twin, scipy and exact enumeration wrote `tests/vectors/stats/{splitmix,permutation,bca}.toml`
//! (`gen_stats.py`, which reads no Candler file); a vector row is never edited to make a guard
//! pass.

use std::num::NonZeroU64;
use std::path::PathBuf;

use candler_core::analysis::bca::{jackknife_acceleration, stratified_bca};
use candler_core::analysis::permutation::{UnitBlock, stratified_permutation};
use candler_core::analysis::splitmix::SplitMix64;
use serde::Deserialize;
use serde::de::DeserializeOwned;

#[derive(Deserialize)]
struct Vectors<R> {
    rows: Vec<R>,
}

/// The rows of one vector file, in file order.
fn vector<R: DeserializeOwned>(file: &str) -> Vec<R> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/vectors/stats")
        .join(file);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let parsed: Vectors<R> = toml::from_str(&text).unwrap_or_else(|e| panic!("{file}: {e}"));
    assert!(!parsed.rows.is_empty(), "{file} has no rows");
    parsed.rows
}

fn close(label: &str, got: f64, want: f64) {
    let tolerance = 1e-12 * want.abs().max(1.0);
    assert!(
        (got - want).abs() <= tolerance,
        "{label}: got {got}, want {want}"
    );
}

#[derive(Deserialize)]
struct SplitMixRow {
    kind: String,
    seed: u64,
    #[serde(default)]
    n: String,
    #[serde(default)]
    outputs: Vec<String>,
    #[serde(default)]
    draws: Vec<String>,
    #[serde(default)]
    len: usize,
    #[serde(default)]
    order: Vec<usize>,
}

/// Guards: the generator's outputs, its `below` by rejection (large moduli reject often, so a
/// plain modulo differs) and its Fisher-Yates shuffle are those of the Python twin, bit for bit.
/// Oracle: the twin, which reproduces the published outputs of seed 0 before it writes anything.
#[test]
fn splitmix_matches_twin() {
    for row in vector::<SplitMixRow>("splitmix.toml") {
        let mut rng = SplitMix64::new(row.seed);
        match row.kind.as_str() {
            "next" => {
                assert_eq!(row.outputs.len(), 1000, "seed {}", row.seed);
                for (i, want) in row.outputs.iter().enumerate() {
                    assert_eq!(
                        &format!("{:016x}", rng.next_u64()),
                        want,
                        "seed {} output {i}",
                        row.seed
                    );
                }
            }
            "below" => {
                let n: u64 = row.n.parse().unwrap_or_else(|e| panic!("n {}: {e}", row.n));
                let bound = NonZeroU64::new(n).unwrap_or_else(|| panic!("n is 0"));
                for (i, want) in row.draws.iter().enumerate() {
                    assert_eq!(&rng.below(bound).to_string(), want, "n {n} draw {i}");
                }
            }
            "shuffle" => {
                let mut order: Vec<usize> = (0..row.len).collect();
                rng.shuffle_within(&mut order);
                assert_eq!(order, row.order, "shuffle of {}", row.len);
            }
            other => panic!("unknown splitmix row kind {other}"),
        }
    }
}

#[derive(Deserialize)]
struct Design {
    a_sizes: Vec<usize>,
    b_sizes: Vec<usize>,
    a_num: Vec<f64>,
    a_den: Vec<f64>,
    b_num: Vec<f64>,
    b_den: Vec<f64>,
}

/// The blocks of a flattened design: runs of each block taken in order from the flat lists.
fn blocks_of(design: &Design) -> Vec<UnitBlock> {
    let mut a = design
        .a_num
        .iter()
        .zip(&design.a_den)
        .map(|(&n, &d)| (n, d));
    let mut b = design
        .b_num
        .iter()
        .zip(&design.b_den)
        .map(|(&n, &d)| (n, d));
    design
        .a_sizes
        .iter()
        .zip(&design.b_sizes)
        .map(|(&na, &nb)| UnitBlock {
            a: a.by_ref().take(na).collect(),
            b: b.by_ref().take(nb).collect(),
        })
        .collect()
}

#[derive(Deserialize)]
struct PermRow {
    name: String,
    seed: u64,
    b: u32,
    #[serde(flatten)]
    design: Design,
    statistic: f64,
    p: f64,
    extreme: u32,
}

/// Guards: the statistic, the extreme count and the p-value of the stratified permutation test
/// are the twin's, for blocks of one to three strata, equal data and a design whose block rates
/// are 0 and 1.
/// Oracle: the twin, whose p the generator holds within 3 Monte Carlo SE of the exact p over
/// every within-block relabelling and of scipy's `permutation_test`.
#[test]
fn permutation_matches_twin() {
    for row in vector::<PermRow>("permutation.toml") {
        let blocks = blocks_of(&row.design);
        let got = stratified_permutation(&blocks, row.b, row.seed)
            .unwrap_or_else(|e| panic!("{}: {e}", row.name));
        close(
            &format!("{} statistic", row.name),
            got.statistic,
            row.statistic,
        );
        close(&format!("{} p", row.name), got.p, row.p);
        assert_eq!(got.extreme, row.extreme, "{} extreme", row.name);
    }
}

fn run(num: f64, den: f64) -> (f64, f64) {
    (num, den)
}

/// Guards: labels are shuffled within each block, never across blocks. Two blocks, rates 0 and 1,
/// B differing from A by one run in each: `T = 1/6`, and each block's difference is `+-1/6` with
/// equal chance, so `|T*| >= 1/6` exactly when the two agree, `p = 1/2`. A shuffle across blocks
/// mixes the rate-0 and rate-1 runs, so `|T*|` reaches `1/6` far less often than half the time and `p` is about 0.26.
/// Oracle: exact arithmetic, `p = 1/2`, held to 3 Monte Carlo SE at B = 10 000.
#[test]
fn permutation_stays_within_blocks() {
    let a1 = vec![run(0.0, 1.0); 6];
    let mut b1 = vec![run(0.0, 1.0); 5];
    b1.push(run(1.0, 1.0));
    let mut a2 = vec![run(1.0, 1.0); 5];
    a2.push(run(0.0, 1.0));
    let b2 = vec![run(1.0, 1.0); 6];
    let blocks = [UnitBlock { a: a1, b: b1 }, UnitBlock { a: a2, b: b2 }];
    let got = stratified_permutation(&blocks, 10_000, 7).unwrap_or_else(|e| panic!("{e}"));
    close("statistic", got.statistic, 1.0 / 6.0);
    let se = (0.25_f64 / 10_000.0).sqrt();
    assert!(
        (got.p - 0.5).abs() <= 3.0 * se,
        "p {} is not within 3 SE ({}) of 1/2",
        got.p,
        3.0 * se
    );
}

/// Guards: `p = (b + 1) / (B + 1)`, so the observed labelling counts once and `p` is never 0, and
/// when every permuted statistic ties with the observed one, `p = 1`.
/// Oracle: arithmetic. All 20 A runs fail and all 20 B runs pass, so only two of the
/// C(40, 20) relabellings reach `|T| = 1` and 100 replicates see none: `p = 1/101`. Equal data tie
/// in every replicate: `p = 101/101`.
#[test]
fn p_counts_the_observed_once() {
    let separated = [UnitBlock {
        a: vec![run(0.0, 1.0); 20],
        b: vec![run(1.0, 1.0); 20],
    }];
    let got = stratified_permutation(&separated, 100, 1).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(got.extreme, 0);
    close("statistic", got.statistic, 1.0);
    close("p of the observed alone", got.p, 1.0 / 101.0);
    let equal = [UnitBlock {
        a: vec![run(1.0, 2.0); 4],
        b: vec![run(1.0, 2.0); 4],
    }];
    let got = stratified_permutation(&equal, 100, 1).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(got.extreme, 100);
    close("p of full ties", got.p, 1.0);
}

#[derive(Deserialize)]
struct BcaRow {
    name: String,
    seed: u64,
    b: u32,
    level: f64,
    #[serde(flatten)]
    design: Design,
    low: f64,
    high: f64,
}

/// Guards: the `BCa` endpoints (the bias correction, the jackknife acceleration and the
/// interpolated quantiles) are the twin's, for one to three blocks, a binary design with many
/// ties and a degenerate block beside a varying one.
/// Oracle: the twin, whose endpoints the generator holds within 3 Monte Carlo SE of scipy's
/// `bootstrap(method="BCa")` mean over 20 seeds for one block.
#[test]
fn bca_matches_twin() {
    for row in vector::<BcaRow>("bca.toml") {
        let blocks = blocks_of(&row.design);
        let (low, high) = stratified_bca(&blocks, row.b, row.level, row.seed)
            .unwrap_or_else(|e| panic!("{}: {e}", row.name));
        close(&format!("{} low", row.name), low, row.low);
        close(&format!("{} high", row.name), high, row.high);
    }
}

/// Guards: a block whose runs are all equal gives a defined result: with no other block the
/// interval is the point `(T, T)` and `a = 0`; beside a varying block the interval is finite and
/// ordered.
/// Oracle: the documented result of the module (arithmetic: `T = 1/2 - 1/2 = 0`), and finiteness.
#[test]
fn bca_degenerate_block() {
    let flat = UnitBlock {
        a: vec![run(1.0, 2.0); 4],
        b: vec![run(1.0, 2.0); 5],
    };
    let alone = [flat.clone()];
    assert_eq!(stratified_bca(&alone, 200, 0.95, 3), Ok((0.0, 0.0)));
    assert_eq!(jackknife_acceleration(&alone), Ok(0.0));
    let varying = UnitBlock {
        a: vec![run(0.0, 2.0), run(1.0, 2.0), run(2.0, 2.0), run(1.0, 2.0)],
        b: vec![run(2.0, 2.0), run(1.0, 2.0), run(2.0, 2.0), run(0.0, 2.0)],
    };
    let (low, high) =
        stratified_bca(&[flat, varying], 500, 0.95, 3).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        low.is_finite() && high.is_finite() && low <= high,
        "({low}, {high})"
    );
}
