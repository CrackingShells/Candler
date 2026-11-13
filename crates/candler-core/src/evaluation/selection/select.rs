//! The current outcome record per key, and the I4 check across requests.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use candler_schemas::outcome::{OutcomeRecord, Status};
use candler_schemas::primitives::{Digest, ExpectationId, NameVersion, RunKey};

use super::fields::FourFields;

/// The I4 key of an outcome: the evaluator, the run, the expectation, the digest of the context
/// the evaluator received and the digest of the suite entry it was given.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub struct I4Key {
    /// The evaluator, `name@version`.
    pub evaluator: NameVersion,
    /// The run that was evaluated.
    pub run: RunKey,
    /// The expectation that was evaluated.
    pub expectation: ExpectationId,
    /// The digest of the context the evaluator received.
    pub context_digest: Digest,
    /// The digest of the suite entry the evaluator was given.
    pub suite_entry_digest: Digest,
}

impl I4Key {
    /// The key of `record`.
    #[must_use]
    pub fn of(record: &OutcomeRecord) -> Self {
        Self {
            // BORROW: the key owns copies of the record's identifiers
            evaluator: record.evaluator().clone(),
            run: record.run().clone(), // BORROW: the key owns the run
            expectation: record.expectation_id().clone(), // BORROW: the key owns the expectation
            context_digest: record.context_digest().clone(), // BORROW: the key owns the digest
            suite_entry_digest: record.suite_entry_digest().clone(), // BORROW: the key owns the digest
        }
    }
}

/// Two records under one I4 key that differ in `status`, `value`, `weight` or `cause`, where the
/// earlier one is not an error outcome that the later one recovers from.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct I4Violation {
    /// The key both records share.
    pub key: I4Key,
    /// The earlier record, the current one when the later one arrived.
    pub first: OutcomeRecord,
    /// The later record, by request id.
    pub second: OutcomeRecord,
}

/// The current record under each I4 key, and every I4 violation found on the way.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct Selection {
    current: BTreeMap<I4Key, OutcomeRecord>,
    violations: Vec<I4Violation>,
}

impl Selection {
    /// The current records, in key order.
    pub fn current<'a>(&'a self) -> impl Iterator<Item = &'a OutcomeRecord> {
        self.current.values()
    }

    /// The outcome of each `(evaluator, run, expectation)`: of its current records, which differ
    /// only in their context and suite entry digests, the one with the latest request id. These
    /// are the records a rate counts, one per run and expectation, in key order.
    pub fn outcomes<'a>(&'a self) -> impl Iterator<Item = &'a OutcomeRecord> {
        // The key orders by evaluator, run and expectation first, so the records of one outcome
        // are adjacent.
        let mut records = self.current.values().peekable();
        std::iter::from_fn(move || {
            let mut latest = records.next()?;
            while let Some(next) = records.next_if(|next| same_outcome(latest, next)) {
                if next.request_id() > latest.request_id() {
                    latest = next;
                }
            }
            Some(latest)
        })
    }

    /// The current record under `key`, if there is one.
    #[must_use]
    pub fn get<'a>(&'a self, key: &I4Key) -> Option<&'a OutcomeRecord> {
        self.current.get(key)
    }

    /// The I4 violations, in the order the later records arrived.
    #[must_use]
    pub const fn violations<'a>(&'a self) -> &'a [I4Violation] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.violations.as_slice()
    }

    /// Reports whether the selection holds no current record.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.current.is_empty()
    }

    /// The part of the selection that belongs to `evaluator`, violations included.
    #[must_use]
    pub fn for_evaluator(&self, evaluator: &NameVersion) -> Self {
        Self {
            current: self
                .current
                .iter()
                .filter(|(key, _)| &key.evaluator == evaluator)
                // BORROW: the new selection owns copies of the records
                .map(|(key, record)| (key.clone(), record.clone()))
                .collect(),
            violations: self
                .violations
                .iter()
                .filter(|violation| &violation.key.evaluator == evaluator)
                .cloned()
                .collect(),
        }
    }

    /// Admits the next record in request order.
    fn admit(&mut self, record: OutcomeRecord) {
        match self.current.entry(I4Key::of(&record)) {
            Entry::Vacant(slot) => {
                slot.insert(record);
            }
            Entry::Occupied(mut slot) => {
                let earlier = FourFields::of(slot.get());
                let later = FourFields::of(&record);
                let recovery = earlier.is_error() && later.status != Status::NotMeasured;
                if earlier != later && !recovery {
                    self.violations.push(I4Violation {
                        key: slot.key().clone(),   // BORROW: the violation owns the key
                        first: slot.get().clone(), // BORROW: the violation owns the record
                        second: record.clone(),    // BORROW: the violation owns the record
                    });
                }
                slot.insert(record);
            }
        }
    }
}

/// Reports whether `a` and `b` are records of one evaluator, run and expectation.
fn same_outcome(a: &OutcomeRecord, b: &OutcomeRecord) -> bool {
    a.evaluator() == b.evaluator() && a.run() == b.run() && a.expectation_id() == b.expectation_id()
}

/// Selects the current record under each I4 key and flags every change under one key.
///
/// Records under a key are ordered by request id, which is a v7 UUID and so time-ordered; equal
/// ids keep their input order. The latest record is current. A record that differs from the one
/// before it in `status`, `value`, `weight` or `cause` is an [`I4Violation`], except that a
/// measured record after an error outcome (`not_measured` with the cause `instrument_error` or
/// `deadline`) is a recovery and is not flagged. A measured record is any status but
/// `not_measured`.
#[must_use]
pub fn select<I: IntoIterator<Item = OutcomeRecord>>(records: I) -> Selection {
    let mut ordered: Vec<OutcomeRecord> = records.into_iter().collect();
    ordered.sort_by(|a, b| a.request_id().cmp(b.request_id()));
    let mut selection = Selection::default();
    for record in ordered {
        selection.admit(record);
    }
    selection
}
