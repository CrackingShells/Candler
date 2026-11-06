//! The detached supervisor of a job: it reads the job's spec, takes the job's lock and runs the
//! body under a supervisor of its own.

pub mod detach;
pub mod reap;
pub mod session;
pub mod signals;
pub mod sweep;

use std::sync::Arc;

use candler_harness::driver::HarnessDriver;
use candler_schemas::primitives::JobId;
use futures::future::BoxFuture;

use super::model::{JobBody, JobOutcome, JobSpec};
use crate::runner::exec::SkillTree;
use crate::store::Store;

/// What the harness gives every run of a job. Core cannot name the harness's crate, so the
/// binary that composes the job builds these and hands them to the supervisor.
#[derive(Clone)]
pub struct JobServices {
    // TRAIT_OBJECT: the harness's driver, or a recording fake in tests
    /// What the runner needs from the harness, as data.
    pub driver: Arc<dyn HarnessDriver>,
    // TRAIT_OBJECT: the harness's archive reader, or a fake in tests
    /// Where a treatment's skill tree comes from.
    pub skills: Arc<dyn SkillTree>,
}

/// Everything the supervisor of one job starts from. The spec's digest was checked against
/// `job.json` before this is built.
pub struct JobEntry {
    /// The store the job's directory is in.
    pub store: Store,
    /// The job.
    pub job: JobId,
    /// The job's spec, read from the supervisor's stdin.
    pub spec: JobSpec,
    /// What the harness gives every run.
    pub services: JobServices,
}

/// Why the supervisor of a job failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RuntimeError {
    /// The supervisor is not implemented.
    #[error("job runtime is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
}

/// Runs `body` as the supervisor of the job `entry` names.
pub fn supervise_entry<'a>(
    entry: JobEntry,
    body: JobBody,
) -> BoxFuture<'a, Result<JobOutcome, RuntimeError>> {
    let _ = (entry, body);
    Box::pin(async {
        Err(RuntimeError::Unavailable {
            detail: "supervise_entry".to_owned(),
        })
    })
}
