//! The directory of one job.

use std::path::{Path, PathBuf};

use candler_schemas::primitives::JobId;

use super::view::{ChildEntry, JobView, JobViewError};

/// Why a job directory operation failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum JobError {
    /// The operation is not implemented.
    #[error("job directory is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
    /// Another process holds the job's lock.
    #[error("the job is held by another process")]
    Held,
}

/// A job's directory and the records in it.
#[derive(Debug, Clone)]
pub struct JobDir {
    job: JobId,
    path: PathBuf,
}

impl JobDir {
    /// The directory `path` of the job `job`.
    #[must_use]
    pub const fn new(job: JobId, path: PathBuf) -> Self {
        Self { job, path }
    }

    /// The directory's path.
    #[must_use]
    pub fn path<'a>(&'a self) -> &'a Path {
        self.path.as_path()
    }
}

impl JobView for JobDir {
    fn job<'a>(&'a self) -> &'a JobId {
        &self.job
    }

    fn children(&self) -> Result<Vec<ChildEntry>, JobViewError> {
        Err(JobViewError::Unavailable {
            detail: "JobDir::children".to_owned(),
        })
    }

    fn is_lost(&self, job: &JobId) -> Result<bool, JobViewError> {
        let _ = job;
        Err(JobViewError::Unavailable {
            detail: "JobDir::is_lost".to_owned(),
        })
    }
}
