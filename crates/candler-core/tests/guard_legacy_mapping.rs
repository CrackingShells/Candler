#![cfg(test)]
//! Guards for the mapping of a legacy results folder to run manifests and outcome records, over
//! the vendored mini-corpus. The expected rows and run facts come from the independent oracle; the
//! other expectations are literals written by hand after reading the raw files with `jq` and
//! `python3 -I`.

#[path = "support/legacy_rules.rs"]
mod legacy_rules;
#[path = "oracle/mod.rs"]
mod oracle;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use candler_core::import::ImportError;
use candler_core::import::grading::map_grading;
use candler_core::import::manifest::{MappedRun, map_runs};
use candler_core::import::read::{LegacyFolder, SourceFile};
use flate2::Compression;
use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use oracle::{ExpectedIntegrity, ExpectedRow, expected_rows, expected_runs};
use serde_json::Value;
use sha2::Digest as _;

const V3: &str = "20260806-035940_v0.2.3_claude-haiku-4-5_model-claude-haiku-4-5_skill_version-htn-creator-v0.2.3";
const V6: &str = "20260806-035940_v0.2.6_claude-haiku-4-5_model-claude-haiku-4-5_skill_version-htn-creator-0.2.6";
const SYNTHETIC: &str = "synthetic_cell";

fn mini() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini-corpus")
}

/// The value of `key`, or a panic that names the key and the document.
fn field<'v>(value: &'v Value, key: &str) -> &'v Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("no `{key}` in {value}"))
}

/// The string at `key`, owned.
fn text_of(value: &Value, key: &str) -> String {
    // BORROW: `.to_owned()` of a `&str` for the owned result
    field(value, key)
        // BORROW: explicit .as_str() instead of Deref coercion
        .as_str()
        .unwrap_or_else(|| panic!("`{key}` is not a string in {value}"))
        // BORROW: .to_owned() makes the owned copy
        .to_owned()
}

/// The folder, its mapped runs and the serialized records of each mapped run, in mapped order.
struct Imported {
    folder: LegacyFolder,
    mapped: Vec<MappedRun>,
    records: Vec<Vec<Value>>,
}

impl Imported {
    fn read(root: &Path) -> Self {
        let rules = legacy_rules::rules();
        let folder = LegacyFolder::read(root, &rules).expect("the folder reads");
        let mapped = map_runs(&folder, &rules).expect("the runs map");
        let records = mapped
            .iter()
            .map(|run| {
                map_grading(&folder, run, &rules)
                    .expect("the grading maps")
                    .iter()
                    .map(|record| serde_json::to_value(record).expect("a record serializes"))
                    .collect()
            })
            .collect();
        Self {
            folder,
            mapped,
            records,
        }
    }

    /// The cell directory name and the run number of the mapped run at `index`.
    fn name(&self, index: usize) -> (String, u32) {
        let mapped = self.mapped.get(index).expect("a mapped run");
        let (cell, run) = self
            .folder
            .run(mapped.run)
            .expect("the run of a mapped run");
        // BORROW: `.clone()` copies the cell name
        (cell.name.clone(), run.number)
    }

    /// The index of the mapped run of `cell` and `number`.
    fn index(&self, cell: &str, number: u32) -> usize {
        (0..self.mapped.len())
            // BORROW: `.to_owned()` makes the owned cell name to compare
            .find(|&index| self.name(index) == (cell.to_owned(), number))
            .unwrap_or_else(|| panic!("no mapped run {cell} run-{number}"))
    }

    /// The serialized manifest of the run of `cell` and `number`.
    fn manifest(&self, cell: &str, number: u32) -> Value {
        let mapped = self
            .mapped
            .get(self.index(cell, number))
            .expect("a mapped run");
        serde_json::to_value(&mapped.manifest).expect("a manifest serializes")
    }

    /// The serialized records of the run of `cell` and `number`.
    fn records_of(&self, cell: &str, number: u32) -> &[Value] {
        self.records
            .get(self.index(cell, number))
            .expect("the records of a mapped run")
    }
}

/// A record as the oracle's plain strings.
fn project(cell: &str, run_n: u32, record: &Value) -> ExpectedRow {
    let value = record
        .get("value")
        // BORROW: explicit `.as_str()` of the owned kind to match on it
        .map(|value| match text_of(value, "kind").as_str() {
            "text" => format!("text:{}", text_of(value, "text")),
            "count" => format!("count:{}", field(value, "count")),
            "number" => format!("number:{}", field(value, "number")),
            other => panic!("unknown value kind {other}"),
        });
    let weight = record
        .get("weight")
        .map(|weight| format!("{}/{}", field(weight, "num"), field(weight, "den")));
    let cause = record.get("cause").map(|cause| {
        if let Some(code) = cause.get("legacy") {
            // BORROW: explicit `.as_str()` of the JSON string
            format!("legacy: {}", code.as_str().expect("a legacy code"))
        } else if let Some(path) = cause.get("artifact") {
            // BORROW: explicit `.as_str()` of the JSON string
            format!("artifact: {}", path.as_str().expect("an artifact path"))
        } else {
            format!("reason: {}", text_of(cause, "reason"))
        }
    });
    ExpectedRow {
        // BORROW: `.to_owned()` makes the owned cell name
        cell: cell.to_owned(),
        run_n,
        expectation_id: text_of(record, "expectation_id"),
        role: text_of(record, "role"),
        status: text_of(record, "status"),
        value,
        weight,
        cause,
    }
}

/// Guards: D6 — the mapped records, projected to cell, run number, expectation id, role, status,
/// value, weight and cause, equal the oracle's rows as a multiset, the records of one expectation
/// id of the real cells share one suite entry digest, and the synthetic cell's reworded A1 has
/// another. Oracle: `oracle::expected_rows`, which reads the
/// raw grading files and cross-checks `expectations.jsonl`.
#[test]
fn the_mapped_records_are_the_oracle_rows() {
    let imported = Imported::read(&mini());
    let mut mapped_rows: Vec<ExpectedRow> = Vec::new();
    let mut digests: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (index, records) in imported.records.iter().enumerate() {
        let (cell, run_n) = imported.name(index);
        for record in records {
            mapped_rows.push(project(&cell, run_n, record));
            if cell != SYNTHETIC {
                digests
                    .entry(text_of(record, "expectation_id"))
                    .or_default()
                    .insert(text_of(record, "suite_entry_digest"));
            }
        }
    }
    let mut expected = expected_rows(&mini());
    mapped_rows.sort();
    expected.sort();
    assert_eq!(mapped_rows.len(), expected.len(), "number of records");
    assert_eq!(mapped_rows, expected);
    assert!(!digests.is_empty());
    for (id, set) in &digests {
        assert_eq!(set.len(), 1, "`{id}` has more than one suite entry digest");
    }
    // The synthetic cell words A1 differently, so its entry is another entry.
    let synthetic = imported
        .records_of(SYNTHETIC, 2)
        .iter()
        .find(|record| text_of(record, "expectation_id") == "A1")
        .map(|record| text_of(record, "suite_entry_digest"))
        .expect("a synthetic A1");
    assert!(
        digests
            .get("A1")
            .is_some_and(|set| !set.contains(&synthetic))
    );
}

/// The `(path, value)` pairs of the measured values of a serialized manifest.
fn measured(manifest: &Value) -> Vec<(String, Value)> {
    let list = field(manifest, "measured")
        .as_array()
        .expect("measured is a list");
    let mut pairs: Vec<(String, Value)> = list
        .iter()
        // BORROW: `.clone()` copies the measured value
        .map(|item| (text_of(item, "path"), field(item, "value").clone()))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
}

/// Guards: D6 — a run's integrity, harness version and measured values equal the oracle's, the
/// manifests list the factors that differ between the cells, and a run's start and end are the
/// timestamps worked out by hand. Oracle: `oracle::expected_runs` for integrity, harness and
/// measured values; the factor list read with `jq` over the three `experiment.json` files (the
/// leaves that differ are the model, the run count and three skill version spellings); start and
/// end from `run_complete.json` and the `elapsed_seconds` of `progress.jsonl`:
/// 2086.386 - 1882.076 = 204.310 s before `19:34:27.030762`, and 15 - 10 = 5 s before `00:00:11`.
#[test]
fn the_mapped_runs_are_the_oracle_runs() {
    let imported = Imported::read(&mini());
    let expected = expected_runs(&mini());
    assert_eq!(expected.len(), 6);
    assert_eq!(imported.mapped.len(), expected.len());
    for run in &expected {
        let manifest = imported.manifest(&run.cell, run.run_n);
        let integrity = field(&manifest, "integrity");
        match &run.integrity {
            ExpectedIntegrity::Valid => assert_eq!(text_of(integrity, "kind"), "valid"),
            ExpectedIntegrity::Invalid(reason) => {
                assert_eq!(text_of(integrity, "kind"), "invalid");
                assert_eq!(&text_of(integrity, "reason"), reason);
            }
        }
        let harness = field(field(&manifest, "comparable"), "harness");
        assert_eq!(text_of(harness, "version"), run.harness_version);
        let wanted: Vec<(String, Value)> = run
            .measured
            .iter()
            .map(|(path, json)| {
                (
                    // BORROW: `.clone()` copies the path
                    path.clone(),
                    serde_json::from_str(json).expect("the oracle's JSON text parses"),
                )
            })
            .collect();
        assert_eq!(
            measured(&manifest),
            wanted,
            "{} run-{}",
            run.cell,
            run.run_n
        );
        let factors: Vec<String> = field(&manifest, "factors")
            .as_array()
            .expect("factors is a list")
            .iter()
            // BORROW: `.as_str()` and `.to_owned()` make the owned factor path
            .map(|factor| factor.as_str().expect("a factor path").to_owned())
            .collect();
        assert_eq!(
            factors,
            [
                "plan.model",
                "plan.runs",
                "plan.skill_version",
                "provenance.skill_version",
                "resolved_skill_version",
            ]
        );
    }
    let first = imported.manifest(V3, 6);
    assert_eq!(text_of(&first, "started_at"), "2026-08-05T19:31:02.720762Z");
    assert_eq!(
        text_of(&first, "ended_at"),
        "2026-08-05T19:34:27.030762+00:00"
    );
    let leaked = imported.manifest(SYNTHETIC, 1);
    assert_eq!(text_of(&leaked, "started_at"), "2026-08-06T00:00:06Z");
    assert_eq!(text_of(&leaked, "ended_at"), "2026-08-06T00:00:11+00:00");
}

/// The `[[vector]]` tables of a vector file of the oracle.
fn vectors(name: &str) -> Vec<toml::Table> {
    let path = mini()
        .join("..")
        .join("..")
        .join("oracle/vectors")
        .join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let table: toml::Table = text.parse().unwrap_or_else(|e| panic!("{name}: {e}"));
    table
        .get("vector")
        .and_then(toml::Value::as_array)
        .unwrap_or_else(|| panic!("{name} has no vectors"))
        .iter()
        // BORROW: `.clone()` copies the vector table
        .map(|item| item.as_table().expect("a vector table").clone())
        .collect()
}

fn vector_text<'t>(vector: &'t toml::Table, key: &str) -> &'t str {
    vector
        .get(key)
        // BORROW: Value::as_str borrows the JSON string without copying
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| panic!("vector has no `{key}`"))
}

/// Guards: D6 — the mapped comparable and cell digest equal the `cell_digest.toml` vectors of the
/// three mini-corpus cells, the request id of each real run equals `request_id.toml`, and the
/// suite entry digest of one expectation equals the digest of its hand-written suite entry.
/// Oracle: the vector files of the oracle, generated by `gen_vectors.py` with `rfc8785`; the suite
/// entry JSON below and its digest are written by hand with `python3 -I` (`json.dumps` with sorted
/// keys, compact separators, `sha256` of the UTF-8 bytes; the text is ASCII, so it is the RFC 8785
/// form).
#[test]
fn the_mapped_identity_is_the_oracle_vectors() {
    let imported = Imported::read(&mini());
    let mut cells = 0;
    for vector in vectors("cell_digest.toml") {
        let cell = vector_text(&vector, "cell");
        let Some(run) = imported
            .folder
            .runs()
            .find(|(c, _)| c.name == cell)
            .map(|(_, r)| r.number)
        else {
            continue;
        };
        cells += 1;
        let manifest = imported.manifest(cell, run);
        let wanted: Value = serde_json::from_str(vector_text(&vector, "comparable"))
            .expect("the vector's comparable parses");
        assert_eq!(field(&manifest, "comparable"), &wanted, "{cell}");
        assert_eq!(
            text_of(field(&manifest, "run"), "cell"),
            vector_text(&vector, "digest"),
            "{cell}"
        );
    }
    assert_eq!(cells, 3);
    let mut requests = 0;
    for vector in vectors("request_id.toml") {
        let cell = vector_text(&vector, "cell");
        if !imported.folder.cells.iter().any(|c| c.name == cell) {
            continue;
        }
        requests += 1;
        let number = u32::try_from(
            vector
                .get("run")
                .and_then(toml::Value::as_integer)
                .expect("the vector's run"),
        )
        .expect("a run number");
        let manifest = imported.manifest(cell, number);
        let listed = field(field(&manifest, "import"), "requests")
            .as_array()
            .expect("requests is a list");
        assert_eq!(listed.len(), 1);
        let request = listed.first().expect("one request");
        assert_eq!(
            text_of(request, "request"),
            vector_text(&vector, "request_id")
        );
        assert_eq!(text_of(request, "evaluator"), "htn-creator-eval@0.4.2");
    }
    assert_eq!(requests, 3);
    // The entry is {"depends_on":[],"kind":"legacy-check","params":{"scale":"binary","text":
    // "Running scripts/validate_domain.py on the generated domain.py exits with code 0 (no
    // structural errors or warnings)","tier":"structure"},"role":"gate"}.
    let a1 = imported
        .records_of(V3, 6)
        .iter()
        .find(|record| text_of(record, "expectation_id") == "A1")
        .expect("a record of A1");
    assert_eq!(
        text_of(a1, "suite_entry_digest"),
        "sha256:b0db76be61754c4fdffa4c605e1c2d9dd9f526c6f659550b16b920c374d44701"
    );
}

/// Guards: I10 — a contaminated run is `invalid` with the reason `input_leak` and has no outcome
/// record, and an `incomplete` run is `valid` with every row it has. Oracle: the run listing of
/// `PROVENANCE.json` read with `jq` (`synthetic_cell` run-1 is `contaminated`, `{V3}` run-17 is
/// `incomplete`) and the 43 entries per real run counted by hand in the raw grading files.
#[test]
fn contaminated_runs_have_no_records_and_incomplete_runs_have_all() {
    let imported = Imported::read(&mini());
    let leaked = imported.manifest(SYNTHETIC, 1);
    let integrity = field(&leaked, "integrity");
    assert_eq!(text_of(integrity, "kind"), "invalid");
    assert_eq!(text_of(integrity, "reason"), "input_leak");
    assert_eq!(
        text_of(
            field(field(&leaked, "import"), "legacy_integrity"),
            "verdict"
        ),
        "contaminated"
    );
    assert_eq!(imported.records_of(SYNTHETIC, 1).len(), 0);
    let incomplete = imported.manifest(V3, 17);
    assert_eq!(text_of(field(&incomplete, "integrity"), "kind"), "valid");
    assert_eq!(
        text_of(
            field(field(&incomplete, "import"), "legacy_integrity"),
            "verdict"
        ),
        "incomplete"
    );
    assert_eq!(imported.records_of(V3, 17).len(), 43);
    assert_eq!(imported.records_of(V3, 6).len(), 43);
    assert_eq!(imported.records_of(V6, 12).len(), 43);
}

/// Copies the directory at `from` to `to`.
fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create the copy");
    for entry in fs::read_dir(from).expect("read the directory") {
        let entry = entry.expect("a directory entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy a file");
        }
    }
}

/// Every regular file under `root`, sorted.
fn walk(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    // BORROW: `.to_owned()` makes the owned root to push on the stack
    let mut pending = vec![root.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).expect("read the directory") {
            let entry = entry.expect("a directory entry");
            if entry.file_type().expect("a file type").is_dir() {
                pending.push(entry.path());
            } else {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    files
}

/// The files named `name` of `run-<number>` under the cell directory `cell`.
fn run_files(root: &Path, cell: &str, number: u32, name: &str) -> Vec<PathBuf> {
    let tail = format!("/run-{number}/{name}");
    walk(&root.join(cell))
        .into_iter()
        .filter(|path| path.to_string_lossy().ends_with(&tail))
        .collect()
}

fn edit_json(path: &Path, edit: impl FnOnce(&mut Value)) {
    let text = fs::read_to_string(path).expect("read the file");
    let mut value: Value = serde_json::from_str(&text).expect("the file is JSON");
    edit(&mut value);
    fs::write(path, serde_json::to_string(&value).expect("serialize")).expect("write the file");
}

/// The entry of `PROVENANCE.json` for `V3` run-6.
fn edit_provenance_verdict(root: &Path, verdict: &str) {
    edit_json(&root.join("PROVENANCE.json"), |value| {
        let runs = value
            .get_mut("runs")
            .and_then(Value::as_array_mut)
            .expect("runs");
        let entry = runs
            .iter_mut()
            .find(|entry| {
                // BORROW: Value::as_str borrows the JSON string without copying
                entry.get("cell").and_then(Value::as_str) == Some(V3)
                    // BORROW: Value::as_str borrows the JSON string without copying
                    && entry.get("run").and_then(Value::as_str) == Some("run-6")
            })
            .expect("the entry of run-6");
        *entry
            .get_mut("original_integrity")
            .and_then(|block| block.get_mut("verdict"))
            .expect("the verdict") = Value::String(verdict.into());
    });
}

/// Sets the status of A1 of `V3` run-6 in `expectations.jsonl`.
fn edit_expectation_status(root: &Path, status: &str) {
    let path = root.join("expectations.jsonl");
    let text = fs::read_to_string(&path).expect("read expectations.jsonl");
    let mut out = String::new();
    let mut changed = 0;
    for line in text.lines() {
        let mut row: Value = serde_json::from_str(line).expect("a row");
        // BORROW: Value::as_str borrows the JSON string without copying
        if row.get("cell").and_then(Value::as_str) == Some(V3)
            // BORROW: Value::as_str borrows the JSON string without copying
            && row.get("run").and_then(Value::as_str) == Some("run-6")
            // BORROW: Value::as_str borrows the JSON string without copying
            && row.get("id").and_then(Value::as_str) == Some("A1")
        {
            *row.get_mut("status").expect("a status") = Value::String(status.into());
            changed += 1;
        }
        out.push_str(&serde_json::to_string(&row).expect("serialize"));
        out.push('\n');
    }
    assert_eq!(changed, 1);
    fs::write(&path, out).expect("write expectations.jsonl");
}

/// Sets the status of A1 in the grading file of `V3` run-6.
fn edit_grading_status(root: &Path, status: &str) {
    let files = run_files(root, V3, 6, "eval-1-grading.json");
    assert_eq!(files.len(), 1);
    edit_json(files.first().expect("a grading file"), |value| {
        let entry = value
            .get_mut("expectations")
            .and_then(Value::as_array_mut)
            .and_then(|list| {
                list.iter_mut()
                    // BORROW: Value::as_str borrows the JSON string without copying
                    .find(|entry| entry.get("id").and_then(Value::as_str) == Some("A1"))
            })
            .expect("A1");
        *entry.get_mut("status").expect("a status") = Value::String(status.into());
    });
}

/// Reads, maps and grades every run of the folder at `root`.
fn import_all(root: &Path) -> Result<usize, ImportError> {
    let rules = legacy_rules::rules();
    let folder = LegacyFolder::read(root, &rules)?;
    let mapped = map_runs(&folder, &rules)?;
    let mut records = 0;
    for run in &mapped {
        records += map_grading(&folder, run, &rules)?.len();
    }
    Ok(records)
}

/// One injected fault: what it is, how to make it, the variant of error that refuses it and the
/// texts the error names.
struct Fault {
    name: &'static str,
    inject: fn(&Path),
    refused: fn(&ImportError) -> bool,
    names: &'static [&'static str],
}

fn faults() -> Vec<Fault> {
    vec![
        Fault {
            name: "an unknown verdict",
            inject: |root| edit_provenance_verdict(root, "murky"),
            refused: |e| matches!(e, ImportError::Verdict { .. }),
            names: &["PROVENANCE.json", "run-6", "murky"],
        },
        Fault {
            name: "PROVENANCE and the contamination catalog disagree",
            inject: |root| edit_provenance_verdict(root, "incomplete"),
            refused: |e| matches!(e, ImportError::Disagree { .. }),
            names: &["PROVENANCE.json", "contamination_catalog.jsonl", "run-6"],
        },
        Fault {
            name: "eval_metadata provenance copies disagree within a run",
            inject: |root| {
                let files = run_files(root, V3, 6, "eval_metadata.json");
                assert!(files.len() > 1);
                edit_json(files.first().expect("a copy"), |value| {
                    *value
                        .pointer_mut("/provenance/planner/sandbox/version")
                        .expect("the sandbox version") = Value::String("9.9.9".into());
                });
            },
            refused: |e| matches!(e, ImportError::Disagree { .. }),
            names: &["eval_metadata.json", "run-6", "9.9.9"],
        },
        Fault {
            name: "two harness versions in a cell",
            inject: |root| {
                let files = run_files(root, V3, 17, "agent_stream.jsonl.gz");
                assert_eq!(files.len(), 1);
                let path = files.first().expect("a stream");
                let mut text = String::new();
                MultiGzDecoder::new(fs::File::open(path).expect("open"))
                    .read_to_string(&mut text)
                    .expect("gunzip");
                assert!(text.contains("2.1.220"));
                let text = text.replacen("2.1.220", "2.1.221", 1);
                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(text.as_bytes()).expect("gzip");
                fs::write(path, encoder.finish().expect("finish")).expect("write");
            },
            refused: |e| matches!(e, ImportError::Disagree { .. }),
            names: &["2.1.220", "2.1.221", "run-17"],
        },
        Fault {
            name: "copied_inputs disagree within a cell",
            inject: |root| {
                let files = run_files(root, V3, 17, "timing.json");
                assert_ne!(files.len(), 0);
                for file in &files {
                    edit_json(file, |value| {
                        let input = value
                            .pointer_mut("/copied_inputs/0/sha256")
                            .expect("a copied input");
                        *input = Value::String("0".repeat(64));
                    });
                }
            },
            refused: |e| matches!(e, ImportError::Disagree { .. }),
            names: &["timing.json", "run-17"],
        },
        Fault {
            name: "a run without outputs/",
            inject: |root| {
                let path = root.join(V3).join("_run_cache/with_skill/run-6/outputs");
                fs::remove_dir_all(path).expect("remove outputs/");
            },
            refused: |e| matches!(e, ImportError::NoOutputs { .. }),
            names: &["run-6", "outputs"],
        },
        Fault {
            name: "a status differs between expectations.jsonl and its grading file",
            inject: |root| edit_grading_status(root, "fail"),
            refused: |e| matches!(e, ImportError::Disagree { .. }),
            names: &["eval-1-grading.json", "expectations.jsonl", "run-6"],
        },
        Fault {
            name: "an unknown legacy status",
            inject: |root| {
                edit_expectation_status(root, "maybe");
                edit_grading_status(root, "maybe");
            },
            refused: |e| matches!(e, ImportError::Status { .. }),
            names: &["expectations.jsonl", "run-6", "maybe", "A1"],
        },
        Fault {
            name: "a grading file name in two eval directories",
            inject: |root| {
                let from = run_files(root, V3, 6, "eval-1-grading.json");
                let others = run_files(root, V3, 6, "eval_metadata.json");
                let from = from.first().expect("a grading file");
                let dir = from.parent().expect("a directory");
                let other = others
                    .iter()
                    .filter_map(|path| path.parent())
                    .find(|parent| *parent != dir)
                    .expect("another eval directory");
                fs::copy(from, other.join("eval-1-grading.json")).expect("copy");
            },
            refused: |e| matches!(e, ImportError::DuplicateGrading { .. }),
            names: &["eval-1-grading.json", "run-6"],
        },
    ]
}

/// Guards: D6 — a fault in the folder is refused with an error that names the file and the
/// run, and the same folder without the fault imports: the unmodified copy is the positive
/// control. Oracle: the faults are written by hand, one per row of the table, into a copy of the
/// mini-corpus; the expected variant and the file and run names come from the error contract of
/// the leaf and are literals, and the control imports the 136 rows counted by hand.
#[test]
fn every_fault_is_refused_naming_its_file_and_run() {
    let control = tempfile::tempdir().expect("a temporary directory");
    copy_dir(&mini(), control.path());
    assert_eq!(
        import_all(control.path()).expect("the control imports"),
        136
    );
    for fault in faults() {
        let copy = tempfile::tempdir().expect("a temporary directory");
        copy_dir(&mini(), copy.path());
        (fault.inject)(copy.path());
        let error = import_all(copy.path()).expect_err(&format!("`{}` is not refused", fault.name));
        assert!(
            (fault.refused)(&error),
            "{}: wrong error {error:?}",
            fault.name
        );
        let message = error.to_string();
        for name in fault.names {
            assert!(
                message.contains(name),
                "{}: the error does not name `{name}`: {message}",
                fault.name
            );
        }
    }
}

/// The hex digest of `bytes` as `sha256:<hex>`, computed here and not by the importer.
fn digest(bytes: &[u8]) -> String {
    let mut hex = String::new();
    for byte in sha2::Sha256::digest(bytes) {
        hex.extend(char::from_digit(u32::from(byte >> 4), 16));
        hex.extend(char::from_digit(u32::from(byte & 0xf), 16));
    }
    format!("sha256:{hex}")
}

/// Guards: I12 — every file of the folder is exactly one of a source of the folder, a cell or a
/// run, an artifact under `outputs/`, a stream or stderr file, or a skipped entry whose directory
/// covers the files below it; every file of the model exists on disk, with the digest of its
/// bytes; a file the model neither holds nor lists as skipped with its reason would be left out
/// silently. Oracle: a `std::fs` walk of the mini-corpus and a SHA-256 of each file's bytes made in
/// the test.
#[test]
fn every_file_is_accounted_for_exactly_once() {
    let imported = Imported::read(&mini());
    let folder = &imported.folder;
    let mut sources: BTreeMap<String, &SourceFile> = BTreeMap::new();
    let mut artifacts: BTreeMap<String, &SourceFile> = BTreeMap::new();
    let mut streams: BTreeMap<String, &SourceFile> = BTreeMap::new();
    for file in &folder.sources {
        // BORROW: `.to_owned()` makes the owned path key
        sources.insert(file.path.as_str().to_owned(), file);
    }
    for cell in &folder.cells {
        for file in &cell.sources {
            // BORROW: `.to_owned()` makes the owned path key
            sources.insert(file.path.as_str().to_owned(), file);
        }
        for run in &cell.runs {
            for file in &run.sources {
                // BORROW: `.to_owned()` makes the owned path key
                sources.insert(file.path.as_str().to_owned(), file);
            }
            for file in &run.outputs {
                // BORROW: `.to_owned()` makes the owned path key
                artifacts.insert(file.path.as_str().to_owned(), file);
            }
            for file in std::iter::once(&run.stream).chain(run.stderr.iter()) {
                // BORROW: `.to_owned()` makes the owned path key
                streams.insert(file.path.as_str().to_owned(), file);
            }
        }
    }
    let skipped: Vec<String> = folder
        .skipped
        .iter()
        // BORROW: `.to_owned()` makes the owned path
        .map(|entry| entry.path.as_str().to_owned())
        .collect();
    assert_ne!(skipped.len(), 0);
    let root = mini();
    let mut counts = [0_usize; 4];
    for path in walk(&root) {
        let relative = path
            .strip_prefix(&root)
            .expect("under the root")
            .to_string_lossy()
            .into_owned();
        let covered = skipped
            .iter()
            .any(|entry| relative == *entry || relative.starts_with(&format!("{entry}/")));
        let held = [
            sources.get(&relative),
            artifacts.get(&relative),
            streams.get(&relative),
        ];
        let kinds = held.iter().flatten().count() + usize::from(covered);
        assert_eq!(kinds, 1, "`{relative}` is accounted for {kinds} times");
        for (slot, file) in held.iter().enumerate() {
            if let Some(file) = file {
                let bytes = fs::read(&path).expect("read the file");
                // BORROW: explicit `.as_str()` instead of Deref coercion
                assert_eq!(file.path.as_str(), relative);
                assert_eq!(file.sha256.to_string(), digest(&bytes), "{relative}");
                assert_eq!(file.bytes, bytes, "{relative}");
                if let Some(count) = counts.get_mut(slot) {
                    *count += 1;
                }
            }
        }
        if covered && let Some(count) = counts.get_mut(3) {
            *count += 1;
        }
    }
    assert!(counts.iter().all(|&count| count > 0), "{counts:?}");
    let on_disk: BTreeSet<String> = walk(&root)
        .iter()
        .map(|path| {
            path.strip_prefix(&root)
                .expect("under the root")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    for path in sources.keys().chain(artifacts.keys()).chain(streams.keys()) {
        assert!(
            on_disk.contains(path),
            "the model holds `{path}`, which is not on disk"
        );
    }
}
