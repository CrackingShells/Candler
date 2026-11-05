//! Guards for the import of the 240-run golden corpus. Both tests are `#[ignore]`: they read the
//! corpus named by `CANDLER_GOLDEN_CORPUS` (and panic when it is unset), and the second one writes
//! the import report to the file named by `CANDLER_GOLDEN_REPORT` when that is set. The store is
//! read back through its own readers and compared with the independent oracle. A run's cell is a
//! digest in the store and a directory name in the oracle, so the two sides are matched by what
//! each cell holds, never by parsing a name.
#![cfg(test)]

#[path = "support/legacy_import.rs"]
mod legacy_import;
#[path = "support/legacy_rules.rs"]
mod legacy_rules;
#[path = "oracle/mod.rs"]
mod oracle;
mod support;

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use candler_core::store::Store;
use legacy_import::{field, import_with_reader, project, text_of};
use oracle::{ExpectedIntegrity, ExpectedRow, expected_rows, expected_runs};
use serde_json::Value;

/// The evaluator every golden record and request names, written out from the corpus's
/// `PROVENANCE.json`.
const EVALUATOR: &str = "htn-creator-eval@0.4.2";
/// The pinned case: a valid run the legacy grading called `incomplete`, with 43 records.
const PINNED_CELL: &str =
    "20260806-035940_v0.2.4_claude-sonnet-5_model-claude-sonnet-5_skill_version-htn-creator-0.2.4";

/// The corpus the environment names.
///
/// # Panics
///
/// Panics when `CANDLER_GOLDEN_CORPUS` is unset.
fn corpus() -> PathBuf {
    PathBuf::from(
        env::var("CANDLER_GOLDEN_CORPUS")
            .expect("CANDLER_GOLDEN_CORPUS names the golden corpus folder"),
    )
}

/// The plain row without its cell, so rows of the store and of the oracle compare by content.
fn bare(row: &ExpectedRow) -> ExpectedRow {
    ExpectedRow {
        cell: String::new(),
        // BORROW: `.clone()` copies the row to blank its cell
        ..row.clone()
    }
}

/// Matches each cell of the oracle with the one store cell that holds the same items, and
/// panics when a cell has no twin or two cells share one.
fn match_cells<T: Eq + Ord>(
    stored: &BTreeMap<String, Vec<T>>,
    expected: &BTreeMap<String, Vec<T>>,
) -> BTreeMap<String, String> {
    let mut free: Vec<&String> = stored.keys().collect();
    let mut matched = BTreeMap::new();
    for (cell, items) in expected {
        let at = free
            .iter()
            .position(|digest| stored.get(*digest) == Some(items))
            .unwrap_or_else(|| panic!("no store cell holds what the oracle's `{cell}` holds"));
        // BORROW: `.clone()` copies the digest and the cell name into the pairing
        matched.insert(cell.clone(), free.remove(at).clone());
    }
    assert!(free.is_empty(), "store cells nothing in the oracle matches");
    matched
}

/// The stored runs, grouped by the digest of their cell: `(replicate, done, manifest JSON)`.
fn stored_cells(store: &Store) -> BTreeMap<String, Vec<(u32, bool, Value)>> {
    let mut cells: BTreeMap<String, Vec<(u32, bool, Value)>> = BTreeMap::new();
    for entry in store.runs().expect("the runs list") {
        let manifest = serde_json::to_value(store.read_manifest(&entry.key).expect("a manifest"))
            .expect("a manifest serializes");
        cells
            .entry(entry.key.cell().to_string())
            .or_default()
            .push((entry.key.replicate(), entry.done, manifest));
    }
    for runs in cells.values_mut() {
        runs.sort_by_key(|run| run.0);
    }
    cells
}

/// Guards: D6, I10 and I12 — importing the golden corpus stores records that all validate against
/// `outcome-record.schema.json`, equal the oracle's 10,320 rows over 240 runs, count 8,291 pass,
/// 802 blocked, 742 `not_measured` and 485 fail, and name `htn-creator-eval@0.4.2` in every outcome
/// file and every `import.requests[]` entry, never `0.3.4`. Oracle: `oracle::expected_rows`, the
/// schema file, and the status counts of `PROVENANCE.json` (`status_counts_by_expectation`)
/// summed by hand.
#[test]
#[ignore = "reads the golden corpus: set CANDLER_GOLDEN_CORPUS"]
fn the_golden_corpus_imports_to_the_oracle_rows() {
    let corpus = corpus();
    let mut expected = expected_rows(&corpus);
    let runs = expected_runs(&corpus);
    assert_eq!(runs.len(), 240, "oracle runs");
    assert_eq!(expected.len(), 10_320, "oracle rows");

    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(store_dir.path()).expect("the store initialises");
    let report = import_with_reader(&corpus, &store).expect("the golden corpus imports");
    assert_eq!(report.runs, 240);
    assert_eq!(report.outcomes, 10_320);

    let schema_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("candler-schemas")
        .join("schema")
        .join("outcome-record.schema.json");
    let schema: Value =
        serde_json::from_str(&fs::read_to_string(schema_path).expect("the schema file reads"))
            .expect("the schema is JSON");
    let validator = jsonschema::validator_for(&schema).expect("the schema compiles");

    let mut by_cell: BTreeMap<String, Vec<ExpectedRow>> = BTreeMap::new();
    let mut statuses: BTreeMap<String, usize> = BTreeMap::new();
    for entry in store.runs().expect("the runs list") {
        let files = store
            .outcome_files(&entry.key)
            .expect("the outcome files list");
        for file in &files {
            let evaluator = serde_json::to_value(&file.evaluator).expect("an evaluator");
            // BORROW: explicit .as_str() of the JSON string
            assert_eq!(evaluator.as_str(), Some(EVALUATOR), "evaluator of {file:?}");
            for record in store.read_outcomes(file).expect("the outcome file reads") {
                let value = serde_json::to_value(&record).expect("a record serializes");
                assert!(
                    validator.is_valid(&value),
                    "a record of {} does not validate: {value}",
                    entry.key
                );
                *statuses.entry(text_of(&value, "status")).or_default() += 1;
                by_cell
                    .entry(entry.key.cell().to_string())
                    .or_default()
                    .push(bare(&project("", entry.key.replicate(), &value)));
            }
        }
        let manifest = serde_json::to_value(store.read_manifest(&entry.key).expect("a manifest"))
            .expect("a manifest serializes");
        let requests = field(field(&manifest, "import"), "requests")
            .as_array()
            .expect("requests is a list");
        assert!(!requests.is_empty(), "{} names no request", entry.key);
        for request in requests {
            assert_eq!(text_of(request, "evaluator"), EVALUATOR);
        }
    }
    let literal = |status: &str| statuses.get(status).copied().unwrap_or(0);
    assert_eq!(literal("pass"), 8_291);
    assert_eq!(literal("blocked"), 802);
    assert_eq!(literal("not_measured"), 742);
    assert_eq!(literal("fail"), 485);
    assert_eq!(statuses.values().sum::<usize>(), 10_320);

    expected.sort();
    let mut expected_by_cell: BTreeMap<String, Vec<ExpectedRow>> = BTreeMap::new();
    for row in &expected {
        expected_by_cell
            // BORROW: `.clone()` copies the cell name as the key
            // BORROW: .clone() copies the cell name as the key
            .entry(row.cell.clone())
            .or_default()
            .push(bare(row));
    }
    for rows in by_cell.values_mut().chain(expected_by_cell.values_mut()) {
        rows.sort();
    }
    assert_eq!(by_cell.len(), 12);
    assert_eq!(expected_by_cell.len(), 12);
    let matched = match_cells(&by_cell, &expected_by_cell);
    assert_eq!(matched.len(), 12);
}

/// The oracle's run facts in the plain form the stored manifests are projected to: the run
/// number, `valid` or `invalid:<reason>`, the harness version, the measured values as JSON and the
/// run's expectation ids with their statuses, which tell apart cells that share every other fact.
type Facts = (u32, String, String, Vec<(String, String)>, String);

/// The plain facts of a stored manifest.
fn facts_of(replicate: u32, manifest: &Value, graded: String) -> Facts {
    let integrity = field(manifest, "integrity");
    // BORROW: explicit .as_str() of the owned kind
    let state = match text_of(integrity, "kind").as_str() {
        // BORROW: `.to_owned()` makes the owned label
        "valid" => "valid".to_owned(),
        _ => format!("invalid:{}", text_of(integrity, "reason")),
    };
    let version = text_of(field(field(manifest, "comparable"), "harness"), "version");
    let mut measured: Vec<(String, String)> = field(manifest, "measured")
        .as_array()
        .expect("measured is a list")
        .iter()
        .map(|item| (text_of(item, "path"), field(item, "value").to_string()))
        .collect();
    measured.sort();
    (replicate, state, version, measured, graded)
}

/// Guards: D6, I10 and I12 — importing the golden corpus stores 240 done runs over 12 distinct
/// cell digests whose integrity, harness version and measured values equal the oracle's, and the
/// pinned run-8 of the v0.2.4 sonnet-5 cell is valid, was `incomplete` for the legacy grading and
/// holds 43 outcome records; no trace failed, all 240 runs have a trace and every manifest keeps
/// the 0.3.4 `provenance.eval_infra.*` values; the import report is written to `CANDLER_GOLDEN_REPORT` when that
/// is set. Oracle: `oracle::expected_runs`, and the 43 expectations per run counted in the raw
/// grading files.
#[test]
#[ignore = "reads the golden corpus: set CANDLER_GOLDEN_CORPUS"]
fn the_golden_corpus_imports_to_the_oracle_runs() {
    let corpus = corpus();
    let runs = expected_runs(&corpus);
    assert_eq!(runs.len(), 240, "oracle runs");

    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::init(store_dir.path()).expect("the store initialises");
    let report = import_with_reader(&corpus, &store).expect("the golden corpus imports");
    if let Ok(target) = env::var("CANDLER_GOLDEN_REPORT") {
        fs::write(target, report.to_json().expect("the report serializes"))
            .expect("the report is written");
    }

    let cells = stored_cells(&store);
    assert_eq!(cells.len(), 12, "distinct cell digests");
    assert_eq!(cells.values().map(Vec::len).sum::<usize>(), 240);
    assert!(
        cells.values().flatten().all(|run| run.1),
        "every run is done"
    );

    // V6: no trace failed, every run has a non-empty trace, and every manifest keeps the 0.3.4
    // evaluator provenance beside the 0.4.2 evaluator that graded it.
    assert!(
        report.trace_failures.is_empty(),
        "trace failures: {:?}",
        report.trace_failures
    );
    let infra: Vec<(&str, Value)> = vec![
        ("provenance.eval_infra.commit", Value::from("0e84a75")),
        ("provenance.eval_infra.commits_ahead", Value::from(4)),
        ("provenance.eval_infra.dirty", Value::from(true)),
        (
            "provenance.eval_infra.tag",
            Value::from("htn-creator-eval@0.3.4"),
        ),
    ];
    let mut traced = 0_usize;
    for entry in store.runs().expect("the runs list") {
        let trace = store.layout().trace(&entry.key);
        assert!(
            trace.metadata().is_ok_and(|meta| meta.len() > 0),
            "no trace for {}",
            trace.display()
        );
        store.read_trace(&entry.key).expect("the trace reads");
        traced += 1;
        let manifest = serde_json::to_value(store.read_manifest(&entry.key).expect("a manifest"))
            .expect("a manifest serializes");
        let values = field(field(&manifest, "comparable"), "values")
            .as_array()
            .expect("values is a list");
        for (path, want) in &infra {
            let found = values
                .iter()
                .find(|item| text_of(item, "path") == *path)
                .unwrap_or_else(|| panic!("{} lacks {path}", trace.display()));
            assert_eq!(field(found, "value"), want, "{path} of {}", trace.display());
        }
    }
    assert_eq!(traced, 240, "runs with a trace");

    let mut graded: BTreeMap<(String, u32), String> = BTreeMap::new();
    for entry in store.runs().expect("the runs list") {
        let mut ids: Vec<String> = Vec::new();
        for file in store.outcome_files(&entry.key).expect("the files list") {
            for record in store.read_outcomes(&file).expect("the file reads") {
                let value = serde_json::to_value(&record).expect("a record serializes");
                ids.push(format!(
                    "{}={}",
                    text_of(&value, "expectation_id"),
                    text_of(&value, "status")
                ));
            }
        }
        ids.sort();
        graded.insert(
            (entry.key.cell().to_string(), entry.key.replicate()),
            ids.join(","),
        );
    }
    let mut by_run: BTreeMap<(String, u32), Vec<String>> = BTreeMap::new();
    for row in expected_rows(&corpus) {
        by_run
            // BORROW: .clone() copies the cell name into the key
            .entry((row.cell.clone(), row.run_n))
            .or_default()
            .push(format!("{}={}", row.expectation_id, row.status));
    }
    let mut stored: BTreeMap<String, Vec<Facts>> = BTreeMap::new();
    for (digest, list) in &cells {
        let mut facts: Vec<Facts> = list
            .iter()
            .map(|(replicate, _, manifest)| {
                facts_of(
                    *replicate,
                    manifest,
                    // BORROW: .clone() copies the digest into the lookup key and the ids into the facts
                    graded
                        .get(&(digest.clone(), *replicate))
                        .expect("graded for every run")
                        .clone(),
                )
            })
            .collect();
        facts.sort();
        // BORROW: `.clone()` copies the digest as the key
        // BORROW: .clone() copies the digest as the key
        stored.insert(digest.clone(), facts);
    }
    let mut wanted: BTreeMap<String, Vec<Facts>> = BTreeMap::new();
    for run in &runs {
        let state = match &run.integrity {
            // BORROW: `.to_owned()` makes the label
            ExpectedIntegrity::Valid => "valid".to_owned(),
            ExpectedIntegrity::Invalid(reason) => format!("invalid:{reason}"),
        };
        let measured: Vec<(String, String)> = run
            .measured
            .iter()
            .map(|(path, json)| {
                let value: Value = serde_json::from_str(json).expect("the oracle's JSON parses");
                // BORROW: `.clone()` copies the path
                (path.clone(), value.to_string())
            })
            .collect();
        wanted
            // BORROW: `.clone()` copies the cell name as the key
            .entry(run.cell.clone())
            .or_default()
            .push((
                run.run_n,
                state,
                // BORROW: `.clone()` copies the version
                run.harness_version.clone(),
                measured,
                {
                    let mut ids = by_run
                        // BORROW: `.clone()` copies the cell name
                        .remove(&(run.cell.clone(), run.run_n))
                        .unwrap_or_default();
                    ids.sort();
                    ids.join(",")
                },
            ));
    }
    for facts in wanted.values_mut() {
        facts.sort();
    }
    assert_eq!(wanted.len(), 12);
    let matched = match_cells(&stored, &wanted);

    // The pinned case.
    let digest = matched
        .get(PINNED_CELL)
        .unwrap_or_else(|| panic!("the oracle has no cell `{PINNED_CELL}`"));
    let (_, _, manifest) = cells
        .get(digest)
        .and_then(|list| list.iter().find(|run| run.0 == 8))
        .expect("the pinned cell has a run-8");
    assert_eq!(text_of(field(manifest, "integrity"), "kind"), "valid");
    assert_eq!(
        text_of(
            field(field(manifest, "import"), "legacy_integrity"),
            "verdict"
        ),
        "incomplete"
    );
    let key = store
        .runs()
        .expect("the runs list")
        .into_iter()
        .find(|entry| entry.key.replicate() == 8 && entry.key.cell().to_string() == *digest)
        .expect("the pinned run is stored")
        .key;
    let records: usize = store
        .outcome_files(&key)
        .expect("the outcome files list")
        .iter()
        .map(|file| store.read_outcomes(file).expect("the file reads").len())
        .sum();
    assert_eq!(records, 43);
}
