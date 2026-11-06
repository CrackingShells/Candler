//! Accounting for every item of a request: each one ends with exactly one outcome.
//!
//! An item the evaluator reported keeps its record. An item it did not report is
//! `not_measured`, with the reason `deadline` when the request deadline fired and
//! `instrument_error` otherwise. The client cannot write the record of such an item: the role, the
//! evaluator and the suite entry digest are the engine's, so the client names the item, the reason
//! and the evidence in an [`Unreported`] and the engine writes the record.

use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::ExpectationId;

/// The code that marks an item the request deadline cut off.
const REASON_DEADLINE: &str = "deadline";

/// The code that marks an item the evaluator failed to report.
const REASON_INSTRUMENT_ERROR: &str = "instrument_error";

/// Why an item has no record from the evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UnreportedReason {
    /// The request deadline fired before the evaluator reported the item. This is the only path
    /// that writes `deadline`.
    Deadline,
    /// The evaluator ended, failed or broke the protocol before reporting the item.
    InstrumentError,
}

impl UnreportedReason {
    /// The reason code of the `not_measured` record's cause `{reason: <code>}`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Deadline => REASON_DEADLINE,
            Self::InstrumentError => REASON_INSTRUMENT_ERROR,
        }
    }
}

/// An item that no record answers, with what the client can say about why.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Unreported {
    /// The item.
    pub expectation_id: ExpectationId,
    /// Why there is no record.
    pub reason: UnreportedReason,
    /// A sentence of evidence: the request deadline, or what the evaluator did instead.
    pub evidence: String,
}

/// The one outcome of one requested item.
#[derive(Debug, Clone, PartialEq)]
// EXHAUSTIVE: an item is either reported or not; a third state would be a change of the model
pub enum ItemOutcome {
    /// The evaluator reported this record.
    Reported(Box<OutcomeRecord>),
    /// The evaluator did not report the item; the engine writes `not_measured`.
    Unreported(Unreported),
}

impl ItemOutcome {
    /// The item this outcome answers.
    #[must_use]
    pub fn expectation_id<'a>(&'a self) -> &'a ExpectationId {
        match self {
            Self::Reported(record) => record.expectation_id(),
            Self::Unreported(missing) => &missing.expectation_id,
        }
    }
}

/// Why the items that are still unreported have no record, and the evidence for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Shortfall {
    pub(crate) reason: UnreportedReason,
    pub(crate) evidence: String,
}

/// The outcomes of a request in item order, and the protocol faults met while matching them.
#[derive(Debug)]
pub(crate) struct Accounted {
    pub(crate) items: Vec<ItemOutcome>,
    pub(crate) violations: Vec<String>,
}

/// Matches `reported` to `requested` by item id and gives every item exactly one outcome.
///
/// The first record of an item wins. A record of an unknown item, and a second record of an item,
/// are dropped and named in the violations, so no item is answered twice. An unreported item takes `shortfall`; for `instrument_error` its evidence also names every
/// violation.
pub(crate) fn account_items(
    requested: &[ExpectationId],
    reported: Vec<OutcomeRecord>,
    shortfall: &Shortfall,
) -> Accounted {
    let mut slots: Vec<Option<OutcomeRecord>> = requested.iter().map(|_| None).collect();
    let mut violations = Vec::new();
    for record in reported {
        let place = requested
            .iter()
            .position(|id| id == record.expectation_id());
        match place.and_then(|at| slots.get_mut(at)) {
            None => violations.push(format!(
                "the evaluator reported unknown item {}",
                record.expectation_id()
            )),
            Some(slot) if slot.is_some() => violations.push(format!(
                "the evaluator reported item {} twice",
                record.expectation_id()
            )),
            Some(slot) => *slot = Some(record),
        }
    }
    let evidence =
        if shortfall.reason == UnreportedReason::InstrumentError && !violations.is_empty() {
            format!("{}; {}", shortfall.evidence, violations.join("; "))
        } else {
            // BORROW: .clone() gives each unreported item its own evidence
            shortfall.evidence.clone()
        };
    let items = requested
        .iter()
        .zip(slots)
        .map(|(id, slot)| match slot {
            Some(record) => ItemOutcome::Reported(Box::new(record)),
            None => ItemOutcome::Unreported(Unreported {
                // BORROW: .clone() keeps the id in the outcome
                expectation_id: id.clone(),
                reason: shortfall.reason,
                // BORROW: .clone() gives each unreported item its own evidence
                evidence: evidence.clone(),
            }),
        })
        .collect();
    Accounted { items, violations }
}
