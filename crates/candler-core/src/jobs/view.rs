//! A read-only view of a job's records, the one the container guard decides from.

use std::path::PathBuf;

use candler_schemas::container::ContainerRecord;
use candler_schemas::primitives::{ContainerId, JobId};
use candler_schemas::process::ChildRecord;

/// One record in the job directory.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ChildEntry {
    /// A process child.
    Process(ChildRecord),
    /// A container.
    Container(ContainerRecord),
    /// A record that could not be read.
    Unreadable {
        /// The file.
        path: PathBuf,
        /// Why it could not be read.
        error: String,
    },
}

/// Why the view could not answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum JobViewError {
    /// The view is not implemented.
    #[error("job view is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
    /// The directory could not be read.
    #[error("the job directory could not be read: {detail}")]
    Read {
        /// What went wrong.
        detail: String,
    },
}

/// What the guard may ask about a job.
pub trait JobView: Send + Sync {
    /// The job this view is of.
    fn job<'a>(&'a self) -> &'a JobId;

    /// The records of the children and containers the job started.
    ///
    /// # Errors
    /// [`JobViewError::Read`] when the directory cannot be read.
    fn children(&self) -> Result<Vec<ChildEntry>, JobViewError>;

    /// The record of the container `cid`, if the job has a readable one.
    ///
    /// # Errors
    /// [`JobViewError::Read`] when the directory cannot be read.
    fn container(&self, cid: &ContainerId) -> Result<Option<ContainerRecord>, JobViewError> {
        Ok(self.children()?.into_iter().find_map(|entry| match entry {
            ChildEntry::Container(record) if record.cid() == cid => Some(record),
            ChildEntry::Container(_) | ChildEntry::Process(_) | ChildEntry::Unreadable { .. } => {
                None
            }
        }))
    }

    /// Whether `job` is a job whose supervisor was lost.
    ///
    /// # Errors
    /// [`JobViewError::Read`] when the directory cannot be read.
    fn is_lost(&self, job: &JobId) -> Result<bool, JobViewError>;
}
