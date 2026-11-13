#![cfg(test)]
//! The event kinds of a job's `events.jsonl` that this version knows.

use std::fs;
use std::path::Path;

use candler_schemas::execution::{Envelope, EventPayload};
use serde_json::{Value, json};

const JOB: &str = "018f2b3c-4d5e-7a6b-8c7d-9e0f1a2b3c4d";

fn line(payload: &str) -> String {
    format!(
        r#"{{"v":1,"seq":4,"at":"2026-01-02T03:04:05Z","job_id":"{JOB}","kind":"evaluation.planned","payload":{payload}}}"#
    )
}

/// Guards: C6 (P6) — `evaluation.planned {outcomes}` is a known kind: it round-trips through the
/// envelope of `events.jsonl` and validates against the committed envelope schema, `outcomes` is
/// required and a non-negative integer that fits 32 bits, and the kind is no longer read as
/// `Unknown` nor accepted as one.
/// Oracle: the lines below, written by hand.
/// Bite: the kind missing from `KNOWN_EVENT_KINDS` (it reads back as `Unknown` and `unknown` accepts
/// it); `outcomes` made optional or signed.
#[test]
fn c6_evaluation_planned_round_trips_and_is_known() {
    let text = line(r#"{"outcomes":12}"#);
    let envelope: Envelope = serde_json::from_str(text.as_str()).expect("a planned event"); // BORROW: explicit .as_str()
    assert_eq!(envelope.event().kind(), "evaluation.planned");
    assert!(
        matches!(
            envelope.event(),
            EventPayload::EvaluationPlanned { outcomes: 12, .. }
        ),
        "read as another event: {:?}",
        envelope.event()
    );
    assert_eq!(serde_json::to_string(&envelope).expect("writes"), text);
    assert_eq!(
        EventPayload::evaluation_planned(12),
        envelope.event().clone(), // BORROW: .clone() compares an owned payload
    );

    let schema_text = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/event-envelope.schema.json"),
    )
    .expect("the committed schema exists");
    let schema: Value = serde_json::from_str(&schema_text).expect("the schema is JSON");
    let validator = jsonschema::validator_for(&schema).expect("the schema compiles");
    let written: Value = serde_json::from_str(text.as_str()).expect("json"); // BORROW: explicit .as_str()
    assert!(
        validator.is_valid(&written),
        "the schema refuses a planned event"
    );

    for payload in [
        "{}",
        r#"{"outcomes":-1}"#,
        r#"{"outcomes":1.5}"#,
        r#"{"outcomes":null}"#,
        r#"{"outcomes":"12"}"#,
    ] {
        let bad = line(payload);
        assert!(
            serde_json::from_str::<Envelope>(bad.as_str()).is_err(), // BORROW: explicit .as_str()
            "the reader accepts {payload}"
        );
        let value: Value = serde_json::from_str(bad.as_str()).expect("json"); // BORROW: explicit .as_str()
        assert!(!validator.is_valid(&value), "the schema accepts {payload}");
    }

    // JSON Schema cannot state the 32-bit limit, which the schema's `$comment` leaves to the reader.
    assert!(
        serde_json::from_str::<Envelope>(line(r#"{"outcomes":4294967296}"#).as_str()).is_err(), // BORROW: explicit .as_str()
        "the reader accepts a count beyond 32 bits"
    );
    assert!(
        EventPayload::unknown(String::from("evaluation.planned"), json!({"outcomes": 1})).is_err(),
        "a known kind cannot be built as an unknown one"
    );
}
