#![cfg(test)]
//! The origin of an imported run: a raw trace has no base commit and no legacy integrity block, and
//! a legacy import still writes the bytes it wrote before.

use std::fs;
use std::path::Path;
use std::str::FromStr;

use candler_schemas::execution::ImportRecord;
use candler_schemas::primitives::NameToken;
use serde_json::{Value, json};

/// The schema of an import record: `$defs.ImportRecord` of the committed run manifest schema, with
/// its definitions beside it.
fn import_record_validator() -> jsonschema::Validator {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/run-manifest.schema.json");
    let text = fs::read_to_string(path).expect("the committed schema exists");
    let schema: Value = serde_json::from_str(&text).expect("the schema is JSON");
    let record = json!({"$ref": "#/$defs/ImportRecord", "$defs": schema["$defs"]});
    jsonschema::validator_for(&record).expect("the schema compiles")
}

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

/// Guards: C5 (P2) — an import record may lack a commit and a legacy block, as a raw trace's does.
/// `ImportRecord::raw` round-trips and validates against the blessed schema; an explicit `null` for
/// either field is refused by the reader and by the schema; a legacy record writes the same bytes as
/// before the fields became optional, and a record that holds them reads back as `new` builds it.
/// Oracle: the texts below, written by hand from the wire at v0.6.0.
/// Bite: `base_commit` or `legacy_integrity` left required in the schema (the raw record fails
/// validation); `null` accepted (the null records are read).
#[test]
fn c5_an_import_record_may_lack_a_commit_and_a_legacy_block() {
    let validator = import_record_validator();
    let format = NameToken::from_str("claude-code-trace@1").expect("a format label");

    let raw = ImportRecord::raw(format.clone()); // BORROW: .clone() reuses the label
    let written = serde_json::to_value(&raw).expect("writes");
    assert_eq!(
        written,
        json!({"format": "claude-code-trace@1", "sources": [], "requests": []})
    );
    assert!(
        validator.is_valid(&written),
        "the schema refuses a raw origin"
    );
    let read: ImportRecord = serde_json::from_value(written).expect("reads back");
    assert_eq!(read, raw);
    assert_eq!(read.base_commit, None);
    assert_eq!(read.legacy_integrity, None);

    for field in ["base_commit", "legacy_integrity"] {
        let mut record = json!({
            "format": "claude-code-trace@1", "sources": [], "requests": []
        });
        record[field] = Value::Null;
        assert!(
            serde_json::from_value::<ImportRecord>(record.clone()).is_err(),
            "the reader accepts a null {field}"
        );
        assert!(
            !validator.is_valid(&record),
            "the schema accepts a null {field}"
        );
    }

    let legacy_text = format!(
        r#"{{"format":"campaign-import@v1","base_commit":"{COMMIT}","legacy_integrity":{{"valid":true}},"sources":[],"requests":[]}}"#
    );
    let legacy: ImportRecord = serde_json::from_str(legacy_text.as_str()).expect("reads"); // BORROW: explicit .as_str()
    assert_eq!(
        serde_json::to_string(&legacy).expect("writes"),
        legacy_text,
        "a legacy record writes the bytes it wrote before"
    );
    assert!(
        validator.is_valid(&serde_json::from_str::<Value>(legacy_text.as_str()).expect("json"))
    ); // BORROW: explicit .as_str()
    assert!(legacy.base_commit.is_some() && legacy.legacy_integrity.is_some());

    let ruled_text = format!(
        r#"{{"format":"campaign-import@v1","base_commit":"{COMMIT}","legacy_integrity":1,"sources":[],"requests":[],"rules":"sha256:{}"}}"#,
        "a".repeat(64)
    );
    let ruled: ImportRecord = serde_json::from_str(ruled_text.as_str()).expect("reads"); // BORROW: explicit .as_str()
    assert_eq!(serde_json::to_string(&ruled).expect("writes"), ruled_text);
}
