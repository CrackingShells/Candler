//! The runs of a job in flight, and how they are told to stop.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use candler_schemas::primitives::RunKey;
use tokio::sync::watch;

use crate::jobs::model::JobCause;

/// The sender through which one run in flight is told to stop.
pub type RunStop = watch::Sender<Option<JobCause>>;

/// The runs in flight, each with the sender that stops it. A run is in the table from the moment
/// it starts until its `exec_run` returns; a run that ended is never in it.
#[derive(Debug, Default)]
pub struct LiveRuns {
    runs: Mutex<BTreeMap<RunKey, RunStop>>,
}

impl LiveRuns {
    /// A table with no run in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Puts `key` in flight and returns the receiver its run listens on.
    pub fn enter(&self, key: &RunKey) -> watch::Receiver<Option<JobCause>> {
        let (stop, listen) = watch::channel(None);
        self.runs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key.clone(), stop); // BORROW: the table owns the key
        listen
    }

    /// Takes `key` out of flight, because its run ended and was sealed.
    pub fn leave(&self, key: &RunKey) {
        self.runs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(key);
    }

    /// The runs in flight.
    #[must_use]
    pub fn keys(&self) -> Vec<RunKey> {
        self.runs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .cloned() // BORROW: the caller owns the keys
            .collect()
    }
}

/// Tells every run in `live` to stop with `cause`. A run that has ended is not in `live`, and a
/// run that has not started has no channel, so neither hears it.
pub fn stop_live(live: &LiveRuns, cause: JobCause) {
    let runs = live.runs.lock().unwrap_or_else(PoisonError::into_inner);
    for stop in runs.values() {
        stop.send_replace(Some(cause));
    }
}
