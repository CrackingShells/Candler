//! The records of a request and its outcome file.
//!
//! The engine writes every record itself. What the evaluator reported contributes the verdict
//! (`status`, `value`, `weight`, `probability`, `evidence`, `cause`); the role, the evaluator, the
//! instruments, both digests and the request id come from the engine's own plan, because the
//! evaluator cannot know them.

use std::path::PathBuf;

use candler_schemas::evaluation::SuiteEntry;
use candler_schemas::outcome::{
    Cause, Evidence, Instrument, OutcomeError, OutcomeRecord, OutcomeRecordBuilder, Status,
};
use candler_schemas::primitives::{
    Digest, ExpectationId, NameVersion, ReasonCode, RequestId, RunKey,
};
use tokio::sync::watch;

use crate::evaluation::client::Unreported;
use crate::store::outcome::OutcomeWriter;
use crate::store::{Store, StoreError};

/// The reason code of an entry the engine does not send because it does not apply to the run.
const REASON_NOT_APPLICABLE: &str = "not_applicable_to_arm";

/// The reason code of an item whose record cannot be kept.
const REASON_INSTRUMENT_ERROR: &str = "instrument_error";

/// What every record of one request shares.
#[derive(Debug, Clone)]
pub(crate) struct Stamp {
    /// The run the request grades.
    pub(crate) run: RunKey,
    /// The evaluator, `name@version`, as `describe` reported it.
    pub(crate) evaluator: NameVersion,
    /// The instruments `describe` reported.
    pub(crate) instruments: Vec<Instrument>,
    /// The digest of the context the evaluator received, or of the empty context when no
    /// evaluator ran.
    pub(crate) context_digest: Digest,
    /// The request the records belong to.
    pub(crate) request: RequestId,
}

impl Stamp {
    /// A builder holding the fields of `entry` and of the request.
    fn builder(&self, entry: &SuiteEntry) -> OutcomeRecordBuilder {
        let mut builder = OutcomeRecord::builder()
            // BORROW: .clone() gives the record its own copy of each shared field
            .run(self.run.clone())
            .expectation_id(entry.id().clone())
            .role(entry.role())
            .evaluator(self.evaluator.clone())
            .suite_entry_digest(entry.digest())
            .context_digest(self.context_digest.clone())
            .request_id(self.request.clone());
        for instrument in &self.instruments {
            // BORROW: .clone() gives the record its own copy of each instrument
            builder = builder.instrument(instrument.clone());
        }
        builder
    }
}

/// A reason code from the codes this module owns.
fn reason(code: &str) -> Result<Cause, OutcomeError> {
    code.parse::<ReasonCode>()
        .map(Cause::Reason)
        .map_err(|_| OutcomeError::TextEmpty { field: "reason" })
}

/// The record of an entry that does not apply to the run.
pub(crate) fn not_applicable(
    stamp: &Stamp,
    entry: &SuiteEntry,
) -> Result<OutcomeRecord, OutcomeError> {
    stamp
        .builder(entry)
        .status(Status::NotApplicable)
        .cause(reason(REASON_NOT_APPLICABLE)?)
        .evidence(Evidence::new(
            None,
            Some("the entry does not apply to this run".to_owned()),
        )?)
        .build()
}

/// The record of an entry whose dependency `by` ended without a passing verdict.
pub(crate) fn blocked(
    stamp: &Stamp,
    entry: &SuiteEntry,
    by: &ExpectationId,
) -> Result<OutcomeRecord, OutcomeError> {
    stamp
        .builder(entry)
        .status(Status::Blocked)
        // BORROW: .clone() keeps the id in the cause
        .cause(Cause::Expectation(by.clone()))
        .evidence(Evidence::new(
            None,
            Some(format!("the dependency {by} did not pass")),
        )?)
        .build()
}

/// The `not_measured` record of an entry with `code` as its reason and `note` as its evidence.
fn not_measured(
    stamp: &Stamp,
    entry: &SuiteEntry,
    code: &str,
    note: String,
) -> Result<OutcomeRecord, OutcomeError> {
    stamp
        .builder(entry)
        .status(Status::NotMeasured)
        .cause(reason(code)?)
        .evidence(Evidence::new(None, Some(note))?)
        .build()
}

/// The record of an item the client could not get a record for.
pub(crate) fn unreported(
    stamp: &Stamp,
    entry: &SuiteEntry,
    missing: &Unreported,
) -> Result<OutcomeRecord, OutcomeError> {
    not_measured(
        stamp,
        entry,
        missing.reason.code(),
        // BORROW: .clone() hands the evidence to the record
        missing.evidence.clone(),
    )
}

/// The record the engine keeps for what the evaluator reported: its verdict, under the engine's
/// own fields. A report that does not make a valid record becomes `not_measured` with the reason
/// `instrument_error`.
pub(crate) fn kept(
    stamp: &Stamp,
    entry: &SuiteEntry,
    reported: &OutcomeRecord,
) -> Result<OutcomeRecord, OutcomeError> {
    let mut builder = stamp.builder(entry).status(reported.status());
    if let Some(value) = reported.value() {
        // BORROW: .clone() copies the verdict out of the evaluator's record
        builder = builder.value(value.clone());
    }
    if let Some(weight) = reported.weight() {
        builder = builder.weight(weight);
    }
    if let Some(probability) = reported.probability() {
        // BORROW: .clone() copies the verdict out of the evaluator's record
        builder = builder.probability(probability.clone());
    }
    for evidence in reported.evidence() {
        // BORROW: .clone() copies the verdict out of the evaluator's record
        builder = builder.evidence(evidence.clone());
    }
    if let Some(cause) = reported.cause() {
        // BORROW: .clone() copies the verdict out of the evaluator's record
        builder = builder.cause(cause.clone());
    }
    match builder.build() {
        Ok(record) => Ok(record),
        Err(error) => not_measured(
            stamp,
            entry,
            REASON_INSTRUMENT_ERROR,
            format!(
                "the evaluator's record for {} is invalid: {error}",
                entry.id()
            ),
        ),
    }
}

/// Writes `records` as the outcome file of `request` and publishes it, unless `cancel` is set at
/// either check; a cancelled write leaves nothing a reader can see. Returns the path, or `None`
/// when it was cancelled. It blocks, and runs on a blocking thread.
pub(crate) fn commit(
    store: &Store,
    stamp: &Stamp,
    records: &[OutcomeRecord],
    cancel: &watch::Receiver<bool>,
) -> Result<Option<PathBuf>, StoreError> {
    if *cancel.borrow() {
        return Ok(None);
    }
    let mut writer = OutcomeWriter::create(store, &stamp.run, &stamp.evaluator, &stamp.request)?;
    for record in records {
        writer.append(record)?;
    }
    if *cancel.borrow() {
        return Ok(None);
    }
    writer.commit()?;
    Ok(Some(store.layout().outcome(
        &stamp.run,
        &stamp.evaluator,
        &stamp.request,
    )))
}
