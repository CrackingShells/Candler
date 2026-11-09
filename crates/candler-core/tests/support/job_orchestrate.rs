//! Helpers for the guards of job orchestration: builders and fixtures, never expected values.
//!
//! The world of a job here is a store in a temporary directory, a PATH-fake docker client behind a
//! wrapper script, a gateway that serves nothing, and a harness that describes a command and
//! reads nothing. The fake client's `run` verb is the agent container: it sleeps for the time its
//! `run.delay` file names.
#![allow(dead_code, reason = "each guard binary uses a subset of the helpers")]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use candler_core::isolation::container::DockerIsolation;
use candler_core::isolation::docker::{ClientEnv, DockerCli, DockerError};
use candler_core::isolation::driver::DockerDriver;
use candler_core::isolation::gateway::{
    Gateway, GatewayError, GatewaySignal, GatewayStart, GatewayStatus,
};
use candler_core::isolation::model::{
    ContainerOutcome, ContainerPlan, ContainerRef, GatewayHandle, Labels, NetworkName,
};
use candler_core::isolation::{EndFacts, IsolatedRequest, Isolation, IsolationError, RunOutputs};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::model::JobSpec;
use candler_core::jobs::orchestrate::schedule::{Clock, wall_clock};
use candler_core::jobs::orchestrate::session::{JobHost, OrchestrateError};
use candler_core::jobs::slots::Slots;
use candler_core::runner::Runtime;
use candler_core::runner::exec::SkillTree;
use candler_core::store::Store;
use candler_core::supervise::spawn::SpawnRequest;
use candler_core::supervise::stop::Exit;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::TraceReader;
use candler_harness::spec::{
    BudgetCaps, Capabilities, Cell, CollectSpec, CommandSpec, HarnessId, Observed, Prepared,
    PreparedFile, ProcessExit, StopSignal, StopSpec, Workspace,
};
use candler_schemas::execution::{Budget, EventPayload, ExperimentFile, JobState, TreatmentRef};
use candler_schemas::primitives::{JobId, Marker, RunKey};
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};
use futures::future::BoxFuture;
use serde_json::json;
use tempfile::TempDir;

use super::job_store::{job_id, job_record};
use super::run_plan::{Facts, fresh_store, resolution};

/// The container id the fake gateway reports: twelve lowercase hex digits.
const GATEWAY_CID: &str = "0123456789ab";

/// Drops every event.
pub struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// A gateway that starts instantly, serves, and saw nothing of any run.
pub struct FakeGateway;

impl Gateway for FakeGateway {
    fn start<'a>(
        &'a self,
        _sup: &'a Supervisor,
        // TRAIT_OBJECT: the driver is docker, or a fake in tests
        _docker: &'a dyn DockerDriver,
        request: GatewayStart<'a>,
    ) -> BoxFuture<'a, Result<GatewayHandle, GatewayError>> {
        Box::pin(async move {
            // Provenance: the label names of the container model, as a real sidecar carries them.
            let labels = Labels::new()
                // BORROW: explicit .as_str() instead of Deref coercion
                .with("candler.job", request.job.as_str())
                .with("candler.role", "gateway");
            let container = ContainerRef::new(
                GATEWAY_CID.parse().expect("a container id"),
                labels,
                "2026-11-06T10:00:00Z".parse().expect("a timestamp"),
            );
            let handle = GatewayHandle::new(
                container,
                url::Url::parse("http://gateway:8080").expect("a url"),
                NetworkName::for_job(request.job),
                NetworkName::egress_for_job(request.job),
                String::from("dummy-token"),
            )
            .expect("a gateway handle");
            Ok(handle)
        })
    }

    fn probe<'a>(&'a self, _handle: &'a GatewayHandle) -> BoxFuture<'a, GatewayStatus> {
        Box::pin(async { GatewayStatus::Ready })
    }

    fn run_signals<'a>(
        &'a self,
        _handle: &'a GatewayHandle,
        _run: &'a RunKey,
    ) -> BoxFuture<'a, Result<Vec<GatewaySignal>, GatewayError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

/// What the fake run leaves behind: no container, so nothing to inspect.
struct NoContainer;

impl EndFacts for NoContainer {
    fn final_inspect(&self) -> Option<ContainerOutcome> {
        None
    }
}

/// The isolation of a job over the fake client: the real plan, a run that is the fake client's
/// `run` verb, and a destroy with nothing to destroy.
pub struct Placed {
    inner: DockerIsolation,
    docker: PathBuf,
}

impl Isolation for Placed {
    fn plan(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<ContainerPlan, IsolationError> {
        self.inner.plan(cell, workspace)
    }

    fn run_request(
        &self,
        plan: &ContainerPlan,
        _command: CommandSpec,
        marker: Marker,
        key: RunKey,
        outputs: RunOutputs,
    ) -> Result<IsolatedRequest, IsolationError> {
        let record_dir = plan
            .cidfile
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| IsolationError::Plan {
                detail: String::from("the cidfile has no directory"),
            })?;
        let path = std::env::var("PATH").unwrap_or_default();
        let request = SpawnRequest::builder()
            .argv(vec![
                self.docker.to_string_lossy().into_owned(),
                String::from("run"),
                key.to_string(),
            ])
            .env(vec![(String::from("PATH"), path)])
            .marker(marker)
            .run(key)
            .stdout(outputs.stdout)
            .stderr(outputs.stderr)
            .deadline(plan.expiry)
            .record_dir(record_dir)
            .build()
            .map_err(|error| IsolationError::Plan {
                detail: error.to_string(),
            })?;
        Ok(IsolatedRequest::new(request, Arc::new(NoContainer)))
    }

    fn collect_and_destroy<'a>(
        &'a self,
        _plan: &'a ContainerPlan,
        _workspace: &'a Workspace<'a>,
        _exit: &'a Exit,
        _key: &'a RunKey,
    ) -> BoxFuture<'a, Result<ContainerOutcome, IsolationError>> {
        Box::pin(async { Ok(ContainerOutcome::default()) })
    }
}

/// The machine around a job: the fake client, the fake gateway, the fake placement.
pub struct FakeHost {
    docker: PathBuf,
    /// The milliseconds the job's clock reads, when a test drives it; the wall clock otherwise.
    ticks: Option<Arc<AtomicU64>>,
}

impl JobHost for FakeHost {
    fn docker_cli(
        &self,
        supervisor: Arc<Supervisor>,
        record_dir: PathBuf,
    ) -> Result<DockerCli, DockerError> {
        Ok(DockerCli::with_binary(
            supervisor,
            self.docker.clone(),
            ClientEnv::from_process(),
            record_dir,
        ))
    }

    fn gateway<'a>(
        &'a self,
        _rt: &'a Runtime,
        _spec: &'a JobSpec,
        // TRAIT_OBJECT: the gateway is a fake, as the session takes it from the host
    ) -> BoxFuture<'a, Result<Arc<dyn Gateway>, OrchestrateError>> {
        Box::pin(async {
            // TRAIT_OBJECT: the session takes the gateway as a trait object
            let service: Arc<dyn Gateway> = Arc::new(FakeGateway);
            Ok(service)
        })
    }

    fn clock(&self) -> Clock {
        match &self.ticks {
            Some(ticks) => {
                let ticks = Arc::clone(ticks); // BORROW: the clock shares the counter the test advances
                Arc::new(move || Duration::from_millis(ticks.load(Ordering::SeqCst)))
            }
            None => wall_clock(),
        }
    }

    // TRAIT_OBJECT: the host hands the runs an isolation as a trait object
    fn isolation(&self, isolation: DockerIsolation) -> Arc<dyn Isolation> {
        Arc::new(Placed {
            inner: isolation,
            docker: self.docker.clone(),
        })
    }
}

/// A harness that describes one command and reads no output.
pub struct QuietDriver;

impl HarnessDriver for QuietDriver {
    fn id(&self) -> HarnessId {
        HarnessId::new("claude-code".parse().expect("a harness name"))
    }

    fn supported(&self) -> semver::VersionReq {
        semver::VersionReq::parse(">=2.1, <3").expect("a version requirement")
    }

    fn version_probe(&self) -> CommandSpec {
        CommandSpec::builder()
            .argv(vec![String::from("claude"), String::from("--version")])
            .build()
            .expect("a command")
    }

    fn parse_version(&self, _output: &[u8]) -> Result<semver::Version, DriverError> {
        Ok(semver::Version::new(2, 1, 0))
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::builder()
            .graceful_stop(true)
            .budgets(BudgetCaps::new(true, true))
            .build()
    }

    fn prepare(
        &self,
        _cell: &Cell<'_>,
        _workspace: &Workspace<'_>,
    ) -> Result<Prepared, DriverError> {
        Ok(Prepared::default())
    }

    fn command(
        &self,
        _cell: &Cell<'_>,
        _workspace: &Workspace<'_>,
    ) -> Result<CommandSpec, DriverError> {
        Ok(CommandSpec::builder()
            .argv(vec![String::from("agent")])
            .build()?)
    }

    fn stop(&self) -> StopSpec {
        StopSpec::new(None, StopSignal::Term, Duration::from_secs(5))
    }

    // TRAIT_OBJECT: the host hands out its trace reader as a trait object
    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        None
    }

    // TRAIT_OBJECT: the host hands out its trace reader as a trait object
    fn offline_reader(&self, _stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        None
    }

    fn classify_exit(&self, _exit: &ProcessExit, _sessions: &[Session]) -> EndedBy {
        EndedBy::Completed
    }

    fn collect(&self, _workspace: &Workspace<'_>, _observed: &Observed) -> Vec<CollectSpec> {
        Vec::new()
    }
}

/// A skill tree that archives with a command that prints nothing and holds no file.
pub struct EmptyTree;

impl SkillTree for EmptyTree {
    fn archive_command(
        &self,
        _treatment: &TreatmentRef,
        _source: &Path,
    ) -> Result<CommandSpec, DriverError> {
        Ok(CommandSpec::builder()
            .argv(vec![
                String::from("/bin/sh"),
                String::from("-c"),
                String::from(":"),
            ])
            .build()?)
    }

    fn files(
        &self,
        _treatment: &TreatmentRef,
        _archive: &[u8],
    ) -> Result<Vec<PreparedFile>, DriverError> {
        Ok(Vec::new())
    }
}

/// The world of one job: a store, a job directory, the fake client and its log.
pub struct Rig {
    /// Keeps the directory alive.
    pub dir: TempDir,
    /// The store the runs are written to.
    pub store: Store,
    /// The log the fake client appends one JSON array of argv per call to.
    pub log: PathBuf,
    /// The wrapper script that is the client the job runs.
    pub docker: PathBuf,
    /// The job's id.
    pub job: JobId,
}

/// Writes `text` to `path` as an executable script.
fn write_script(path: &Path, text: &str) {
    fs::write(path, text).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("a mode");
}

impl Rig {
    /// A world whose fake `run` sleeps `delay_s` seconds before it exits; not at all when 0.
    pub fn new(delay_s: u32) -> Self {
        let (dir, store, _lock) = fresh_store();
        let canned = dir.path().join("canned");
        let bin = dir.path().join("bin");
        for sub in [&canned, &bin] {
            fs::create_dir_all(sub).expect("a directory");
        }
        if delay_s > 0 {
            fs::write(canned.join("run.delay"), delay_s.to_string()).expect("a delay");
        }
        let log = dir.path().join("fake.log");
        let docker = bin.join("docker");
        write_script(
            &docker,
            &format!(
                "#!/bin/sh\nexport FAKE_DOCKER_LOG='{}' FAKE_DOCKER_DIR='{}'\nexec '{}' \"$@\"\n",
                log.display(),
                canned.display(),
                super::fake_docker().display()
            ),
        );
        let job = job_id(34);
        JobDir::create(
            store.layout(),
            job.clone(), // BORROW: the directory owns a copy of the id
            &job_record(&job, JobState::Running),
        )
        .expect("the job directory");
        Self {
            dir,
            store,
            log,
            docker,
            job,
        }
    }

    /// The host of the job over this world.
    pub fn host(&self) -> Arc<FakeHost> {
        Arc::new(FakeHost {
            docker: self.docker.clone(),
            ticks: None,
        })
    }

    /// The host of the job over this world, whose clock reads `ticks` milliseconds from zero
    /// and moves only when the test stores a new value.
    pub fn host_on(&self, ticks: &Arc<AtomicU64>) -> Arc<FakeHost> {
        Arc::new(FakeHost {
            docker: self.docker.clone(),
            ticks: Some(Arc::clone(ticks)), // BORROW: the host shares the counter the test advances
        })
    }

    /// The runtime of the job: the real supervisor, the fake client for the read half of docker,
    /// and no isolation, no gateway and no cell until the job sets them.
    pub fn runtime(&self) -> Runtime {
        // TRAIT_OBJECT: the supervisor and the runs take their sink as a trait object
        let sink: Arc<dyn EventSink> = Arc::new(Silent);
        let supervisor = Arc::new(Supervisor::new(Arc::clone(&sink)));
        let records = self.dir.path().join("records");
        fs::create_dir_all(&records).expect("a directory");
        let docker = DockerCli::with_binary(
            Arc::clone(&supervisor),
            self.docker.clone(),
            ClientEnv::from_process(),
            records,
        );
        let slots_dir = self.dir.path().join("slots");
        fs::create_dir_all(&slots_dir).expect("a directory");
        Runtime {
            supervisor,
            store: self.store.clone(),
            sink,
            slots: Slots::open(&slots_dir, 4).expect("the slots"),
            docker: Arc::new(docker),
            isolation: None,
            gateway: None,
            job: self.job.clone(),
            driver: Arc::new(QuietDriver),
            skills: Arc::new(EmptyTree),
            cells: BTreeMap::new(),
        }
    }

    /// How many `run` calls the fake client logged.
    pub fn runs_logged(&self) -> usize {
        fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<Vec<String>>(line).ok())
            .filter(|argv| argv.first().is_some_and(|verb| verb == "run"))
            .count()
    }
}

/// The spec of a job over the experiment of `models` and the facts `facts`, under `wall_s`
/// seconds of job wall clock and `parallel` runs at once.
pub fn spec(models: &[&str], facts: &Facts, wall_s: u32, parallel: u32) -> JobSpec {
    let experiment: ExperimentFile = super::run_plan::experiment(models, 1);
    let mut resolved = serde_json::to_value(resolution(facts)).expect("resolution JSON");
    resolved["settings"] = json!([
        {"path": "model.token_secret", "value": "TEST_SECRET"},
        {"path": "request.treatment_source", "value": "/nonexistent/treatment"},
    ]);
    // Provenance: a resolution holds a source for each value and each setting it resolved, here
    // the experiment file; a factor path, which the cells set, has none.
    resolved["sources"] = json!(
        [
            "budget.per_run.wall_s",
            "harness",
            "inputs",
            "isolation.image",
            "model.token_secret",
            "request.treatment_source",
            "treatment",
        ]
        .map(|path| json!({"path": path, "source": {"kind": "experiment_file"}}))
    );
    let budget: Budget = serde_json::from_value(json!({
        "per_run": {"wall_s": 3600},
        "wall_s": wall_s,
        "max_parallel": parallel,
    }))
    .expect("a budget");
    JobSpec {
        experiment,
        resolution: serde_json::from_value(resolved).expect("a resolution"),
        plans: Vec::new(),
        budget,
        force: false,
        inputs: Vec::new(),
    }
}
