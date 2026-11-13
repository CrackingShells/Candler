#![cfg(test)]
//! Guards for outcome selection and the rates: the I4 text decides what is flagged, and the
//! `_outcome.py` formula, as R03 transcribes it, decides the sums. Every expected number is a
//! literal.

use candler_core::evaluation::selection::rates::{rates, rates_toml};
use candler_core::evaluation::selection::select::select;
use candler_schemas::outcome::{Cause, Evidence, OutcomeRecord, Role, Status, Value, Weight};
use candler_schemas::primitives::{Digest, NameVersion, RequestId, RunKey};

const EVALUATOR: &str = "live-eval@0.5.0";

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

fn run(cell: char, replicate: u32) -> RunKey {
    format!("{}-r{replicate}-a1", String::from(cell).repeat(64))
        .parse()
        .expect("a run key")
}

fn reason(code: &str) -> Cause {
    Cause::Reason(code.parse().expect("a reason code"))
}

/// A record under the key (`EVALUATOR`, cell `a` run 1, `E1`, digests `c`/`d`), request `n`.
fn keyed(n: u64, status: Status) -> candler_schemas::outcome::OutcomeRecordBuilder {
    OutcomeRecord::builder()
        .run(run('a', 1))
        .expectation_id("E1".parse().expect("an expectation id"))
        .role(Role::Gate)
        .evaluator(EVALUATOR.parse::<NameVersion>().expect("an evaluator"))
        .suite_entry_digest(digest('d'))
        .context_digest(digest('c'))
        .request_id(request(n))
        .status(status)
}

fn built(builder: candler_schemas::outcome::OutcomeRecordBuilder) -> OutcomeRecord {
    builder.build().expect("a valid record")
}

fn instrument_error() -> Cause {
    reason("instrument_error")
}

/// Guards: R5 — two records under one I4 key that differ in status, value, weight or cause are an
/// `I4Violation` (the later request id second, whatever the input order); an error outcome
/// followed by a measured one is a recovery and is not flagged; evidence, a default weight spelled
/// out and another context digest are not violations. Oracle: the I4 text of R03 (§I4).
#[test]
fn sel1_a_changed_field_under_one_key_is_flagged_and_recovery_is_not() {
    let base = || keyed(1, Status::Pass).value(Value::count(3));
    let later = |builder: candler_schemas::outcome::OutcomeRecordBuilder| {
        built(builder.request_id(request(2)))
    };

    // Each of the four fields, changed in the later record.
    let changed = [
        (
            "status",
            later(keyed(2, Status::Fail).value(Value::count(3))),
        ),
        (
            "value",
            later(keyed(2, Status::Pass).value(Value::count(4))),
        ),
        (
            "weight",
            later(
                keyed(2, Status::Pass)
                    .value(Value::count(3))
                    .weight(Weight::new(2, 2).expect("a weight")),
            ),
        ),
        (
            "cause",
            later(
                keyed(2, Status::Blocked)
                    .value(Value::count(3))
                    .cause(reason("some_reason")),
            ),
        ),
    ];
    for (field, second) in changed {
        let first = built(base());
        // The later record is listed first: request ids decide the order, not the input.
        let selection = select([second.clone(), first.clone()]);
        let violations = selection.violations();
        assert_eq!(violations.len(), 1, "{field}: one violation");
        let violation = violations.first().expect("one violation");
        assert_eq!(violation.first, first, "{field}");
        assert_eq!(violation.second, second, "{field}");
        assert_eq!(violation.key.expectation.as_str(), "E1", "{field}");
        assert_eq!(selection.current().count(), 1, "{field}");
    }

    // Not violations: the same fields with other evidence, a default weight written out, and the
    // same request twice with a repeated record.
    let noted = built(
        base()
            .request_id(request(2))
            .weight(Weight::new(1, 1).expect("a weight"))
            .evidence(Evidence::new(None, Some("another reading".into())).expect("evidence")),
    );
    let selection = select([built(base()), noted.clone()]);
    assert!(
        selection.violations().is_empty(),
        "evidence and default weight are not compared"
    );
    assert_eq!(
        selection.current().next(),
        Some(&noted),
        "the latest record is current"
    );

    // Another context digest is another key, so a different outcome there is no violation.
    let elsewhere = built(keyed(2, Status::Fail).context_digest(digest('e')));
    let selection = select([built(base()), elsewhere]);
    assert!(
        selection.violations().is_empty(),
        "another context digest is another key"
    );
    assert_eq!(selection.current().count(), 2);

    // Recovery: an error outcome, then a measured one. The later record is current.
    for cause in [instrument_error(), reason("deadline")] {
        let error = built(keyed(1, Status::NotMeasured).cause(cause));
        let measured = built(keyed(2, Status::Pass).value(Value::count(3)));
        let selection = select([error, measured.clone()]);
        assert!(
            selection.violations().is_empty(),
            "error then measured is a recovery"
        );
        assert_eq!(selection.current().next(), Some(&measured));
    }

    // The reverse is a change: a measured outcome followed by an error one, and a not_measured
    // whose cause is not an error code followed by a measured one.
    let measured = built(keyed(1, Status::Pass).value(Value::count(3)));
    let error = built(keyed(2, Status::NotMeasured).cause(instrument_error()));
    assert_eq!(select([measured, error]).violations().len(), 1);
    let other = built(keyed(1, Status::NotMeasured).cause(reason("needs_review")));
    let measured = built(keyed(2, Status::Pass));
    assert_eq!(select([other, measured]).violations().len(), 1);
}

/// A cell, its gate pair and its quality pair.
type CellRow = (String, (u64, u64), (u64, u64));

/// One record of the RT1 table.
fn rate_record(
    cell: char,
    replicate: u32,
    expectation: &str,
    role: Role,
    status: Status,
    weight: Option<(u64, u64)>,
    n: u64,
) -> OutcomeRecord {
    let mut builder = OutcomeRecord::builder()
        .run(run(cell, replicate))
        .expectation_id(expectation.parse().expect("an expectation id"))
        .role(role)
        .evaluator(EVALUATOR.parse::<NameVersion>().expect("an evaluator"))
        .suite_entry_digest(digest('d'))
        .context_digest(digest('c'))
        .request_id(request(n))
        .status(status);
    if let Some((num, den)) = weight {
        builder = builder.weight(Weight::new(num, den).expect("a weight"));
    }
    builder = match status {
        Status::Blocked => builder.cause(Cause::Artifact("outputs/x".parse().expect("a path"))),
        Status::NotMeasured => builder.cause(instrument_error()),
        Status::NotApplicable => builder.cause(reason("empty_population")),
        Status::Pass | Status::Fail => builder,
    };
    built(builder)
}

/// Guards: R1 — per cell the gate and quality pairs are the sums of the effective weights of the
/// current pass, fail and blocked records by their own role; `blocked` counts with numerator 0;
/// `not_applicable`, `not_measured`, descriptive records, a 0/0 pass and another evaluator's records
/// add nothing, and the TOML is `gate = [num, den]` and `quality = [num, den]` per cell. Oracle:
/// the `_outcome.py` formula as R03 §Rates states it, summed by hand.
#[test]
fn rt1_three_hand_computed_cells() {
    use Role::{Descriptive, Gate, Quality};
    use Status::{Blocked, Fail, NotApplicable, NotMeasured, Pass};
    let mut records = vec![
        // Cell a: gate 1/1 + 0/1 (fail) + 0/1 (blocked) = 1/3; quality 2/2 + 1/3 = 3/5.
        rate_record('a', 1, "G1", Gate, Pass, None, 1),
        rate_record('a', 1, "G2", Gate, Fail, None, 2),
        rate_record('a', 1, "G3", Gate, Blocked, None, 3),
        rate_record('a', 1, "Q1", Quality, Pass, Some((2, 2)), 4),
        rate_record('a', 1, "Q2", Quality, Fail, Some((1, 3)), 5),
        // Cell a, adding nothing: not applicable, not measured, descriptive, a 0/0 pass.
        rate_record('a', 1, "G4", Gate, NotApplicable, None, 6),
        rate_record('a', 1, "Q3", Quality, NotMeasured, None, 7),
        rate_record('a', 1, "D1", Descriptive, Pass, None, 8),
    ];
    let zero = OutcomeRecord::builder()
        .run(run('a', 1))
        .expectation_id("G5".parse().expect("an expectation id"))
        .role(Gate)
        .evaluator(EVALUATOR.parse::<NameVersion>().expect("an evaluator"))
        .suite_entry_digest(digest('d'))
        .context_digest(digest('c'))
        .request_id(request(9))
        .status(Pass)
        .weight(Weight::new(0, 0).expect("a weight"))
        .cause({
            #[expect(deprecated, reason = "the guard builds a `Legacy` cause")]
            Cause::Legacy("zero_weight".parse().expect("a reason code"))
        });
    records.push(built(zero));
    // Another evaluator's gate pass in cell a is not this evaluator's.
    records.push(built(
        OutcomeRecord::builder()
            .run(run('a', 1))
            .expectation_id("G1".parse().expect("an expectation id"))
            .role(Gate)
            .evaluator(
                "other-eval@1.0.0"
                    .parse::<NameVersion>()
                    .expect("an evaluator"),
            )
            .suite_entry_digest(digest('d'))
            .context_digest(digest('c'))
            .request_id(request(10))
            .status(Pass),
    ));
    // Cell b, two runs: gate 1/1 + 1/1 = 2/2; quality blocked at 0/4 plus a not applicable = 0/4.
    records.push(rate_record('b', 1, "G1", Gate, Pass, None, 11));
    records.push(rate_record('b', 2, "G1", Gate, Pass, None, 12));
    records.push(rate_record(
        'b',
        1,
        "Q1",
        Quality,
        Blocked,
        Some((0, 4)),
        13,
    ));
    records.push(rate_record('b', 2, "Q1", Quality, NotApplicable, None, 14));
    // Cell c: nothing counts, so both pairs are 0/0.
    records.push(rate_record('c', 1, "G1", Gate, NotApplicable, None, 15));
    records.push(rate_record('c', 1, "Q1", Quality, NotMeasured, None, 16));

    let selection = select(records);
    let evaluator: NameVersion = EVALUATOR.parse().expect("an evaluator");
    let table = rates(&selection, &evaluator);
    let by_cell: Vec<CellRow> = table
        .iter()
        .map(|(cell, rate)| (cell.to_string(), rate.gate(), rate.quality()))
        .collect();
    assert_eq!(
        by_cell,
        [
            ("a".repeat(64), (1, 3), (3, 5)),
            ("b".repeat(64), (2, 2), (0, 4)),
            ("c".repeat(64), (0, 0), (0, 0)),
        ]
    );

    for cell in table.keys() {
        let text = cell.to_string();
        assert_eq!(text.len(), 64, "bare hex, no sha256: prefix");
        assert_eq!(&Digest::from_hex(&text).expect("bare hex"), cell.digest());
    }

    let expected = format!(
        "[cell.\"{a}\"]\ngate = [1, 3]\nquality = [3, 5]\n\n[cell.\"{b}\"]\ngate = [2, 2]\nquality = [0, 4]\n\n[cell.\"{c}\"]\ngate = [0, 0]\nquality = [0, 0]\n",
        a = "a".repeat(64),
        b = "b".repeat(64),
        c = "c".repeat(64)
    );
    assert_eq!(rates_toml(&table), expected);
}
