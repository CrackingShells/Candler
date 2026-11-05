#![cfg(test)]
//! The patterns of the new text formats read alike in every engine.

use std::path::Path;
use std::str::FromStr;

use candler_schemas::generate::GeneratedSchema;
use candler_schemas::primitives::{FieldPath, FormatError, JobId, Marker};
use schemars::JsonSchema;
use serde_json::{Value, json};

/// The accept and refuse strings of one pattern, and the pattern the probes replay them against.
struct Table {
    pattern: String,
    accept: Vec<String>,
    refuse: Vec<String>,
}

fn table(name: &str) -> Table {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/patterns/cases.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let document: toml::Table = text.parse().unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let entry = document
        .get(name)
        .and_then(toml::Value::as_table)
        .unwrap_or_else(|| panic!("{name} table"));
    let strings = |key: &str| -> Vec<String> {
        entry
            .get(key)
            .and_then(toml::Value::as_array)
            .unwrap_or_else(|| panic!("{name}.{key}"))
            .iter()
            .map(|v| v.as_str().unwrap_or_else(|| panic!("{name}.{key} string")))
            // BORROW: str::to_owned makes the owned text the table keeps
            .map(str::to_owned)
            .collect()
    };
    Table {
        pattern: entry
            .get("pattern")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("{name}.pattern"))
            // BORROW: .to_owned() makes the owned text the table keeps
            .to_owned(),
        accept: strings("accept"),
        refuse: strings("refuse"),
    }
}

fn assert_vectors<T>(name: &str)
where
    T: FromStr<Err = FormatError> + JsonSchema,
{
    let table = table(name);
    assert!(table.accept.len() >= 25, "{name}: accept vectors");
    assert!(table.refuse.len() >= 40, "{name}: refuse vectors");

    let generated = GeneratedSchema::of::<T>(name).schema.to_value();
    assert_eq!(
        // INDEX: a published text schema always has a `pattern` key
        generated["pattern"],
        // BORROW: explicit .as_str() to build the JSON string from the table's text
        Value::from(table.pattern.as_str()),
        "{name}: cases.toml names a different pattern than the schema publishes"
    );
    let validator = jsonschema::validator_for(&generated).expect("the schema compiles");

    for text in &table.accept {
        assert!(text.parse::<T>().is_ok(), "{name}: type refuses {text:?}");
        assert!(
            validator.is_valid(&json!(text)),
            "{name}: schema refuses {text:?}"
        );
    }
    for text in &table.refuse {
        assert!(text.parse::<T>().is_err(), "{name}: type accepts {text:?}");
        assert!(
            !validator.is_valid(&json!(text)),
            "{name}: schema accepts {text:?}"
        );
    }
}

/// Guards: I12, I15 — the patterns that name a resolved value's field (`FieldPath`), a job
/// (`JobId`) and a child's run (`Marker`) accept and refuse the same hand-written
/// strings (trailing newline, Unicode digits and spaces among them) in the Rust parser and in the
/// schema's pattern. Oracle: the hand-written strings of `tests/vectors/patterns/cases.toml`,
/// replayed once under Python `re` and Node by `probe.py` and `probe.mjs`.
#[test]
fn field_path_and_marker_patterns_agree_with_their_parsers_on_hand_written_vectors() {
    assert_vectors::<FieldPath>("field_path");
    assert_vectors::<Marker>("marker");
    assert_vectors::<JobId>("job_id");
}
