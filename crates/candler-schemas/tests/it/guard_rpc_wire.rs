#![cfg(test)]
//! The JSON-RPC envelope of the evaluator protocol, decided by hand-written vectors.

use std::fs;
use std::path::{Path, PathBuf};

use candler_schemas::evaluation::rpc::Message;
use candler_schemas::primitives::RequestId;
use serde_json::Value;

/// The `evaluator-notification` root, committed before the envelope existed.
const NOTIFICATION_SCHEMA: &str = include_str!("../../schema/evaluator-notification.schema.json");

fn rpc_dir(kind: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rpc")
        .join(kind)
}

/// The vectors of one folder as `(file name, text)`, in name order.
fn vectors(kind: &str) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = fs::read_dir(rpc_dir(kind))
        .expect("the vector folder is readable")
        .map(|entry| {
            let path = entry.expect("a folder entry is readable").path();
            let text = fs::read_to_string(&path).expect("a vector is readable");
            // BORROW: .to_string_lossy().into_owned() makes the file name a String
            (
                path.file_name()
                    .expect("a vector has a name")
                    .to_string_lossy()
                    .into_owned(),
                text,
            )
        })
        .collect();
    files.sort();
    files
}

fn vector(kind: &str, name: &str) -> Value {
    let text = fs::read_to_string(rpc_dir(kind).join(format!("{name}.json")))
        .expect("the vector is readable");
    serde_json::from_str(&text).expect("the vector is JSON")
}

fn accepted(value: &Value) -> bool {
    serde_json::from_value::<Message>(value.clone()).is_ok()
}

/// Guards: R8 — every valid JSON-RPC 2.0 message of the hand-written vectors is accepted, every
/// single-fault copy is refused, and a freshly minted request id is accepted in a request.
/// Oracle: the JSON-RPC 2.0 text, §4 to §5.1, as written into the vector files.
#[test]
fn the_envelope_accepts_valid_messages_and_refuses_single_faults() {
    let valid = vectors("valid");
    let invalid = vectors("invalid");
    assert_eq!(valid.len(), 9, "the valid vectors are all present");
    assert_eq!(invalid.len(), 6, "the invalid vectors are all present");
    for (name, text) in &valid {
        let result = serde_json::from_str::<Message>(text);
        assert!(result.is_ok(), "{name} is refused: {result:?}");
    }
    for (name, text) in &invalid {
        assert!(
            serde_json::from_str::<Message>(text).is_err(),
            "{name} is accepted"
        );
    }
    let mut request = vector("valid", "request_evaluate");
    // INDEX: "id" is a key of the request object
    request["id"] = Value::from(RequestId::new_v7().to_string());
    assert!(accepted(&request), "a minted id is refused: {request}");
}

/// Guards: R8 — a payload the existing `evaluator-notification` root accepts is accepted inside
/// the envelope, and a copy of it that lacks one required field is refused by both.
/// Oracle: `schema/evaluator-notification.schema.json`, validated with `jsonschema`.
#[test]
fn the_envelope_wraps_the_notification_root() {
    let schema: Value = serde_json::from_str(NOTIFICATION_SCHEMA).expect("the schema is JSON");
    let validator = jsonschema::validator_for(&schema).expect("the schema compiles");
    // The notification vector, and the path of the required field its fault removes.
    let cases: [(&str, &[&str]); 2] = [
        ("notification_outcome", &["record", "run"]),
        ("notification_cancel", &["request_id"]),
    ];
    for (name, fault) in cases {
        let enveloped = vector("valid", name);
        let mut bare = enveloped.clone();
        bare.as_object_mut()
            .expect("a message is an object")
            .remove("jsonrpc");
        assert!(validator.is_valid(&bare), "{name}: the root refuses it");
        assert!(accepted(&enveloped), "{name}: the envelope refuses it");

        let (last, parents) = fault.split_last().expect("a fault names a field");
        let mut broken = enveloped;
        let mut at = broken.get_mut("params").expect("a notification has params");
        for key in parents {
            at = at.get_mut(*key).expect("the fault path exists");
        }
        at.as_object_mut()
            .expect("the parent is an object")
            .remove(*last);
        let mut broken_bare = broken.clone();
        broken_bare
            .as_object_mut()
            .expect("a message is an object")
            .remove("jsonrpc");
        assert!(
            !validator.is_valid(&broken_bare),
            "{name}: the root accepts the fault"
        );
        assert!(!accepted(&broken), "{name}: the envelope accepts the fault");
    }
}
