//! The lock that says a supervisor owns a job; a second holder gets `JobError::Held`.
//!
//! The lock is an exclusive `flock` on the job's `lock` file. The kernel releases it when the
//! holder dies, so a job whose lock is free and whose log has no `job.ended` was lost.

use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use candler_schemas::primitives::JobId;
use rustix::fs::{FlockOperation, flock};
use rustix::io::Errno;

use super::dir::{JobDir, JobError, io_error};
use super::view::JobView;

/// The file the lock is taken on.
const LOCK_FILE: &str = "lock";
/// How long [`JobLock::acquire`] retries, so that a concurrent probe is never read as a holder.
const RETRY_FOR: Duration = Duration::from_millis(500);
/// The pause between two tries.
const RETRY_EVERY: Duration = Duration::from_millis(10);

/// Whether some process holds the lock of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LockState {
    /// A process holds the lock.
    Held,
    /// Nobody holds the lock.
    Free,
}

/// The lock of a job, held for the life of the value.
#[derive(Debug)]
pub struct JobLock {
    // The descriptor holds the lock; dropping it releases it.
    _file: File,
    // The directory the lock was taken on, and so the job it names.
    dir: JobDir,
}

fn open_lock(dir: &JobDir) -> Result<(File, PathBuf), JobError> {
    let path = dir.path().join(LOCK_FILE);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(io_error("open the job lock", &path))?;
    Ok((file, path))
}

fn try_lock(file: &File, path: &std::path::Path) -> Result<bool, JobError> {
    match flock(file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(true),
        Err(Errno::WOULDBLOCK) => Ok(false),
        Err(error) => Err(JobError::Io {
            op: "lock the job",
            path: path.to_path_buf(),
            detail: error.to_string(),
        }),
    }
}

impl JobLock {
    /// Takes the lock, retrying for about 500 ms.
    ///
    /// # Errors
    /// [`JobError::Held`] when another process still holds it, [`JobError::Io`] when the lock
    /// cannot be tried.
    pub fn acquire(dir: &JobDir) -> Result<Self, JobError> {
        let (file, path) = open_lock(dir)?;
        let started = Instant::now();
        loop {
            if try_lock(&file, &path)? {
                return Ok(Self {
                    _file: file,
                    dir: dir.clone(), // BORROW: the lock owns its directory
                });
            }
            if started.elapsed() >= RETRY_FOR {
                return Err(JobError::Held);
            }
            std::thread::sleep(RETRY_EVERY);
        }
    }

    /// The job whose lock this is.
    #[must_use]
    pub fn job<'a>(&'a self) -> &'a JobId {
        self.dir.job()
    }

    /// The directory of the job whose lock this is.
    #[must_use]
    pub(crate) fn dir<'a>(&'a self) -> &'a JobDir {
        &self.dir
    }

    /// Whether the lock is held, without keeping it. Only `EWOULDBLOCK` reads as held; any
    /// other error is an error, never "free".
    ///
    /// # Errors
    /// [`JobError::Io`] when the lock cannot be tried.
    pub fn probe(dir: &JobDir) -> Result<LockState, JobError> {
        let (file, path) = open_lock(dir)?;
        if try_lock(&file, &path)? {
            Ok(LockState::Free)
        } else {
            Ok(LockState::Held)
        }
    }
}
