//! Ending the group of a child known only by its record, such as one a crashed run left behind.
//!
//! The record is not trusted. Its leader is read from the operating system again: the process
//! id must exist and have the start time the record holds. Only then is the group taken into the
//! signal guard's table and ended by the ladder of [`stop`](super::stop); every signal is checked
//! by the guard once more. A record whose leader is gone, or whose process id now names another
//! process, is never signalled: the stray sweep by marker is all that runs.

use candler_harness::spec::StopSpec;
use candler_schemas::execution::StrayProcess;
use candler_schemas::primitives::RunKey;
use candler_schemas::process::ChildRecord;

use super::platform::{Liveness, liveness};
use super::signal::{group_exists, process_exists};
use super::stop::{Ladder, Leader, Mode, StepRecord};
use super::{SuperviseError, Supervisor};

/// What reaping a record did.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ReapReport {
    /// Whether the leader the record names was running, with the record's start time, when the
    /// reap began.
    pub was_alive: bool,
    /// The steps taken, in order: the whole ladder for a running leader, the sweep alone
    /// otherwise.
    pub steps: Vec<StepRecord>,
    /// What the sweep found and killed.
    pub strays: Vec<StrayProcess>,
    /// Whether the process group was empty afterwards.
    pub group_empty: bool,
    /// Why no signal was sent to the group although the record named one, if that is so: the
    /// process id now names another process, or the record does not describe a group this
    /// supervisor may signal.
    pub refusal: Option<String>,
}

impl ReapReport {
    /// A report.
    #[must_use]
    pub const fn new(
        was_alive: bool,
        steps: Vec<StepRecord>,
        strays: Vec<StrayProcess>,
        group_empty: bool,
        refusal: Option<String>,
    ) -> Self {
        Self {
            was_alive,
            steps,
            strays,
            group_empty,
            refusal,
        }
    }
}

impl Supervisor {
    /// Ends the group `record` names and what its run left outside the group.
    ///
    /// When the leader is running with the record's start time, the group is taken into the
    /// guard's table and the ladder of `stop` runs on it, then the sweep. When the leader is gone,
    /// or its process id names a later process, only the sweep runs, and the report says which of
    /// the two it was. The same events as for a stop are emitted under `run`, except
    /// `process.exited`: the status of a process that is not this process's child is unknown.
    ///
    /// Must be called within a Tokio runtime.
    ///
    /// # Errors
    /// The first [`SuperviseError::Event`] or [`SuperviseError::Clock`] met while reporting a
    /// step; the ladder has been climbed regardless.
    pub async fn reap(
        &self,
        run: Option<&RunKey>,
        record: &ChildRecord,
        stop: &StopSpec,
    ) -> Result<ReapReport, SuperviseError> {
        let pid = record.pid();
        // The record is read against the operating system, not taken at its word.
        let was_alive = liveness(record) == Liveness::Alive;
        let mut refusal = None;
        let mut adopted = false;
        let held_before = self.guard.holds(record.pgid());
        if was_alive {
            if record.pgid() == pid {
                match self.guard.register(pid, record.start().clone()) {
                    // BORROW: .clone() gives the table its own start
                    Ok(()) => adopted = true,
                    Err(error) => refusal = Some(error.to_string()),
                }
            } else {
                refusal = Some("the record's group is not led by its own process".to_owned()); // BORROW: .to_owned() makes the owned reason from a literal
            }
        } else if process_exists(pid) {
            refusal = Some("the record's process id now names another process".to_owned()); // BORROW: .to_owned() makes the owned reason from a literal
        }

        let ladder = Ladder {
            sink: &*self.sink,
            sweeper: self.sweeper.as_ref(),
            guard: &self.guard,
            record,
            run,
            stop,
            env: &[],
            cwd: None,
        };
        let mut leader = Leader::Record {
            pid,
            start: record.start(),
        };
        let mode = if adopted { Mode::Full } else { Mode::SweepOnly };
        let climb = ladder.climb(&mut leader, mode).await;
        if adopted && !held_before {
            // A group this supervisor spawned stays in its table for its own child to use.
            self.guard.forget(pid);
        }
        let group_empty = !group_exists(record.pgid());
        if let Some(error) = climb.error {
            return Err(error);
        }
        Ok(ReapReport::new(
            was_alive,
            climb.steps,
            climb.strays,
            group_empty,
            refusal,
        ))
    }
}
