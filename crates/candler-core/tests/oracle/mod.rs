//! An oracle for the legacy import: expected outcome rows and run facts, derived from raw corpus
//! files read as `serde_json::Value`.
//!
//! The oracle reads the per-run `eval-K-grading.json` files, `PROVENANCE.json`, each cell's
//! `experiment.json` and each run's `eval_metadata.json` and stream. It uses no Candler type, and
//! it cross-checks every row against the corpus's own `expectations.jsonl`.
//!
//! Included by a test binary with `#[path = "oracle/mod.rs"] mod oracle;`.
#![allow(dead_code, reason = "each guard binary uses a subset of the oracle")]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::MultiGzDecoder;
use serde_json::Value;

/// One expected outcome record, as plain strings and integers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExpectedRow {
    /// The cell directory name.
    pub cell: String,
    /// The run number `N` of `run-N`.
    pub run_n: u32,
    /// The expectation id.
    pub expectation_id: String,
    /// `gate`, `quality` or `descriptive`.
    pub role: String,
    /// `pass`, `fail`, `blocked`, `not_measured` or `not_applicable`.
    pub status: String,
    /// `count:<n>`, `number:<f64 as serde_json prints it>` or `text:<s>`; absent without a value.
    pub value: Option<String>,
    /// `<num>/<den>`; absent when the status default applies or the status has no weight.
    pub weight: Option<String>,
    /// `legacy: <code>`, `artifact: <path>` or `reason: <code>`.
    pub cause: Option<String>,
}

/// The integrity a run is imported with.
#[derive(Debug, Clone, PartialEq, Eq)]
// EXHAUSTIVE: a legacy verdict is valid or invalid
pub enum ExpectedIntegrity {
    /// A `clean` or `incomplete` legacy verdict.
    Valid,
    /// A `contaminated` legacy verdict, with the invalid reason.
    Invalid(String),
}

/// What one run's raw files say about the run itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedRun {
    /// The cell directory name.
    pub cell: String,
    /// The run number `N` of `run-N`.
    pub run_n: u32,
    /// The imported integrity.
    pub integrity: ExpectedIntegrity,
    /// The `claude_code_version` of the stream's init line.
    pub harness_version: String,
    /// `(path, JSON text)` of each `provenance.*` leaf that `experiment.json` holds as `null`,
    /// sorted by path.
    pub measured: Vec<(String, String)>,
}

/// A legacy grading status. The match over it has no catch-all arm.
#[derive(Debug, Clone, Copy)]
enum LegacyStatus {
    Pass,
    Fail,
    Blocked,
    NotMeasured,
    NotApplicable,
}

/// One run listed in `PROVENANCE.json`.
struct RunId {
    cell: String,
    run_n: u32,
    verdict: String,
}

fn read_json(path: &Path) -> Value {
    let text =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()))
}

fn parse_status(text: &str, context: &str) -> LegacyStatus {
    match text {
        "pass" => LegacyStatus::Pass,
        "fail" => LegacyStatus::Fail,
        "blocked" => LegacyStatus::Blocked,
        "not_measured" => LegacyStatus::NotMeasured,
        "not_applicable" => LegacyStatus::NotApplicable,
        other => panic!("unknown legacy status {other:?} at {context}"),
    }
}

fn parse_run_n(run: &str) -> u32 {
    run.strip_prefix("run-")
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("run folder {run:?} is not run-N"))
}

fn runs_of(corpus: &Path) -> Vec<RunId> {
    let provenance = read_json(&corpus.join("PROVENANCE.json"));
    // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
    let listed = provenance["runs"]
        .as_array()
        .unwrap_or_else(|| panic!("PROVENANCE.json has no runs[]"));
    let mut runs: Vec<RunId> = listed
        .iter()
        .map(|r| RunId {
            // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
            cell: str_of(&r["cell"], "runs[].cell"),
            run_n: parse_run_n(&str_of(&r["run"], "runs[].run")),
            verdict: str_of(
                &r["original_integrity"]["verdict"],
                "runs[].original_integrity.verdict",
            ),
        })
        .collect();
    runs.sort_by(|a, b| (&a.cell, a.run_n).cmp(&(&b.cell, b.run_n)));
    runs
}

fn str_of(value: &Value, context: &str) -> String {
    // BORROW: explicit `.to_owned()` of a `&str` for the owned field
    value
        .as_str()
        .unwrap_or_else(|| panic!("{context} is not a string: {value}"))
        .to_owned()
}

fn int_of(value: &Value, context: &str) -> i64 {
    value
        .as_i64()
        .unwrap_or_else(|| panic!("{context} is not an integer: {value}"))
}

/// The eval directories of a cell, sorted by name.
fn eval_dirs(cell_dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(cell_dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", cell_dir.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|e| panic!("directory entry: {e}"))
                .path()
        })
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("eval-"))
        })
        .collect();
    dirs.sort();
    dirs
}

/// Whether `name` is `eval-<digits>-grading.json`; a macOS resource-fork file `._eval-K-grading.json` is not.
fn is_grading_file(name: &str) -> bool {
    name.strip_prefix("eval-")
        .and_then(|rest| rest.strip_suffix("-grading.json"))
        .is_some_and(|k| !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit()))
}

/// The grading files of one run, sorted by path.
fn grading_files(cell_dir: &Path, run_n: u32) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in eval_dirs(cell_dir) {
        let run_dir = dir.join("with_skill").join(format!("run-{run_n}"));
        let Ok(entries) = fs::read_dir(&run_dir) else {
            continue;
        };
        for entry in entries {
            let path = entry
                .unwrap_or_else(|e| panic!("directory entry: {e}"))
                .path();
            let is_grading = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(is_grading_file);
            if is_grading {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// The value of a grading entry: the number token decides `count` or `number`.
fn value_of(entry: &Value, context: &str) -> Option<String> {
    match entry.get("value") {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => {
            if n.is_i64() || n.is_u64() {
                Some(format!("count:{n}"))
            } else {
                Some(format!("number:{n}"))
            }
        }
        Some(Value::String(s)) => Some(format!("text:{s}")),
        Some(Value::Bool(_) | Value::Array(_) | Value::Object(_)) => {
            panic!("value of unsupported type at {context}")
        }
    }
}

/// The legacy code of a `not_measured` row: the `reason=` of its replay marker, `zero_mappings`
/// when the mapping declares no entries, else `unrecorded`.
fn not_measured_code(evidence: &str) -> String {
    const MARKER: &str = "TIER_D_REPLAY";
    const ZERO: &str = "declares zero 'mappings' entries";
    if let Some(at) = evidence.find(MARKER) {
        // INDEX: slicing at a `find` offset of an ASCII needle stays on a char boundary
        let line = evidence[at..].lines().next().unwrap_or_default();
        if let Some(r) = line.find("reason=") {
            // INDEX: `r + len("reason=")` is a char boundary after an ASCII needle
            let code: String = line[r + "reason=".len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !code.is_empty() {
                return code;
            }
        }
    }
    if evidence.contains(ZERO) {
        // BORROW: explicit `.to_owned()` of a `&str` for the owned field
        return "zero_mappings".to_owned();
    }
    // BORROW: explicit `.to_owned()` of a `&str` for the owned field
    "unrecorded".to_owned()
}

fn row_of(cell: &str, run_n: u32, entry: &Value) -> ExpectedRow {
    // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
    let id = str_of(&entry["id"], "id");
    let context = format!("{cell} run-{run_n} {id}");
    // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getters panic with the location
    let status_text = str_of(&entry["status"], &format!("status at {context}"));
    let status = parse_status(&status_text, &context);
    let score = int_of(&entry["score"], &format!("score at {context}"));
    let max = int_of(&entry["max_score"], &format!("max_score at {context}"));
    let fraction = format!("{score}/{max}");
    let (weight, cause) = match status {
        LegacyStatus::Pass => {
            if max == 0 {
                assert_eq!(score, 0, "a zero-weight pass scores 0 at {context}");
                // BORROW: explicit `.to_owned()` of a `&str` for the owned field
                (Some(fraction), Some("legacy: zero_weight".to_owned()))
            } else {
                assert_eq!(score, max, "a pass has num = den at {context}");
                ((!(score == 1 && max == 1)).then_some(fraction), None)
            }
        }
        LegacyStatus::Fail => {
            assert!(max >= 1 && score < max, "a fail has num < den at {context}");
            ((!(score == 0 && max == 1)).then_some(fraction), None)
        }
        LegacyStatus::Blocked => {
            assert!(
                max >= 1 && score == 0,
                "a blocked row has num = 0 at {context}"
            );
            // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
            let path = str_of(&entry["blocked_by"], &format!("blocked_by at {context}"));
            (
                (!(score == 0 && max == 1)).then_some(fraction),
                Some(format!("artifact: {path}")),
            )
        }
        LegacyStatus::NotMeasured => {
            // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
            let evidence = str_of(&entry["evidence"], &format!("evidence at {context}"));
            (
                None,
                Some(format!("legacy: {}", not_measured_code(&evidence))),
            )
        }
        // BORROW: explicit `.to_owned()` of a `&str` for the owned field
        LegacyStatus::NotApplicable => (None, Some("reason: not_applicable_to_arm".to_owned())),
    };
    ExpectedRow {
        // BORROW: explicit `.to_owned()` of a `&str` for the owned field
        cell: cell.to_owned(),
        run_n,
        expectation_id: id,
        // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
        role: str_of(&entry["role"], &format!("role at {context}")),
        status: status_text,
        value: value_of(entry, &context),
        weight,
        cause,
    }
}

/// Every row of `expectations.jsonl`, keyed by `(cell, run, id)`, with the grading file name.
fn legacy_table(corpus: &Path) -> BTreeMap<(String, u32, String), (String, Value)> {
    let path = corpus.join("expectations.jsonl");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut table = BTreeMap::new();
    for line in text.lines().filter(|l| !l.is_empty()) {
        let row: Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("bad expectations.jsonl line: {e}"));
        // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
        let key = (
            str_of(&row["cell"], "cell"),
            parse_run_n(&str_of(&row["run"], "run")),
            str_of(&row["id"], "id"),
        );
        // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
        assert!(
            table
                .insert(key.clone(), (str_of(&row["eval"], "eval"), row))
                .is_none(),
            "duplicate {key:?}"
        );
    }
    table
}

/// Asserts that `entry` and the `expectations.jsonl` row agree on every field they share.
fn cross_check(
    table: &mut BTreeMap<(String, u32, String), (String, Value)>,
    cell: &str,
    run_n: u32,
    file: &Path,
    entry: &Value,
) {
    // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
    let id = str_of(&entry["id"], "id");
    // BORROW: explicit `.to_owned()` of a `&str` for the owned field
    let key = (cell.to_owned(), run_n, id.clone());
    let (eval, row) = table
        .remove(&key)
        .unwrap_or_else(|| panic!("{key:?} is not in expectations.jsonl"));
    let name = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    assert_eq!(eval, name, "grading file of {key:?}");
    // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
    for field in ["role", "status", "score", "max_score"] {
        assert_eq!(row[field], entry[field], "{field} of {key:?}");
    }
    let from_entry = entry.get("value").cloned().unwrap_or(Value::Null);
    // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
    assert_eq!(row["value"], from_entry, "value of {key:?}");
}

/// The expected outcome rows of every valid run, sorted by `(cell, run_n, expectation_id)`.
///
/// # Panics
///
/// Panics, naming the location, on an unknown legacy status or integrity verdict, on a row that
/// breaks the weight rules, and when the grading files and `expectations.jsonl` disagree.
#[must_use]
pub fn expected_rows(corpus: &Path) -> Vec<ExpectedRow> {
    let mut table = legacy_table(corpus);
    let mut rows = Vec::new();
    for run in runs_of(corpus) {
        let integrity = integrity_of(&run);
        let cell_dir = corpus.join(&run.cell);
        for file in grading_files(&cell_dir, run.run_n) {
            let doc = read_json(&file);
            // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
            let entries = doc["expectations"]
                .as_array()
                .unwrap_or_else(|| panic!("{} has no expectations[]", file.display()));
            for entry in entries {
                cross_check(&mut table, &run.cell, run.run_n, &file, entry);
                if integrity == ExpectedIntegrity::Valid {
                    rows.push(row_of(&run.cell, run.run_n, entry));
                }
            }
        }
    }
    assert!(
        table.is_empty(),
        "{} rows of expectations.jsonl have no grading entry, first {:?}",
        table.len(),
        table.keys().next()
    );
    rows.sort();
    rows
}

fn integrity_of(run: &RunId) -> ExpectedIntegrity {
    // BORROW: `.as_str()` views the owned verdict as a `&str` to match on
    match run.verdict.as_str() {
        "clean" | "incomplete" => ExpectedIntegrity::Valid,
        // BORROW: explicit `.to_owned()` of a `&str` for the owned field
        "contaminated" => ExpectedIntegrity::Invalid("input_leak".to_owned()),
        other => panic!(
            "unknown legacy verdict {other:?} at {} run-{}",
            run.cell, run.run_n
        ),
    }
}

/// The `(path, value)` leaves of a JSON object, with dotted paths; arrays are leaves.
fn leaves(value: &Value, prefix: &str, out: &mut Vec<(String, Value)>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                leaves(inner, &path, out);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            // BORROW: explicit `.to_owned()` of a `&str` for the owned field
            out.push((prefix.to_owned(), value.clone()));
        }
    }
}

fn harness_version(stream: &Path) -> String {
    let bytes =
        fs::read(stream).unwrap_or_else(|e| panic!("cannot read {}: {e}", stream.display()));
    let mut text = String::new();
    // BORROW: the decoder reads the slice of `bytes` in place
    MultiGzDecoder::new(bytes.as_slice())
        .read_to_string(&mut text)
        .unwrap_or_else(|e| panic!("cannot gunzip {}: {e}", stream.display()));
    let first = text.lines().next().unwrap_or_default();
    let init: Value = serde_json::from_str(first)
        .unwrap_or_else(|e| panic!("first line of {} is not JSON: {e}", stream.display()));
    // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
    assert_eq!(
        init["subtype"],
        "init",
        "first line of {} is the init line",
        stream.display()
    );
    // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
    str_of(&init["claude_code_version"], "claude_code_version")
}

fn measured_of(corpus: &Path, run: &RunId) -> Vec<(String, String)> {
    let cell_dir = corpus.join(&run.cell);
    let experiment = read_json(&cell_dir.join("experiment.json"));
    let mut cell_leaves = Vec::new();
    // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
    leaves(&experiment["provenance"], "provenance", &mut cell_leaves);
    let null_paths: BTreeSet<String> = cell_leaves
        .into_iter()
        .filter(|(_, v)| v.is_null())
        .map(|(p, _)| p)
        .collect();
    let mut seen: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for dir in eval_dirs(&cell_dir) {
        let meta = dir
            .join("with_skill")
            .join(format!("run-{}", run.run_n))
            .join("eval_metadata.json");
        if !meta.is_file() {
            continue;
        }
        let doc = read_json(&meta);
        let mut run_leaves = Vec::new();
        // INDEX: `Value` indexing by key reads a missing key as `Null`, so the typed getter panics with the location
        leaves(&doc["provenance"], "provenance", &mut run_leaves);
        for (path, value) in run_leaves {
            if null_paths.contains(&path) && !value.is_null() {
                seen.entry(path).or_default().insert(value.to_string());
            }
        }
    }
    seen.into_iter()
        .map(|(path, values)| {
            assert_eq!(
                values.len(),
                1,
                "eval_metadata copies of {path} disagree in {} run-{}",
                run.cell,
                run.run_n
            );
            let value = values.into_iter().next().unwrap_or_default();
            (path, value)
        })
        .collect()
}

/// The expected run facts of every run listed in `PROVENANCE.json`, sorted by `(cell, run_n)`.
///
/// # Panics
///
/// Panics on an unknown legacy verdict, on a harness version that differs within a cell, and on
/// `eval_metadata.json` copies that disagree within a run.
#[must_use]
pub fn expected_runs(corpus: &Path) -> Vec<ExpectedRun> {
    let mut by_cell: BTreeMap<String, String> = BTreeMap::new();
    let mut out = Vec::new();
    for run in runs_of(corpus) {
        let stream = corpus
            .join(&run.cell)
            .join("_run_cache/with_skill")
            .join(format!("run-{}", run.run_n))
            .join("agent_stream.jsonl.gz");
        let version = harness_version(&stream);
        let first = by_cell
            .entry(run.cell.clone())
            .or_insert_with(|| version.clone());
        assert_eq!(
            *first, version,
            "harness version differs within cell {}",
            run.cell
        );
        out.push(ExpectedRun {
            integrity: integrity_of(&run),
            harness_version: version,
            measured: measured_of(corpus, &run),
            cell: run.cell,
            run_n: run.run_n,
        });
    }
    out
}
