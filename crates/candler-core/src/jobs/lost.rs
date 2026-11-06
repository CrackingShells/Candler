//! Recovery of a job whose supervisor was lost: its processes and containers are ended and the
//! job is sealed.

use candler_schemas::execution::{EventPayload, JobEnd};
use futures::future::BoxFuture;

use super::dir::{JobDir, JobError};
use super::events::{JobEvents, TailPos};
use super::lock::{JobLock, LockState};
use crate::isolation::container::sweeper::ContainerSweeper;
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

/// What a recovery did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecoverReport {
    /// How many processes it ended.
    pub processes: u32,
    /// How many containers it destroyed.
    pub containers: u32,
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
}

/// Ends what the lost job left behind.
pub fn recover<'a>(
    sup: &'a Supervisor,
    sweep: &'a dyn ContainerSweeper,
    dir: &'a JobDir,
) -> BoxFuture<'a, Result<RecoverReport, RecoverError>> {
    let _ = (sup, sweep, dir);
    Box::pin(async {
        Err(RecoverError::Unavailable {
            detail: "recover".to_owned(),
        })
    })
}
