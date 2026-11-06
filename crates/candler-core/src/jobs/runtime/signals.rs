//! SIGTERM and SIGINT, turned into the one cause a job is stopped by.

use std::io;
use std::sync::Arc;

use tokio::signal::unix::{Signal, SignalKind, signal};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::jobs::model::{JobCause, JobControl};

/// The handler of a supervisor's termination signals. While it lives, SIGTERM and SIGINT reach
/// the job's body as [`JobCause::Killed`] and do nothing else: the handler signals no process
/// and re-raises nothing, so every stop goes through the ladders of the runs.
#[derive(Debug)]
pub struct SignalWatch {
    task: JoinHandle<()>,
}

/// Waits for either stream; `false` when the runtime is gone.
async fn next(term: &mut Signal, int: &mut Signal) -> bool {
    tokio::select! {
        got = term.recv() => got.is_some(),
        got = int.recv() => got.is_some(),
    }
}

impl SignalWatch {
    /// Registers the handlers before returning, so a signal that arrives before the task first
    /// runs is queued and not lost, and sends [`JobCause::Killed`] on `control` for the first
    /// one. The value is stored, so a body that looks after the signal still sees it.
    ///
    /// # Errors
    /// The system's error when a handler cannot be registered.
    pub fn install(control: JobControl) -> io::Result<Self> {
        let mut term = signal(SignalKind::terminate())?;
        let mut int = signal(SignalKind::interrupt())?;
        let cancel: Arc<watch::Sender<Option<JobCause>>> = control.cancel;
        let task = tokio::spawn(async move {
            while next(&mut term, &mut int).await {
                cancel.send_replace(Some(JobCause::Killed));
            }
        });
        Ok(Self { task })
    }
}

impl Drop for SignalWatch {
    fn drop(&mut self) {
        self.task.abort();
    }
}
