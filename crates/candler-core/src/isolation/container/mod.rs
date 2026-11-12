//! The container half of isolation: mounts, the run command, expiry, the stop hook, the guard and
//! the label sweeper, and the [`DockerIsolation`] that ties them together.

pub mod expiry;
pub mod guard;
pub mod mounts;
pub mod run;
pub mod stop;
pub mod sweeper;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use candler_harness::spec::{Cell, CommandSpec, Workspace};
use candler_schemas::container::{ContainerLabels, ContainerRecord, ContainerRole};
use candler_schemas::primitives::{ContainerId, Digest, FieldPath, JobId, Marker, RunKey};
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;
use sha2::{Digest as _, Sha256};

use self::guard::{ContainerGuard, GuardError};
use self::mounts::{MountMode, MountRoots, MountSpec, mounts};
use self::run::build_run_request;
use self::stop::ContainerStopHook;
use self::sweeper::ContainerSweeper;
use super::driver::DockerDriver;
use super::gateway::ChildRecorder;
use super::layout::{
    CONTAINER_HOME, CONTAINER_WORKDIR, LABEL_EXPERIMENT, LABEL_JOB, LABEL_ROLE, LABEL_RUN,
};
use super::model::{
    ContainerOutcome, ContainerPlan, ContainerRef, ContainerSweep, GatewayHandle, ImageId,
    ImageRef, Inspect, LabelSel, Limits, Refusal, SweepScope, UserSpec,
};
use super::{EndFacts, IsolatedRequest, Isolation, IsolationError, RunOutputs};
use crate::experiment::paths;
use crate::supervise::hook::StopHook;
use crate::supervise::stop::Exit;

/// The process limit of a container whose cell sets none.
const DEFAULT_PIDS: u32 = 512;

/// The memory limit of a container whose cell sets none.
const DEFAULT_MEMORY: &str = "4g";

/// The cpu limit of a container whose cell sets none.
const DEFAULT_CPUS: f64 = 2.0;

/// The `PATH` of the agent: the environment layer first.
const AGENT_PATH: &str = "/opt/env/bin:/usr/local/bin:/usr/bin:/bin";

/// The virtual environment the image carries.
const AGENT_VENV: &str = "/opt/env";

/// The locale of the agent.
const AGENT_LANG: &str = "C.UTF-8";

/// The host directories a plan is made against, which no cell holds because a cell holds no
/// absolute path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerDirs {
    /// The job's `children/` directory, where the container's cidfile and the docker client's
    /// record are written.
    pub children: PathBuf,
    /// The project root a mount source may sit under.
    pub project_root: PathBuf,
    /// The store root a mount source may sit under.
    pub store_root: PathBuf,
}

/// Isolation through docker containers. It is built once per job, after the gateway starts, and
/// holds no plan and no hook: both belong to one run.
pub struct DockerIsolation {
    gateway: GatewayHandle,
    // TRAIT_OBJECT: the driver is docker, or a recording fake in tests
    docker: Arc<dyn DockerDriver>,
    guard: Arc<ContainerGuard>,
    // TRAIT_OBJECT: the sweeper is the label sweeper, or a fake in tests
    sweeper: Option<Arc<dyn ContainerSweeper>>,
    // TRAIT_OBJECT: the recorder is the job directory, or a fake in tests
    recorder: Option<Arc<dyn ChildRecorder>>,
    recorded: Mutex<BTreeSet<ContainerId>>,
    dirs: Option<DockerDirs>,
}

/// The end facts of a request, read from its own hook.
struct HookFacts {
    // The hook is the request's own; the facts are what it kept.
    hook: Arc<ContainerStopHook>,
}

impl EndFacts for HookFacts {
    fn final_inspect(&self) -> Option<ContainerOutcome> {
        self.hook.final_inspect()
    }
}

/// The host directory a run's agent container mounts as its home, under the run directory
/// `run_dir`. The harness's state directories are made here too, so the mounted directory and
/// the state paths are one place.
#[must_use]
pub fn agent_home(run_dir: &Path) -> PathBuf {
    run_dir.join("home")
}

impl DockerIsolation {
    /// An isolation that reaches the provider through `gateway`.
    #[must_use]
    pub fn new(
        gateway: GatewayHandle,
        docker: Arc<dyn DockerDriver>,
        guard: Arc<ContainerGuard>,
    ) -> Self {
        // TRAIT_OBJECT: the driver is docker, or a recording fake in tests
        Self {
            gateway,
            docker,
            guard,
            sweeper: None,
            recorder: None,
            recorded: Mutex::new(BTreeSet::new()),
            dirs: None,
        }
    }

    /// The same isolation whose stop hooks sweep through `sweeper`.
    #[must_use]
    pub fn with_sweeper(mut self, sweeper: Arc<dyn ContainerSweeper>) -> Self {
        // TRAIT_OBJECT: the sweeper is chosen by the job runtime and by tests
        self.sweeper = Some(sweeper);
        self
    }

    /// The same isolation whose agent containers are recorded through `recorder`.
    #[must_use]
    pub fn with_recorder(mut self, recorder: Arc<dyn ChildRecorder>) -> Self {
        // TRAIT_OBJECT: the recorder is chosen by the job runtime and by tests
        self.recorder = Some(recorder);
        self
    }

    /// The same isolation planning against `dirs`.
    #[must_use]
    pub fn with_dirs(mut self, dirs: DockerDirs) -> Self {
        self.dirs = Some(dirs);
        self
    }

    /// The gateway the agent's containers reach the provider through.
    #[must_use]
    pub const fn gateway<'a>(&'a self) -> &'a GatewayHandle {
        &self.gateway
    }

    /// The read driver the isolation inspects through.
    #[must_use]
    pub fn docker(&self) -> Arc<dyn DockerDriver> {
        Arc::clone(&self.docker)
    }

    /// The guard that destroys the run's containers.
    #[must_use]
    pub fn guard(&self) -> Arc<ContainerGuard> {
        Arc::clone(&self.guard)
    }
}

/// The value of the cell at `path`.
fn cell_value<'a>(
    cell: &Cell<'a>,
    path: &str,
) -> Result<Option<&'a serde_json::Value>, IsolationError> {
    let field = FieldPath::from_str(path).map_err(|source| IsolationError::Plan {
        detail: source.to_string(),
    })?;
    Ok(cell.get(&field).map(JsonValue::as_value))
}

fn plan_error(detail: &str) -> IsolationError {
    IsolationError::Plan {
        // BORROW: .to_owned() keeps the description in the error
        detail: detail.to_owned(),
    }
}

/// The hex of the first eight bytes of the digest of `text`.
fn short_digest(text: &[u8]) -> String {
    Sha256::digest(text)
        .iter()
        .take(8)
        .fold(String::new(), |mut hex, byte| {
            // INFALLIBLE: writing to a String cannot fail
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// The user the container runs as: the host user on a Linux engine, the image's own elsewhere.
#[cfg(target_os = "linux")]
fn container_user() -> UserSpec {
    UserSpec::Host {
        uid: rustix::process::getuid().as_raw(),
        gid: rustix::process::getgid().as_raw(),
    }
}

/// The user the container runs as: the host user on a Linux engine, the image's own elsewhere.
#[cfg(not(target_os = "linux"))]
const fn container_user() -> UserSpec {
    UserSpec::Engine
}

/// The limits the cell sets, over the defaults.
fn cell_limits(cell: &Cell<'_>) -> Result<Limits, IsolationError> {
    let pids = match cell_value(cell, paths::ISOLATION_LIMITS_PIDS)? {
        None => DEFAULT_PIDS,
        Some(value) => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| plan_error("isolation.limits.pids is not a process count"))?,
    };
    let memory = match cell_value(cell, paths::ISOLATION_LIMITS_MEMORY)? {
        None => String::from(DEFAULT_MEMORY),
        // BORROW: .to_owned() copies the limit out of the cell
        Some(value) => value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| plan_error("isolation.limits.memory is not a string"))?,
    };
    let cpus = match cell_value(cell, paths::ISOLATION_LIMITS_CPUS)? {
        None => DEFAULT_CPUS,
        Some(value) => value
            .as_f64()
            .filter(|n| n.is_finite() && *n > 0.0)
            .ok_or_else(|| plan_error("isolation.limits.cpus is not a positive number"))?,
    };
    Ok(Limits {
        pids_limit: pids,
        memory,
        cpus,
    })
}

impl Isolation for DockerIsolation {
    fn plan(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<ContainerPlan, IsolationError> {
        let dirs = self
            .dirs
            .as_ref()
            .ok_or_else(|| plan_error("the isolation has no directories to plan against"))?;
        let image = cell_value(cell, paths::ISOLATION_IMAGE)?
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| plan_error("isolation.image is missing"))?;
        let (network, egress) =
            match cell_value(cell, paths::ISOLATION_NETWORK)?.and_then(serde_json::Value::as_str) {
                // BORROW: the owned copy is made where it is needed
                None | Some("internal") => (self.gateway.network.clone(), None),
                // BORROW: the owned copy is made where it is needed
                Some("bridge") => (
                    self.gateway.network.clone(),
                    Some(self.gateway.egress.clone()),
                ),
                Some(_) => {
                    return Err(plan_error(
                        "isolation.network is neither internal nor bridge",
                    ));
                }
            };
        let wall = cell_value(cell, paths::BUDGET_PER_RUN_WALL_S)?
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| plan_error("budget.per_run.wall_s is missing"))?;
        let job = self
            .gateway
            .container
            .labels
            .get(LABEL_JOB)
            .ok_or_else(|| plan_error("the gateway container carries no job label"))?;
        let mut labels = super::model::Labels::new()
            .with(LABEL_JOB, job)
            .with(LABEL_ROLE, "agent");
        if let Some(experiment) = self.gateway.container.labels.get(LABEL_EXPERIMENT) {
            labels = labels.with(LABEL_EXPERIMENT, experiment);
        }
        let root = workspace.root();
        let run_dir = root
            .parent()
            .ok_or_else(|| plan_error("the workspace has no run directory"))?;
        let home = agent_home(run_dir);
        std::fs::create_dir_all(&home).map_err(|source| IsolationError::Plan {
            detail: format!("the home directory cannot be made: {source}"),
        })?;
        let mut plan = ContainerPlan {
            image: ImageRef {
                // BORROW: the owned copy is made where it is needed
                id: ImageId(image.to_owned()),
                harness_version: None,
            },
            labels,
            mounts: vec![
                MountSpec {
                    // BORROW: .to_path_buf() copies the root into the mount
                    source: root.to_path_buf(),
                    target: String::from(CONTAINER_WORKDIR),
                    mode: MountMode::ReadWrite,
                },
                MountSpec {
                    source: home,
                    target: String::from(CONTAINER_HOME),
                    mode: MountMode::ReadWrite,
                },
            ],
            env: vec![
                (String::from("PATH"), String::from(AGENT_PATH)),
                (String::from("HOME"), String::from(CONTAINER_HOME)),
                (String::from("VIRTUAL_ENV"), String::from(AGENT_VENV)),
                (String::from("LANG"), String::from(AGENT_LANG)),
            ],
            gateway: None,
            state: Vec::new(),
            network,
            egress,
            limits: cell_limits(cell)?,
            cidfile: dirs.children.join(format!(
                "agent-{}.cid",
                short_digest(root.as_os_str().as_encoded_bytes())
            )),
            user: container_user(),
            workdir: String::from(CONTAINER_WORKDIR),
            home: String::from(CONTAINER_HOME),
            expiry: Duration::from_secs(wall),
            // BORROW: the roots are copied out of the directories
            project_root: dirs.project_root.clone(),
            // BORROW: the roots are copied out of the directories
            store_root: dirs.store_root.clone(),
        };
        plan.mounts = mounts(&plan, &MountRoots::from_plan(&plan)).map_err(|source| {
            IsolationError::Plan {
                detail: source.to_string(),
            }
        })?;
        Ok(plan)
    }

    fn run_request(
        &self,
        plan: &ContainerPlan,
        command: CommandSpec,
        marker: Marker,
        key: RunKey,
        outputs: RunOutputs,
    ) -> Result<IsolatedRequest, IsolationError> {
        let base_url = self.gateway.run_base_url(&key);
        let sweeper =
            self.sweeper
                .as_ref()
                .map(Arc::clone)
                .ok_or_else(|| IsolationError::Unavailable {
                    detail: String::from("DockerIsolation::with_sweeper"),
                })?;
        let job: JobId = plan
            .labels
            .get(LABEL_JOB)
            .and_then(|text| text.parse().ok())
            .ok_or_else(|| IsolationError::Plan {
                detail: format!("the plan carries no valid `{LABEL_JOB}` label"),
            })?;
        let hook = Arc::new(ContainerStopHook::new(self.guard(), sweeper, job));
        // TRAIT_OBJECT: the request carries its hook as the supervisor's trait
        let attached: Arc<dyn StopHook> = Arc::<ContainerStopHook>::clone(&hook);
        let request = build_run_request(
            plan,
            &command,
            marker,
            &key,
            &base_url,
            &self.gateway.dummy_token,
            Some(attached),
            outputs,
        )
        .map_err(|source| IsolationError::Plan {
            detail: source.to_string(),
        })?;
        // TRAIT_OBJECT: the caller reads the facts without the docker type
        let end_facts: Arc<dyn EndFacts> = Arc::new(HookFacts { hook });
        Ok(IsolatedRequest::new(request, end_facts))
    }

    fn collect_and_destroy<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        workspace: &'a Workspace<'a>,
        exit: &'a Exit,
        key: &'a RunKey,
    ) -> BoxFuture<'a, Result<ContainerOutcome, IsolationError>> {
        let _ = (workspace, exit);
        Box::pin(async move {
            let cid = std::fs::read_to_string(&plan.cidfile)
                .ok()
                .and_then(|text| ContainerId::from_str(text.trim()).ok());
            let Some(cid) = cid else {
                // The client wrote no cidfile: no container of this run is known to destroy.
                return Ok(ContainerOutcome {
                    inspect: None,
                    sweep: ContainerSweep {
                        unknown: true,
                        ..ContainerSweep::default()
                    },
                });
            };
            let reader = self.guard.reader();
            // BORROW: .as_str() hands the id to docker
            let inspect = reader.inspect(cid.as_str()).await.ok();
            let target = match &inspect {
                Some(taken) => Some(ContainerRef::new(
                    taken.id.clone(),      // BORROW: the reference owns the id
                    taken.labels.clone(),  // BORROW: the reference owns the labels
                    taken.created.clone(), // BORROW: the reference owns the time
                )),
                None => self.listed_agent(plan, &cid).await,
            };
            let mut sweep = ContainerSweep::default();
            let Some(target) = target else {
                sweep.unknown = true;
                return Ok(ContainerOutcome { inspect, sweep });
            };
            // The plan's job label is what the container must carry; the run is the caller's.
            if target.labels.get(LABEL_JOB) != plan.labels.get(LABEL_JOB) {
                sweep.refused.push(Refusal {
                    container: target.cid,
                    reason: String::from("job_mismatch"),
                });
                return Ok(ContainerOutcome { inspect, sweep });
            }
            let scope = SweepScope::Run(key.clone()); // BORROW: the scope owns its key
            match self.guard.destroy(&target, &scope).await {
                Ok(destroyed) if !destroyed.absent => sweep.destroyed.push(target),
                Ok(_) => {}
                Err(GuardError::Refused(why)) => sweep.refused.push(Refusal {
                    container: target.cid,
                    reason: why.code().to_owned(), // BORROW: owned reason code
                }),
                Err(GuardError::Docker { .. } | GuardError::View { .. }) => sweep.unknown = true,
            }
            Ok(ContainerOutcome { inspect, sweep })
        })
    }

    fn record_container<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        key: &'a RunKey,
    ) -> BoxFuture<'a, Result<Option<ContainerId>, IsolationError>> {
        Box::pin(async move {
            let recorder = self
                .recorder
                .as_ref()
                .ok_or_else(|| IsolationError::Unavailable {
                    detail: String::from("no recorder is attached to the isolation"),
                })?;
            let Some(cid) = std::fs::read_to_string(&plan.cidfile)
                .ok()
                .and_then(|text| ContainerId::from_str(text.trim()).ok())
            else {
                return Ok(None);
            };
            if self
                .recorded
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(&cid)
            {
                return Ok(Some(cid));
            }
            // BORROW: .as_str() hands the id to docker
            let Ok(inspect) = self.guard.reader().inspect(cid.as_str()).await else {
                return Ok(None);
            };
            let record = Self::agent_record(plan, key, &inspect)?;
            recorder
                .record_child(&record)
                .map_err(|error| IsolationError::Unavailable {
                    detail: format!("the container record cannot be written: {error}"),
                })?;
            self.recorded
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(cid.clone()); // BORROW: the set keeps its own id
            Ok(Some(cid))
        })
    }
}

impl DockerIsolation {
    /// The `container-record/1` of the agent container `inspect` describes, for the run `key` of
    /// the plan's job.
    fn agent_record(
        plan: &ContainerPlan,
        key: &RunKey,
        inspect: &Inspect,
    ) -> Result<ContainerRecord, IsolationError> {
        let label = |name: &str| {
            inspect
                .labels
                .get(name)
                .ok_or_else(|| plan_error("the agent container lacks a candler label"))
        };
        let job: JobId = label(LABEL_JOB)?
            .parse()
            .map_err(|_| plan_error("the agent container's job label is not a job id"))?;
        let planned: Option<JobId> = plan
            .labels
            .get(LABEL_JOB)
            .and_then(|text| text.parse().ok());
        if planned.as_ref() != Some(&job) {
            return Err(plan_error("the agent container belongs to another job"));
        }
        let run = Marker::for_run(key);
        // BORROW: .as_str() compares the label with the run's marker
        if label(LABEL_RUN)? != run.as_str() {
            return Err(plan_error("the agent container belongs to another run"));
        }
        let experiment: Digest = label(LABEL_EXPERIMENT)?
            .parse()
            .map_err(|_| plan_error("the agent container's experiment label is not a digest"))?;
        let image_id: Digest = inspect
            .image
            .0
            // BORROW: .as_str() reads the image id as a digest
            .as_str()
            .parse()
            .map_err(|_| plan_error("the agent container's image id is not a digest"))?;
        let spawned_at = crate::supervise::now().map_err(|error| IsolationError::Unavailable {
            detail: format!("the clock cannot be read: {error}"),
        })?;
        ContainerRecord::builder()
            // BORROW: .clone() gives the record its own id and time
            .cid(inspect.id.clone())
            .labels(ContainerLabels::new(job, run, experiment))
            .created_at(inspect.created.clone())
            .image_id(image_id)
            .role(ContainerRole::Agent)
            .spawned_at(spawned_at)
            .build()
            .map_err(|error| plan_error(&error.to_string()))
    }

    /// The agent container `cid` of the plan's job as docker lists it by label, when an inspect
    /// of it failed: the guard needs its labels and creation time to act.
    async fn listed_agent(&self, plan: &ContainerPlan, cid: &ContainerId) -> Option<ContainerRef> {
        let selector = LabelSel {
            key: LABEL_JOB.to_owned(),                     // BORROW: owned label key
            value: plan.labels.get(LABEL_JOB)?.to_owned(), // BORROW: owned label value
        };
        let listed = self.guard.reader().ps_by_label(&selector).await.ok()?;
        listed.into_iter().find(|found| found.cid == *cid)
    }
}
