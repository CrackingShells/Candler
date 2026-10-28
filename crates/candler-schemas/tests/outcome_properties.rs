//! Domain-free properties of the outcome record: a generated valid record satisfies the committed
//! schema and the Rust reader and round-trips; one mutation per rule is refused by both; and where
//! the two readers must differ, the difference is pinned.

// Marks the file as test code, so clippy applies its test allowances to the helper functions.
#![cfg(test)]

use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use candler_schemas::outcome::{
    Cause, Evidence, Finite, Fraction, Instrument, OutcomeRecord, Probability, Role, Status, Value,
    Weight,
};
use candler_schemas::primitives::{Digest, EvidenceRef, InstrumentDigest, NameVersion, RunKey};
use proptest::prelude::*;
use serde_json::json;

static SCHEMA: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schema")
        .join("outcome-record.schema.json");
    let text = fs::read_to_string(path).expect("the committed schema exists");
    let schema: serde_json::Value = serde_json::from_str(&text).expect("the schema is JSON");
    jsonschema::validator_for(&schema).expect("the schema compiles")
});

fn schema_accepts(document: &serde_json::Value) -> bool {
    SCHEMA.is_valid(document)
}

fn rust_accepts(document: &serde_json::Value) -> bool {
    serde_json::from_value::<OutcomeRecord>(document.clone()).is_ok()
}

#[allow(deprecated)]
fn legacy(code: &str) -> Cause {
    Cause::Legacy(code.parse().expect("a reason code"))
}

// ---- strategies --------------------------------------------------------------------------

/// Text of `min` to `max` characters, with quotes, backslashes, control characters and any other
/// character.
fn strings(min: usize, max: usize) -> impl Strategy<Value = String> {
    let character = prop_oneof![
        Just('"'),
        Just('\\'),
        Just('\n'),
        Just('\u{0}'),
        Just('\u{1f}'),
        proptest::char::any(),
    ];
    prop::collection::vec(character, min..max).prop_map(|chars| chars.into_iter().collect())
}

fn digest() -> impl Strategy<Value = Digest> {
    "[0-9a-f]{64}".prop_map(|hex| Digest::from_hex(&hex).expect("a digest"))
}

fn name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_-]{0,10}"
}

/// A `SemVer` 2.0 version, with a pre-release and build metadata in some cases.
fn semver() -> impl Strategy<Value = String> {
    (
        0u64..50,
        0u64..50,
        0u64..50,
        prop::option::of("[a-z]{1,4}\\.[1-9][0-9]?"),
        prop::option::of("[0-9A-Za-z-]{1,5}(\\.[0-9A-Za-z-]{1,5}){0,2}"),
    )
        .prop_map(|(major, minor, patch, pre, build)| {
            let mut version = format!("{major}.{minor}.{patch}");
            if let Some(pre) = pre {
                version = format!("{version}-{pre}");
            }
            if let Some(build) = build {
                version = format!("{version}+{build}");
            }
            version
        })
}

fn name_version() -> impl Strategy<Value = NameVersion> {
    (name(), semver())
        .prop_map(|(name, version)| format!("{name}@{version}").parse().expect("a name@version"))
}

/// The parts of an instrument: its identity, its digest and whether it may be dirty.
fn instrument_parts() -> impl Strategy<Value = (NameVersion, String, bool)> {
    let content = (digest(), any::<bool>()).prop_map(|(digest, dirty)| (digest.to_string(), dirty));
    let git = prop_oneof![
        "[0-9a-f]{40}".prop_map(|hex| format!("git:{hex}")),
        "[0-9a-f]{64}".prop_map(|hex| format!("git:{hex}")),
    ]
    .prop_map(|digest| (digest, false));
    (name_version(), prop_oneof![content, git])
        .prop_map(|(identity, (digest, dirty))| (identity, digest, dirty))
}

fn instrument(clean_only: bool) -> impl Strategy<Value = Instrument> {
    instrument_parts().prop_map(move |(identity, digest, dirty)| {
        let digest: InstrumentDigest = digest.parse().expect("an instrument digest");
        Instrument::new(identity, digest, dirty && !clean_only).expect("a valid instrument")
    })
}

fn rel_path() -> impl Strategy<Value = String> {
    prop::collection::vec("[A-Za-z0-9_][A-Za-z0-9_.-]{0,6}", 1..4)
        .prop_map(|segments| segments.join("/"))
}

fn evidence_ref() -> impl Strategy<Value = String> {
    prop_oneof![
        ("[A-Za-z0-9_-]{1,8}", prop_oneof![Just(0u64), 1u64..100_000])
            .prop_map(|(session, seq)| format!("event:{session}#{seq}")),
        (rel_path(), prop::option::of((1u64..500, 0u64..50))).prop_map(|(path, lines)| {
            match lines {
                None => format!("path:{path}"),
                Some((first, 0)) => format!("path:{path}#L{first}"),
                Some((first, extra)) => format!("path:{path}#L{first}-{}", first + extra),
            }
        }),
    ]
}

fn reference() -> impl Strategy<Value = EvidenceRef> {
    evidence_ref().prop_map(|text| text.parse().expect("an evidence ref"))
}

fn evidence() -> impl Strategy<Value = Evidence> {
    prop_oneof![
        reference().prop_map(|reference| Evidence::new(Some(reference), None).expect("evidence")),
        strings(1, 30).prop_map(|note| Evidence::new(None, Some(note)).expect("evidence")),
        (reference(), strings(1, 30)).prop_map(|(reference, note)| {
            Evidence::new(Some(reference), Some(note)).expect("evidence")
        }),
    ]
}

/// A finite `f64` of any magnitude: normal, subnormal, either zero.
fn number() -> impl Strategy<Value = f64> {
    prop_oneof![
        prop::num::f64::POSITIVE
            | prop::num::f64::NEGATIVE
            | prop::num::f64::NORMAL
            | prop::num::f64::SUBNORMAL
            | prop::num::f64::ZERO,
        -1.0e9f64..1.0e9,
        Just(-0.0),
        Just(0.0),
        Just(3.0),
        Just(f64::MAX),
        Just(f64::MIN),
        Just(f64::MIN_POSITIVE),
        Just(f64::from_bits(1)),
    ]
}

fn value() -> impl Strategy<Value = Value> {
    prop_oneof![
        number().prop_map(|number| Value::number(number).expect("finite")),
        any::<u64>().prop_map(Value::count),
        (any::<u64>(), 1u64..=u64::MAX)
            .prop_map(|(num, den)| Value::fraction(num, den).expect("a fraction")),
        strings(0, 20).prop_map(Value::text),
        prop::collection::btree_map(strings(0, 6), any::<u64>(), 0..4).prop_map(Value::counts),
    ]
}

fn probability() -> impl Strategy<Value = Probability> {
    (0.0f64..=1.0, strings(1, 12))
        .prop_map(|(p, reference)| Probability::new(p, reference).expect("a probability"))
}

fn status() -> impl Strategy<Value = Status> {
    prop::sample::select(vec![
        Status::Pass,
        Status::Fail,
        Status::Blocked,
        Status::NotMeasured,
        Status::NotApplicable,
    ])
}

fn role() -> impl Strategy<Value = Role> {
    prop::sample::select(vec![Role::Gate, Role::Quality, Role::Descriptive])
}

fn code() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,12}"
}

fn cause() -> impl Strategy<Value = Cause> {
    prop_oneof![
        "[A-Za-z0-9][A-Za-z0-9._-]{0,8}"
            .prop_map(|id| Cause::Expectation(id.parse().expect("an id"))),
        rel_path().prop_map(|path| Cause::Artifact(path.parse().expect("a path"))),
        code().prop_map(|code| Cause::Reason(code.parse().expect("a code"))),
        code().prop_map(|code| legacy(&code)),
    ]
}

fn denominator() -> impl Strategy<Value = u64> {
    prop_oneof![1u64..1000, 1u64..=u64::MAX]
}

/// The status of a record with the cause and the weight that its rules allow: a pass weighs
/// `num = den`, a fail `num < den`, a blocked `num = 0`, and the zero weight needs the legacy cause.
fn status_cause_weight() -> impl Strategy<Value = (Status, Option<Cause>, Option<Weight>)> {
    (
        status(),
        cause(),
        any::<bool>(),
        any::<bool>(),
        denominator(),
        any::<u64>(),
        0u8..6,
    )
        .prop_map(
            |(status, cause, with_legacy, with_weight, den, seed, zero_pick)| {
                let required = matches!(
                    status,
                    Status::Blocked | Status::NotMeasured | Status::NotApplicable
                );
                let mut cause = if required || (with_legacy && cause.is_legacy()) {
                    Some(cause)
                } else {
                    None
                };
                let (num, den) = match status {
                    Status::Pass if zero_pick == 0 => (0, 0),
                    Status::Pass => (den, den),
                    Status::Fail => (seed % den, den),
                    Status::Blocked => (0, den),
                    Status::NotMeasured | Status::NotApplicable => (0, 0),
                };
                let weight = match status {
                    Status::NotMeasured | Status::NotApplicable => None,
                    Status::Pass | Status::Fail | Status::Blocked => {
                        with_weight.then(|| Weight::new(num, den).expect("num <= den"))
                    }
                };
                if weight.is_some_and(|weight| weight.is_zero())
                    && !cause.as_ref().is_some_and(Cause::is_legacy)
                {
                    cause = Some(legacy("zero_weight"));
                }
                (status, cause, weight)
            },
        )
}

fn record() -> impl Strategy<Value = OutcomeRecord> {
    let identity = (
        digest(),
        1u32..1000,
        1u32..20,
        "[A-Za-z0-9][A-Za-z0-9._-]{0,10}",
        role(),
        name_version(),
        digest(),
        digest(),
        "[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}",
    );
    let outcome = (
        status_cause_weight(),
        prop::option::of(value()),
        prop::option::of(probability()),
        prop::collection::vec(evidence(), 0..3),
        prop::collection::vec(instrument(false), 0..3),
        prop::collection::vec(instrument(true), 0..3),
    );
    (identity, outcome).prop_map(
        |(
            (cell, replicate, attempt, expectation, role, evaluator, suite, context, request),
            ((status, cause, weight), value, probability, evidence, instruments, clean),
        )| {
            // A gate or a quality record carries no probability and no dirty instrument.
            let graded = matches!(role, Role::Gate | Role::Quality);
            let (probability, instruments) = if graded {
                (None, clean)
            } else {
                (probability, instruments)
            };
            let mut builder = OutcomeRecord::builder()
                .run(RunKey::new(cell, replicate, attempt).expect("a run key"))
                .expectation_id(expectation.parse().expect("an id"))
                .role(role)
                .evaluator(evaluator)
                .suite_entry_digest(suite)
                .context_digest(context)
                .request_id(request.parse().expect("a request id"))
                .status(status);
            if let Some(cause) = cause {
                builder = builder.cause(cause);
            }
            if let Some(weight) = weight {
                builder = builder.weight(weight);
            }
            if let Some(value) = value {
                builder = builder.value(value);
            }
            if let Some(probability) = probability {
                builder = builder.probability(probability);
            }
            for evidence in evidence {
                builder = builder.evidence(evidence);
            }
            for instrument in instruments {
                builder = builder.instrument(instrument);
            }
            builder.build().expect("generated records are valid")
        },
    )
}

fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::with_cases(256)
    }
}

// ---- valid records ------------------------------------------------------------------------

proptest! {
    #![proptest_config(config())]

    #[test]
    fn outcome_valid_records_satisfy_the_schema_and_round_trip(record in record()) {
        let document = serde_json::to_value(&record).expect("a record serialises");
        let errors: Vec<String> = SCHEMA.iter_errors(&document).map(|e| e.to_string()).collect();
        prop_assert!(errors.is_empty(), "the schema refuses {document}: {errors:?}");
        let text = serde_json::to_string(&record).expect("text");
        prop_assert!(!text.contains("null"));
        let back: OutcomeRecord = serde_json::from_str(&text).expect("reads back");
        prop_assert!(back.validate().is_ok());
        prop_assert_eq!(&back, &record);
        prop_assert_eq!(serde_json::to_value(&back).expect("serialises"), document);
        prop_assert_eq!(back.effective_weight(), record.effective_weight());
    }
}

// ---- mutations ----------------------------------------------------------------------------

/// A change to a valid record that makes it invalid: fields replaced, then fields removed.
#[derive(Debug, Clone)]
struct Mutation {
    name: String,
    sets: Vec<(&'static str, serde_json::Value)>,
    removes: Vec<&'static str>,
}

impl Mutation {
    fn apply(&self, record: &serde_json::Value) -> serde_json::Value {
        let mut mutated = record.clone();
        let object = mutated.as_object_mut().expect("a record is an object");
        for key in &self.removes {
            object.remove(*key);
        }
        for (key, value) in &self.sets {
            object.insert((*key).into(), value.clone());
        }
        mutated
    }
}

fn mutation(
    name: impl Into<String>,
    sets: &[(&'static str, serde_json::Value)],
    removes: &[&'static str],
) -> Mutation {
    Mutation {
        name: name.into(),
        sets: sets.to_vec(),
        removes: removes.to_vec(),
    }
}

const FIELDS: [&str; 14] = [
    "run",
    "expectation_id",
    "role",
    "evaluator",
    "instruments",
    "suite_entry_digest",
    "context_digest",
    "request_id",
    "status",
    "value",
    "weight",
    "probability",
    "evidence",
    "cause",
];

fn hex(digit: char, count: usize) -> String {
    std::iter::repeat_n(digit, count).collect()
}

fn good_digest() -> String {
    format!("sha256:{}", hex('a', 64))
}

/// Digests the format refuses.
fn bad_digests() -> Vec<String> {
    vec![
        String::new(),
        "sha256:".into(),
        "sha256:abc".into(),
        format!("sha256:{}", hex('a', 63)),
        format!("sha256:{}", hex('a', 65)),
        format!("sha256:{}", hex('A', 64)),
        format!("sha256:{}", hex('g', 64)),
        format!("SHA256:{}", hex('a', 64)),
        format!(" sha256:{}", hex('a', 64)),
        format!("md5:{}", hex('a', 32)),
        hex('a', 64),
    ]
}

fn bad_paths() -> Vec<&'static str> {
    vec![
        "", "/abs", "a/../b", "..", ".", "./a", "a/.", "a//b", "a\\b", "a/", "/",
    ]
}

/// Every mutation the schema and the Rust reader both refuse.
static MUTATIONS: LazyLock<Vec<Mutation>> = LazyLock::new(mutations);

fn mutations() -> Vec<Mutation> {
    let reason = json!({"reason": "deadline"});
    let mut all = Vec::new();

    for field in FIELDS {
        all.push(mutation(
            format!("null {field}"),
            &[(field, json!(null))],
            &[],
        ));
    }
    all.push(mutation(
        "null evidence ref",
        &[("evidence", json!([{"ref": null, "note": "n"}]))],
        &[],
    ));
    all.push(mutation(
        "null evidence note",
        &[("evidence", json!([{"ref": "event:s#0", "note": null}]))],
        &[],
    ));
    all.push(mutation(
        "null count payload",
        &[("value", json!({"kind": "count", "count": null}))],
        &[],
    ));
    all.push(mutation(
        "null weight denominator",
        &[("weight", json!({"num": 0, "den": null}))],
        &[],
    ));

    for spelling in [
        "not-measured",
        "not measured",
        "NotMeasured",
        "NOT_MEASURED",
        "Pass",
        "skipped",
        "",
    ] {
        all.push(mutation(
            format!("status {spelling:?}"),
            &[("status", json!(spelling))],
            &[],
        ));
    }
    for role in ["Gate", "gates", "", "descriptive "] {
        all.push(mutation(
            format!("role {role:?}"),
            &[("role", json!(role))],
            &[],
        ));
    }

    for status in ["pass", "fail"] {
        for cause in [
            json!({"reason": "deadline"}),
            json!({"artifact": "out/a.py"}),
            json!({"expectation": "B2"}),
        ] {
            all.push(mutation(
                format!("{status} with cause {cause}"),
                &[("status", json!(status)), ("cause", cause)],
                &["weight"],
            ));
        }
    }
    for status in ["blocked", "not_measured", "not_applicable"] {
        all.push(mutation(
            format!("{status} without a cause"),
            &[("status", json!(status))],
            &["cause", "weight"],
        ));
    }
    for status in ["not_measured", "not_applicable"] {
        for weight in [
            json!({"num": 1, "den": 2}),
            json!({"num": 0, "den": 1}),
            json!({"num": 0, "den": 0}),
        ] {
            all.push(mutation(
                format!("{status} with weight {weight}"),
                &[
                    ("status", json!(status)),
                    ("cause", json!({"legacy": "x"})),
                    ("weight", weight),
                ],
                &[],
            ));
        }
    }
    for (status, cause) in [
        ("pass", None),
        ("fail", None),
        ("blocked", Some(reason.clone())),
    ] {
        let mut sets = vec![
            ("status", json!(status)),
            ("weight", json!({"num": 0, "den": 0})),
        ];
        let mut removes = Vec::new();
        match cause {
            Some(cause) => sets.push(("cause", cause)),
            None => removes.push("cause"),
        }
        all.push(mutation(
            format!("0/0 on {status} without the legacy cause"),
            &sets,
            &removes,
        ));
    }

    for value in [
        json!({"kind": "fraction", "num": 1, "den": 0}),
        json!({"kind": "fraction", "num": 0, "den": 0}),
        json!({"kind": "fraction", "num": 1, "den": -1}),
        json!({"kind": "fraction", "num": -1, "den": 2}),
        json!({"kind": "fraction", "num": 1}),
        json!({"kind": "fraction", "den": 2}),
        json!({"kind": "unknown", "unknown": 1}),
        json!({"kind": "count", "count": -1}),
        json!({"kind": "count", "count": 1.5}),
        json!({"kind": "count", "count": "1"}),
        json!({"kind": "count"}),
        json!({"kind": "number", "number": "1"}),
        json!({"kind": "number"}),
        json!({"kind": "text", "text": 1}),
        json!({"kind": "text"}),
        json!({"kind": "counts", "counts": {"a": -1}}),
        json!({"kind": "counts", "counts": {"a": 1.5}}),
        json!({"kind": "counts", "counts": [1]}),
        json!({"number": 1}),
        json!(1.5),
        json!("text"),
    ] {
        all.push(mutation(format!("value {value}"), &[("value", value)], &[]));
    }

    for weight in [
        json!({"num": 1}),
        json!({"den": 1}),
        json!({"num": -1, "den": 1}),
        json!({"num": 1, "den": -1}),
        json!({"num": "1", "den": 1}),
        json!({"num": 0.5, "den": 1}),
        json!(0.5),
    ] {
        all.push(mutation(
            format!("weight {weight}"),
            &[("weight", weight)],
            &[],
        ));
    }

    for field in ["suite_entry_digest", "context_digest"] {
        for bad in bad_digests() {
            all.push(mutation(
                format!("{field} {bad:?}"),
                &[(field, json!(bad))],
                &[],
            ));
        }
    }
    for bad in bad_digests() {
        all.push(mutation(
            format!("run cell {bad:?}"),
            &[("run", json!({"cell": bad, "replicate": 1, "attempt": 1}))],
            &[],
        ));
    }
    for run in [
        json!({"cell": good_digest(), "replicate": 0, "attempt": 1}),
        json!({"cell": good_digest(), "replicate": 1, "attempt": 0}),
        json!({"cell": good_digest(), "replicate": -1, "attempt": 1}),
        json!({"cell": good_digest(), "replicate": 1}),
        json!({"replicate": 1, "attempt": 1}),
        json!(format!("{}-r1-a1", hex('a', 64))),
    ] {
        all.push(mutation(format!("run {run}"), &[("run", run)], &[]));
    }
    for bad in [
        "",
        "not-a-uuid",
        "550e8400-e29b-41d4-a716-446655440000",
        "01926B3A-7C3E-7A1B-8C4D-5E6F7A8B9C0D",
        "01926b3a7c3e7a1b8c4d5e6f7a8b9c0d",
        "01926b3a-7c3e-7a1b-0c4d-5e6f7a8b9c0d",
        "{01926b3a-7c3e-7a1b-8c4d-5e6f7a8b9c0d}",
        "01926b3a-7c3e-7a1b-8c4d-5e6f7a8b9c0d ",
    ] {
        all.push(mutation(
            format!("request_id {bad:?}"),
            &[("request_id", json!(bad))],
            &[],
        ));
    }
    for bad in ["", "-a", ".a", "_a", "a b", "a/b", "a@b", "é"] {
        all.push(mutation(
            format!("expectation_id {bad:?}"),
            &[("expectation_id", json!(bad))],
            &[],
        ));
    }
    for bad in [
        "name",
        "name@",
        "@1.0.0",
        "name@1.0",
        "name@v1.0.0",
        "name@01.0.0",
        "na me@1.0.0",
        "a@b@1.0.0",
        "name@1.0.0 ",
    ] {
        all.push(mutation(
            format!("evaluator {bad:?}"),
            &[("evaluator", json!(bad))],
            &[],
        ));
    }

    for bad in bad_paths() {
        all.push(mutation(
            format!("artifact cause {bad:?}"),
            &[
                ("status", json!("blocked")),
                ("cause", json!({"artifact": bad})),
                ("weight", json!({"num": 0, "den": 1})),
            ],
            &[],
        ));
    }
    for bad in ["", "Deadline", "1abc", "a-b", "a b", "é"] {
        for key in ["reason", "legacy"] {
            all.push(mutation(
                format!("{key} cause {bad:?}"),
                &[("status", json!("blocked")), ("cause", json!({key: bad}))],
                &["weight"],
            ));
        }
    }
    for bad in ["", "-x", "a b"] {
        all.push(mutation(
            format!("expectation cause {bad:?}"),
            &[
                ("status", json!("blocked")),
                ("cause", json!({"expectation": bad})),
            ],
            &["weight"],
        ));
    }
    for shape in [
        json!({}),
        json!({"reason": "a", "artifact": "b"}),
        json!({"reason": "a", "legacy": "b"}),
        json!({"unknown": "x"}),
        json!({"reason": 1}),
        json!("deadline"),
        json!(["reason", "deadline"]),
    ] {
        all.push(mutation(
            format!("cause shape {shape}"),
            &[("status", json!("blocked")), ("cause", shape)],
            &["weight"],
        ));
    }

    for bad in [
        "",
        "event:",
        "event:s",
        "event:#1",
        "event:s#",
        "event:s#01",
        "event:s#-1",
        "event: s#1",
        "event:s #1",
        "path:",
        "path:/a",
        "path:a/../b",
        "path:a#",
        "path:a#L0",
        "path:a#L1-",
        "path:a#x",
        "path:a#L1#L2",
        "file:a",
        "a",
    ] {
        all.push(mutation(
            format!("evidence ref {bad:?}"),
            &[("evidence", json!([{"ref": bad}]))],
            &[],
        ));
    }
    for evidence in [
        json!([{}]),
        json!([{"note": ""}]),
        json!([{"ref": 5}]),
        json!([{"note": 5}]),
        json!([5]),
        json!({"note": "n"}),
        json!("n"),
    ] {
        all.push(mutation(
            format!("evidence {evidence}"),
            &[("evidence", evidence)],
            &[],
        ));
    }

    let git = format!("git:{}", hex('b', 40));
    for (label, bad) in [
        (
            "name with @",
            one_instrument("a@b", "1.0.0", &good_digest(), &json!(false)),
        ),
        (
            "empty name",
            one_instrument("", "1.0.0", &good_digest(), &json!(false)),
        ),
        (
            "name with a space",
            one_instrument("a b", "1.0.0", &good_digest(), &json!(false)),
        ),
        (
            "version without a patch",
            one_instrument("a", "1.0", &good_digest(), &json!(false)),
        ),
        (
            "version with a v",
            one_instrument("a", "v1.0.0", &good_digest(), &json!(false)),
        ),
        (
            "version with a leading zero",
            one_instrument("a", "01.0.0", &good_digest(), &json!(false)),
        ),
        (
            "empty version",
            one_instrument("a", "", &good_digest(), &json!(false)),
        ),
        (
            "short digest",
            one_instrument("a", "1.0.0", "sha256:abc", &json!(false)),
        ),
        (
            "short git digest",
            one_instrument("a", "1.0.0", "git:abc", &json!(false)),
        ),
        (
            "uppercase git digest",
            one_instrument(
                "a",
                "1.0.0",
                &format!("git:{}", hex('B', 40)),
                &json!(false),
            ),
        ),
        (
            "dirty git digest",
            one_instrument("a", "1.0.0", &git, &json!(true)),
        ),
        (
            "dirty as text",
            one_instrument("a", "1.0.0", &good_digest(), &json!("false")),
        ),
        (
            "missing dirty",
            json!([{"name": "a", "version": "1.0.0", "digest": good_digest()}]),
        ),
        ("not a list", json!({"name": "a"})),
    ] {
        all.push(mutation(
            format!("instrument: {label}"),
            &[("instruments", bad)],
            &[],
        ));
    }
    for field in ["instruments", "evidence"] {
        all.push(mutation(format!("missing {field}"), &[], &[field]));
    }

    for probability in [
        json!({"p": 1.5, "calibration_ref": "c"}),
        json!({"p": -0.1, "calibration_ref": "c"}),
        json!({"p": 1.000_000_1, "calibration_ref": "c"}),
        json!({"p": 0.5, "calibration_ref": ""}),
        json!({"p": 0.5}),
        json!({"calibration_ref": "c"}),
        json!({"p": "0.5", "calibration_ref": "c"}),
        json!({"p": 0.5, "calibration_ref": 1}),
        json!(0.5),
    ] {
        all.push(mutation(
            format!("probability {probability}"),
            &[("probability", probability)],
            &[],
        ));
    }
    // A blocked weighs num 0 and den of at least 1, with any cause; the schema states it.
    for (status, num, den) in [("blocked", 1, 2), ("blocked", 2, 2)] {
        let mut sets = vec![
            ("status", json!(status)),
            ("weight", json!({"num": num, "den": den})),
        ];
        if status == "blocked" {
            sets.push(("cause", reason.clone()));
        }
        all.push(mutation(
            format!("{status} weighing {num}/{den}"),
            &sets,
            &[],
        ));
    }
    all.push(mutation(
        "blocked weighing 0/0 with the legacy cause",
        &[
            ("status", json!("blocked")),
            ("weight", json!({"num": 0, "den": 0})),
            ("cause", json!({"legacy": "zero_weight"})),
        ],
        &[],
    ));

    // A gate or quality record has no probability and no dirty instrument.
    let dirty = one_instrument("local_checker", "2.0.0", &good_digest(), &json!(true));
    for role in ["gate", "quality"] {
        all.push(mutation(
            format!("probability on a {role}"),
            &[
                ("role", json!(role)),
                ("probability", json!({"p": 0.5, "calibration_ref": "c"})),
            ],
            &[],
        ));
        all.push(mutation(
            format!("dirty instrument on a {role}"),
            &[("role", json!(role)), ("instruments", dirty.clone())],
            &[],
        ));
    }

    // A struct written as a list is refused by the reader as well as by the schema.
    for (field, list) in [
        ("weight", json!([1, 2])),
        ("probability", json!([0.5, "c"])),
        ("evidence", json!([["event:s#0", "n"]])),
        ("instruments", json!([["i", "1.0.0", good_digest(), false]])),
        ("value", json!(["fraction", 1, 2])),
    ] {
        all.push(mutation(
            format!("{field} as a list"),
            &[(field, list)],
            &[],
        ));
    }
    all
}

/// A list of one instrument, as JSON.
fn one_instrument(
    name: &str,
    version: &str,
    digest: &str,
    dirty: &serde_json::Value,
) -> serde_json::Value {
    json!([{"name": name, "version": version, "digest": digest, "dirty": dirty}])
}

/// A valid record with the given status, as JSON.
fn base(status: &str) -> serde_json::Value {
    let weight = match status {
        "pass" => json!({"num": 2, "den": 2}),
        "fail" => json!({"num": 1, "den": 2}),
        _ => json!({"num": 0, "den": 2}),
    };
    let mut base = json!({
        "run": {"cell": good_digest(), "replicate": 2, "attempt": 1},
        "expectation_id": "A1",
        "role": "descriptive",
        "evaluator": "htn-creator-eval@0.4.2",
        "instruments": [{
            "name": "htn_creator_scripts",
            "version": "0.4.2",
            "digest": format!("git:{}", hex('b', 40)),
            "dirty": false,
        }],
        "suite_entry_digest": good_digest(),
        "context_digest": good_digest(),
        "request_id": "01926b3a-7c3e-7a1b-8c4d-5e6f7a8b9c0d",
        "status": status,
        "value": {"kind": "fraction", "num": 1, "den": 2},
        "weight": weight,
        "probability": {"p": 0.5, "calibration_ref": "calibration/1"},
        "evidence": [{"ref": "path:out/a.py#L1-2", "note": "n"}],
    });
    if matches!(status, "blocked" | "not_measured" | "not_applicable") {
        base["cause"] = json!({"reason": "deadline"});
    }
    if matches!(status, "not_measured" | "not_applicable")
        && let Some(object) = base.as_object_mut()
    {
        object.remove("weight");
    }
    base
}

#[test]
fn outcome_pristine_bases_are_valid_for_both_readers() {
    for status in ["pass", "fail", "blocked", "not_measured", "not_applicable"] {
        let document = base(status);
        assert!(
            schema_accepts(&document),
            "{status}: the schema refuses the base"
        );
        assert!(
            rust_accepts(&document),
            "{status}: the reader refuses the base"
        );
    }
}

#[test]
fn outcome_every_mutation_is_refused_by_both_readers() {
    let all = &*MUTATIONS;
    assert!(all.len() > 200, "only {} mutations", all.len());
    for status in ["pass", "blocked", "not_measured"] {
        let base = base(status);
        for mutation in all {
            let mutated = mutation.apply(&base);
            assert!(
                !schema_accepts(&mutated),
                "the schema accepts `{}` on a {status} base: {mutated}",
                mutation.name
            );
            assert!(
                !rust_accepts(&mutated),
                "the reader accepts `{}` on a {status} base: {mutated}",
                mutation.name
            );
        }
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn outcome_a_mutated_generated_record_is_refused_by_both_readers(
        record in record(),
        index in any::<prop::sample::Index>(),
    ) {
        let mutation = index.get(&MUTATIONS);
        let document = serde_json::to_value(&record).expect("a record serialises");
        prop_assert!(schema_accepts(&document));
        let mutated = mutation.apply(&document);
        prop_assert!(!schema_accepts(&mutated), "schema accepts `{}`: {}", mutation.name, mutated);
        prop_assert!(!rust_accepts(&mutated), "reader accepts `{}`: {}", mutation.name, mutated);
    }
}

// ---- where the two readers differ ---------------------------------------------------------

#[test]
fn outcome_json_schema_reads_3_0_as_an_integer_and_the_rust_reader_does_not() {
    let base = base("pass");
    for (label, edit) in [
        (
            "replicate",
            json!({"run": {"cell": good_digest(), "replicate": 3.0, "attempt": 1}}),
        ),
        (
            "attempt",
            json!({"run": {"cell": good_digest(), "replicate": 1, "attempt": 2.0}}),
        ),
        ("count", json!({"value": {"kind": "count", "count": 3.0}})),
        (
            "fraction num",
            json!({"value": {"kind": "fraction", "num": 1.0, "den": 2}}),
        ),
        (
            "fraction den",
            json!({"value": {"kind": "fraction", "num": 1, "den": 2.0}}),
        ),
        (
            "counts entry",
            json!({"value": {"kind": "counts", "counts": {"a": 3.0}}}),
        ),
        ("weight num", json!({"weight": {"num": 2.0, "den": 2}})),
        ("weight den", json!({"weight": {"num": 2, "den": 2.0}})),
    ] {
        let mut document = base.clone();
        for (key, value) in edit.as_object().expect("an object") {
            document[key] = value.clone();
        }
        assert!(schema_accepts(&document), "the schema refuses {label} 3.0");
        assert!(!rust_accepts(&document), "the reader accepts {label} 3.0");
    }
}

#[test]
fn outcome_writers_emit_integer_literals_and_both_readers_read_a_number_written_as_an_integer() {
    let record: OutcomeRecord = serde_json::from_value(base("pass")).expect("a record");
    let text = serde_json::to_string(&record).expect("text");
    assert!(text.contains(r#""replicate":2"#), "{text}");
    assert!(text.contains(r#""num":2,"den":2"#), "{text}");
    assert!(!text.contains("1.0") && !text.contains("2.0"), "{text}");
    for literal in ["3", "3.0"] {
        let mut document = base("pass");
        document["value"] =
            serde_json::from_str(&format!(r#"{{"kind": "number", "number": {literal}}}"#))
                .expect("JSON");
        assert!(
            schema_accepts(&document),
            "the schema refuses a number written {literal}"
        );
        assert!(
            rust_accepts(&document),
            "the reader refuses a number written {literal}"
        );
    }
}

/// An edit of a record's JSON.
type Edit = fn(&mut serde_json::Value);

#[test]
fn outcome_the_schema_refuses_unknown_fields_and_the_rust_reader_ignores_them() {
    let base = base("blocked");
    let edits: [(&str, Edit); 8] = [
        ("record", |d| d["future"] = json!(1)),
        ("run", |d| d["run"]["future"] = json!(1)),
        ("value", |d| d["value"]["future"] = json!(1)),
        ("weight", |d| d["weight"]["future"] = json!(1)),
        ("probability", |d| d["probability"]["future"] = json!(1)),
        ("evidence", |d| d["evidence"][0]["future"] = json!(1)),
        ("instrument", |d| d["instruments"][0]["future"] = json!(1)),
        ("cause", |d| {
            d["cause"] = json!({"reason": "deadline", "future": 1});
        }),
    ];
    for (label, edit) in edits {
        let mut document = base.clone();
        edit(&mut document);
        if label == "cause" {
            // A cause has exactly one key: the reader refuses a second one too.
            assert!(!schema_accepts(&document));
            assert!(!rust_accepts(&document));
        } else {
            assert!(
                !schema_accepts(&document),
                "the schema accepts an unknown field on {label}"
            );
            assert!(
                rust_accepts(&document),
                "the reader refuses an unknown field on {label}"
            );
        }
    }
}

#[test]
fn outcome_the_schema_cannot_compare_weight_num_with_den_and_the_rust_reader_does() {
    for (num, den) in [(2, 1), (3, 2), (1, 0), (0, 0), (u64::MAX, u64::MAX - 1)] {
        let mut document = base("fail");
        document["weight"] = json!({"num": num, "den": den});
        document["cause"] = json!({"legacy": "zero_weight"});
        assert!(schema_accepts(&document), "the schema refuses {num}/{den}");
        assert!(!rust_accepts(&document), "the reader accepts {num}/{den}");
    }
}

#[test]
fn outcome_the_schema_states_only_the_blocked_weight_rule_and_the_rust_reader_states_all_three() {
    // A pass weighs num = den and a fail num < den; the schema compares neither.
    for (status, num, den) in [
        ("pass", 1, 2),
        ("pass", 0, 3),
        ("fail", 2, 2),
        ("fail", 3, 3),
        ("fail", u64::MAX, u64::MAX),
    ] {
        let mut document = base(status);
        document["weight"] = json!({"num": num, "den": den});
        assert!(
            schema_accepts(&document),
            "the schema refuses {status} {num}/{den}"
        );
        assert!(
            !rust_accepts(&document),
            "the reader accepts {status} {num}/{den}"
        );
    }
    // A blocked weighs num 0, and both readers refuse anything else.
    for (num, den) in [(1, 2), (2, 2), (1, 1)] {
        let mut document = base("blocked");
        document["weight"] = json!({"num": num, "den": den});
        assert!(
            !schema_accepts(&document),
            "the schema accepts blocked {num}/{den}"
        );
        assert!(
            !rust_accepts(&document),
            "the reader accepts blocked {num}/{den}"
        );
    }
    for (status, num, den) in [
        ("pass", 5, 5),
        ("fail", 0, 1),
        ("fail", 4, 5),
        ("blocked", 0, 7),
    ] {
        let mut document = base(status);
        document["weight"] = json!({"num": num, "den": den});
        assert!(
            schema_accepts(&document) && rust_accepts(&document),
            "{status} {num}/{den}"
        );
    }
}

#[test]
fn outcome_the_schema_bounds_an_integer_only_by_its_type_and_the_rust_reader_by_64_bits() {
    let past: serde_json::Value =
        serde_json::from_str("18446744073709551616").expect("a number past u64");
    for (label, edit) in [
        ("count", json!({"value": {"kind": "count", "count": past}})),
        (
            "fraction num",
            json!({"value": {"kind": "fraction", "num": past, "den": 2}}),
        ),
        (
            "fraction den",
            json!({"value": {"kind": "fraction", "num": 1, "den": past}}),
        ),
        (
            "counts entry",
            json!({"value": {"kind": "counts", "counts": {"a": past}}}),
        ),
        ("weight num", json!({"weight": {"num": past, "den": past}})),
        ("weight den", json!({"weight": {"num": 2, "den": past}})),
    ] {
        let mut document = base("pass");
        for (key, value) in edit.as_object().expect("an object") {
            document[key] = value.clone();
        }
        assert!(
            schema_accepts(&document),
            "the schema refuses {label} past 64 bits"
        );
        assert!(
            !rust_accepts(&document),
            "the reader accepts {label} past 64 bits"
        );
    }
}

#[test]
fn outcome_run_key_counters_past_32_bits_pass_the_schema_and_fail_the_reader() {
    for field in ["replicate", "attempt"] {
        let mut document = base("pass");
        document["run"][field] = json!(4_294_967_296_u64);
        assert!(
            schema_accepts(&document),
            "the schema refuses a {field} past 32 bits"
        );
        assert!(
            !rust_accepts(&document),
            "the reader accepts a {field} past 32 bits"
        );
        document["run"][field] = json!(u32::MAX);
        assert!(schema_accepts(&document) && rust_accepts(&document));
    }
}

#[test]
fn outcome_a_probability_of_exactly_zero_and_one_is_valid_for_both_readers() {
    for p in [0.0, 1.0] {
        let mut document = base("pass");
        document["probability"] = json!({"p": p, "calibration_ref": "c"});
        assert!(schema_accepts(&document));
        assert!(rust_accepts(&document));
    }
}

#[test]
fn outcome_both_readers_refuse_a_struct_written_as_a_list() {
    let lists = [
        ("run", json!([good_digest(), 2, 1])),
        ("weight", json!([2, 2])),
        ("probability", json!([0.5, "c"])),
        ("evidence", json!([["event:s#0", "n"]])),
        ("instruments", json!([["i", "1.0.0", good_digest(), false]])),
        ("value", json!(["fraction", 1, 2])),
        ("value", json!({"kind": "fraction", "num": [1], "den": 2})),
    ];
    for (field, list) in lists {
        let mut document = base("pass");
        document[field] = list;
        assert!(
            !schema_accepts(&document),
            "the schema accepts a {field} list"
        );
        assert!(
            !rust_accepts(&document),
            "the reader accepts a {field} list"
        );
    }
    let whole = json!([base("pass")]);
    assert!(!schema_accepts(&whole));
    assert!(!rust_accepts(&whole));
    let record = base("pass");
    let fields: Vec<serde_json::Value> = record
        .as_object()
        .expect("an object")
        .values()
        .cloned()
        .collect();
    let list = serde_json::Value::Array(fields);
    assert!(
        !schema_accepts(&list),
        "the schema accepts a record written as a list"
    );
    assert!(
        !rust_accepts(&list),
        "the reader accepts a record written as a list"
    );
}

#[test]
fn outcome_a_built_value_cannot_be_mutated_into_an_invalid_record() {
    let build = |value: Value| {
        let cell = Digest::from_hex(&"a".repeat(64)).expect("a digest");
        OutcomeRecord::builder()
            .run(RunKey::new(cell, 1, 1).expect("a run key"))
            .expectation_id("A1".parse().expect("an id"))
            .role(Role::Descriptive)
            .evaluator("e@1.0.0".parse().expect("name@version"))
            .suite_entry_digest(Digest::from_hex(&"b".repeat(64)).expect("a digest"))
            .context_digest(Digest::from_hex(&"c".repeat(64)).expect("a digest"))
            .request_id(
                "01926b3a-7c3e-7a1b-8c4d-5e6f7a8b9c0d"
                    .parse()
                    .expect("a request id"),
            )
            .status(Status::Pass)
            .value(value)
            .build()
            .expect("a valid record")
    };
    let originals = [
        Value::number(1.5).expect("finite"),
        Value::count(1),
        Value::fraction(1, 2).expect("a fraction"),
        Value::text("a".into()),
        Value::counts(std::collections::BTreeMap::new()),
    ];
    // The payload of a number is replaced only by another `Finite`; no non-finite one exists.
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(Finite::new(bad).is_err());
    }
    let goods = [-0.0, f64::MAX, f64::from_bits(1)];
    for original in &originals {
        for good in goods {
            let mut value = original.clone();
            match &mut value {
                Value::Number { number, .. } => *number = Finite::new(good).expect("finite"),
                Value::Count { count, .. } => *count = u64::MAX,
                Value::Text { text, .. } => *text = "\"\\\u{0}".into(),
                Value::Counts { counts, .. } => {
                    counts.insert("a\"".into(), u64::MAX);
                    counts.insert(String::new(), 0);
                }
                Value::Fraction(fraction) => {
                    *fraction = Fraction::new(u64::MAX, u64::MAX).expect("a fraction");
                }
                _ => {
                    // EXPLICIT: any other variant is covered by the generated records
                }
            }
            check_value_record(&build(value));
        }
    }
}

/// A record with a mutated value still satisfies the schema, writes no `null` and reads back.
fn check_value_record(record: &OutcomeRecord) {
    let document = serde_json::to_value(record).expect("a record serialises");
    assert!(schema_accepts(&document), "the schema refuses {document}");
    assert!(!document.to_string().contains("null"), "{document}");
    let back: OutcomeRecord = serde_json::from_value(document).expect("reads back");
    assert_eq!(&back, record);
    assert!(back.validate().is_ok());
}

#[test]
fn outcome_the_schema_cannot_order_the_two_lines_of_a_range_and_the_rust_reader_does() {
    for reference in ["path:a#L3-2", "path:a#L10-9"] {
        let mut document = base("pass");
        document["evidence"] = json!([{"ref": reference}]);
        assert!(schema_accepts(&document), "the schema refuses {reference}");
        assert!(!rust_accepts(&document), "the reader accepts {reference}");
    }
}
