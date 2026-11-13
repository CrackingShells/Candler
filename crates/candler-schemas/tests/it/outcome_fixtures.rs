//! Outcome records read from fixture files. The synthetic files hold one record for each variant of
//! every member of the record. The golden files hold one record for each row of
//! `legacy/golden_rows.jsonl`; the tests compare each with the status, value, weight, cause and
//! role that a table written in the test gives for the raw row and its grading entry.

// Marks the file as test code, so clippy applies its test allowances to the helper functions.
#![cfg(test)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use candler_schemas::outcome::{OutcomeRecord, Value};
use serde_json::json;

fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn fixtures(sub: &str) -> PathBuf {
    tests_dir().join("fixtures").join(sub)
}

fn committed_schema() -> jsonschema::Validator {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schema")
        .join("outcome-record.schema.json");
    let text = fs::read_to_string(path).expect("the committed schema exists");
    let schema: serde_json::Value = serde_json::from_str(&text).expect("the schema is JSON");
    jsonschema::validator_for(&schema).expect("the schema compiles")
}

/// The `.json` files of a fixture directory, in name order, with their parsed content.
fn json_files(dir: &Path) -> Vec<(String, serde_json::Value)> {
    let mut files: Vec<(String, serde_json::Value)> = fs::read_dir(dir)
        .expect("the fixture directory exists")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .map(|path| {
            let text = fs::read_to_string(&path).expect("a readable fixture");
            let name = path
                .file_name()
                .expect("a file name")
                .to_string_lossy()
                // BORROW: Cow::into_owned makes the owned file name
                .into_owned();
            (name, serde_json::from_str(&text).expect("a JSON fixture"))
        })
        .collect();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

/// The text of a JSON string.
fn text(value: &serde_json::Value) -> &str {
    // BORROW: serde_json::Value::as_str reads a JSON string as text
    value.as_str().expect("a JSON string")
}

/// The text of a JSON string, or `None` for any other JSON value.
fn maybe_text(value: &serde_json::Value) -> Option<&str> {
    // BORROW: serde_json::Value::as_str reads a JSON string as text
    value.as_str()
}

fn jsonl(path: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(path)
        .expect("a readable file")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect()
}

/// A record is valid in every way a fixture can be checked: the schema accepts it, the reader
/// reads it, `validate()` passes, and writing it back gives the same JSON.
fn assert_valid(name: &str, document: &serde_json::Value, schema: &jsonschema::Validator) {
    assert!(schema.is_valid(document), "{name}: the schema refuses it");
    let record: OutcomeRecord =
        serde_json::from_value(document.clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
    record
        .validate()
        .unwrap_or_else(|e| panic!("{name}: validate() refuses it: {e}"));
    let written = serde_json::to_value(&record).expect("a record serialises");
    assert_eq!(&written, document, "{name}: it does not write back as read");
    assert!(
        !serde_json::to_string(&record)
            .expect("text")
            .contains("null"),
        "{name}: null is written"
    );
}

#[test]
fn outcome_synthetic_fixtures_validate_read_and_write_back() {
    let schema = committed_schema();
    let files = json_files(&fixtures("outcomes/synthetic"));
    assert!(files.len() >= 15, "only {} synthetic fixtures", files.len());
    for (name, document) in &files {
        assert_valid(name, document, &schema);
    }
}

/// Collects, for a set of records, the variants of every member they exercise.
fn variants(records: &[(String, serde_json::Value)]) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    for (_, record) in records {
        seen.insert(format!(
            "status:{}",
            maybe_text(&record["status"]).unwrap_or("?")
        ));
        seen.insert(format!(
            "role:{}",
            maybe_text(&record["role"]).unwrap_or("?")
        ));
        for key in ["value", "weight", "probability", "cause"] {
            if record.get(key).is_some() {
                seen.insert(format!("has:{key}"));
            }
        }
        if let Some(kind) = maybe_text(&record["value"]["kind"]) {
            seen.insert(format!("value:{kind}"));
        }
        if let Some(cause) = record["cause"].as_object() {
            for key in cause.keys() {
                seen.insert(format!("cause:{key}"));
            }
        }
        for evidence in record["evidence"].as_array().into_iter().flatten() {
            let form = match (evidence.get("ref"), evidence.get("note")) {
                (Some(_), Some(_)) => "both",
                (Some(_), None) => "ref",
                (None, Some(_)) => "note",
                (None, None) => "none",
            };
            seen.insert(format!("evidence:{form}"));
            let reference = maybe_text(&evidence["ref"]).unwrap_or("");
            for prefix in ["event:", "path:"] {
                if reference.starts_with(prefix) {
                    seen.insert(format!("evidence-ref:{prefix}"));
                }
            }
            if reference.contains("#L") {
                seen.insert("evidence-ref:lines".into());
            }
        }
        seen.insert(format!(
            "instruments:{}",
            record["instruments"].as_array().map_or(0, Vec::len).min(2)
        ));
        for instrument in record["instruments"].as_array().into_iter().flatten() {
            let digest = maybe_text(&instrument["digest"]).unwrap_or("");
            let kind = if digest.starts_with("git:") {
                "git"
            } else {
                "sha256"
            };
            seen.insert(format!("instrument:{kind}:dirty={}", instrument["dirty"]));
        }
        if record["weight"]["den"] == json!(0) {
            seen.insert("weight:0/0".into());
        }
    }
    seen
}

#[test]
fn outcome_synthetic_fixtures_cover_every_variant_of_every_member() {
    let files = json_files(&fixtures("outcomes/synthetic"));
    let seen = variants(&files);
    let mut wanted: Vec<String> = Vec::new();
    for status in ["pass", "fail", "blocked", "not_measured", "not_applicable"] {
        wanted.push(format!("status:{status}"));
    }
    for role in ["gate", "quality", "descriptive"] {
        wanted.push(format!("role:{role}"));
    }
    for kind in ["number", "count", "fraction", "text", "counts"] {
        wanted.push(format!("value:{kind}"));
    }
    for key in ["expectation", "artifact", "reason", "legacy"] {
        wanted.push(format!("cause:{key}"));
    }
    for form in ["ref", "note", "both"] {
        wanted.push(format!("evidence:{form}"));
    }
    for extra in [
        "has:value",
        "has:weight",
        "has:probability",
        "has:cause",
        "evidence-ref:event:",
        "evidence-ref:path:",
        "evidence-ref:lines",
        "instruments:0",
        "instruments:1",
        "instruments:2",
        "instrument:git:dirty=false",
        "instrument:sha256:dirty=true",
        "weight:0/0",
    ] {
        wanted.push(extra.into());
    }
    let missing: Vec<&String> = wanted.iter().filter(|item| !seen.contains(*item)).collect();
    assert!(
        missing.is_empty(),
        "no synthetic fixture exercises {missing:?}"
    );
}

#[test]
fn outcome_synthetic_fixtures_have_distinct_identities() {
    let files = json_files(&fixtures("outcomes/synthetic"));
    let keys: BTreeSet<String> = files
        .iter()
        .map(|(_, record)| {
            format!(
                "{}|{}|{}|{}",
                record["run"],
                record["expectation_id"],
                record["suite_entry_digest"],
                record["context_digest"]
            )
        })
        .collect();
    assert_eq!(keys.len(), files.len());
}

// ---- golden rows -------------------------------------------------------------------------

/// What the mapping of one corpus row must give, written from the row and its grading entry.
#[derive(Debug, PartialEq)]
struct Expected {
    status: String,
    role: String,
    value: Option<serde_json::Value>,
    weight: Option<serde_json::Value>,
    cause: Option<serde_json::Value>,
}

/// The weight a status carries when its record states none; `None` where the status has no weight.
const DEFAULT_WEIGHT: [(&str, Option<(u64, u64)>); 5] = [
    ("pass", Some((1, 1))),
    ("fail", Some((0, 1))),
    ("blocked", Some((0, 1))),
    ("not_measured", None),
    ("not_applicable", None),
];

/// The text that precedes the reason code of a replay marker in a grading entry's evidence.
const MARKER: &str = "TIER_D_REPLAY: status=not_measured reason=";

/// The text of an entry whose mapping declares no entries.
const ZERO_MAPPINGS_TEXT: &str = "declares zero 'mappings' entries";

fn expected_for(row: &serde_json::Value, entry: &serde_json::Value) -> Expected {
    let status = text(&row["status"]).to_string();
    let role = text(&row["role"]).to_string();

    let value = match &row["value"] {
        serde_json::Value::Null => None,
        serde_json::Value::Number(n) if n.is_i64() || n.is_u64() => {
            Some(json!({"kind": "count", "count": n}))
        }
        serde_json::Value::Number(n) => Some(json!({"kind": "number", "number": n})),
        other @ (serde_json::Value::Bool(_)
        | serde_json::Value::String(_)
        | serde_json::Value::Array(_)
        | serde_json::Value::Object(_)) => panic!("a corpus value of an unmapped kind: {other}"),
    };

    let (score, max_score) = (
        row["score"].as_u64().expect("a score"),
        row["max_score"].as_u64().expect("a max score"),
    );
    let default = DEFAULT_WEIGHT
        .iter()
        .find(|(name, _)| *name == status)
        .map(|(_, weight)| *weight)
        .expect("a known status");
    let mut cause = None;
    let weight = match default {
        None => None,
        Some(default) if (score, max_score) == default => None,
        Some(_) => {
            if (score, max_score) == (0, 0) {
                cause = Some(json!({"legacy": "zero_weight"}));
            }
            Some(json!({"num": score, "den": max_score}))
        }
    };

    let evidence = maybe_text(&entry["evidence"]).unwrap_or("");
    if status == "blocked" {
        cause = Some(json!({"artifact": entry["blocked_by"]}));
    } else if status == "not_measured" {
        let code = evidence.split_once(MARKER).map(|(_, rest)| {
            rest.chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
                .collect::<String>()
        });
        let code = match code {
            Some(code) => code,
            None if evidence.contains(ZERO_MAPPINGS_TEXT) => "zero_mappings".to_string(),
            None => "unrecorded".to_string(),
        };
        cause = Some(json!({"legacy": code}));
    }
    Expected {
        status,
        role,
        value,
        weight,
        cause,
    }
}

fn golden_rows() -> Vec<(serde_json::Value, serde_json::Value)> {
    let legacy = fixtures("legacy");
    let rows = jsonl(&legacy.join("golden_rows.jsonl"));
    let gradings = jsonl(&legacy.join("golden_grading.jsonl"));
    assert_eq!(rows.len(), 14, "one row per shape class");
    assert_eq!(rows.len(), gradings.len());
    rows.into_iter()
        .zip(gradings)
        .map(|(row, grading)| {
            for key in ["cell", "run", "eval", "id"] {
                assert_eq!(
                    row[key], grading[key],
                    "row and grading entry differ in {key}"
                );
            }
            let entry = grading["entry"].clone();
            (row, entry)
        })
        .collect()
}

fn golden_records() -> Vec<(String, serde_json::Value)> {
    json_files(&fixtures("outcomes/golden"))
}

#[test]
fn outcome_golden_fixtures_validate_read_and_write_back() {
    let schema = committed_schema();
    for (name, document) in &golden_records() {
        assert_valid(name, document, &schema);
    }
}

#[test]
fn outcome_golden_records_match_the_mapping_of_their_raw_rows() {
    let rows = golden_rows();
    let records = golden_records();
    assert_eq!(records.len(), rows.len());
    let mut by_cell: Vec<(String, String)> = Vec::new();
    for (index, ((row, entry), (name, record))) in rows.iter().zip(&records).enumerate() {
        let expected = expected_for(row, entry);
        let id = text(&row["id"]);
        assert_eq!(
            *name,
            format!("{:02}-{}-{id}.json", index + 1, expected.status),
            "file order follows row order"
        );

        // The fixture file, as parsed, and the record as the typed reader writes it back.
        let typed: OutcomeRecord = serde_json::from_value(record.clone()).expect("a record");
        let rewritten = serde_json::to_value(&typed).expect("a record serialises");
        for document in [record, &rewritten] {
            let got = Expected {
                status: text(&document["status"]).to_string(),
                role: text(&document["role"]).to_string(),
                value: document.get("value").cloned(),
                weight: document.get("weight").cloned(),
                cause: document.get("cause").cloned(),
            };
            assert_eq!(got, expected, "{name} does not match its row");
            assert_eq!(document["expectation_id"], row["id"], "{name}");
        }

        let replicate = maybe_text(&row["run"]).and_then(|run| run.strip_prefix("run-"));
        assert_eq!(
            record["run"]["replicate"].to_string(),
            replicate.expect("a run-N name"),
            "{name}"
        );
        assert_eq!(record["run"]["attempt"], json!(1), "{name}");
        assert_eq!(
            record["evaluator"],
            json!("htn-creator-eval@0.4.2"),
            "{name}"
        );
        assert_eq!(
            record["instruments"],
            json!([{
                "name": "htn_creator_scripts",
                "version": "0.4.2",
                "digest": "git:28be2c158fbc6ee786a789242c2d8fd47bfde9a8",
                "dirty": false,
            }]),
            "{name}"
        );
        assert_eq!(record["evidence"], json!([]), "{name}");
        by_cell.push((
            text(&row["cell"]).to_string(),
            text(&record["run"]["cell"]).to_string(),
        ));
    }
    // The same cell has one cell digest, and two cells have two.
    for (left_cell, left_digest) in &by_cell {
        for (right_cell, right_digest) in &by_cell {
            assert_eq!(left_cell == right_cell, left_digest == right_digest);
        }
    }
}

/// The expected status, role, weight and cause of one corpus row.
type TableRow = (
    &'static str,
    &'static str,
    Option<(u64, u64)>,
    Option<(&'static str, &'static str)>,
);

#[test]
fn outcome_golden_expectations_are_built_from_a_table_not_from_the_fixtures() {
    // The expected values for the corpus's shape classes, written out one by one.
    let rows = golden_rows();
    let table: [TableRow; 14] = [
        ("pass", "gate", None, None),
        ("pass", "quality", Some((35, 35)), None),
        (
            "blocked",
            "gate",
            None,
            Some(("artifact", "cont-train.json")),
        ),
        ("fail", "gate", None, None),
        ("pass", "descriptive", None, None),
        ("pass", "descriptive", None, None),
        ("fail", "gate", Some((0, 3)), None),
        (
            "pass",
            "gate",
            Some((0, 0)),
            Some(("legacy", "zero_weight")),
        ),
        (
            "not_measured",
            "descriptive",
            None,
            Some(("legacy", "no_replay_target")),
        ),
        ("pass", "gate", Some((2, 2)), None),
        ("fail", "gate", Some((1, 2)), None),
        ("blocked", "gate", None, Some(("artifact", "domain.py"))),
        (
            "not_measured",
            "descriptive",
            None,
            Some(("legacy", "zero_mappings")),
        ),
        ("fail", "gate", None, None),
    ];
    for ((row, entry), (status, role, weight, cause)) in rows.iter().zip(table) {
        let expected = expected_for(row, entry);
        assert_eq!(expected.status, status);
        assert_eq!(expected.role, role);
        assert_eq!(
            expected.weight,
            weight.map(|(num, den)| json!({"num": num, "den": den}))
        );
        assert_eq!(expected.cause, cause.map(|(key, code)| json!({key: code})));
    }
    let kinds: Vec<Option<&str>> = rows
        .iter()
        .map(|(row, entry)| {
            expected_for(row, entry)
                .value
                .map(|value| match maybe_text(&value["kind"]) {
                    Some("count") => "count",
                    Some("number") => "number",
                    _ => "other",
                })
        })
        .collect();
    assert_eq!(
        kinds,
        [
            None,
            None,
            None,
            None,
            Some("count"),
            Some("number"),
            Some("number"),
            None,
            None,
            Some("number"),
            None,
            Some("number"),
            None,
            Some("number"),
        ]
    );
}

#[test]
fn outcome_golden_set_has_one_record_for_each_shape_class_the_corpus_has() {
    let records = golden_records();
    let seen = variants(&records);
    for needed in [
        "status:pass",
        "status:fail",
        "status:blocked",
        "status:not_measured",
        "role:gate",
        "role:quality",
        "role:descriptive",
        "value:count",
        "value:number",
        "cause:artifact",
        "cause:legacy",
        "has:weight",
        "weight:0/0",
    ] {
        assert!(seen.contains(needed), "no golden record exercises {needed}");
    }
    assert!(!seen.contains("status:not_applicable"));
    assert!(!seen.contains("value:text"));
}

#[test]
fn outcome_golden_floats_serialise_to_the_text_the_corpus_has() {
    let path = fixtures("legacy").join("golden_floats.json");
    let text = fs::read_to_string(path).expect("the float list exists");
    let tokens: Vec<&str> = text
        .lines()
        .map(|line| line.trim().trim_end_matches(','))
        .filter(|line| !matches!(*line, "[" | "]" | ""))
        .collect();
    let parsed: Vec<f64> = serde_json::from_str(&text).expect("a list of floats");
    assert_eq!(tokens.len(), parsed.len());
    assert!(tokens.len() > 50);
    for (token, number) in tokens.iter().zip(parsed) {
        let value = Value::number(number).expect("a finite number");
        let written = serde_json::to_string(&value).expect("a value serialises");
        assert_eq!(written, format!(r#"{{"kind":"number","number":{token}}}"#));
        let read: Value = serde_json::from_str(&written).expect("a value reads");
        assert_eq!(read, value);
    }
}
