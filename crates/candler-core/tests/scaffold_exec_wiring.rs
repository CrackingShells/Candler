//! Scaffold: the wiring of `exec_run` over a fake isolation, a fake driver and a recording sink.
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

use candler_core::isolation::driver::DockerDriver;
use candler_core::isolation::model::{
    ContainerOutcome, ContainerPlan, ImageId, ImageRef, Labels, Limits, NetworkName, UserSpec,
};
use candler_core::isolation::testing::RecordingDocker;
use candler_core::isolation::{EndFacts, IsolatedRequest, Isolation, IsolationError, RunOutputs};
use candler_core::jobs::model::JobCause;
use candler_core::jobs::slots::{Holder, Slots};
use candler_core::runner::exec::{CellRun, ExecError, SkillTree, exec_run};
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
use candler_schemas::execution::TreatmentRef;
use candler_schemas::primitives::{JobId, Marker, RunKey};
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};
use futures::future::BoxFuture;
use serde_json::json;
use support::run_plan::{Facts, Run, fresh_store, manifest};
use tempfile::TempDir;
use tokio::sync::watch;

type Log = Arc<Mutex<Vec<String>>>;

fn note(log: &Log, line: &str) {
    log.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(line.to_owned());
}

fn lines(log: &Log) -> Vec<String> {
    log.lock().unwrap_or_else(PoisonError::into_inner).clone()
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
}

impl Isolation for FakeIsolation {
    fn plan(
        &self,
        _cell: &Cell<'_>,
        _workspace: &Workspace<'_>,
    ) -> Result<ContainerPlan, IsolationError> {
        note(&self.log, "plan");
        Ok(ContainerPlan {
            image: ImageRef {
                id: ImageId(String::from("sha256:abc")),
                cc_version: None,
            },
            labels: Labels::new(),
            mounts: Vec::new(),
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
            project_root: PathBuf::from("/p"),
            store_root: PathBuf::from("/s"),
        })
    }

    fn run_request(
        &self,
        _plan: &ContainerPlan,
        _command: CommandSpec,
        marker: Marker,
        key: RunKey,
        outputs: RunOutputs,
    ) -> Result<IsolatedRequest, IsolationError> {
        note(&self.log, "run_request");
        fs::create_dir_all(&self.record).map_err(|e| IsolationError::Plan {
            detail: e.to_string(),
        })?;
        let request = SpawnRequest::builder()
            .argv(vec![TRUE.to_owned()])
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
        Box::pin(async { Ok(ContainerOutcome::default()) })
    }
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
        Vec::new()
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

struct Fx {
    rt: Runtime,
    plan: RunPlan,
    ctl: RunControl,
    store: Store,
    log: Log,
    _tx: watch::Sender<Option<JobCause>>,
    _dir: TempDir,
}

fn fixture(isolated: bool, fail_prepare: bool) -> Fx {
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
    let cell_run = CellRun::new(
        stored.comparable().clone(),
        stored.experiment().clone(),
        stored.factors().to_vec(),
        stored.sources().to_vec(),
        stored.secrets().to_vec(),
        dir.path().join("checkout"),
    );
    let slots_dir = dir.path().join("slots");
    let isolation: Option<Arc<dyn Isolation>> = if isolated {
        let fake = FakeIsolation {
            log: Arc::clone(&log),
            record: dir.path().join("record"),
            job: job.clone(),
        };
        Some(Arc::new(fake))
    } else {
        None
    };
    let docker: Arc<dyn DockerDriver> = Arc::new(RecordingDocker::new());
    let rt = Runtime {
        supervisor: Arc::new(Supervisor::new(Arc::new(Silent))),
        store: store.clone(),
        sink: Arc::new(Sink(Arc::clone(&log))),
        slots: Slots::open(&slots_dir, 1).expect("slots"),
        docker,
        isolation,
        gateway: None,
        job,
        driver: Arc::new(FakeDriver { fail_prepare }),
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
        log,
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
async fn no_isolation_is_an_isolation_error_and_makes_no_run() {
    let fx = fixture(false, false);
    let slot = fx
        .rt
        .slots
        .try_acquire(&holder())
        .expect("slot")
        .expect("free");
    let result = exec_run(&fx.rt, &fx.plan, &slot, &fx.ctl).await;
    assert!(matches!(result, Err(ExecError::Isolation(_))), "{result:?}");
    assert!(!fx.store.layout().run_dir(&fx.plan.key).exists());
    assert_eq!(lines(&fx.log), Vec::<String>::new());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_run_calls_plan_then_request_then_collect_and_seals() {
    let fx = fixture(true, false);
    let slot = fx
        .rt
        .slots
        .try_acquire(&holder())
        .expect("slot")
        .expect("free");
    let outcome = exec_run(&fx.rt, &fx.plan, &slot, &fx.ctl)
        .await
        .expect("a run");
    let log = lines(&fx.log);
    let at = |needle: &str| {
        log.iter()
            .position(|line| line.starts_with(needle))
            .unwrap_or_else(|| panic!("no `{needle}` in {log:?}"))
    };
    assert!(at("plan") < at("run_request"));
    assert!(at("run_request") < at("event") && at("event") < at("collect_and_destroy"));
    assert!(
        at("collect_and_destroy") < log.len() - 1,
        "run_ended comes last: {log:?}"
    );
    assert_eq!(outcome.key, fx.plan.key);
    assert!(matches!(
        outcome.status,
        RunStatus::Measured | RunStatus::Invalid
    ));
    assert!(
        fx.store
            .layout()
            .run_dir(&fx.plan.key)
            .join("run.done")
            .exists()
    );
    drop(slot);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_error_leaves_the_slot_to_its_owner_and_the_run_unsealed() {
    let fx = fixture(true, true);
    let slot = fx
        .rt
        .slots
        .try_acquire(&holder())
        .expect("slot")
        .expect("free");
    let result = exec_run(&fx.rt, &fx.plan, &slot, &fx.ctl).await;
    assert!(matches!(result, Err(ExecError::Driver(_))), "{result:?}");
    let dir = fx.store.layout().run_dir(&fx.plan.key);
    assert!(dir.exists());
    assert!(!dir.join("run.done").exists());
    assert!(
        fx.rt.slots.try_acquire(&holder()).expect("slots").is_none(),
        "the owner still holds the slot"
    );
    drop(slot);
    assert!(fx.rt.slots.try_acquire(&holder()).expect("slots").is_some());
}
