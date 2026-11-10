#![cfg(test)]
//! Guards for the diff of two evaluator versions: each class of change is seen, and only the rows
//! an equivalence file lists, showing exactly its pair in its direction, are excused. Every
//! expected number is a literal written from the rule text, never computed by the diff.

use std::collections::BTreeMap;
use std::path::Path;

use candler_core::evaluation::selection::diff::{
    ChangeClass, DiffReport, Equivalence, EquivalenceError, diff,
};
use candler_core::evaluation::selection::select::{Selection, select};
use candler_schemas::outcome::{Cause, Evidence, OutcomeRecord, Role, Status, Value};
use candler_schemas::primitives::{Digest, NameVersion, RequestId, RunKey};
use serde_json::json;

const FROM: &str = "legacy-eval@0.4.2";
const TO: &str = "live-eval@0.5.0";
const RUN1: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-r1-a1";
const RUN2: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-r2-a1";
/// The sha256 of `fixtures/selection/equivalence.json`, from `shasum -a 256` on the file.
const FIXTURE_SHA256: &str = "d426cddf814aaa7c0ac6c99134df8e69d84237e96bba7f693bcb3371c2d8d07f";

/// The four compared fields of a record, and a note that is evidence and so never compared.
#[derive(Clone)]
struct Spec {
    status: Status,
    value: Option<Value>,
    weight: Option<(u64, u64)>,
    cause: Option<Cause>,
    note: Option<&'static str>,
}

impl Spec {
    fn new(status: Status) -> Self {
        Self {
            status,
            value: None,
            weight: None,
            cause: None,
            note: None,
        }
    }
    fn value(mut self, value: Value) -> Self {
        self.value = Some(value);
        self
    }
    fn weight(mut self, num: u64, den: u64) -> Self {
        self.weight = Some((num, den));
        self
    }
    fn cause(mut self, cause: Cause) -> Self {
        self.cause = Some(cause);
        self
    }
    fn note(mut self, note: &'static str) -> Self {
        self.note = Some(note);
        self
    }
}

fn reason(code: &str) -> Cause {
    Cause::Reason(code.parse().expect("a reason code"))
}

#[expect(deprecated, reason = "the guard builds `Legacy` causes")]
fn legacy(code: &str) -> Cause {
    Cause::Legacy(code.parse().expect("a reason code"))
}

fn digest(c: char) -> Digest {
    format!("sha256:{}", String::from(c).repeat(64))
        .parse()
        .expect("a digest")
}

fn request(n: u64) -> RequestId {
    format!("01926b3a-7c3e-7a1b-8c4d-{n:012x}")
        .parse()
        .expect("a request id")
}

fn record(evaluator: &str, run: &str, expectation: &str, spec: &Spec) -> OutcomeRecord {
    let mut builder = OutcomeRecord::builder()
        .run(run.parse::<RunKey>().expect("a run key"))
        .expectation_id(expectation.parse().expect("an expectation id"))
        .role(Role::Gate)
        .evaluator(evaluator.parse::<NameVersion>().expect("an evaluator"))
        .suite_entry_digest(digest('d'))
        .context_digest(digest('c'))
        .request_id(request(1))
        .status(spec.status);
    if let Some(value) = &spec.value {
        builder = builder.value(value.clone());
    }
    if let Some((num, den)) = spec.weight {
        builder =
            builder.weight(candler_schemas::outcome::Weight::new(num, den).expect("a weight"));
    }
    if let Some(cause) = &spec.cause {
        builder = builder.cause(cause.clone());
    }
    if let Some(note) = spec.note {
        builder = builder.evidence(Evidence::new(None, Some(note.into())).expect("evidence"));
    }
    builder.build().expect("a valid record")
}

fn selection(evaluator: &str, rows: &[(&str, &str, Spec)]) -> Selection {
    select(
        rows.iter()
            .map(|(run, expectation, spec)| record(evaluator, run, expectation, spec)),
    )
}

fn pass() -> Spec {
    Spec::new(Status::Pass)
}

/// One planted change: the subject row `E1` of run 1 on each side, `None` for absent.
struct Case {
    name: &'static str,
    from: Option<Spec>,
    to: Option<Spec>,
    expected: Option<ChangeClass>,
}

/// The base: `E0` on both runs, and `E1` on run 2, identical on both sides, plus the subject.
fn base(evaluator: &str, subject: Option<&Spec>) -> Selection {
    let mut rows = vec![
        (RUN1, "E0", pass()),
        (RUN2, "E0", pass()),
        (RUN2, "E1", pass()),
    ];
    if let Some(spec) = subject {
        rows.push((RUN1, "E1", spec.clone()));
    }
    selection(evaluator, &rows)
}

/// Guards: R2 — the diff names each class of change in `(status, value, weight, cause)` and
/// both kinds of absent row, sees nothing in evidence or in a default weight spelled out, and a
/// base that diffs clean is the positive control of every row. Oracle: hand literals from the
/// rule text (`3` is not `3.0`; a missing row and an extra row are changes).
#[test]
fn df1_every_planted_class_is_seen_and_evidence_is_not() {
    let artifact = Cause::Artifact("outputs/plan.json".parse().expect("a path"));
    let expectation = Cause::Expectation("E0".parse().expect("an expectation id"));
    let three = Value::count(3);
    let cases = [
        Case {
            name: "status",
            from: Some(pass()),
            to: Some(Spec::new(Status::Fail)),
            expected: Some(ChangeClass::Status),
        },
        Case {
            name: "value 3 -> 4",
            from: Some(pass().value(three.clone())),
            to: Some(pass().value(Value::count(4))),
            expected: Some(ChangeClass::Value),
        },
        Case {
            name: "value 3 -> 3.0",
            from: Some(pass().value(three)),
            to: Some(pass().value(Value::number(3.0).expect("a number"))),
            expected: Some(ChangeClass::Value),
        },
        Case {
            name: "weight",
            from: Some(pass().weight(2, 2)),
            to: Some(pass().weight(3, 3)),
            expected: Some(ChangeClass::Weight),
        },
        Case {
            name: "cause kind",
            from: Some(Spec::new(Status::Blocked).cause(artifact)),
            to: Some(Spec::new(Status::Blocked).cause(expectation)),
            expected: Some(ChangeClass::Cause),
        },
        Case {
            name: "cause present -> absent",
            from: Some(pass().cause(legacy("note"))),
            to: Some(pass()),
            expected: Some(ChangeClass::Cause),
        },
        Case {
            name: "missing row",
            from: Some(pass()),
            to: None,
            expected: Some(ChangeClass::Missing),
        },
        Case {
            name: "extra row",
            from: None,
            to: Some(pass()),
            expected: Some(ChangeClass::Extra),
        },
        Case {
            name: "evidence only",
            from: Some(pass().note("first reading")),
            to: Some(pass().note("a second, different reading")),
            expected: None,
        },
        Case {
            name: "default weight spelled out",
            from: Some(pass()),
            to: Some(pass().weight(1, 1)),
            expected: None,
        },
    ];
    for case in &cases {
        let same = case.from.clone().or_else(|| case.to.clone());
        let control = diff(&base(FROM, same.as_ref()), &base(TO, same.as_ref()), None);
        assert_eq!(
            (control.compared, control.unchanged, control.changed),
            (4, 4, 0),
            "{}: the base must diff clean",
            case.name
        );
        assert!(control.changes.is_empty(), "{}", case.name);
        assert!(
            serde_json::to_value(&control).expect("json")["equivalence"].is_null(),
            "{}: no equivalence file is null in the report",
            case.name
        );

        let report = diff(
            &base(FROM, case.from.as_ref()),
            &base(TO, case.to.as_ref()),
            None,
        );
        assert_eq!(report.compared, 4, "{}", case.name);
        assert_eq!(
            report.unchanged + report.excused + report.changed,
            report.compared,
            "{}",
            case.name
        );
        assert_eq!(report.excused, 0, "{}", case.name);
        match case.expected {
            Some(class) => {
                assert_eq!(
                    (report.changed, report.unchanged),
                    (1, 3),
                    "{}: one row changed",
                    case.name
                );
                let change = report.changes.first().expect("one change");
                assert_eq!(change.class, class, "{}", case.name);
                assert_eq!(change.run.to_string(), RUN1, "{}", case.name);
                assert_eq!(change.expectation.as_str(), "E1", "{}", case.name);
                assert_eq!(change.from.is_none(), case.from.is_none(), "{}", case.name);
                assert_eq!(change.to.is_none(), case.to.is_none(), "{}", case.name);
            }
            None => {
                assert_eq!(
                    (report.changed, report.unchanged),
                    (0, 4),
                    "{}: nothing changed",
                    case.name
                );
            }
        }
    }
}

/// A pass at weight 0/0 with the legacy cause `zero_weight`: the rule's `from`.
fn zero_weight_pass() -> Spec {
    pass().weight(0, 0).cause(legacy("zero_weight"))
}

/// A not applicable with the reason `empty_population`: the rule's `to`.
fn empty_population() -> Spec {
    Spec::new(Status::NotApplicable).cause(reason("empty_population"))
}

fn fixture() -> Equivalence {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/selection/equivalence.json");
    Equivalence::load(&path).expect("the fixture loads")
}

/// The `diff` of the planted table of rows against the fixture file; `Z1` is the exact pair.
fn planted() -> (Selection, Selection) {
    let from = vec![
        (RUN1, "Z1", zero_weight_pass()),
        (RUN1, "Z2", zero_weight_pass()),
        (RUN1, "Z3", pass()),
        (RUN1, "Z4", pass().weight(0, 0).cause(legacy("other_code"))),
        (RUN1, "Z5", empty_population()),
        (RUN1, "Z6", zero_weight_pass()),
        (RUN1, "Z7", zero_weight_pass()),
        (RUN2, "Z7", pass()),
        (RUN2, "Z8", pass()),
        (RUN1, "Z9", zero_weight_pass()),
    ];
    let to = vec![
        (RUN1, "Z1", empty_population()),
        (RUN1, "Z2", empty_population()),
        (RUN1, "Z3", empty_population()),
        (RUN1, "Z4", empty_population()),
        (RUN1, "Z5", zero_weight_pass()),
        (RUN1, "Z6", zero_weight_pass()),
        (RUN2, "Z7", pass()),
        (RUN1, "Z8", zero_weight_pass()),
        (RUN2, "Z8", pass()),
        (
            RUN1,
            "Z9",
            Spec::new(Status::NotApplicable).cause(reason("other_reason")),
        ),
    ];
    (selection(FROM, &from), selection(TO, &to))
}

fn classes_by_expectation(report: &DiffReport) -> BTreeMap<String, ChangeClass> {
    report
        .changes
        .iter()
        .map(|change| (change.expectation.to_string(), change.class))
        .collect()
}

/// Guards: R2 — only a listed row that shows exactly `pass 0/0 {legacy: zero_weight}` then
/// `not_applicable {empty_population}` is excused, counted apart from unchanged, with the file's
/// sha256 recorded; the same change unlisted, a 1/1 pass, another legacy cause, another reason,
/// the reverse direction, an unchanged row and an absent side are each a change with their class.
/// Oracle: the hand-written fixture `equivalence.json` and its `shasum -a 256`.
#[test]
fn df2_only_the_exact_listed_pair_is_excused() {
    let (from, to) = planted();
    let equivalence = fixture();
    let report = diff(&from, &to, Some(&equivalence));
    // Z1 excused; Z7 and Z8 on run 2 unchanged; the other eight rows changed.
    assert_eq!(report.compared, 11);
    assert_eq!(report.excused, 1);
    assert_eq!(report.unchanged, 2);
    assert_eq!(report.changed, 8);
    let expected: BTreeMap<String, ChangeClass> = [
        ("Z2", ChangeClass::Status),
        ("Z3", ChangeClass::EquivalenceMismatch),
        ("Z4", ChangeClass::EquivalenceMismatch),
        ("Z5", ChangeClass::EquivalenceMismatch),
        ("Z6", ChangeClass::EquivalenceMismatch),
        ("Z7", ChangeClass::EquivalenceMissing),
        ("Z8", ChangeClass::EquivalenceMissing),
        ("Z9", ChangeClass::EquivalenceMismatch),
    ]
    .into_iter()
    .map(|(id, class)| (id.to_owned(), class))
    .collect();
    assert_eq!(classes_by_expectation(&report), expected);

    let json = serde_json::to_value(&report).expect("json");
    assert_eq!(
        json["equivalence"],
        json!({"sha256": FIXTURE_SHA256, "rows": 8})
    );
    let keys: Vec<&str> = json
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        [
            "changed",
            "changes",
            "compared",
            "equivalence",
            "excused",
            "unchanged",
            "uncompared"
        ]
    );
    assert_eq!(equivalence.sha256(), FIXTURE_SHA256);

    // Without the file, the one excused row is an ordinary status change.
    let plain = diff(&from, &to, None);
    assert_eq!(plain.excused, 0);
    assert_eq!(
        classes_by_expectation(&plain).get("Z1"),
        Some(&ChangeClass::Status)
    );
}

fn refused(text: &str) -> EquivalenceError {
    Equivalence::from_bytes(text.as_bytes()).expect_err("the file is refused")
}

/// Guards: R2 — an equivalence file with an unknown key anywhere, or a row listed twice (in one
/// rule or in two), is refused rather than read loosely. Oracle: the format's text, as inline
/// literals each one edit away from a valid file.
#[test]
fn df2_unknown_keys_and_duplicate_rows_are_refused() {
    let row = format!(r#"{{"run": "{RUN1}", "expectation": "Z1"}}"#);
    let side = r#"{"status": "pass"}"#;
    let valid =
        format!(r#"{{"rules": [{{"name": "n", "from": {side}, "to": {side}, "rows": [{row}]}}]}}"#);
    assert!(
        Equivalence::from_bytes(valid.as_bytes()).is_ok(),
        "the control file loads"
    );

    let unknown = [
        r#"{"extra": 1, "rules": []}"#.to_owned(),
        format!(
            r#"{{"rules": [{{"name": "n", "extra": 1, "from": {side}, "to": {side}, "rows": []}}]}}"#
        ),
        format!(
            r#"{{"rules": [{{"name": "n", "from": {{"status": "pass", "extra": 1}}, "to": {side}, "rows": []}}]}}"#
        ),
        format!(
            r#"{{"rules": [{{"name": "n", "from": {side}, "to": {side}, "rows": [{{"run": "{RUN1}", "expectation": "Z1", "extra": 1}}]}}]}}"#
        ),
    ];
    for text in &unknown {
        assert!(
            matches!(refused(text), EquivalenceError::Parse { .. }),
            "unknown key accepted: {text}"
        );
    }
    let twice_in_one = format!(
        r#"{{"rules": [{{"name": "n", "from": {side}, "to": {side}, "rows": [{row}, {row}]}}]}}"#
    );
    let twice_in_two = format!(
        r#"{{"rules": [{{"name": "a", "from": {side}, "to": {side}, "rows": [{row}]}}, {{"name": "b", "from": {side}, "to": {side}, "rows": [{row}]}}]}}"#
    );
    for text in [&twice_in_one, &twice_in_two] {
        assert!(
            matches!(refused(text), EquivalenceError::DuplicateRow { .. }),
            "duplicate row accepted: {text}"
        );
    }
}

/// Guards: R2 — an expectation only one suite holds is left out of the comparison and listed with
/// its row count, and a `to` with no records compares nothing: `compared == 0`, never "no change".
/// Oracle: hand-counted rows of the planted selections.
#[test]
fn df2_uncompared_rows_are_listed_with_counts_and_nothing_compared_is_zero() {
    // U1 is in `from` only on two runs; U2 is in `to` only on one run; E0 is in both.
    let from = selection(
        FROM,
        &[
            (RUN1, "E0", pass()),
            (RUN1, "U1", pass()),
            (RUN2, "U1", pass()),
        ],
    );
    let to = selection(TO, &[(RUN1, "E0", pass()), (RUN1, "U2", pass())]);
    let report = diff(&from, &to, None);
    assert_eq!(
        (report.compared, report.unchanged, report.changed),
        (1, 1, 0)
    );
    let listed: Vec<(String, u64)> = report
        .uncompared
        .iter()
        .map(|u| (u.expectation.to_string(), u.count))
        .collect();
    assert_eq!(listed, [("U1".to_owned(), 2), ("U2".to_owned(), 1)]);

    let empty = diff(&from, &Selection::default(), None);
    assert_eq!((empty.compared, empty.unchanged, empty.changed), (0, 0, 0));
    let listed: Vec<(String, u64)> = empty
        .uncompared
        .iter()
        .map(|u| (u.expectation.to_string(), u.count))
        .collect();
    assert_eq!(listed, [("E0".to_owned(), 1), ("U1".to_owned(), 2)]);
}
