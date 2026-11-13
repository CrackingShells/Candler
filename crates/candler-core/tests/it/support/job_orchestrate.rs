//! Helpers for the guards of job orchestration: builders and fixtures, never expected values.
//!
//! The world of a job here is a store in a temporary directory, a PATH-fake docker client behind a
//! wrapper script, a gateway that serves nothing, and a harness that describes a command and
//! reads nothing. The fake client's `run` verb is the agent container: it sleeps for the time its
//! `run.delay` file names.
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use candler_core::isolation::container::DockerIsolation;
use candler_core::isolation::container::stop::ContainerStopHook;
use candler_core::isolation::container::sweeper::ContainerSweeper;
use candler_core::isolation::docker::{ClientEnv, DockerCli, DockerError};
use candler_core::isolation::driver::DockerDriver;
use candler_core::isolation::gateway::{
    Gateway, GatewayError, GatewaySignal, GatewayStart, GatewayStatus,
};
use candler_core::isolation::image::PackageReadings;
use candler_core::isolation::model::{
    ContainerOutcome, ContainerPlan, ContainerRef, GatewayHandle, ImageRef, Labels, NetworkName,
};
use candler_core::isolation::{EndFacts, IsolatedRequest, Isolation, IsolationError, RunOutputs};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::model::JobSpec;
use candler_core::jobs::orchestrate::schedule::{Clock, wall_clock};
use candler_core::jobs::orchestrate::session::{JobHost, OrchestrateError};
use candler_core::jobs::slots::Slots;
use candler_core::runner::Runtime;
use candler_core::runner::evidence::provenance::EvalInfra;
use candler_core::store::Store;
use candler_core::supervise::hook::StopHook;
use candler_core::supervise::spawn::SpawnRequest;
use candler_core::supervise::stop::Exit;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::TraceReader;
use candler_harness::skills::SkillTree;
use candler_harness::spec::{
    BudgetCaps, Capabilities, Cell, CollectSpec, CommandSpec, GatewayEnv, HarnessId, Observed,
    Prepared, PreparedFile, ProcessExit, StopSignal, StopSpec, Workspace,
};
use candler_schemas::execution::{Budget, EventPayload, ExperimentFile, JobState, TreatmentRef};
use candler_schemas::primitives::{ContainerId, JobId, Marker, RunKey};
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
        _since: std::time::SystemTime,
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

/// An agent that writes a cidfile: the run is a first-party shell that writes `cid` to the plan's
/// cidfile, then sleeps `sleep_s` seconds. The container it names exists only in the scripted
/// answers of the fake client.
pub struct FakeAgent {
    /// The id the run's cidfile holds.
    pub cid: ContainerId,
    /// How long the run lives, in seconds.
    pub sleep_s: u32,
    /// The sweeper the run's stop hook sweeps through; no hook is attached when `None`.
    // TRAIT_OBJECT: the sweeper is a recording fake in tests
    pub sweeper: Option<Arc<dyn ContainerSweeper>>,
    /// What the run prints on its standard output before it sleeps.
    pub stdout: String,
}

/// The isolation of a job over the fake client: the real plan, a run that is the fake client's
/// `run` verb, and a destroy with nothing to destroy. With a [`FakeAgent`] the run writes a
/// cidfile and the real isolation records and destroys the container it names.
pub struct Placed {
    inner: DockerIsolation,
    docker: PathBuf,
    agent: Option<Arc<FakeAgent>>,
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
        let observer = outputs.observer.clone(); // BORROW: the hook shares the live view
        let path = std::env::var("PATH").unwrap_or_default();
        let argv = match &self.agent {
            Some(agent) => vec![
                String::from("/bin/sh"),
                String::from("-c"),
                String::from("printf %s \"$1\" > \"$2\"; printf %s \"$4\"; exec sleep \"$3\""),
                String::from("agent"),
                agent.cid.as_str().to_owned(),
                plan.cidfile.to_string_lossy().into_owned(),
                agent.sleep_s.to_string(),
                agent.stdout.clone(), // BORROW: the argv owns the text
            ],
            None => vec![
                self.docker.to_string_lossy().into_owned(),
                String::from("run"),
                key.to_string(),
            ],
        };
        let mut builder = SpawnRequest::builder()
            .argv(argv)
            .env(vec![(String::from("PATH"), path)])
            .marker(marker)
            .run(key)
            .stdout(outputs.stdout)
            .stderr(outputs.stderr)
            .deadline(plan.expiry)
            .record_dir(record_dir);
        // TRAIT_OBJECT: the facts come from the hook the request carries, or from none
        let mut facts: Arc<dyn EndFacts> = Arc::new(NoContainer);
        if let Some(sweeper) = self.agent.as_ref().and_then(|agent| agent.sweeper.as_ref()) {
            let job = plan
                .labels
                .get("candler.job")
                .and_then(|text| text.parse().ok())
                .ok_or_else(|| IsolationError::Plan {
                    detail: String::from("the plan carries no job label"),
                })?;
            let mut stop_hook =
                ContainerStopHook::new(self.inner.guard(), Arc::clone(sweeper), job);
            if let Some(view) = observer {
                stop_hook = stop_hook.with_observer(view);
            }
            let hook = Arc::new(stop_hook);
            // TRAIT_OBJECT: the request carries its hook as the supervisor's trait
            let attached: Arc<dyn StopHook> = hook.clone(); // BORROW: the facts share the hook
            builder = builder.stop_hook(attached);
            facts = hook;
        }
        let request = builder.build().map_err(|error| IsolationError::Plan {
            detail: error.to_string(),
        })?;
        Ok(IsolatedRequest::new(request, facts))
    }

    fn record_container<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        key: &'a RunKey,
    ) -> BoxFuture<'a, Result<Option<ContainerId>, IsolationError>> {
        self.inner.record_container(plan, key)
    }

    fn collect_exec<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        command: &'a CommandSpec,
    ) -> BoxFuture<'a, Result<Vec<u8>, IsolationError>> {
        if self.agent.is_some() {
            return self.inner.collect_exec(plan, command);
        }
        Box::pin(async {
            Err(IsolationError::Unavailable {
                detail: String::from("collect_exec"),
            })
        })
    }

    fn collect_and_destroy<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        workspace: &'a Workspace<'a>,
        exit: &'a Exit,
        key: &'a RunKey,
    ) -> BoxFuture<'a, Result<ContainerOutcome, IsolationError>> {
        if self.agent.is_some() {
            return self.inner.collect_and_destroy(plan, workspace, exit, key);
        }
        Box::pin(async { Ok(ContainerOutcome::default()) })
    }
}

/// What a container read of the env image answers: the packages the image holds, and the names
/// each read asked for.
#[derive(Default)]
pub struct FakeReads {
    /// The packages the fake image holds, by import name.
    pub held: PackageReadings,
    /// The names of each read, in order.
    pub asked: Mutex<Vec<Vec<String>>>,
}

/// The machine around a job: the fake client, the fake gateway, the fake placement.
pub struct FakeHost {
    /// The container read of the env image, when a test gives the image packages.
    reads: Option<Arc<FakeReads>>,
    docker: PathBuf,
    /// The agent the runs are, when a test gives them a container; the fake client's `run` otherwise.
    agent: Option<Arc<FakeAgent>>,
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

    fn packages<'a>(
        &'a self,
        _rt: &'a Runtime,
        _image: &'a ImageRef,
        names: &'a [String],
    ) -> BoxFuture<'a, Option<PackageReadings>> {
        Box::pin(async move {
            let reads = self.reads.as_ref()?;
            reads
                .asked
                .lock()
                .expect("the log of reads")
                .push(names.to_vec()); // BORROW: the log owns the names
            Some(
                reads
                    .held
                    .iter()
                    .filter(|(name, _)| names.contains(name))
                    .map(|(name, reading)| (name.clone(), reading.clone())) // BORROW: the answer owns its readings
                    .collect(),
            )
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
            agent: self.agent.clone(), // BORROW: the isolation shares the agent
        })
    }
}

/// The gateway variables of the quiet harness.
pub fn quiet_gateway() -> GatewayEnv {
    let names = [
        String::from("QUIET_GATEWAY_URL"),
        String::from("QUIET_GATEWAY_TOKEN"),
    ];
    GatewayEnv::new(names[0].clone(), names[1].clone(), names.to_vec()) // BORROW: the names are owned by the environment
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

    fn gateway(&self) -> Option<GatewayEnv> {
        Some(quiet_gateway())
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

/// The quiet harness with a live reader that reads Claude Code's stream-json, and an end of its
/// own: the exit classifies as completed and the observed end as an agent error, so the end a run
/// records says which of the two the core asked.
pub struct ObservingDriver(pub QuietDriver);

impl HarnessDriver for ObservingDriver {
    fn id(&self) -> HarnessId {
        self.0.id()
    }

    fn supported(&self) -> semver::VersionReq {
        self.0.supported()
    }

    fn version_probe(&self) -> CommandSpec {
        self.0.version_probe()
    }

    fn parse_version(&self, output: &[u8]) -> Result<semver::Version, DriverError> {
        self.0.parse_version(output)
    }

    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }

    fn gateway(&self) -> Option<GatewayEnv> {
        self.0.gateway()
    }

    fn prepare(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Prepared, DriverError> {
        self.0.prepare(cell, workspace)
    }

    fn command(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<CommandSpec, DriverError> {
        self.0.command(cell, workspace)
    }

    fn stop(&self) -> StopSpec {
        self.0.stop()
    }

    // TRAIT_OBJECT: the host hands out its trace reader as a trait object
    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        let reader = candler_harness_claude_code::ClaudeCodeReader::new().ok()?;
        Some(Box::new(reader))
    }

    // TRAIT_OBJECT: the host hands out its trace reader as a trait object
    fn offline_reader(&self, _stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        None
    }

    fn classify_exit(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy {
        self.0.classify_exit(exit, sessions)
    }

    fn classify_end(
        &self,
        _exit: &ProcessExit,
        _sessions: &[Session],
        _observed: &Observed,
    ) -> EndedBy {
        EndedBy::agent_error(None)
    }

    fn collect(&self, workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec> {
        self.0.collect(workspace, observed)
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
                "#!/bin/sh\nexport FAKE_DOCKER_LOG='{}' FAKE_DOCKER_DIR='{}'\n\
                 if [ \"$1 $2\" = 'container rm' ] && [ -f \"$FAKE_DOCKER_DIR/container-rm.once\" ]; then\n\
                 rm -f \"$FAKE_DOCKER_DIR/container-rm.once\"; echo 'rm refused' >&2; exit 1; fi\n\
                 exec '{}' \"$@\"\n",
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
            reads: None,
            docker: self.docker.clone(),
            agent: None,
            ticks: None,
        })
    }

    /// The host of the job over this world, whose env image answers `reads`.
    #[must_use]
    pub fn host_reading(&self, reads: &Arc<FakeReads>) -> Arc<FakeHost> {
        Arc::new(FakeHost {
            reads: Some(Arc::clone(reads)), // BORROW: the host shares the log the test reads
            docker: self.docker.clone(),
            agent: None,
            ticks: None,
        })
    }

    /// The host of the job over this world, whose runs are `agent`.
    #[must_use]
    pub fn host_with_agent(&self, agent: FakeAgent) -> Arc<FakeHost> {
        Arc::new(FakeHost {
            reads: None,
            docker: self.docker.clone(),
            agent: Some(Arc::new(agent)),
            ticks: None,
        })
    }

    /// The directory of the fake client's canned answers.
    #[must_use]
    pub fn canned(&self) -> PathBuf {
        self.dir.path().join("canned")
    }

    /// The host of the job over this world, whose clock reads `ticks` milliseconds from zero
    /// and moves only when the test stores a new value.
    pub fn host_on(&self, ticks: &Arc<AtomicU64>) -> Arc<FakeHost> {
        Arc::new(FakeHost {
            reads: None,
            docker: self.docker.clone(),
            agent: None,
            ticks: Some(Arc::clone(ticks)), // BORROW: the host shares the counter the test advances
        })
    }

    /// The runtime of the job: the real supervisor, the fake client for the read half of docker,
    /// and no isolation, no gateway and no cell until the job sets them.
    pub fn runtime(&self) -> Runtime {
        // TRAIT_OBJECT: the supervisor and the runs take their sink as a trait object
        self.runtime_with_sink(Arc::new(Silent))
    }

    /// The runtime of the job, whose supervisor and runs report to `sink`.
    // TRAIT_OBJECT: the supervisor and the runs take their sink as a trait object
    pub fn runtime_with_sink(&self, sink: Arc<dyn EventSink>) -> Runtime {
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
            eval_infra: EvalInfra::default(),
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
        {"path": "request.treatment", "value": {"name": "htn", "source": "/nonexistent/treatment"}},
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
            "request.treatment",
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
        evaluation_inputs: Vec::new(),
    }
}
