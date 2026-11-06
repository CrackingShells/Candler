//! Scaffold: the inputs of a cell in `exec_run`: a read-only mount for each, and the leak check.
//! The wiring of `exec_run` runs over a fake isolation, a fake driver and a recording sink.
//! It checks the order of the calls, that a failed run leaves its slot to its owner and its run
//! unsealed, and that a runtime without isolation makes no run directory. The wiring is deleted
//! when the job supervisor's own tests cover it.
#![cfg(test)]

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use candler_core::isolation::container::mounts::{MountMode, MountSpec};
use candler_core::isolation::driver::DockerDriver;
use candler_core::isolation::model::{
    ContainerOutcome, ContainerPlan, ImageId, ImageRef, Inspect, InspectState, Labels, Limits,
    NetworkName, UserSpec,
};
use candler_core::isolation::testing::RecordingDocker;
use candler_core::isolation::{EndFacts, IsolatedRequest, Isolation, IsolationError, RunOutputs};
use candler_core::jobs::model::JobCause;
use candler_core::jobs::slots::{Holder, Slots};
use candler_core::runner::evidence::{InputDigest, Inputs};
use candler_core::runner::exec::{CellRun, SkillTree, exec_run};
use candler_core::runner::plan::RunPlan;
use candler_core::runner::{JobMeter, RunControl, RunStatus, Runtime};
use candler_core::store::Store;
use candler_core::supervise::spawn::SpawnRequest;
use candler_core::supervise::stop::Exit;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::TraceReader;
use candler_harness::spec::{
    BudgetCaps, Capabilities, Cell, CollectSpec, CommandSpec, HarnessId, Observed, Prepared,
    ProcessExit, StopSignal, StopSpec, Workspace,
};
use candler_schemas::execution::EventPayload;
use candler_schemas::execution::{InvalidReason, TreatmentRef};
use candler_schemas::primitives::{ContainerId, Digest, JobId, Marker, RunKey, Timestamp};
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};
use futures::future::BoxFuture;
use serde_json::json;
use sha2::Digest as _;
use support::run_plan::{Facts, Run, fresh_store, manifest};
use tempfile::TempDir;
use tokio::sync::watch;

type Log = Arc<Mutex<Vec<String>>>;

fn note(log: &Log, line: &str) {
    log.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(line.to_owned());
}

const TRUE: &str = "/usr/bin/true";

struct NoFacts;

impl EndFacts for NoFacts {
    fn final_inspect(&self) -> Option<ContainerOutcome> {
        None
    }
}

struct FakeIsolation {
    log: Log,
    record: PathBuf,
    job: JobId,
    project: PathBuf,
    store_root: PathBuf,
    copy: Option<PathBuf>,
    seen: Arc<Mutex<Vec<MountSpec>>>,
}

impl Isolation for FakeIsolation {
    fn plan(
        &self,
        _cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<ContainerPlan, IsolationError> {
        note(&self.log, "plan");
        let root = workspace.root();
        let home = root.parent().expect("a run directory").join("home");
        fs::create_dir_all(&home).expect("a home");
        Ok(ContainerPlan {
            image: ImageRef {
                id: ImageId(String::from("sha256:abc")),
                cc_version: None,
            },
            labels: Labels::new(),
            mounts: vec![
                MountSpec {
                    source: root.to_path_buf(),
                    target: String::from("/work"),
                    mode: MountMode::ReadWrite,
                },
                MountSpec {
                    source: home,
                    target: String::from("/home/agent"),
                    mode: MountMode::ReadWrite,
                },
            ],
            env: Vec::new(),
            network: NetworkName::for_job(&self.job),
            egress: None,
            limits: Limits {
                pids_limit: 64,
                memory: String::from("1g"),
                cpus: 1.0,
            },
            cidfile: self.record.join("a.cid"),
            user: UserSpec::Engine,
            workdir: String::from("/work"),
            home: String::from("/home/agent"),
            expiry: Duration::from_secs(10),
            project_root: self.project.clone(),
            store_root: self.store_root.clone(),
        })
    }

    fn run_request(
        &self,
        plan: &ContainerPlan,
        _command: CommandSpec,
        marker: Marker,
        key: RunKey,
        outputs: RunOutputs,
    ) -> Result<IsolatedRequest, IsolationError> {
        note(&self.log, "run_request");
        fs::create_dir_all(&self.record).map_err(|e| IsolationError::Plan {
            detail: e.to_string(),
        })?;
        *self.seen.lock().unwrap_or_else(PoisonError::into_inner) = plan.mounts.clone();
        let argv = match &self.copy {
            Some(blob) => vec![
                String::from("/bin/cp"),
                blob.display().to_string(),
                plan.mounts[0].source.join("copy.txt").display().to_string(),
            ],
            None => vec![TRUE.to_owned()],
        };
        let request = SpawnRequest::builder()
            .argv(argv)
            .marker(marker)
            .run(key)
            .stdout(outputs.stdout)
            .stderr(outputs.stderr)
            .record_dir(self.record.clone())
            .build()
            .map_err(|e| IsolationError::Plan {
                detail: e.to_string(),
            })?;
        Ok(IsolatedRequest::new(request, Arc::new(NoFacts)))
    }

    fn collect_and_destroy<'a>(
        &'a self,
        _plan: &'a ContainerPlan,
        _workspace: &'a Workspace<'a>,
        _exit: &'a Exit,
    ) -> BoxFuture<'a, Result<ContainerOutcome, IsolationError>> {
        note(&self.log, "collect_and_destroy");
        Box::pin(async {
            Ok(ContainerOutcome {
                inspect: Some(inspect()),
                ..ContainerOutcome::default()
            })
        })
    }
}

/// The recorded inspect of a clean exit.
fn inspect() -> Inspect {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/docker/inspect/clean.json");
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(path).expect("a recording")).expect("a recording");
    let first = value.get(0).expect("one container");
    let word = |v: &serde_json::Value, key: &str| v[key].as_str().expect(key).to_owned();
    let state = &first["State"];
    Inspect::new(
        ContainerId::from_str(&word(first, "Id")).expect("an id"),
        ImageId(word(first, "Image")),
        Timestamp::from_str(&word(first, "Created")).expect("a time"),
        Labels::new(),
        InspectState::new(word(state, "Status"), false, 0, false, None, None),
    )
}

struct FakeDriver {
    fail_prepare: bool,
}

impl HarnessDriver for FakeDriver {
    fn id(&self) -> HarnessId {
        HarnessId::new("claude-code".parse().expect("a name"))
    }

    fn supported(&self) -> semver::VersionReq {
        semver::VersionReq::STAR
    }

    fn version_probe(&self) -> CommandSpec {
        command()
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
        if self.fail_prepare {
            return Err(DriverError::VersionOutput {
                detail: String::from("refused"),
            });
        }
        Ok(Prepared::default())
    }

    fn command(
        &self,
        _cell: &Cell<'_>,
        _workspace: &Workspace<'_>,
    ) -> Result<CommandSpec, DriverError> {
        Ok(command())
    }

    fn stop(&self) -> StopSpec {
        StopSpec::new(None, StopSignal::Term, Duration::from_secs(10))
    }

    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        None
    }

    fn offline_reader(&self, _stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        None
    }

    fn classify_exit(&self, _exit: &ProcessExit, _sessions: &[Session]) -> EndedBy {
        EndedBy::Completed
    }

    fn collect(&self, _workspace: &Workspace<'_>, _observed: &Observed) -> Vec<CollectSpec> {
        vec![CollectSpec::file(String::from("**/*"))]
    }
}

fn command() -> CommandSpec {
    CommandSpec::builder()
        .argv(vec![TRUE.to_owned()])
        .build()
        .expect("a command")
}

struct Tree;

impl SkillTree for Tree {
    fn archive_command(
        &self,
        _t: &TreatmentRef,
        _source: &Path,
    ) -> Result<CommandSpec, DriverError> {
        Ok(command())
    }

    fn files(
        &self,
        _archive: &[u8],
    ) -> Result<Vec<candler_harness::spec::PreparedFile>, DriverError> {
        Ok(Vec::new())
    }
}

struct Sink(Log);

impl EventSink for Sink {
    fn emit(&self, _run: Option<&RunKey>, payload: EventPayload) {
        note(&self.0, &format!("event {payload:?}"));
    }
}

struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

const CONTENT: &[u8] = b"the declared input";

struct Fx {
    rt: Runtime,
    plan: RunPlan,
    ctl: RunControl,
    store: Store,
    blob: PathBuf,
    seen: Arc<Mutex<Vec<MountSpec>>>,
    _tx: watch::Sender<Option<JobCause>>,
    _dir: TempDir,
}

fn fixture(copy: bool) -> Fx {
    let (dir, store, _lock) = fresh_store();
    let log: Log = Arc::default();
    let job = JobId::from_str("01900000-0000-7000-8000-000000000000").expect("a job id");
    let facts = Facts::base();
    let record = Run {
        model: "m",
        replicate: 1,
        attempt: 1,
        facts: &facts,
        integrity: json!({"kind": "valid"}),
        sealed: true,
        job: None,
    };
    let stored = manifest(&record);
    let cell = stored.cell_digest();
    let scratch = dir.path().join("input.txt");
    fs::write(&scratch, CONTENT).expect("an input");
    let digest = {
        use std::fmt::Write as _;
        let mut hex = String::from("sha256:");
        for byte in sha2::Sha256::digest(CONTENT) {
            write!(hex, "{byte:02x}").expect("hex");
        }
        Digest::from_str(&hex).expect("a digest")
    };
    let blob = store.layout().blob(&digest);
    fs::create_dir_all(blob.parent().expect("a blob directory")).expect("blobs");
    fs::copy(&scratch, &blob).expect("a blob");
    let inputs = Inputs {
        digests: vec![InputDigest {
            path: String::from("data/in.txt"),
            digest,
        }],
    };
    let cell_run = CellRun::new(
        stored.comparable().clone(),
        stored.experiment().clone(),
        stored.factors().to_vec(),
        stored.sources().to_vec(),
        stored.secrets().to_vec(),
        dir.path().join("checkout"),
        inputs,
    );
    let project = dir.path().join("project");
    fs::create_dir_all(&project).expect("a project");
    let seen: Arc<Mutex<Vec<MountSpec>>> = Arc::default();
    let fake = FakeIsolation {
        log: Arc::clone(&log),
        record: dir.path().join("record"),
        job: job.clone(),
        project,
        store_root: store.layout().root().to_path_buf(),
        copy: copy.then(|| blob.clone()),
        seen: Arc::clone(&seen),
    };
    let docker: Arc<dyn DockerDriver> = Arc::new(RecordingDocker::new());
    let rt = Runtime {
        supervisor: Arc::new(Supervisor::new(Arc::new(Silent))),
        store: store.clone(),
        sink: Arc::new(Sink(log)),
        slots: Slots::open(&dir.path().join("slots"), 1).expect("slots"),
        docker,
        isolation: Some(Arc::new(fake)),
        gateway: None,
        job,
        driver: Arc::new(FakeDriver {
            fail_prepare: false,
        }),
        skills: Arc::new(Tree),
        cells: [(cell.clone(), Arc::new(cell_run))].into_iter().collect(),
    };
    let plan = RunPlan {
        key: stored.run().clone(),
        cell,
        stored: None,
        attempt: 1,
        deviations: Vec::new(),
    };
    let (tx, rx) = watch::channel(None);
    Fx {
        rt,
        plan,
        ctl: RunControl {
            cancel: rx,
            meter: Arc::new(JobMeter::new()),
        },
        store,
        blob,
        seen,
        _tx: tx,
        _dir: dir,
    }
}

fn holder() -> Holder {
    Holder {
        job: String::from("j"),
        run: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_input_gets_a_read_only_mount() {
    let fx = fixture(false);
    let slot = fx
        .rt
        .slots
        .try_acquire(&holder())
        .expect("slot")
        .expect("free");
    let outcome = exec_run(&fx.rt, &fx.plan, &slot, &fx.ctl)
        .await
        .expect("a run");
    assert_ne!(outcome.invalid, Some(InvalidReason::InputLeak));
    let seen = fx
        .seen
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let blob = fs::canonicalize(&fx.blob).expect("a blob");
    let mount = seen
        .iter()
        .find(|m| m.target == "/work/data/in.txt")
        .unwrap_or_else(|| panic!("no input mount in {seen:?}"));
    assert_eq!(mount.mode, MountMode::ReadOnly);
    assert_eq!(mount.source, blob);
    assert!(
        fx.store
            .layout()
            .run_dir(&fx.plan.key)
            .join("run.done")
            .exists()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_output_equal_to_an_input_is_an_input_leak() {
    let fx = fixture(true);
    let slot = fx
        .rt
        .slots
        .try_acquire(&holder())
        .expect("slot")
        .expect("free");
    let outcome = exec_run(&fx.rt, &fx.plan, &slot, &fx.ctl)
        .await
        .expect("a run");
    assert_eq!(outcome.status, RunStatus::Invalid);
    assert_eq!(outcome.invalid, Some(InvalidReason::InputLeak));
}
