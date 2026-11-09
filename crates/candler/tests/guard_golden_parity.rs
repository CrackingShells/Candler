#![cfg(test)]
#![cfg(unix)]
//! The corpus regrade guard: the 240-run golden corpus is imported as `htn-creator-eval@0.4.2`
//! and regraded live twice, through `candler evaluate`, with the shipped HTN plug-in
//! `htn-creator-eval@0.5.2` (its tagged manifest, its `suite/suite.json`, its import rules).
//! Every expected number is read from `tests/oracle/` (written by `gen_oracle.py` from the raw
//! corpus, importing nothing from Candler) or is a hand literal below.
//!
//! The guards are ignored: they need the corpus, the plug-in and about three minutes. Run them with
//!
//! ```text
//! CANDLER_GOLDEN_CORPUS=<corpus> CANDLER_HTN_PLUGIN=<plug-in uv project> \
//!   cargo nextest run -p candler --run-ignored only -E 'binary(guard_golden_parity)'
//! ```
//!
//! `CANDLER_HTN_PLUGIN` is the plug-in's uv project directory; its manifest is one level up
//! (`../candler-plugin.toml`). `CANDLER_KEEP_STORE=<dir>` names the directory that holds the store
//! (`<dir>/store`), the plug-in environment and the two regrade reports; it is kept. Without it the
//! directory is `<temp>/candler-golden-parity`. Each test is its own process, so they share one
//! built fixture under a file lock. A finished fixture is reused only when its marker names the
//! same `candler` binary, rules file, manifest, suite and corpus, and the same digest of every tracked
//! file of the plug-in project and of the helper source; otherwise it is rebuilt. A build
//! that fails writes `failed.json`, with that marker and the failure message, before it panics: a
//! process that takes the lock and finds a `failed.json` with its own marker panics with the same
//! message at once, so a failed build costs one build and not one per test process.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::Instant;

use candler_core::evaluation::selection::select::{I4Key, select};
use candler_core::store::Store;
use serde_json::{Value, json};

/// The evaluator the import writes, and the reference side of the diff.
const FROM: &str = "htn-creator-eval@0.4.2";
/// The live evaluator, and the compared side of the diff.
const TO: &str = "htn-creator-eval@0.5.2";
/// The sha256 of the HTN rules file `import/htn-results-rules.toml`, from `shasum -a 256`.
const RULES_SHA256: &str =
    "sha256:5d25aa60db68a8340bad2856a8c9fb1e1f99d13dce161d6f08539afb22e56952";
/// The prefix of the pinned instrument's digest: commit `28be2c15` of the HTN repo.
const PINNED: &str = "git:28be2c15";
/// The runs of the corpus: 12 cells of 20 replicates.
const RUNS: u64 = 240;
/// The Tier D replay expectations the plug-in does not grade, each over every run: the rows the
/// diff must list as uncompared.
const UNCOMPARED: [(&str, u64); 6] = [
    ("D-E1", 240),
    ("D-E2", 240),
    ("D-E3", 240),
    ("D-E4", 240),
    ("D-E5", 240),
    ("D-E6", 240),
];
/// The concurrency of each regrade.
const CONCURRENCY: &str = "8";

/// The row key: the cell digest hex, the replicate, the attempt and the expectation.
type Key = (String, u64, u64, String);

/// The oracle directory.
fn oracle() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle")
}

/// The value of the variable `name`, which the guard cannot run without.
fn required(name: &str) -> PathBuf {
    match std::env::var_os(name) {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => panic!("{name} is not set: the golden parity guard needs it"),
    }
}

/// The lowercase sha256 hex of the file at `path`, from `shasum -a 256`.
fn sha256_of(path: &Path) -> String {
    let out = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .expect("shasum runs");
    assert!(out.status.success(), "shasum read {}", path.display());
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .expect("a digest")
        .to_owned()
}

/// The sha256 of every file `git ls-files` tracks under `dir`, in path order: one line of
/// `<digest>  <path>` each, hashed again as one text, so an edit to any of them changes it.
fn tree_digest(dir: &Path) -> String {
    let listed = Command::new("git")
        .args(["ls-files", "-z", "."])
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(listed.status.success(), "git listed {}", dir.display());
    let mut names: Vec<String> = String::from_utf8_lossy(&listed.stdout)
        .split('\0')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    assert!(!names.is_empty(), "{} tracks no file", dir.display());
    names.sort();
    let mut text = String::new();
    for name in &names {
        writeln!(text, "{}  {name}", sha256_of(&dir.join(name))).expect("a string writes");
    }
    let mut hasher = Command::new("shasum")
        .args(["-a", "256"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("shasum runs");
    hasher
        .stdin
        .take()
        .expect("a stdin pipe")
        .write_all(text.as_bytes())
        .expect("the text writes");
    let out = hasher.wait_with_output().expect("shasum finishes");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .expect("a digest")
        .to_owned()
}

/// What the fixture was built from, and where.
struct Fixture {
    /// The directory that holds the store, the environments and the reports.
    dir: PathBuf,
    /// The first regrade's report, as `candler evaluate` printed it.
    first: Value,
    /// The second regrade's report.
    second: Value,
}

/// The fixture of this process, built or reused once.
fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(open_fixture)
}

impl Fixture {
    /// The store root.
    fn store(&self) -> PathBuf {
        self.dir.join("store")
    }

    /// Runs `candler` over the fixture's store.
    fn candler(&self, args: &[&str]) -> Output {
        run_candler(&self.dir, args)
    }
}

/// Runs the binary with `args` over `<dir>/store`, with the state and configuration homes inside
/// `dir` (so the plug-in environment is built there) and the rest of the environment kept: `uv`
/// needs `PATH` and `HOME`.
fn run_candler(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .current_dir(dir)
        .env_remove("CANDLER_STORE")
        .env("XDG_STATE_HOME", dir.join("state"))
        .env("XDG_CONFIG_HOME", dir.join("cfg"))
        .arg("--store")
        .arg(dir.join("store"))
        .args(args)
        .output()
        .expect("the binary runs")
}

/// The stdout of a command that must have exited 0, parsed as JSON.
fn json_of(out: &Output, what: &str) -> Value {
    assert_eq!(
        out.status.code(),
        Some(0),
        "{what}: {}",
        String::from_utf8_lossy(&out.stderr)
            .lines()
            .filter(|line| !line.contains("event"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{what}: stdout is not JSON: {e}"))
}

/// The directory of the fixture, taken from `CANDLER_KEEP_STORE` or the temp directory.
fn fixture_dir() -> PathBuf {
    std::env::var_os("CANDLER_KEEP_STORE")
        .filter(|value| !value.is_empty())
        .map_or_else(
            || std::env::temp_dir().join("candler-golden-parity"),
            PathBuf::from,
        )
}

/// Opens the fixture: reuses a finished one whose marker matches, else wipes and builds it.
fn open_fixture() -> Fixture {
    let corpus = required("CANDLER_GOLDEN_CORPUS");
    let plugin_project = required("CANDLER_HTN_PLUGIN");
    let manifest = plugin_project.join("../candler-plugin.toml");
    let suite = plugin_project.join("suite/suite.json");
    let rules = plugin_project.join("import/htn-results-rules.toml");
    for path in [&corpus, &manifest, &suite, &rules] {
        assert!(path.exists(), "{} does not exist", path.display());
    }
    let dir = fixture_dir();
    fs::create_dir_all(&dir).expect("the fixture directory");
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("lock"))
        .expect("the lock file");
    lock.lock().expect("the fixture lock");

    let marker = json!({
        "binary": sha256_of(Path::new(env!("CARGO_BIN_EXE_candler"))),
        "rules": sha256_of(&rules),
        "manifest": sha256_of(&manifest),
        "suite": sha256_of(&suite),
        "plugin_tree": tree_digest(&plugin_project),
        "helper_src": tree_digest(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../python/candler-evaluator/src"),
        ),
        "corpus": corpus.display().to_string(),
    });
    let marker_path = dir.join("complete.json");
    let finished = fs::read_to_string(&marker_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .is_some_and(|found| found == marker);
    if !finished {
        rebuild_unless_failed(&dir, &marker, &corpus, &manifest, &suite, &rules);
        fs::write(&marker_path, marker.to_string()).expect("the marker writes");
    }
    let read = |name: &str| -> Value {
        let text = fs::read_to_string(dir.join(name)).expect("a regrade report reads");
        serde_json::from_str(&text).expect("a regrade report is JSON")
    };
    let fixture = Fixture {
        first: read("regrade1.json"),
        second: read("regrade2.json"),
        dir,
    };
    check_import_rules(&fixture.store());
    drop(lock);
    fixture
}

/// Builds the fixture, unless an earlier process already failed to build it from the same inputs.
/// The failure of a build is written to `failed.json` with `marker` before it is raised again.
fn rebuild_unless_failed(
    dir: &Path,
    marker: &Value,
    corpus: &Path,
    manifest: &Path,
    suite: &Path,
    rules: &Path,
) {
    let failed_path = dir.join("failed.json");
    if let Some(failed) = fs::read_to_string(&failed_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(|found| found["marker"] == *marker)
    {
        panic!(
            "the fixture build failed in an earlier test process: {}",
            failed["message"].as_str().unwrap_or("no message")
        );
    }
    let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        build(dir, corpus, manifest, suite, rules);
    }));
    if let Err(payload) = built {
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("the build panicked without a message");
        let failed = json!({"marker": marker, "message": message});
        fs::write(&failed_path, failed.to_string()).expect("the failure marker writes");
        std::panic::resume_unwind(payload);
    }
}

/// Wipes `dir` (but its lock), imports the corpus and regrades it twice, and records each step's
/// wall-clock in `timings.txt`.
fn build(dir: &Path, corpus: &Path, manifest: &Path, suite: &Path, rules: &Path) {
    for entry in fs::read_dir(dir).expect("the fixture directory reads") {
        let path = entry.expect("an entry").path();
        if path.file_name().is_some_and(|name| name == "lock") {
            continue;
        }
        if path.is_dir() {
            fs::remove_dir_all(&path).expect("a stale directory goes");
        } else {
            fs::remove_file(&path).expect("a stale file goes");
        }
    }
    let mut timings = String::new();

    let total = oracle_counts()["total"].as_integer().expect("a total");
    let started = Instant::now();
    let imported = json_of(
        &run_candler(
            dir,
            &[
                "import",
                corpus.to_str().expect("a UTF-8 path"),
                "--rules",
                rules.to_str().expect("a UTF-8 path"),
            ],
        ),
        "import",
    );
    writeln!(timings, "import {:?}", started.elapsed()).expect("a string writes");
    assert_eq!(
        imported,
        json!({"runs": RUNS, "records": total}),
        "the import holds 240 runs and the oracle's 10,320 records"
    );

    for (n, name) in [(1, "regrade1.json"), (2, "regrade2.json")] {
        let started = Instant::now();
        let out = run_candler(
            dir,
            &[
                "evaluate",
                "--plugin",
                manifest.to_str().expect("a UTF-8 path"),
                "--suite",
                suite.to_str().expect("a UTF-8 path"),
                "--concurrency",
                CONCURRENCY,
            ],
        );
        writeln!(timings, "regrade {n} {:?}", started.elapsed()).expect("a string writes");
        let report = json_of(&out, "evaluate");
        assert_eq!(
            report["failed_requests"],
            json!([]),
            "regrade {n} lost no request"
        );
        fs::write(dir.join(name), &out.stdout).expect("a regrade report writes");
    }
    let mut file = fs::File::create(dir.join("timings.txt")).expect("the timings file");
    file.write_all(timings.as_bytes())
        .expect("the timings write");
}

/// Every run's manifest names the HTN rules file by its sha256: the hand literal above.
fn check_import_rules(store: &Path) {
    let mut seen = 0_u64;
    for entry in fs::read_dir(store.join("runs")).expect("the runs directory reads") {
        let path = entry.expect("an entry").path().join("manifest.json");
        let text = fs::read_to_string(&path).expect("a manifest reads");
        let manifest: Value = serde_json::from_str(&text).expect("a manifest is JSON");
        assert_eq!(manifest["import"]["rules"], RULES_SHA256, "{path:?}");
        seen += 1;
    }
    assert_eq!(seen, RUNS, "every run was imported under the HTN rules");
}

/// `counts.toml`.
fn oracle_counts() -> toml::Table {
    fs::read_to_string(oracle().join("counts.toml"))
        .expect("counts.toml reads")
        .parse()
        .expect("counts.toml parses")
}

/// The statuses of `[live_expected]` with a non-zero count.
fn live_expected() -> BTreeMap<String, u64> {
    let counts = oracle_counts();
    counts["live_expected"]
        .as_table()
        .expect("a [live_expected] table")
        .iter()
        .filter_map(|(status, n)| {
            let n = u64::try_from(n.as_integer().expect("an integer")).expect("a count");
            (n > 0).then(|| (status.clone(), n))
        })
        .collect()
}

/// The `(cell hex, run, expectation)` of each line of `na_pairs.txt`.
fn na_pairs() -> BTreeSet<Key> {
    fs::read_to_string(oracle().join("na_pairs.txt"))
        .expect("na_pairs.txt reads")
        .lines()
        .map(|line| {
            let mut parts = line.split('\t');
            let cell = parts.next().expect("a cell").to_owned();
            let run: u64 = parts.next().expect("a run").parse().expect("a number");
            let expectation = parts.next().expect("an expectation").to_owned();
            (cell, run, 1, expectation)
        })
        .collect()
}

/// The key of an outcome record as raw JSON.
fn key_of(record: &Value) -> Key {
    let run = &record["run"];
    let cell = run["cell"].as_str().expect("a cell");
    (
        cell.strip_prefix("sha256:").unwrap_or(cell).to_owned(),
        run["replicate"].as_u64().expect("a replicate"),
        run["attempt"].as_u64().expect("an attempt"),
        record["expectation_id"]
            .as_str()
            .expect("an expectation")
            .to_owned(),
    )
}

/// The four fields that decide whether two rows are the same grade; an absent field and `null`
/// read alike.
fn four(record: &Value) -> [Value; 4] {
    ["status", "value", "weight", "cause"].map(|name| {
        record
            .get(name)
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or(Value::Null)
    })
}

/// The records of the outcome files `paths`, by key; a key twice is a defect.
fn rows_of<I: IntoIterator<Item = PathBuf>>(paths: I) -> BTreeMap<Key, Value> {
    let mut rows = BTreeMap::new();
    for path in paths {
        let text = fs::read_to_string(&path).expect("an outcome file reads");
        for line in text.lines() {
            let record: Value = serde_json::from_str(line).expect("a record is JSON");
            let key = key_of(&record);
            assert!(rows.insert(key.clone(), record).is_none(), "{key:?} twice");
        }
    }
    rows
}

/// The outcome files a regrade report names.
fn files_of(report: &Value) -> Vec<PathBuf> {
    report["outcome_files"]
        .as_array()
        .expect("outcome_files is a list")
        .iter()
        .map(|path| PathBuf::from(path.as_str().expect("a path")))
        .collect()
}

/// The outcome files of the evaluator `evaluator` in the store, whatever request wrote them.
fn files_of_evaluator(store: &Path, evaluator: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for run in fs::read_dir(store.join("outcomes")).expect("the outcomes directory reads") {
        let dir = run.expect("an entry").path().join(evaluator);
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries {
                found.push(entry.expect("an entry").path());
            }
        }
    }
    found
}

/// The statuses of `rows`, counted.
fn status_counts(rows: &BTreeMap<Key, Value>) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for record in rows.values() {
        let status = record["status"].as_str().expect("a status").to_owned();
        *counts.entry(status).or_insert(0) += 1;
    }
    counts
}

/// Guards: R1 — the live A-C grades equal the reference: the status counts of the 8,880 live rows
/// and of `show` equal `[live_expected]` of `counts.toml` (7,790 pass, 209 not applicable, 415
/// fail, 466 blocked); row by row on status, value, weight and cause every live row equals the
/// imported `0.4.2` row, except that the 209 `(cell, run, expectation)` pairs of `na_pairs.txt` are
/// imported as `pass` 0/0 with the legacy `zero_weight` cause and live as `not_applicable` with the
/// reason `empty_population`; and `candler show --rates` prints `rates_by_cell.toml` byte for byte.
/// Oracle: `counts.toml`, `na_pairs.txt`, `rates_by_cell.toml`, all generated from the raw corpus
/// by `gen_oracle.py`; the imported rows are the store's own `0.4.2` records.
#[test]
#[ignore = "needs CANDLER_GOLDEN_CORPUS and CANDLER_HTN_PLUGIN; about three minutes"]
fn the_live_ac_rows_and_rates_equal_the_reference() {
    let fixture = fixture();
    let expected = live_expected();

    let live = rows_of(files_of(&fixture.first));
    assert_eq!(
        live.len(),
        8880,
        "the first regrade holds the 8,880 A-C rows"
    );
    assert_eq!(status_counts(&live), expected, "the live status counts");

    let shown = json_of(
        &fixture.candler(&["show", "--evaluator", TO, "--format", "json"]),
        "show",
    );
    let shown_counts: BTreeMap<String, u64> =
        serde_json::from_value(shown["counts"][TO].clone()).expect("the counts of the evaluator");
    assert_eq!(
        shown_counts, expected,
        "`show` counts the same current rows"
    );

    let imported: BTreeMap<Key, Value> = rows_of(files_of_evaluator(&fixture.store(), FROM))
        .into_iter()
        .filter(|(key, _)| !key.3.starts_with("D-E"))
        .collect();
    assert_eq!(imported.len(), 8880, "the import holds the 8,880 A-C rows");

    let pairs = na_pairs();
    assert_eq!(pairs.len(), 209, "the oracle names 209 excused pairs");
    let mut matched = 0_u32;
    for (key, old) in &imported {
        let new = live
            .get(key)
            .unwrap_or_else(|| panic!("{key:?} has no live row"));
        if pairs.contains(key) {
            assert_eq!(
                four(old),
                [
                    json!("pass"),
                    Value::Null,
                    json!({"num": 0, "den": 0}),
                    json!({"legacy": "zero_weight"}),
                ],
                "{key:?} imports as a zero-weight pass"
            );
            assert_eq!(
                four(new),
                [
                    json!("not_applicable"),
                    Value::Null,
                    Value::Null,
                    json!({"reason": "empty_population"}),
                ],
                "{key:?} is live not applicable"
            );
            matched += 1;
        } else {
            assert_eq!(four(old), four(new), "{key:?} differs from the import");
        }
    }
    assert_eq!(
        matched, 209,
        "every oracle pair met an imported and a live row"
    );

    let rates = fixture.candler(&["show", "--rates", "--evaluator", TO, "--format", "toml"]);
    assert_eq!(rates.status.code(), Some(0), "show --rates");
    let published = fs::read(oracle().join("rates_by_cell.toml")).expect("the rates read");
    assert!(
        rates.stdout == published,
        "`show --rates` differs from rates_by_cell.toml:\n{}",
        String::from_utf8_lossy(&rates.stdout)
    );
}

/// Guards: R2, R1 — the version diff is exact and complete: `candler diff` from `0.4.2` to `0.5.2`
/// with the committed `equivalence.json` exits 0 with 8,880 compared, none changed, `unchanged +
/// excused + changed == compared`, `excused` equal to the line count of `na_pairs.txt`, the
/// `equivalence.sha256` equal to the sha256 of the committed file's bytes, `compared` plus the
/// uncompared counts equal to the 10,320 imported rows, and the uncompared expectations exactly
/// D-E1 to D-E6 with 240 rows each. Oracle: the hand-written D-E literal, `na_pairs.txt`, the
/// bytes of `equivalence.json` (`shasum -a 256`) and `counts.toml`'s `total`.
#[test]
#[ignore = "needs CANDLER_GOLDEN_CORPUS and CANDLER_HTN_PLUGIN; about three minutes"]
fn the_version_diff_compares_everything_it_can_and_excuses_only_the_named_rows() {
    let fixture = fixture();
    let equivalence = oracle().join("equivalence.json");
    let out = fixture.candler(&[
        "diff",
        "--from",
        FROM,
        "--to",
        TO,
        "--equivalence",
        equivalence.to_str().expect("a UTF-8 path"),
        "--format",
        "json",
    ]);
    let report = json_of(&out, "diff");
    let number = |name: &str| {
        report[name]
            .as_u64()
            .unwrap_or_else(|| panic!("`{name}` missing"))
    };

    assert_eq!(number("compared"), 8880, "{report}");
    assert_eq!(number("changed"), 0, "{report}");
    assert_eq!(
        report["changes"],
        json!([]),
        "no change, whatever its class"
    );
    assert_eq!(
        number("unchanged") + number("excused") + number("changed"),
        number("compared"),
        "every compared row is unchanged, excused or changed"
    );
    let lines = fs::read_to_string(oracle().join("na_pairs.txt"))
        .expect("na_pairs.txt reads")
        .lines()
        .count();
    assert_eq!(
        number("excused"),
        u64::try_from(lines).expect("a count"),
        "the excused rows are the lines of na_pairs.txt"
    );
    assert_eq!(number("excused"), 209, "and the oracle holds 209");
    assert_eq!(
        report["equivalence"]["sha256"],
        sha256_of(&equivalence),
        "the diff names the committed equivalence file's bytes"
    );
    assert_eq!(report["equivalence"]["rows"], 209, "{report}");

    let uncompared: Vec<(String, u64)> = report["uncompared"]
        .as_array()
        .expect("uncompared is a list")
        .iter()
        .map(|entry| {
            (
                entry["expectation"]
                    .as_str()
                    .expect("an expectation")
                    .to_owned(),
                entry["count"].as_u64().expect("a count"),
            )
        })
        .collect();
    let literal: Vec<(String, u64)> = UNCOMPARED
        .iter()
        .map(|(id, count)| ((*id).to_owned(), *count))
        .collect();
    assert_eq!(
        uncompared, literal,
        "the uncompared rows are exactly D-E1 to D-E6"
    );
    let total =
        u64::try_from(oracle_counts()["total"].as_integer().expect("a total")).expect("a count");
    assert_eq!(
        number("compared") + uncompared.iter().map(|(_, n)| n).sum::<u64>(),
        total,
        "compared plus uncompared is every imported row"
    );
}

/// Guards: R5, R7, R6 — the second regrade is new and identical: its request ids are disjoint from
/// the first's, it holds the 8,880 A-C rows, and row by row on status, value, weight and cause it
/// equals the first; the store holds no I4 violation, and each of the 8,880 live I4 keys is held
/// by two records, so the check compared something; every live record's instrument digest starts
/// with `git:28be2c15`; no record carries the reason `deadline`. Oracle: the pinned sha literal,
/// and the first regrade's rows (read raw, compared here, not by the engine).
#[test]
#[ignore = "needs CANDLER_GOLDEN_CORPUS and CANDLER_HTN_PLUGIN; about three minutes"]
fn the_second_regrade_sends_new_requests_and_changes_nothing() {
    let fixture = fixture();
    let ids = |report: &Value| -> BTreeSet<String> {
        report["request_ids"]
            .as_array()
            .expect("request_ids is a list")
            .iter()
            .map(|id| id.as_str().expect("an id").to_owned())
            .collect()
    };
    let (first_ids, second_ids) = (ids(&fixture.first), ids(&fixture.second));
    assert!(
        !first_ids.is_empty() && !second_ids.is_empty(),
        "both regrades sent"
    );
    assert!(
        first_ids.is_disjoint(&second_ids),
        "the second regrade sends new requests"
    );

    let first = rows_of(files_of(&fixture.first));
    let second = rows_of(files_of(&fixture.second));
    assert_eq!(
        second.len(),
        8880,
        "the second regrade holds the 8,880 A-C rows"
    );
    assert_eq!(
        first.keys().collect::<Vec<_>>(),
        second.keys().collect::<Vec<_>>(),
        "both regrades grade the same rows"
    );
    for (key, row) in &first {
        let again = second.get(key).expect("a row of the second regrade");
        assert_eq!(four(row), four(again), "{key:?} changed between regrades");
        let request = |record: &Value| record["request_id"].as_str().expect("a request").to_owned();
        assert!(first_ids.contains(&request(row)), "{key:?}: first request");
        assert!(
            second_ids.contains(&request(again)),
            "{key:?}: second request"
        );
    }

    let store = Store::open(&fixture.store()).expect("the store opens");
    let mut records = Vec::new();
    for entry in store.runs().expect("the runs list") {
        for file in store.outcome_files(&entry.key).expect("the files list") {
            records.extend(store.read_outcomes(&file).expect("a file reads"));
        }
    }
    let mut held: BTreeMap<I4Key, u32> = BTreeMap::new();
    for record in records.iter().filter(|r| r.evaluator().to_string() == TO) {
        *held.entry(I4Key::of(record)).or_insert(0) += 1;
    }
    assert_eq!(held.len(), 8880, "8,880 live I4 keys");
    assert!(
        held.values().all(|&n| n == 2),
        "each live I4 key is held by one record per regrade"
    );
    let selection = select(records);
    assert!(
        selection.violations().is_empty(),
        "{} I4 violations",
        selection.violations().len()
    );

    let mut graded = 0_u32;
    for record in first.values().chain(second.values()) {
        let instruments = record["instruments"].as_array().expect("instruments");
        assert!(!instruments.is_empty(), "{record} names no instrument");
        for instrument in instruments {
            let digest = instrument["digest"].as_str().expect("a digest");
            assert!(
                digest.starts_with(PINNED),
                "{digest} is not the pinned instrument"
            );
        }
        assert_ne!(record["cause"]["reason"], "deadline", "{record}");
        graded += 1;
    }
    assert_eq!(
        graded,
        2 * 8880,
        "every live record of both regrades was checked"
    );
}
