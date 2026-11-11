//! The detached supervisor of a job: it reads the job's spec, takes the job's lock and runs the
//! body under a supervisor of its own.

pub mod detach;
pub mod evaluate;
pub mod launch;
pub mod reap;
pub mod session;
pub mod signals;
pub mod sweep;

use std::sync::Arc;
use std::time::Duration;

use candler_harness::driver::HarnessDriver;
use candler_schemas::primitives::JobId;
use futures::future::BoxFuture;

use super::model::{JobBody, JobOutcome, JobSpec};
use crate::jobs::orchestrate::teardown::TEARDOWN_ALLOWANCE;
use crate::runner::evidence::provenance::EvalInfra;
use crate::runner::exec::SkillTree;
use crate::store::Store;
use crate::supervise::spawn::DEFAULT_GRACE;

/// How long a run's ladder waits between the terminate and the kill of its group's survivors.
const LADDER_KILL_WAIT: Duration = Duration::from_secs(2);
/// How long docker is given to stop a container before it kills it.
const CONTAINER_STOP_GRACE: Duration = Duration::from_secs(5);
/// What the job end allows for the last events, the seal of its record and the lock's release.
const SEAL_ALLOWANCE: Duration = Duration::from_secs(10);

/// How long a stopped job takes to end in order, from the signal to the released lock: the
/// grace of a run's stop ladder (`DEFAULT_GRACE`), the wait after its kill (`LADDER_KILL_WAIT`),
/// the stop grace of a container (`CONTAINER_STOP_GRACE`), the bound of the orphan reap
/// (`reap::REAP_BOUND`), the session's teardown (`TEARDOWN_ALLOWANCE`: the label sweep, then the
/// gateway and the networks) and the allowance for sealing (`SEAL_ALLOWANCE`).
pub const TEARDOWN_BOUND: Duration = DEFAULT_GRACE
    .saturating_add(LADDER_KILL_WAIT)
    .saturating_add(CONTAINER_STOP_GRACE)
    .saturating_add(reap::REAP_BOUND)
    .saturating_add(TEARDOWN_ALLOWANCE)
    .saturating_add(SEAL_ALLOWANCE);

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
    /// Which build of the evaluation infrastructure runs the job.
    pub eval_infra: EvalInfra,
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
    /// The spec's bytes do not match the digest `job.json` records, or are not a spec.
    #[error("the job's spec was refused: {detail}")]
    SpecDigest {
        /// What did not match.
        detail: String,
    },
    /// Another supervisor holds the job's lock.
    #[error("the job is already supervised")]
    AlreadySupervised,
    /// The supervisor's process could not take its place: its session, its subreaper role or its
    /// signal handlers.
    #[error("the supervisor could not set itself up: {detail}")]
    Session {
        /// What failed.
        detail: String,
    },
    /// The job's directory, lock or log failed.
    #[error("the job's directory failed: {detail}")]
    Job {
        /// What failed.
        detail: String,
    },
    /// What the job runs on could not be set up: the configuration, the slots or docker.
    #[error("the job could not be set up: {detail}")]
    Setup {
        /// What failed.
        detail: String,
    },
    /// The body did not return an outcome.
    #[error("the job's body failed: {detail}")]
    Body {
        /// What failed.
        detail: String,
    },
    /// The detached supervisor did not take the job's lock.
    #[error("the supervisor did not start: see {log}")]
    DidNotStart {
        /// The job's directory.
        job_dir: String,
        /// The log the supervisor writes to.
        log: String,
    },
    /// The supervisor could not be started.
    #[error("the supervisor could not be started: {detail}")]
    Detach {
        /// What failed.
        detail: String,
    },
}

/// Runs `body` as the supervisor of the job `entry` names.
pub fn supervise_entry<'a>(
    entry: JobEntry,
    body: JobBody,
) -> BoxFuture<'a, Result<JobOutcome, RuntimeError>> {
    Box::pin(session::run(entry, body))
}
