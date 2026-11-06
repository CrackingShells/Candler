//! Recovery of a job whose supervisor was lost: its processes and containers are ended and the
//! job is sealed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use candler_harness::spec::{StopSignal, StopSpec};
use candler_schemas::container::{ContainerRole, StrayContainer};
use candler_schemas::execution::{EventPayload, JobEnd};
use candler_schemas::primitives::{ContainerId, ReasonCode, RunKey};
use candler_schemas::process::ExitStatus;
use candler_schemas::trace::EndedBy;
use futures::future::BoxFuture;

use super::dir::{JobDir, JobError};
use super::events::{JobEvents, TailPos};
use super::lock::{JobLock, LockState};
use super::view::{ChildEntry, JobView};
use crate::isolation::container::sweeper::{ContainerSweeper, SweepError};
use crate::isolation::layout::{LABEL_ROLE, LABEL_RUN};
use crate::isolation::model::SweepScope;
use crate::store::{Store, StoreError};
use crate::supervise::Supervisor;

/// What a job is, from its lock and its event log alone; its `state` field is never read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum JobLiveness {
    /// A supervisor holds the lock and the log has no `job.ended`.
    Running,
    /// The log has a `job.ended`, with this reason.
    Ended(JobEnd),
    /// The lock is free and the log has no `job.ended`: the supervisor was lost.
    Lost,
}

/// The reason of the first `job.ended` in the log of `dir`, if the log has one.
pub(crate) fn find_end(dir: &JobDir) -> Result<Option<JobEnd>, JobError> {
    let events = JobEvents::open(dir)?;
    let tail = events.tail(TailPos(0))?;
    for envelope in tail.events.iter().rev() {
        if let EventPayload::JobEnded { reason, .. } = envelope.event() {
            return Ok(Some(*reason));
        }
    }
    Ok(None)
}

/// Classifies the job of `dir`. The lock is probed first and the log read second: a supervisor
/// appends `job.ended` before it lets go of the lock, so a free lock means the log is final.
/// A held lock with a `job.ended` is a supervisor still exiting, and reads as ended.
///
/// # Errors
/// [`JobError`] when the lock cannot be probed or the log cannot be read.
pub fn classify(dir: &JobDir) -> Result<JobLiveness, JobError> {
    let lock = JobLock::probe(dir)?;
    Ok(match (find_end(dir)?, lock) {
        (Some(end), LockState::Held | LockState::Free) => JobLiveness::Ended(end),
        (None, LockState::Held) => JobLiveness::Running,
        (None, LockState::Free) => JobLiveness::Lost,
    })
}

/// How long a recovery waits after the first signal before it escalates.
const GRACE: Duration = Duration::from_secs(10);

/// What a recovery found the job to be.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecoverOutcome {
    /// The job was lost and has been recovered.
    #[default]
    Recovered,
    /// The log already has a `job.ended`; nothing was done.
    AlreadyEnded,
    /// A supervisor holds the lock; nothing was done.
    NotLost,
}

/// What a recovery did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecoverReport {
    /// How many processes it ended.
    pub processes: u32,
    /// How many containers it destroyed.
    pub containers: u32,
    /// What the recovery found the job to be.
    pub outcome: RecoverOutcome,
    /// How many open runs it sealed.
    pub runs_sealed: u32,
    /// The containers it destroyed that the job had no record of.
    pub unrecorded: Vec<ContainerId>,
    /// The containers it destroyed whose `candler.role` label is missing or is neither `agent`
    /// nor `gateway`; they are reported here and left out of the `run.stray` event.
    pub role_unknown: Vec<ContainerId>,
    /// The record files it could not read and left alone.
    pub unreadable: Vec<PathBuf>,
}

impl RecoverReport {
    /// The report of a job that a supervisor still holds.
    #[must_use]
    pub fn not_lost() -> Self {
        Self {
            outcome: RecoverOutcome::NotLost,
            ..Self::default()
        }
    }

    /// The report of a job whose log already has a `job.ended`.
    #[must_use]
    pub fn already_ended() -> Self {
        Self {
            outcome: RecoverOutcome::AlreadyEnded,
            ..Self::default()
        }
    }
}

/// Why a recovery failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RecoverError {
    /// Recovery is not implemented.
    #[error("recovery is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
    /// The job directory or its log could not be read or written.
    #[error("the job directory failed: {0}")]
    Job(#[from] JobError),
    /// A host child could not be ended or reported.
    #[error("a host child could not be reaped: {detail}")]
    Reap {
        /// What went wrong.
        detail: String,
    },
    /// The containers could not be listed.
    #[error("the container sweep failed: {0}")]
    Sweep(#[from] SweepError),
    /// The store could not be read or a run could not be sealed.
    #[error("the store failed: {detail}")]
    Store {
        /// What went wrong.
        detail: String,
    },
}

impl From<StoreError> for RecoverError {
    fn from(error: StoreError) -> Self {
        Self::Store {
            detail: error.to_string(),
        }
    }
}

/// Ends what the lost job left behind: reaps its host children and `control/` calls through the
/// supervisor, sweeps its labelled containers through `sweep`, seals every open run it ran, and
/// appends `job.ended(failed)`. It deletes nothing and signals nothing itself. A job whose lock is
/// held is `NotLost`, a job with a `job.ended` is `AlreadyEnded`, and a second call after a
/// recovery appends no event.
pub fn recover<'a>(
    sup: &'a Supervisor,
    sweep: &'a dyn ContainerSweeper,
    dir: &'a JobDir,
) -> BoxFuture<'a, Result<RecoverReport, RecoverError>> {
    Box::pin(recover_lost(sup, sweep, dir))
}

async fn recover_lost(
    sup: &Supervisor,
    sweep: &dyn ContainerSweeper,
    dir: &JobDir,
) -> Result<RecoverReport, RecoverError> {
    let _lock = match JobLock::acquire(dir) {
        Ok(lock) => lock,
        Err(JobError::Held) => return Ok(RecoverReport::not_lost()),
        Err(error) => return Err(error.into()),
    };
    // The lock is ours, so `classify` would read it as held; the log alone says whether it ended.
    if find_end(dir)?.is_some() {
        return Ok(RecoverReport::already_ended());
    }
    let events = JobEvents::open(dir)?;
    let mut report = RecoverReport::default();
    let children = dir.children().map_err(|e| JobError::Parse {
        path: dir.children_dir(),
        detail: e.to_string(),
    })?;

    // Reap: every process record of `children/` and of `control/`.
    let stop = StopSpec::new(None, StopSignal::Term, GRACE);
    let (control, control_unreadable) = dir.control_records()?;
    report.unreadable.extend(control_unreadable);
    let mut records = Vec::new();
    let mut recorded = BTreeSet::new();
    for entry in &children {
        match entry {
            ChildEntry::Process(record) => records.push(record),
            ChildEntry::Container(record) => {
                recorded.insert(record.cid().to_string());
            }
            ChildEntry::Unreadable { path, .. } => report.unreadable.push(path.clone()), // BORROW: the report owns its paths
        }
    }
    records.extend(control.iter());
    let mut first_error = None;
    for record in records {
        let run = record.marker().as_str().parse::<RunKey>().ok(); // BORROW: explicit .as_str() instead of Deref coercion
        match sup.reap(run.as_ref(), record, &stop).await {
            Ok(reaped) => report.processes += u32::from(reaped.was_alive),
            Err(error) => {
                first_error.get_or_insert_with(|| RecoverError::Reap {
                    detail: error.to_string(),
                });
            }
        }
    }

    // Sweep: every labelled container of the lost job is in scope.
    let scope = SweepScope::Lost(dir.job().clone()); // BORROW: the scope owns its job id
    let swept = sweep.sweep(&scope).await?;
    report.containers = u32::try_from(swept.destroyed.len()).unwrap_or(u32::MAX);
    report.unrecorded = swept
        .destroyed
        .iter()
        .map(|container| &container.cid)
        .filter(|cid| !recorded.contains(&cid.to_string()))
        .cloned() // BORROW: the report owns its ids
        .collect();
    // One `run.stray` per run label; a container of no run (the gateway) goes under no run.
    let mut strays: BTreeMap<Option<RunKey>, Vec<StrayContainer>> = BTreeMap::new();
    for container in &swept.destroyed {
        let role = match container.labels.get(LABEL_ROLE) {
            Some("agent") => ContainerRole::Agent,
            Some("gateway") => ContainerRole::Gateway,
            Some(_) | None => {
                report.role_unknown.push(container.cid.clone()); // BORROW: the report owns its ids
                continue;
            }
        };
        let run = container
            .labels
            .get(LABEL_RUN)
            .and_then(|label| label.parse::<RunKey>().ok());
        strays
            .entry(run)
            .or_default()
            .push(StrayContainer::new(container.cid.clone(), role)); // BORROW: the event owns its ids
    }
    for (run, containers) in strays {
        events.append(
            run.as_ref(),
            EventPayload::run_stray_with_containers(Vec::new(), containers),
        )?;
    }
    if swept.unknown {
        let reason = "sweep_unknown"
            .parse::<ReasonCode>()
            .map_err(|e| RecoverError::Store {
                detail: e.to_string(),
            })?;
        events.append(
            None,
            EventPayload::container_sweep_incomplete(None, scope.wire(), reason),
        )?;
    }
    if let Some(error) = first_error {
        // The job stays lost, so a later recovery tries again.
        return Err(error);
    }

    // Seal: every run of this job that has no end.
    seal_runs(dir, &events, &mut report)?;

    // Last: the job ends, and its record says so as a hint.
    events.append(None, EventPayload::job_ended(JobEnd::Failed))?;
    dir.set_state(candler_schemas::execution::JobState::Ended)?;
    Ok(report)
}

/// The store of `dir`: `<store>/jobs/<id>` sits two levels below its root.
fn store_of(dir: &JobDir) -> Result<Store, RecoverError> {
    let root = dir
        .path()
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or_else(|| RecoverError::Store {
            detail: format!("{} is not inside a store", dir.path().display()),
        })?;
    Ok(Store::open(root)?)
}

/// Seals each unsealed run of the job and gives each run that has no end event one.
fn seal_runs(
    dir: &JobDir,
    events: &JobEvents,
    report: &mut RecoverReport,
) -> Result<(), RecoverError> {
    let store = store_of(dir)?;
    let mut ended = BTreeSet::new();
    for envelope in &events.tail(TailPos(0))?.events {
        if let (Some(run), EventPayload::RunEnded { .. } | EventPayload::RunFailed { .. }) =
            (envelope.run(), envelope.event())
        {
            ended.insert(run.clone()); // BORROW: the set owns its keys
        }
    }
    for entry in store.runs()? {
        let Ok(manifest) = store.read_manifest(&entry.key) else {
            report.unreadable.push(store.layout().manifest(&entry.key));
            continue;
        };
        if manifest.job() != Some(dir.job()) {
            continue;
        }
        let mut payload = None;
        if !entry.done {
            match store.seal_interrupted(&entry.key) {
                Ok(_) | Err(StoreError::RunSealed { .. }) => {}
                Err(error) => return Err(error.into()),
            }
            report.runs_sealed += 1;
            payload = Some(EventPayload::run_ended(EndedBy::Interrupted, None));
        }
        if ended.contains(&entry.key) {
            continue;
        }
        let payload = payload.unwrap_or_else(|| sealed_end(&manifest));
        events.append(Some(&entry.key), payload)?;
    }
    Ok(())
}

/// The end of a sealed run that has no end event: what the manifest measured, else unknown.
fn sealed_end(manifest: &candler_schemas::execution::RunManifest) -> EventPayload {
    let measured = |path: &str| {
        manifest
            .measured()
            .iter()
            .find(|m| m.path().as_str() == path) // BORROW: explicit .as_str() instead of Deref coercion
            .map(|m| m.value().as_value().clone()) // BORROW: the payload owns its value
    };
    let ended_by = measured("run.ended_by")
        .and_then(|value| serde_json::from_value::<EndedBy>(value).ok())
        .unwrap_or(EndedBy::Unknown);
    let exit =
        measured("run.exit").and_then(|value| serde_json::from_value::<ExitStatus>(value).ok());
    EventPayload::run_ended(ended_by, exit)
}
