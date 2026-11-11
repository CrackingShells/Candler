//! Running the batches of a run plan: one request, one process and one outcome file per batch.
//!
//! A run's batches go stage by stage; the batches of a stage run at once, each holding a slot of
//! the evaluator pool while its request is in flight. Entries a batch does not send, because they do not apply or because a dependency
//! ended without a passing verdict, are written by the engine into the same file.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use candler_schemas::evaluation::{EvaluateParams, Item, Needs};
use candler_schemas::execution::EventPayload;
use candler_schemas::outcome::{Instrument, OutcomeRecord, Status};
use candler_schemas::primitives::{
    Digest, ExpectationId, Marker, Name, NameVersion, RequestId, RunKey,
};
use futures::future::join_all;
use tokio::sync::watch;

use super::plan::{Batch, RunPlan};
use super::write::{self, Stamp};
use crate::evaluation::client::{Client, ClientError, EvaluatorCommand, ItemOutcome};
use crate::evaluation::context::{assemble, materialise};
use crate::evaluation::registry::CheckedEntry;
use crate::jobs::slots::{Holder, Slot, SlotError, Slots};
use crate::store::Store;
use crate::supervise::{EventSink, Supervisor};

/// What the request deadline adds to the sum of the item deadlines, for the evaluator's start, the
/// frames and the stop (R01 §Deadlines).
pub const REQUEST_DEADLINE_MARGIN: Duration = Duration::from_secs(30);

/// What every evaluator process's marker starts with; the request's own id follows. A marker is
/// the key of the stray sweep, so two requests never share one.
const MARKER_PREFIX: &str = "evaluation-";

/// An evaluator the engine can start: its identity from `describe` and how to run it.
#[derive(Debug)]
pub(crate) struct Evaluator {
    /// `name@version`, as `describe` reported.
    pub(crate) identity: NameVersion,
    /// The instruments `describe` reported.
    pub(crate) instruments: Vec<Instrument>,
    /// The command line.
    pub(crate) argv: Vec<String>,
    /// The exact environment of the process.
    pub(crate) env: Vec<(String, String)>,
    /// The directory the process starts in.
    pub(crate) cwd: PathBuf,
}

/// What every batch of one `evaluate` call shares.
pub(crate) struct Shared {
    /// The store the runs are read from and the outcome files go to.
    pub(crate) store: Store,
    /// The evaluators the suite names.
    pub(crate) evaluators: BTreeMap<Name, Evaluator>,
    /// The supervisor every process is spawned through.
    pub(crate) supervisor: Arc<Supervisor>,
    /// The evaluator slot pool: one slot per request in flight.
    pub(crate) pool: Slots,
    /// Who the slots are held for; each batch adds the run it grades.
    pub(crate) holder: Holder,
    // TRAIT_OBJECT: the sink is the job's event log, or a recording fake in tests
    /// Where each committed outcome record is reported.
    pub(crate) events: Arc<dyn EventSink>,
    /// Turns `true` when the caller cancels.
    pub(crate) cancel: watch::Receiver<bool>,
    /// The directory of this call's workspaces and child records, outside the store.
    pub(crate) work: PathBuf,
    /// Where the evaluator processes' child records go.
    pub(crate) record_dir: PathBuf,
    /// A flat request deadline, or `None` for the sum of the item deadlines plus the margin.
    pub(crate) deadline: Option<Duration>,
}

impl Shared {
    /// Whether the caller has cancelled. A dropped sender reads as "not cancelled".
    fn cancelled(&self) -> bool {
        *self.cancel.borrow()
    }

    /// Takes a slot of the pool for `run`, waiting for one; `None` when the caller cancels first.
    /// A dropped sender reads as "not cancelled".
    async fn acquire_slot(&self, run: &RunKey) -> Result<Option<Slot>, SlotError> {
        let holder = Holder {
            job: self.holder.job.clone(), // BORROW: the holder file owns its text
            run: Some(run.to_string()),
        };
        let mut cancel = self.cancel.clone(); // BORROW: the wait owns its receiver
        let cancelled = async {
            loop {
                if *cancel.borrow_and_update() {
                    return;
                }
                if cancel.changed().await.is_err() {
                    // EXPLICIT: no sender left, so the call can no longer be cancelled
                    std::future::pending::<()>().await;
                }
            }
        };
        tokio::select! {
            slot = self.pool.acquire(&holder) => slot.map(Some),
            () = cancelled => Ok(None),
        }
    }
}

/// What a run came to.
#[derive(Debug, Default)]
pub(crate) struct RunOutput {
    /// The id of every request that started.
    pub(crate) request_ids: Vec<RequestId>,
    /// The deadline given to each request that was sent, in the order of `request_ids`.
    pub(crate) request_deadlines: Vec<(RequestId, Duration)>,
    /// The outcome files published.
    pub(crate) files: Vec<PathBuf>,
    /// The records published, per status.
    pub(crate) counts: BTreeMap<String, u64>,
    /// The requests that failed, each with why.
    pub(crate) failed: Vec<(RequestId, String)>,
}

/// What one batch came to.
enum BatchResult {
    /// The call was cancelled before the batch started.
    NotStarted,
    /// The batch failed and published nothing; `deadline` is the one given to the client, if the
    /// request was sent.
    Failed {
        request: RequestId,
        message: String,
        deadline: Option<Duration>,
    },
    /// The batch published `file`; `deadline` is the one given to the client, if the request was
    /// sent.
    Committed {
        request: RequestId,
        deadline: Option<Duration>,
        file: PathBuf,
        records: Vec<OutcomeRecord>,
    },
}

/// What one batch knows about its request while it runs.
struct Ticket {
    /// The id minted before the client gave one; it names the request directory.
    minted: RequestId,
    /// The deadline given to the client, once the request is sent.
    deadline: Option<Duration>,
}

/// Why a batch failed, with the request id when the client had given one.
struct Fault {
    request: Option<RequestId>,
    message: String,
}

impl Fault {
    /// A fault with no request id yet.
    fn new(message: impl Into<String>) -> Self {
        Self {
            request: None,
            message: message.into(),
        }
    }
}

/// Whether a dependency that ended with `status` stops its dependents.
const fn stops_dependents(status: Status) -> bool {
    match status {
        Status::Fail | Status::Blocked | Status::NotMeasured | Status::NotApplicable => true,
        Status::Pass => false,
    }
}

/// What the dependencies of an entry say about it.
enum Blocker {
    /// Every dependency passed or is still to run in this batch.
    Clear,
    /// This dependency ended without a passing verdict.
    By(ExpectationId),
    /// This dependency has no outcome at all.
    Missing(ExpectationId),
}

/// The first dependency of `entry` that stops it. `known` holds the verdicts so far; a dependency
/// in `pending` has not run yet and is not held against the entry.
fn blocker(
    entry: &candler_schemas::evaluation::SuiteEntry,
    known: &BTreeMap<ExpectationId, Status>,
    pending: &BTreeSet<ExpectationId>,
) -> Blocker {
    for dependency in entry.depends_on() {
        match known.get(dependency) {
            // BORROW: .clone() keeps the id in the verdict
            Some(status) if stops_dependents(*status) => return Blocker::By(dependency.clone()),
            Some(_) => {}
            None if pending.contains(dependency) => {}
            None => return Blocker::Missing(dependency.clone()),
        }
    }
    Blocker::Clear
}

/// Runs `work` on a blocking thread.
async fn blocking<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| format!("a blocking task failed: {error}"))
}

/// Removes a directory and everything in it; absent is fine.
async fn remove_tree(dir: PathBuf) {
    // A failed cleanup leaves a directory under the call's work root, which the call removes again.
    let _ = blocking(move || std::fs::remove_dir_all(dir)).await;
}

/// The digest of the empty context of `run`: what a record carries when no evaluator ran.
fn empty_digest(run: &RunKey, work: &Path) -> Result<Digest, String> {
    candler_schemas::evaluation::RunContext::builder()
        // BORROW: .clone() gives the builder its own key
        .run(run.clone())
        .root(work.display().to_string())
        .build()
        .map(|context| context.context_digest())
        .map_err(|error| format!("the empty context cannot be built: {error}"))
}

/// Runs every batch of `plan`, stage by stage, and returns what came of it.
pub(crate) async fn run_plan(shared: &Shared, plan: &RunPlan) -> RunOutput {
    let mut out = RunOutput::default();
    let mut finished: BTreeMap<ExpectationId, OutcomeRecord> = BTreeMap::new();
    // BORROW: .clone() keeps the ids the batches test their dependencies against
    let na_ids: BTreeSet<ExpectationId> = plan
        .not_applicable
        .iter()
        .map(|checked| checked.entry.id().clone())
        .collect();
    let mut na_left: BTreeMap<Name, Vec<CheckedEntry>> = BTreeMap::new();
    for checked in &plan.not_applicable {
        na_left
            // BORROW: .clone() keys the group by the evaluator name
            .entry(checked.entry.evaluator().name.clone())
            .or_default()
            // BORROW: .clone() gives the group its own entry
            .push(checked.clone());
    }
    let mut at = 0;
    while at < plan.batches.len() && !shared.cancelled() {
        let stage = plan.batches.get(at).map(|batch| batch.stage);
        let end = at
            + plan
                .batches
                .iter()
                .skip(at)
                .take_while(|batch| Some(batch.stage) == stage)
                .count();
        let group: Vec<(&Batch, Vec<CheckedEntry>)> = plan
            .batches
            .get(at..end)
            .into_iter()
            .flatten()
            .map(|batch| {
                let na = na_left.remove(&batch.evaluator).unwrap_or_default();
                (batch, na)
            })
            .collect();
        let results = join_all(
            group
                .iter()
                .map(|(batch, na)| run_batch(shared, &plan.run, batch, na, &finished, &na_ids)),
        )
        .await;
        for result in results {
            absorb(&mut out, &mut finished, result);
        }
        at = end;
    }
    if !shared.cancelled() {
        for (evaluator, entries) in na_left {
            let result = core_only(shared, &plan.run, &evaluator, &entries).await;
            absorb(&mut out, &mut finished, result);
        }
    }
    out
}

/// Adds one batch's result to the run's output.
fn absorb(
    out: &mut RunOutput,
    finished: &mut BTreeMap<ExpectationId, OutcomeRecord>,
    result: BatchResult,
) {
    match result {
        BatchResult::NotStarted => {}
        BatchResult::Failed {
            request,
            message,
            deadline,
        } => {
            // BORROW: .clone() keeps the id in every list
            out.request_ids.push(request.clone());
            if let Some(deadline) = deadline {
                out.request_deadlines.push((request.clone(), deadline)); // BORROW: the list owns the id
            }
            out.failed.push((request, message));
        }
        BatchResult::Committed {
            request,
            deadline,
            file,
            records,
        } => {
            if let Some(deadline) = deadline {
                out.request_deadlines.push((request.clone(), deadline)); // BORROW: the list owns the id
            }
            out.request_ids.push(request);
            out.files.push(file);
            for record in records {
                *out.counts
                    .entry(record.status().as_str().to_owned())
                    .or_default() += 1;
                finished.insert(record.expectation_id().clone(), record); // BORROW: the run keeps the verdicts
            }
        }
    }
}

/// Publishes the `not_applicable` records of an evaluator that has no batch in the run.
async fn core_only(
    shared: &Shared,
    run: &RunKey,
    evaluator: &Name,
    entries: &[CheckedEntry],
) -> BatchResult {
    let request = RequestId::new_v7();
    let outcome = async {
        let identity = shared
            .evaluators
            .get(evaluator)
            .ok_or_else(|| Fault::new(format!("evaluator {evaluator} was not described")))?;
        let stamp = Stamp {
            // BORROW: .clone() gives the stamp its own copy of each field
            run: run.clone(),
            evaluator: identity.identity.clone(),
            instruments: identity.instruments.clone(),
            context_digest: empty_digest(run, &shared.work).map_err(Fault::new)?,
            request: request.clone(),
        };
        let mut records = Vec::with_capacity(entries.len());
        for checked in entries {
            records.push(
                write::not_applicable(&stamp, &checked.entry)
                    .map_err(|error| Fault::new(error.to_string()))?,
            );
        }
        publish(shared, stamp, records).await
    }
    .await;
    settle(request, outcome, None)
}

/// Turns a batch's outcome into its result.
fn settle(
    request: RequestId,
    outcome: Result<BatchResult, Fault>,
    sent: Option<Duration>,
) -> BatchResult {
    match outcome {
        Ok(BatchResult::Committed {
            request,
            file,
            records,
            ..
        }) => BatchResult::Committed {
            request,
            deadline: sent,
            file,
            records,
        },
        Ok(result) => result,
        Err(fault) => BatchResult::Failed {
            request: fault.request.unwrap_or(request),
            message: fault.message,
            deadline: sent,
        },
    }
}

/// Writes and publishes `records` as the outcome file of the stamp's request.
async fn publish(
    shared: &Shared,
    stamp: Stamp,
    records: Vec<OutcomeRecord>,
) -> Result<BatchResult, Fault> {
    let store = shared.store.clone(); // BORROW: the blocking task owns a handle on the store
    let cancel = shared.cancel.clone(); // BORROW: the blocking task owns a receiver
    let request = stamp.request.clone(); // BORROW: the result owns the id
    let stamp_run = stamp.run.clone(); // BORROW: the events own the key
    let (written, records) = blocking(move || {
        let written = write::commit(&store, &stamp, &records, &cancel);
        (written, records)
    })
    .await
    .map_err(Fault::new)?;
    match written {
        Ok(Some(file)) => {
            // Only a published file is reported, so no event names a record a reader cannot see.
            for record in &records {
                shared.events.emit(
                    Some(&stamp_run),
                    EventPayload::evaluation_outcome(record.clone()), // BORROW: the event owns its record
                );
            }
            Ok(BatchResult::Committed {
                request,
                deadline: None,
                file,
                records,
            })
        }
        Ok(None) => Err(Fault {
            request: Some(request),
            message: "the request was cancelled".to_owned(),
        }),
        Err(error) => Err(Fault {
            request: Some(request),
            message: error.to_string(),
        }),
    }
}

/// Runs one batch: waits for an evaluator slot, decides which entries to send, sends them and
/// publishes the file. The slot is held until the batch's outcome is settled.
async fn run_batch(
    shared: &Shared,
    run: &RunKey,
    batch: &Batch,
    not_applicable: &[CheckedEntry],
    finished: &BTreeMap<ExpectationId, OutcomeRecord>,
    na_ids: &BTreeSet<ExpectationId>,
) -> BatchResult {
    let _slot = match shared.acquire_slot(run).await {
        Ok(Some(slot)) => slot,
        Ok(None) => return BatchResult::NotStarted,
        Err(error) => {
            return BatchResult::Failed {
                request: RequestId::new_v7(),
                message: format!("no evaluator slot could be taken: {error}"),
                deadline: None,
            };
        }
    };
    if shared.cancelled() {
        return BatchResult::NotStarted;
    }
    let mut ticket = Ticket {
        minted: RequestId::new_v7(),
        deadline: None,
    };
    let outcome = attempt(
        shared,
        run,
        batch,
        not_applicable,
        finished,
        na_ids,
        &mut ticket,
    )
    .await;
    settle(ticket.minted, outcome, ticket.deadline)
}

/// The work of one batch.
async fn attempt(
    shared: &Shared,
    run: &RunKey,
    batch: &Batch,
    not_applicable: &[CheckedEntry],
    finished: &BTreeMap<ExpectationId, OutcomeRecord>,
    na_ids: &BTreeSet<ExpectationId>,
    ticket: &mut Ticket,
) -> Result<BatchResult, Fault> {
    let evaluator = shared
        .evaluators
        .get(&batch.evaluator)
        .ok_or_else(|| Fault::new(format!("evaluator {} was not described", batch.evaluator)))?;
    // BORROW: .clone() builds the verdict table the dependency checks read
    let mut known: BTreeMap<ExpectationId, Status> = finished
        .iter()
        .map(|(id, record)| (id.clone(), record.status()))
        .collect();
    for id in na_ids {
        known.insert(id.clone(), Status::NotApplicable); // BORROW: the table owns its keys
    }
    let batch_ids: BTreeSet<ExpectationId> = batch
        .entries
        .iter()
        .map(|checked| checked.entry.id().clone()) // BORROW: the set owns its ids
        .collect();
    let mut sendable: Vec<&CheckedEntry> = Vec::new();
    let mut held: BTreeMap<&ExpectationId, ExpectationId> = BTreeMap::new();
    for checked in &batch.entries {
        match blocker(&checked.entry, &known, &batch_ids) {
            Blocker::Clear => sendable.push(checked),
            Blocker::By(dependency) => {
                known.insert(checked.entry.id().clone(), Status::Blocked); // BORROW: the table owns its keys
                held.insert(checked.entry.id(), dependency);
            }
            Blocker::Missing(dependency) => {
                return Err(Fault::new(format!(
                    "{} depends on {dependency}, which has no outcome because its request failed",
                    checked.entry.id()
                )));
            }
        }
    }

    let (request, context_digest, items) = if sendable.is_empty() {
        (
            // BORROW: .clone() keeps the minted id for the file
            ticket.minted.clone(),
            empty_digest(run, &shared.work).map_err(Fault::new)?,
            Vec::new(),
        )
    } else {
        let dir = shared.work.join(ticket.minted.as_str());
        let sent = send(
            shared, run, evaluator, &sendable, finished, &batch_ids, ticket,
        )
        .await;
        remove_tree(dir).await;
        let (outcome, digest) = sent?;
        if shared.cancelled() {
            return Err(Fault {
                request: Some(outcome.request_id),
                message: "the request was cancelled".to_owned(),
            });
        }
        (outcome.request_id, digest, outcome.items)
    };

    let stamp = Stamp {
        // BORROW: .clone() gives the stamp its own copy of each field
        run: run.clone(),
        evaluator: evaluator.identity.clone(),
        instruments: evaluator.instruments.clone(),
        context_digest,
        request,
    };
    let fail = |error: candler_schemas::outcome::OutcomeError| Fault {
        request: Some(stamp.request.clone()), // BORROW: the fault owns the id
        message: error.to_string(),
    };
    let mut rows: BTreeMap<&ExpectationId, OutcomeRecord> = BTreeMap::new();
    for (checked, item) in sendable.iter().zip(items) {
        if item.expectation_id() != checked.entry.id() {
            return Err(Fault {
                request: Some(stamp.request.clone()), // BORROW: the fault owns the id
                message: format!(
                    "the client answered {} where {} was expected",
                    item.expectation_id(),
                    checked.entry.id()
                ),
            });
        }
        // A dependency that ran earlier in this request may have ended without a pass.
        let record = match blocker(&checked.entry, &known, &BTreeSet::new()) {
            Blocker::By(dependency) => {
                write::blocked(&stamp, &checked.entry, &dependency).map_err(fail)?
            }
            Blocker::Clear | Blocker::Missing(_) => match &item {
                ItemOutcome::Reported(reported) => {
                    write::kept(&stamp, &checked.entry, reported).map_err(fail)?
                }
                ItemOutcome::Unreported(missing) => {
                    write::unreported(&stamp, &checked.entry, missing).map_err(fail)?
                }
            },
        };
        known.insert(checked.entry.id().clone(), record.status()); // BORROW: the table owns its keys
        rows.insert(checked.entry.id(), record);
    }
    for checked in &batch.entries {
        if let Some(dependency) = held.get(checked.entry.id()) {
            let record = write::blocked(&stamp, &checked.entry, dependency).map_err(fail)?;
            rows.insert(checked.entry.id(), record);
        }
    }
    let mut records = Vec::with_capacity(not_applicable.len() + rows.len());
    for checked in not_applicable {
        records.push(write::not_applicable(&stamp, &checked.entry).map_err(fail)?);
    }
    for checked in &batch.entries {
        if let Some(record) = rows.remove(checked.entry.id()) {
            records.push(record);
        }
    }
    publish(shared, stamp, records).await
}

/// Builds the context of `run` for `sendable`, sends the request and returns what came back with
/// the digest of the context, and notes the deadline in the ticket. The caller removes the
/// request directory, `work/<minted id>`.
async fn send(
    shared: &Shared,
    run: &RunKey,
    evaluator: &Evaluator,
    sendable: &[&CheckedEntry],
    finished: &BTreeMap<ExpectationId, OutcomeRecord>,
    batch_ids: &BTreeSet<ExpectationId>,
    ticket: &mut Ticket,
) -> Result<(crate::evaluation::client::ClientOutcome, Digest), Fault> {
    let mut needs = Needs::default();
    for checked in sendable {
        let more = &checked.spec.needs;
        needs.trace |= more.trace;
        for artifact in &more.artifacts {
            if !needs.artifacts.contains(artifact) {
                needs.artifacts.push(artifact.clone()); // BORROW: the union owns its globs
            }
        }
        for kind in &more.snapshots {
            if !needs.snapshots.contains(kind) {
                needs.snapshots.push(kind.clone()); // BORROW: the union owns its kinds
            }
        }
        for kind in &more.inputs {
            if !needs.inputs.contains(kind) {
                needs.inputs.push(kind.clone()); // BORROW: the union owns its kinds
            }
        }
        for path in &more.metadata {
            if !needs.metadata.contains(path) {
                needs.metadata.push(path.clone()); // BORROW: the union owns its paths
            }
        }
    }
    let store = shared.store.clone(); // BORROW: the blocking task owns a handle on the store
    let key = run.clone(); // BORROW: the blocking task owns the key
    let dir = shared.work.join(ticket.minted.as_str());
    let workspace = dir.join("workspace");
    let scratch = dir.join("scratch");
    let records_dir = shared.record_dir.clone(); // BORROW: the blocking task owns the path
    let context = blocking(move || {
        std::fs::create_dir_all(&scratch).map_err(|error| {
            format!(
                "cannot create the scratch directory {}: {error}",
                scratch.display()
            )
        })?;
        std::fs::create_dir_all(&records_dir).map_err(|error| {
            format!(
                "cannot create the record directory {}: {error}",
                records_dir.display()
            )
        })?;
        let assembled = assemble(&store, &key, &needs).map_err(|error| error.to_string())?;
        let context = materialise(&assembled, &workspace).map_err(|error| error.to_string())?;
        Ok::<_, String>((context, scratch))
    })
    .await
    .map_err(Fault::new)?
    .map_err(Fault::new)?;
    let (context, scratch) = context;
    let digest = context.context_digest();

    let mut items = Vec::with_capacity(sendable.len());
    let mut total = Duration::ZERO;
    let mut prior: Vec<OutcomeRecord> = Vec::new();
    let mut prior_ids: BTreeSet<&ExpectationId> = BTreeSet::new();
    for checked in sendable {
        let deadline_ms = checked
            .entry
            .deadline_ms()
            .unwrap_or(checked.spec.default_deadline_ms);
        total = total.saturating_add(Duration::from_millis(deadline_ms));
        items.push(Item::new(
            // BORROW: .clone() gives the item its own copy of the entry's fields
            checked.entry.id().clone(),
            checked.entry.kind().clone(),
            checked.entry.params().clone(),
            deadline_ms,
        ));
        for dependency in checked.entry.depends_on() {
            if !batch_ids.contains(dependency)
                && prior_ids.insert(dependency)
                && let Some(record) = finished.get(dependency)
            {
                prior.push(record.clone()); // BORROW: the request owns its prior records
            }
        }
    }
    let deadline = shared
        .deadline
        .unwrap_or_else(|| total.saturating_add(REQUEST_DEADLINE_MARGIN));
    let marker: Marker = format!("{MARKER_PREFIX}{}", ticket.minted)
        .parse()
        .map_err(|_| Fault::new("the evaluation marker is not a marker"))?;
    let command = EvaluatorCommand::new(
        // BORROW: .clone() gives the command its own copy
        evaluator.argv.clone(),
        marker,
        shared.record_dir.clone(), // BORROW: the command owns the path
    )
    .env(evaluator.env.clone()) // BORROW: the command owns its environment
    .cwd(evaluator.cwd.clone()) // BORROW: the command owns its directory
    .run(run.clone()); // BORROW: the command owns the key
    let params = EvaluateParams::new(context, items, prior, scratch.display().to_string());
    ticket.deadline = Some(deadline);
    let outcome = Client::new(&shared.supervisor)
        .evaluate(&command, params, deadline, shared.cancel.clone())
        .await
        .map_err(|error| {
            if matches!(error, ClientError::Cancelled) {
                Fault::new("the request was cancelled")
            } else {
                Fault::new(error.to_string())
            }
        })?;
    Ok((outcome, digest))
}
