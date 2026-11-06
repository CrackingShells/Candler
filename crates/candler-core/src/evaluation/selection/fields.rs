//! The four fields of an outcome that selection and the diff compare: status, value, weight and cause.

use candler_schemas::outcome::{Cause, OutcomeRecord, Status, Value, Weight};
use serde::{Deserialize, Serialize};

/// The `(status, value, weight, cause)` of one outcome, with the weight normalised: a weight equal
/// to the default of its status is absent. Evidence, probability and instruments are never part of
/// it. An absent key is an absent field, so two records are equal only when every field is.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct FourFields {
    /// The status.
    pub status: Status,
    /// What was measured, if the record states a value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// The weight, absent when it is the default of the status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<Weight>,
    /// Why the outcome has its status, if the record states a cause.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<Cause>,
}

/// The weight a status carries when the record states none: 1/1 for `pass`, 0/1 for `fail` and
/// `blocked`, and none for the two statuses that never enter a rate.
const fn default_weight(status: Status) -> Option<Weight> {
    match status {
        Status::Pass => Some(Weight::PASS),
        Status::Fail | Status::Blocked => Some(Weight::FAIL),
        Status::NotMeasured | Status::NotApplicable => None,
    }
}

impl FourFields {
    /// The four fields of `record`, with the weight normalised.
    #[must_use]
    pub fn of(record: &OutcomeRecord) -> Self {
        Self {
            status: record.status(),
            // BORROW: the fields own copies of the record's value and cause
            value: record.value().cloned(),
            weight: record.weight(),
            cause: record.cause().cloned(),
        }
        .normalised()
    }

    /// The same fields with a weight equal to the default of the status removed.
    #[must_use]
    pub fn normalised(mut self) -> Self {
        if self.weight == default_weight(self.status) {
            self.weight = None;
        }
        self
    }

    /// Reports whether the two values are the same: equal, of the same kind, and for numbers equal in bits.
    #[must_use]
    pub fn value_equals(&self, other: &Self) -> bool {
        same_value(self.value.as_ref(), other.value.as_ref())
    }

    /// Reports whether the status is an error outcome: `not_measured` with the cause
    /// `instrument_error` or `deadline`.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.status == Status::NotMeasured
            && matches!(
                &self.cause,
                Some(Cause::Reason(code))
                    // BORROW: explicit .as_str() to compare with the reserved codes
                    if matches!(code.as_str(), "instrument_error" | "deadline")
            )
    }
}

/// Reports whether two optional values are the same JSON number kind and bits, or equal otherwise:
/// a count `3` is not a number `3.0`, and `-0.0` is not `0.0`.
fn same_value(left: Option<&Value>, right: Option<&Value>) -> bool {
    if let (Some(Value::Number { number: a, .. }), Some(Value::Number { number: b, .. })) =
        (left, right)
    {
        a.get().to_bits() == b.get().to_bits()
    } else {
        left == right
    }
}

impl PartialEq for FourFields {
    fn eq(&self, other: &Self) -> bool {
        self.status == other.status
            && self.value_equals(other)
            && self.weight == other.weight
            && self.cause == other.cause
    }
}
