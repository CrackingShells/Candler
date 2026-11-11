//! What a job is given and what it returns.

use std::sync::Arc;

use candler_schemas::execution::{Budget, DigestRef, ExperimentFile, JobEnd};
use futures::future::BoxFuture;
use tokio::sync::watch;

use crate::resolve::layers::Resolution;
use crate::runner::evidence::InputDigest;
use crate::runner::plan::RunPlan;
use crate::runner::{RunOutcome, Runtime};

/// Everything the supervisor of a job needs, written to the job directory by `candler run` and
/// read back by the detached supervisor.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JobSpec {
    /// The experiment file.
    pub experiment: ExperimentFile,
    /// The resolution of the experiment.
    pub resolution: Resolution,
    /// The plans of the runs to start, in order.
    pub plans: Vec<RunPlan>,
    /// The job's budget.
    pub budget: Budget,
    /// Whether `--force` was given: the supervisor plans again under the experiment lock and
    /// must accept the same deviations the foreground check accepted.
    pub force: bool,
    /// The files every run of the job is given, by workspace path, with the digest of the blob
    /// in the store each is mounted from read-only.
    pub inputs: Vec<InputDigest>,
    /// The evaluator-only inputs as `{kind, digest}`: they are recorded in each run's manifest and
    /// never mounted, copied or shown to the agent.
    pub evaluation_inputs: Vec<DigestRef>,
}

/// What the job directory's spec file holds: the spec of the kind of job it is. A run job's file
/// is the bytes of its [`JobSpec`] and no more, so the digest the job record keeps is the digest of
/// the same document. The enum is exhaustive on purpose: a match over it in this workspace fails to
/// build when a kind of job is added, instead of falling into a wildcard arm.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
#[expect(
    clippy::exhaustive_enums,
    reason = "the binary matches every kind of job, and a new kind must fail its build"
)]
pub enum JobSpecFile {
    /// The spec of a job `candler run` starts.
    Run(JobSpec),
}

impl From<JobSpec> for JobSpecFile {
    fn from(spec: JobSpec) -> Self {
        Self::Run(spec)
    }
}

/// How a job ended and what its runs did.
#[derive(Debug, Clone, PartialEq)]
pub struct JobOutcome {
    /// How the job ended.
    pub end: JobEnd,
    /// The outcome of every run the job started.
    pub runs: Vec<RunOutcome>,
}

/// Why a job is being stopped from outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum JobCause {
    /// `candler kill`.
    Killed,
    /// A job cap was reached.
    BudgetStop,
}

/// The handle the job's supervisor uses to stop the job.
#[derive(Debug, Clone)]
pub struct JobControl {
    /// Sending a cause stops every run; runs hold the receiving side.
    pub cancel: Arc<watch::Sender<Option<JobCause>>>,
}

/// The body of a job: what the supervisor runs once it owns the job.
pub type JobBody = fn(Runtime, JobSpec, JobControl) -> BoxFuture<'static, JobOutcome>;
