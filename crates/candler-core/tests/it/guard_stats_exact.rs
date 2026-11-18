#![cfg(test)]
//! The mid-p and the exact conditional test against scipy's hypergeometric and exact arithmetic.
//! The vectors are written by `tests/vectors/stats/gen_stats.py`; a row is never edited to make a
//! guard pass.

use std::path::PathBuf;

use candler_core::analysis::blocking::Stratum;
use candler_core::analysis::exact_strata::exact_conditional;
use candler_core::analysis::midp::midp_two_sided;
use serde::Deserialize;
use serde::de::DeserializeOwned;

const TOLERANCE: f64 = 1e-12;

#[derive(Deserialize)]
struct Vectors<R> {
    rows: Vec<R>,
}

fn vector<R: DeserializeOwned>(file: &str) -> Vec<R> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/vectors/stats")
        .join(file);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let parsed: Vectors<R> = toml::from_str(&text).unwrap_or_else(|e| panic!("{file}: {e}"));
    assert!(!parsed.rows.is_empty(), "{file} has no rows");
    parsed.rows
}

fn stratum(a: (u64, u64), b: (u64, u64)) -> Stratum {
    Stratum {
        key: Vec::new(),
        a,
        b,
    }
}

fn agrees(got: f64, want: f64) -> bool {
    (got - want).abs() <= TOLERANCE * want.abs().max(f64::MIN_POSITIVE)
}

#[derive(Deserialize)]
struct MidpRow {
    a_k: u64,
    a_n: u64,
    b_k: u64,
    b_n: u64,
    p: f64,
    tie: bool,
}

/// Guards: Fisher's two-sided mid-p of every `midp.toml` row (every table with up to 12 runs per
/// arm, empty arms and zero or full passes included, a sample up to 200 runs per arm, and the
/// equal-arm tie tables), to a relative 1e-12; at least 100 rows have another table exactly as
/// likely as the observed one.
/// Oracle: scipy `hypergeom` pmfs, themselves asserted by the generator against exact rational
/// arithmetic and brute-force enumeration.
/// Bite: the half dropped from the tied tables (the mid-p becomes Fisher's p).
#[test]
fn midp_matches_scipy_hypergeom() {
    let rows = vector::<MidpRow>("midp.toml");
    let ties = rows.iter().filter(|row| row.tie).count();
    assert!(ties >= 100, "the vector holds {ties} tie rows");
    for row in &rows {
        let table = stratum((row.a_k, row.a_n), (row.b_k, row.b_n));
        let got = midp_two_sided(&table).unwrap_or_else(|e| {
            panic!(
                "{}/{} against {}/{}: {e}",
                row.a_k, row.a_n, row.b_k, row.b_n
            )
        });
        assert!(
            agrees(got, row.p),
            "{}/{} against {}/{}: got {got:?}, scipy has {:?}",
            row.a_k,
            row.a_n,
            row.b_k,
            row.b_n,
            row.p
        );
    }
}

#[derive(Deserialize)]
struct ExactRow {
    a_k: Vec<u64>,
    a_n: Vec<u64>,
    b_k: Vec<u64>,
    b_n: Vec<u64>,
    p: f64,
}

/// Guards: the mid-p of the exact conditional test of every `exact.toml` row (2 to 4 blocks,
/// zero margins included), to a relative 1e-12.
/// Oracle: the convolution of scipy `hypergeom` pmfs, asserted by the generator against exact
/// rational convolution and brute-force enumeration of every table set.
/// Bite: the last block left out of the convolution.
#[test]
fn exact_strata_matches_convolution() {
    let rows = vector::<ExactRow>("exact.toml");
    for row in &rows {
        let blocks = row.a_k.len();
        assert!(
            [row.a_n.len(), row.b_k.len(), row.b_n.len()]
                .iter()
                .all(|len| *len == blocks),
            "a row has ragged block lists: {:?}",
            row.a_n
        );
        let strata: Vec<Stratum> = (0..blocks)
            .map(|k| {
                stratum(
                    (
                        *row.a_k.get(k).expect("same length"),
                        *row.a_n.get(k).expect("same length"),
                    ),
                    (
                        *row.b_k.get(k).expect("same length"),
                        *row.b_n.get(k).expect("same length"),
                    ),
                )
            })
            .collect();
        let got = exact_conditional(&strata).unwrap_or_else(|e| panic!("{:?}: {e}", row.a_n));
        assert!(
            agrees(got, row.p),
            "blocks a {:?}/{:?}, b {:?}/{:?}: got {got:?}, the convolution has {:?}",
            row.a_k,
            row.a_n,
            row.b_k,
            row.b_n,
            row.p
        );
    }
}
