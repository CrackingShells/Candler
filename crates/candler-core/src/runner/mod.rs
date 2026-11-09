//! Running one cell: planning the runs of a job, executing a run in its container and collecting
//! the evidence the run leaves.
//!
//! A [`Runtime`] holds everything a run needs; the job supervisor builds one and hands a clone of
//! its parts to each run.

pub mod evidence;
pub mod exec;
pub mod plan;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use candler_harness::driver::HarnessDriver;
use candler_schemas::execution::InvalidReason;
use candler_schemas::primitives::{JobId, RunKey};
use candler_schemas::trace::EndedBy;
use tokio::sync::watch;

use crate::isolation::Isolation;
use crate::isolation::driver::DockerDriver;
use crate::isolation::gateway::Gateway;
use crate::isolation::model::{GatewayHandle, Inspect};
use crate::jobs::model::JobCause;
use crate::jobs::slots::Slots;
use crate::store::Store;
use crate::supervise::{EventSink, Supervisor};

use self::evidence::provenance::EvalInfra;
use self::exec::ends::Retry;
use self::exec::{CellRun, SkillTree};
use self::plan::CellId;

/// What a run needs from the job around it.
pub struct Runtime {
    /// The supervisor that spawns and stops the run's children.
    pub supervisor: Arc<Supervisor>,
    /// The store the run is written to.
    pub store: Store,
    // TRAIT_OBJECT: the sink is the job's event log, or a recorder in tests
    /// Where the run's events go.
    pub sink: Arc<dyn EventSink>,
    /// The slots that bound parallelism.
    pub slots: Slots,
    // TRAIT_OBJECT: the driver is docker, or a recording fake in tests
    /// The read half of docker.
    pub docker: Arc<dyn DockerDriver>,
    // TRAIT_OBJECT: the isolation is docker, or a fake in tests
    /// How the run's agent is placed in a container. `None` until the job's gateway has started;
    /// a run that finds `None` fails and never runs on the host.
    pub isolation: Option<Arc<dyn Isolation>>,
    /// The job's gateway. `None` until it has started.
    pub gateway: Option<JobGateway>,
    /// The job the runs belong to.
    pub job: JobId,
    // TRAIT_OBJECT: the driver is the harness's, or a recording fake in tests
    /// What the runner needs from the harness, as data.
    pub driver: Arc<dyn HarnessDriver>,
    // TRAIT_OBJECT: the tree is read through the harness's archive reader, or a fake in tests
    /// Where a treatment's skill tree comes from.
    pub skills: Arc<dyn SkillTree>,
    /// What each cell of the job is, by its digest.
    pub cells: BTreeMap<CellId, Arc<CellRun>>,
    /// The build of the evaluation infrastructure that runs the job.
    pub eval_infra: EvalInfra,
    /// The import package whose distribution and version the job records for the env image;
    /// none records nothing.
    pub planner_package: Option<String>,
}

/// The job's gateway: the service that talks to it and the handle of the running container.
pub struct JobGateway {
    // TRAIT_OBJECT: the service is the gateway container, or a fake in tests
    /// The service.
    pub service: Arc<dyn Gateway>,
    /// The running gateway.
    pub handle: GatewayHandle,
}

/// Tokens and cost a run spent.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Usage {
    /// The tokens spent.
    pub tokens: u64,
    /// The cost in US dollars.
    pub cost_usd: f64,
}

/// How a run came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunStatus {
    /// The run finished and is measured.
    Measured,
    /// The run is invalid.
    Invalid,
}

/// What a run did.
#[derive(Debug, Clone, PartialEq)]
pub struct RunOutcome {
    /// The run.
    pub key: RunKey,
    /// Whether the run is measured or invalid.
    pub status: RunStatus,
    /// How the agent's session ended.
    pub ended_by: EndedBy,
    /// Why the run is invalid, when it is.
    pub invalid: Option<InvalidReason>,
    /// The container's final inspect, when it could be taken.
    pub inspect: Option<Inspect>,
    /// What the run spent.
    pub usage: Usage,
    /// Whether the caller opens another attempt.
    pub retry: Retry,
}

/// What the job tells a run while it runs.
pub struct RunControl {
    /// A cause arrives here when the job stops.
    pub cancel: watch::Receiver<Option<JobCause>>,
    /// The job's running totals.
    pub meter: Arc<JobMeter>,
}

/// The job's running totals of tokens and cost, shared by its runs.
#[derive(Debug, Default)]
pub struct JobMeter {
    tokens: AtomicU64,
    micro_usd: AtomicU64,
}

impl JobMeter {
    /// A meter at zero.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tokens: AtomicU64::new(0),
            micro_usd: AtomicU64::new(0),
        }
    }

    /// Adds `tokens` and `micro_usd` (millionths of a dollar) to the totals.
    pub fn add(&self, tokens: u64, micro_usd: u64) {
        self.tokens.fetch_add(tokens, Ordering::Relaxed);
        self.micro_usd.fetch_add(micro_usd, Ordering::Relaxed);
    }

    /// The tokens so far.
    #[must_use]
    pub fn tokens(&self) -> u64 {
        self.tokens.load(Ordering::Relaxed)
    }

    /// The cost so far, in millionths of a dollar.
    #[must_use]
    pub fn micro_usd(&self) -> u64 {
        self.micro_usd.load(Ordering::Relaxed)
    }
}
