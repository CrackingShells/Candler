#![cfg(test)]
//! The golden-corpus guard of `candler analyze`: the 240-run corpus is imported with the binary
//! into a temporary store and analysed from a directory that is no project. Every expected number
//! is read from `tests/oracle/` (written by `gen_oracle.py` from the raw corpus, importing nothing
//! from Candler) with `toml` and no Candler type.
//!
//! The guards are ignored: they need the corpus and about a minute. Run them with
//!
//! ```text
//! CANDLER_GOLDEN_CORPUS=<corpus> cargo nextest run -p candler --run-ignored only \
//!   -E 'test(/^guard_analyze_golden::/)'
//! ```
//!
//! The corpus is read and never written.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

/// The oracle directory.
fn oracle() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle")
}

/// The corpus the guard cannot run without.
fn corpus() -> PathBuf {
    match std::env::var_os("CANDLER_GOLDEN_CORPUS") {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => panic!("CANDLER_GOLDEN_CORPUS is not set: the analyze golden guard needs it"),
    }
}

fn oracle_table(name: &str) -> toml::Table {
    let path = oracle().join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .parse()
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Runs the binary with the store and an unrelated home, from `home`.
fn candler(home: &Path, store: &Path, args: &[&std::ffi::OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .current_dir(home)
        .env_remove("CANDLER_STORE")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .arg("--store")
        .arg(store)
        .args(args)
        .output()
        .expect("the binary runs")
}

/// Imports the corpus into a fresh store and returns the report of `analyze --format json`.
fn analysed() -> (TempDir, Value) {
    let dir = TempDir::new().expect("a temporary directory");
    let store = dir.path().join("store");
    let rules =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli/mini-corpus-rules.toml");
    let imported = Command::new(env!("CARGO_BIN_EXE_candler"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("CANDLER_STORE")
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join("config"))
        .env("XDG_STATE_HOME", dir.path().join("state"))
        .arg("--store")
        .arg(&store)
        .arg("import")
        .arg(corpus())
        .arg("--rules")
        .arg(&rules)
        .output()
        .expect("the binary runs");
    assert_eq!(
        imported.status.code(),
        Some(0),
        "import: {}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let out = candler(
        dir.path(),
        &store,
        &["analyze".as_ref(), "--format".as_ref(), "json".as_ref()],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "analyze: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report = serde_json::from_slice(&out.stdout).expect("the output is JSON");
    (dir, report)
}

fn pair(value: &Value) -> [u64; 2] {
    [
        value["num"].as_u64().expect("num"),
        value["den"].as_u64().expect("den"),
    ]
}

fn oracle_pair(value: &toml::Value) -> [u64; 2] {
    let list = value.as_array().expect("a [numerator, denominator] pair");
    let at = |i: usize| {
        u64::try_from(
            list.get(i)
                .and_then(toml::Value::as_integer)
                .expect("an integer"),
        )
        .expect("a count")
    };
    [at(0), at(1)]
}

/// The expectations of the oracle: the A-C rows. The imported evaluator also grades the replay
/// expectations `D-E1` to `D-E6`, which the oracle lists apart as uncompared.
fn is_replay(id: &str) -> bool {
    id.starts_with("D-E")
}

fn expectations(arm: &Value) -> &Vec<Value> {
    arm["expectations"].as_array().expect("expectations")
}

fn count(value: &Value, key: &str) -> u64 {
    value[key].as_u64().expect("a count")
}

/// The sum of the effective weights of an arm's expectations of one role, over the A-C rows
/// (`ac`) or over the replay rows.
fn weight_sum(arm: &Value, role: &str, ac: bool) -> [u64; 2] {
    let mut sum = [0_u64; 2];
    for held in expectations(arm) {
        let id = held["id"].as_str().expect("id");
        if held["role"] == role && is_replay(id) != ac {
            sum[0] += count(&held["weight"], "num");
            sum[1] += count(&held["weight"], "den");
        }
    }
    sum
}

/// Guards: A4 — on the real corpus every cell is one of the oracle's 12, and per cell the gate
/// and quality sums of the A-C expectations equal `[numerator, denominator]` of the second
/// engine's `rates_by_cell.toml` exactly; the arm's own `rates` are those sums plus the replay
/// expectations (`D-E3` to `D-E6` are gate rows of the imported evaluator and the oracle does not
/// rate them), so no row is lost or counted twice; and the corpus's denominators, which exceed
/// its run counts, give expectations `weighted`.
/// Oracle: `tests/oracle/rates_by_cell.toml`, derived from the raw corpus by `gen_oracle.py`.
/// Bite: count `not_measured` in a denominator, or `blocked` as a pass, and a cell's pair turns
/// red.
#[test]
#[ignore = "needs CANDLER_GOLDEN_CORPUS and about a minute"]
fn a4_per_cell_rates_equal_the_oracle() {
    let (_dir, report) = analysed();
    let oracle = oracle_table("rates_by_cell.toml");
    let cells = oracle["cell"].as_table().expect("the cell tables");
    let arms = report["arms"].as_array().expect("arms");
    let got: BTreeSet<&str> = arms
        .iter()
        .map(|arm| arm["cell"].as_str().expect("cell"))
        .collect();
    let want: BTreeSet<&str> = cells.keys().map(String::as_str).collect();
    assert_eq!(got, want, "the 12 cells of the oracle");
    assert_eq!(arms.len(), 12);
    for arm in arms {
        let cell = arm["cell"].as_str().expect("cell");
        let row = &cells[cell];
        for role in ["gate", "quality"] {
            assert_eq!(
                weight_sum(arm, role, true),
                oracle_pair(&row[role]),
                "{cell} {role}, A-C rows"
            );
            let [ac_num, ac_den] = weight_sum(arm, role, true);
            let [de_num, de_den] = weight_sum(arm, role, false);
            assert_eq!(
                pair(&arm["rates"][role]),
                [ac_num + de_num, ac_den + de_den],
                "{cell} {role}: the arm's rate is its A-C rows and its replay rows"
            );
        }
    }
    // The denominator of a plain expectation is its graded runs: passes, failures and blocked
    // outcomes, and never a `not_measured` or `not_applicable` outcome or a zero-weight pass.
    for held in arms.iter().flat_map(expectations) {
        if held["role"] == "descriptive" || held["omitted"] == "weighted" {
            continue;
        }
        let graded = count(held, "pass") + count(held, "fail") + count(held, "blocked");
        assert_eq!(count(&held["weight"], "den"), graded, "{}", held["id"]);
    }
    let weighted = arms
        .iter()
        .flat_map(expectations)
        .filter(|e| e["omitted"] == "weighted")
        .count();
    assert!(
        weighted > 0,
        "the oracle's denominators exceed the run count"
    );
}

/// Guards: A4 — over the 12 cells and the A-C expectations, the passes (zero-weight passes
/// included), failures and blocked outcomes equal the `[imported]` table of `counts.toml`, the
/// zero-weight passes equal its `zero_weight`, and `not_measured` is 0: no `not_measured` is
/// counted in a denominator and no `blocked` is counted as a pass. Each replay expectation holds
/// one row per run, 240 as the oracle's `[uncompared]` table says. The oracle counts every role
/// of the A-C rows, descriptive included, so the totals are taken per expectation and not from
/// the arms' gate and quality `status`.
/// Oracle: `tests/oracle/counts.toml`, derived from the raw corpus by `gen_oracle.py`.
/// Bite: count a `blocked` as a pass, or leave the zero-weight passes out of `pass`, and a total
/// turns red.
#[test]
#[ignore = "needs CANDLER_GOLDEN_CORPUS and about a minute"]
fn a4_status_totals_equal_the_oracle_counts() {
    let (_dir, report) = analysed();
    let counts = oracle_table("counts.toml");
    let want = counts["imported"].as_table().expect("[imported]");
    let expected =
        |key: &str| u64::try_from(want[key].as_integer().expect("an integer")).expect("a count");
    let arms = report["arms"].as_array().expect("arms");
    let ac: Vec<&Value> = arms
        .iter()
        .flat_map(expectations)
        .filter(|e| !is_replay(e["id"].as_str().expect("id")))
        .collect();
    let sum = |key: &str| ac.iter().map(|e| count(e, key)).sum::<u64>();
    let causes = |key: &str| {
        ac.iter()
            .map(|e| {
                e[key]
                    .as_object()
                    .expect("causes")
                    .values()
                    .map(|n| n.as_u64().expect("a count"))
                    .sum::<u64>()
            })
            .sum::<u64>()
    };
    assert_eq!(
        sum("pass") + sum("zero_weight"),
        expected("pass"),
        "pass, zero-weight passes included"
    );
    assert_eq!(sum("fail"), expected("fail"));
    assert_eq!(sum("blocked"), expected("blocked"));
    assert_eq!(sum("zero_weight"), expected("zero_weight"));
    assert_eq!(causes("not_measured"), expected("not_measured"));
    // The arms' status totals cover the gate and quality rows of every expectation, replay rows
    // included, and no descriptive row.
    let status = |key: &str| {
        arms.iter()
            .map(|arm| count(&arm["status"], key))
            .sum::<u64>()
    };
    let graded: u64 = arms
        .iter()
        .flat_map(expectations)
        .filter(|e| e["role"] != "descriptive")
        .map(|e| count(e, "pass") + count(e, "zero_weight"))
        .sum();
    assert_eq!(status("pass"), graded);
    let uncompared = counts["uncompared"].as_table().expect("[uncompared]");
    for (id, rows) in uncompared {
        let held: u64 = arms
            .iter()
            .flat_map(expectations)
            .filter(|e| e["id"] == id.as_str())
            .map(|e| {
                ["pass", "fail", "blocked", "zero_weight"]
                    .iter()
                    .map(|key| count(e, key))
                    .sum::<u64>()
                    + ["not_measured", "not_applicable"]
                        .iter()
                        .map(|key| {
                            e[*key]
                                .as_object()
                                .expect("causes")
                                .values()
                                .map(|n| n.as_u64().expect("a count"))
                                .sum::<u64>()
                        })
                        .sum::<u64>()
            })
            .sum();
        assert_eq!(
            held,
            u64::try_from(rows.as_integer().expect("an integer")).expect("a count"),
            "{id} has one row per run"
        );
    }
}
