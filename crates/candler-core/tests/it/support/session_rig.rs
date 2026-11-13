//! A session run over a stub harness: the real run loop, the real Claude Code driver and reader,
//! a fake isolation that starts a script instead of a container, and a clock the test reads.
//!
//! The stub is a Python script that reads the opening input, replays a script of timed stdout
//! lines and hook-log lines, notes when its stdin reaches end of file, and records each of these
//! with a monotonic time in a trace the test reads back.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use candler_core::isolation::model::{
    ContainerOutcome, ContainerPlan, ImageId, ImageRef, Inspect, InspectState, Labels, Limits,
    NetworkName, UserSpec,
};
use candler_core::isolation::{EndFacts, IsolatedRequest, Isolation, IsolationError, RunOutputs};
use candler_core::jobs::model::JobCause;
use candler_core::jobs::slots::Holder;
use candler_core::resolve::steps::session::MODE_SESSION;
use candler_core::runner::evidence::Inputs;
use candler_core::runner::evidence::session::ControlRef;
use candler_core::runner::exec::{CellRun, ExecError, exec_run};
use candler_core::runner::plan::RunPlan;
use candler_core::runner::{JobMeter, RunControl, RunOutcome};
use candler_core::supervise::spawn::{SpawnRequest, StdinSpec};
use candler_core::supervise::stop::Exit;
use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::TraceReader;
use candler_harness::spec::{
    Capabilities, Cell, CollectSpec, CommandSpec, GatewayEnv, HarnessId, Observed, Prepared,
    ProcessExit, SessionSpec, SessionTracker, StatePath, StopSpec, Workspace,
};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::Comparable;
use candler_schemas::primitives::{ContainerId, Digest, Marker, RunKey};
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::support::job_orchestrate::Rig;

/// The stub harness.
const STUB: &str = r#"
import json, os, sys, threading, time
script = json.load(open(sys.argv[1]))
trace = open(script["trace"], "a", buffering=1)
def log(kind, detail=""):
    trace.write("%.6f %s %s\n" % (time.monotonic(), kind, detail))
first = sys.stdin.buffer.readline()
log("opened", str(len(first)))
eof = threading.Event()
def watch():
    sys.stdin.buffer.read()
    log("eof")
    eof.set()
threading.Thread(target=watch, daemon=True).start()
hook = os.environ["HOOKLOG"]
for step in script["steps"]:
    if "sleep" in step:
        time.sleep(step["sleep"])
    elif "out" in step:
        sys.stdout.write(step["out"] + "\n")
        sys.stdout.flush()
        log("out", step.get("tag", ""))
    elif "hook" in step:
        os.makedirs(os.path.dirname(hook), exist_ok=True)
        with open(hook, "a") as f:
            f.write(step["hook"] + "\n")
        log("hook")
    elif "wait_eof" in step:
        eof.wait(step.get("timeout"))
    elif "exit" in step:
        log("exit", str(step["exit"]))
        trace.flush()
        os._exit(step["exit"])
log("end")
"#;

/// The directory of the recorded sessions of the Claude Code crate.
pub fn sessions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../candler-harness-claude-code/tests/fixtures/sessions")
}

/// The lines of the file `name` of the recorded session `session`.
pub fn fixture_lines(session: &str, name: &str) -> Vec<String> {
    let text = fs::read_to_string(sessions_dir().join(session).join(name)).expect("a fixture");
    text.lines().map(str::to_owned).collect()
}

/// One step of a stub script.
pub fn out(line: &str, tag: &str) -> Value {
    json!({"out": line, "tag": tag})
}

/// A stdout line of the stub, tagged by its type (`result` for a turn's end).
pub fn out_of(line: &str) -> Value {
    let tag = serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|value| value["type"].as_str().map(str::to_owned))
        .unwrap_or_default();
    out(line, &tag)
}

/// A hook-log line written by the stub.
pub fn hook(line: &str) -> Value {
    json!({"hook": line})
}

/// A pause of the stub.
pub fn sleep(seconds: f64) -> Value {
    json!({"sleep": seconds})
}

/// The stub waits for end of its stdin, for at most `timeout` seconds.
pub fn wait_eof(timeout: Option<f64>) -> Value {
    json!({"wait_eof": true, "timeout": timeout})
}

/// The stub exits with `code`.
pub fn exit(code: i32) -> Value {
    json!({"exit": code})
}

/// The stub's script of a recorded session: the lines of `stdout.jsonl` in order, and before each
/// `result` line the next hook line of `hooks.jsonl` that is a `Stop`, as the harness writes it.
/// `between` gives the steps to insert before the line with the given 1-based number.
pub fn replay_of(session: &str, between: &BTreeMap<usize, Vec<Value>>) -> Vec<Value> {
    let lines = fixture_lines(session, "stdout.jsonl");
    let mut hooks = fixture_lines(session, "hooks.jsonl")
        .into_iter()
        .filter(|line| line.contains("\"hook_event_name\":\"Stop\""))
        .collect::<Vec<_>>()
        .into_iter();
    let mut steps = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if let Some(extra) = between.get(&(index + 1)) {
            steps.extend(extra.iter().cloned());
        }
        let is_result = line.contains("\"type\":\"result\"");
        if is_result && let Some(stop) = hooks.next() {
            steps.push(hook(&stop));
            steps.push(sleep(0.01));
        }
        steps.push(out_of(line));
    }
    steps
}

/// The values of a session cell, by path.
pub fn cell_values(extra: &[(&str, Value)]) -> Vec<(String, Value)> {
    let mut values: BTreeMap<String, Value> = BTreeMap::new();
    for (path, value) in [
        ("budget.per_run.wall_s", json!(60)),
        (
            "isolation.image",
            json!("sha256:5555555555555555555555555555555555555555555555555555555555555555"),
        ),
        ("model.id", json!("c1")),
        ("permission_mode", json!("bypassPermissions")),
        ("prompt.template", json!("go")),
        ("runner.mode", json!("session")),
        ("runner.idle_s", json!(1)),
    ] {
        values.insert(path.to_owned(), value);
    }
    for (path, value) in extra {
        values.insert((*path).to_owned(), value.clone());
    }
    values.into_iter().collect()
}

/// The comparable of a cell with `values` (sorted by path).
fn comparable_of(values: &[(String, Value)]) -> Comparable {
    let entries: Vec<Value> = values
        .iter()
        .map(|(path, value)| json!({"path": path, "value": value}))
        .collect();
    serde_json::from_value(json!({
        "values": entries,
        "harness": {"name": "claude-code", "version": "2.1.311"},
        "treatment": {"name": "htn"},
        "snapshots": [],
        "inputs": [],
        "environments": [],
    }))
    .expect("a comparable")
}

/// The Claude Code driver with a short grace, and with its session hooks dropped when asked.
pub struct Driver {
    inner: ClaudeCodeDriver,
    grace: Duration,
    hooks: bool,
}

impl Driver {
    /// The driver with a grace of one second past a closed input.
    pub fn new(hooks: bool) -> Self {
        Self {
            inner: ClaudeCodeDriver::new(),
            grace: Duration::from_secs(1),
            hooks,
        }
    }
}

impl HarnessDriver for Driver {
    fn skills(&self) -> candler_harness::skills::SkillTree {
        candler_harness::skills::SkillTree::new(".claude/skills", ".claude/skills")
    }

    fn base_context(&self) -> &'static str {
        "base"
    }

    fn detect(&self, _sample: &[u8]) -> Option<semver::Version> {
        None
    }

    fn id(&self) -> HarnessId {
        self.inner.id()
    }
    fn supported(&self) -> semver::VersionReq {
        self.inner.supported()
    }
    fn version_probe(&self) -> CommandSpec {
        self.inner.version_probe()
    }
    fn parse_version(&self, output: &[u8]) -> Result<semver::Version, DriverError> {
        self.inner.parse_version(output)
    }
    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }
    fn prepare(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Prepared, DriverError> {
        self.inner.prepare(cell, workspace)
    }
    fn command(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<CommandSpec, DriverError> {
        self.inner.command(cell, workspace)
    }
    fn gateway(&self) -> Option<GatewayEnv> {
        self.inner.gateway()
    }
    fn state_paths(&self) -> Vec<StatePath> {
        self.inner.state_paths()
    }
    fn session(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<Option<SessionSpec>, DriverError> {
        let mut spec = self.inner.session(cell, workspace)?;
        if !self.hooks
            && let Some(found) = spec.as_mut()
        {
            found.hooks = None;
        }
        Ok(spec)
    }
    fn session_tracker(&self) -> Option<Box<dyn SessionTracker>> {
        self.inner.session_tracker()
    }
    fn stop(&self) -> StopSpec {
        let real = self.inner.stop();
        StopSpec::new(real.pre_stop, real.signal, self.grace)
    }
    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        self.inner.live_reader()
    }
    fn offline_reader(&self, stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        self.inner.offline_reader(stream)
    }
    fn classify_exit(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy {
        self.inner.classify_exit(exit, sessions)
    }
    fn classify_end(
        &self,
        exit: &ProcessExit,
        sessions: &[Session],
        observed: &Observed,
    ) -> EndedBy {
        self.inner.classify_end(exit, sessions, observed)
    }
    fn collect(&self, workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec> {
        self.inner.collect(workspace, observed)
    }
}

/// What the run left of the container: an exited one that reports exit code 0.
struct Exited;

impl EndFacts for Exited {
    fn final_inspect(&self) -> Option<ContainerOutcome> {
        Some(outcome(0))
    }
}

fn outcome(code: i64) -> ContainerOutcome {
    let id: ContainerId = "0123456789ab".parse().expect("a container id");
    let created = "2026-03-04T05:06:07Z".parse().expect("a time");
    let inspect = Inspect::new(
        id,
        ImageId(String::from(
            "sha256:5555555555555555555555555555555555555555555555555555555555555555",
        )),
        created,
        Labels::new(),
        InspectState::new(String::from("exited"), false, code, false, None, None),
    );
    ContainerOutcome {
        inspect: Some(inspect),
        ..ContainerOutcome::default()
    }
}

/// An isolation that starts the stub script where a container would run.
pub struct StubIsolation {
    /// The directory the stub script, its script file and its trace are in.
    pub dir: PathBuf,
    /// The script file the stub replays, by its path.
    pub script: PathBuf,
}

impl Isolation for StubIsolation {
    fn plan(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<ContainerPlan, IsolationError> {
        let wall = cell
            .get(&"budget.per_run.wall_s".parse().expect("a path"))
            .and_then(|value| value.as_value().as_u64())
            .expect("a wall");
        let interactive = cell
            .get(&"runner.mode".parse().expect("a path"))
            .is_some_and(|value| value.as_value() == MODE_SESSION);
        let run_dir = workspace.root().parent().expect("a run directory");
        let home = run_dir.join("home");
        fs::create_dir_all(&home).expect("a home");
        let job = "01900000-0000-7000-8000-000000000000"
            .parse()
            .expect("a job id");
        Ok(ContainerPlan {
            image: ImageRef {
                id: ImageId(String::from(
                    "sha256:5555555555555555555555555555555555555555555555555555555555555555",
                )),
                harness_version: None,
            },
            labels: Labels::new().with("candler.job", "01900000-0000-7000-8000-000000000000"),
            mounts: Vec::new(),
            env: Vec::new(),
            gateway: None,
            state: Vec::new(),
            network: NetworkName::for_job(&job),
            egress: None,
            limits: Limits {
                pids_limit: 64,
                memory: String::from("1g"),
                cpus: 1.0,
            },
            cidfile: self.dir.join("agent.cid"),
            user: UserSpec::Engine,
            workdir: String::from("/work"),
            home: home.to_string_lossy().into_owned(),
            expiry: Duration::from_secs(wall),
            project_root: self.dir.clone(),
            store_root: self.dir.clone(),
            interactive,
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
        let path = std::env::var("PATH").unwrap_or_default();
        let hook_log = Path::new(&plan.home).join(".candler/hooks.jsonl");
        let mut builder = SpawnRequest::builder()
            .argv(vec![
                String::from("python3"),
                self.dir.join("stub.py").to_string_lossy().into_owned(),
                self.script.to_string_lossy().into_owned(),
            ])
            .env(vec![
                (String::from("PATH"), path),
                (
                    String::from("HOOKLOG"),
                    hook_log.to_string_lossy().into_owned(),
                ),
            ])
            .marker(marker)
            .run(key)
            .stdout(outputs.stdout)
            .stderr(outputs.stderr)
            .deadline(plan.expiry)
            .record_dir(self.dir.clone());
        if plan.interactive {
            builder = builder.stdin(StdinSpec::Pipe);
        }
        let request = builder.build().map_err(|error| IsolationError::Plan {
            detail: error.to_string(),
        })?;
        Ok(IsolatedRequest::new(request, Arc::new(Exited)))
    }

    fn collect_and_destroy<'a>(
        &'a self,
        _plan: &'a ContainerPlan,
        _workspace: &'a Workspace<'a>,
        exit: &'a Exit,
        _key: &'a RunKey,
    ) -> BoxFuture<'a, Result<ContainerOutcome, IsolationError>> {
        let code = match exit.status {
            candler_schemas::process::ExitStatus::Code { code } => i64::from(code),
            candler_schemas::process::ExitStatus::Signal { .. } => 137,
        };
        Box::pin(async move { Ok(outcome(code)) })
    }
}

/// The result of one stub run.
pub struct Ran {
    /// What `exec_run` returned.
    pub result: Result<RunOutcome, ExecError>,
    /// The manifest of the sealed run.
    pub manifest: Value,
    /// The run's directory in the store.
    pub run_dir: PathBuf,
    /// The stub's trace: `(monotonic seconds, kind, detail)`.
    pub trace: Vec<(f64, String, String)>,
    _rig: Rig,
}

impl Ran {
    /// The outcome of the run; the test fails with the error when the run could not be executed.
    pub fn outcome(&self) -> &RunOutcome {
        self.result
            .as_ref()
            .unwrap_or_else(|error| panic!("the run failed: {error}"))
    }

    /// The measured value at `path`.
    pub fn measured(&self, path: &str) -> Option<Value> {
        self.manifest["measured"]
            .as_array()?
            .iter()
            .find(|entry| entry["path"] == path)
            .map(|entry| entry["value"].clone())
    }

    /// The time of the first trace entry of `kind` whose detail is `detail` (any when empty).
    pub fn at(&self, kind: &str, detail: &str) -> Option<f64> {
        self.trace
            .iter()
            .find(|(_, k, d)| k == kind && (detail.is_empty() || d == detail))
            .map(|(t, _, _)| *t)
    }

    /// The time of the last trace entry of `kind` whose detail is `detail`.
    pub fn last_at(&self, kind: &str, detail: &str) -> Option<f64> {
        self.trace
            .iter()
            .rev()
            .find(|(_, k, d)| k == kind && (detail.is_empty() || d == detail))
            .map(|(t, _, _)| *t)
    }
}

/// Runs one session cell over the stub script `steps`, with the cell's values `values` and the
/// driver `driver`. `cancel` is sent to the run once the stub has opened, after `after` seconds.
pub async fn run(
    steps: &[Value],
    values: &[(String, Value)],
    driver: Driver,
    cancel: Option<(JobCause, f64)>,
) -> Ran {
    run_controlled(steps, values, driver, cancel, None).await
}

/// [`run`] for a cell that relies on the control record `control`.
pub async fn run_controlled(
    steps: &[Value],
    values: &[(String, Value)],
    driver: Driver,
    cancel: Option<(JobCause, f64)>,
    control: Option<ControlRef>,
) -> Ran {
    let rig = Rig::new(0);
    let dir = rig.dir.path().join("stub");
    fs::create_dir_all(&dir).expect("a directory");
    fs::write(dir.join("stub.py"), STUB).expect("the stub");
    let trace_path = dir.join("trace.log");
    let script = dir.join("script.json");
    fs::write(
        &script,
        serde_json::to_vec(&json!({"trace": trace_path, "steps": steps})).expect("a script"),
    )
    .expect("a script file");
    let isolation: Arc<dyn Isolation> = Arc::new(StubIsolation {
        dir: dir.clone(),
        script,
    });
    let cancel = cancel.map(|(cause, after)| (cause, after, trace_path.clone()));
    let (result, manifest, run_dir, rig) =
        execute_controlled(rig, isolation, values, driver, cancel, control).await;
    let trace = fs::read_to_string(&trace_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, ' ');
            let at = parts.next()?.parse::<f64>().ok()?;
            Some((
                at,
                parts.next()?.to_owned(),
                parts.next().unwrap_or_default().to_owned(),
            ))
        })
        .collect();
    Ran {
        result,
        manifest,
        run_dir,
        trace,
        _rig: rig,
    }
}

/// What `execute` leaves: the run's result, its sealed manifest and directory, and the world.
pub type Executed = (Result<RunOutcome, ExecError>, Value, PathBuf, Rig);

/// [`execute`] for a cell that relies on the control record `control`.
pub async fn execute_controlled(
    rig: Rig,
    isolation: Arc<dyn Isolation>,
    values: &[(String, Value)],
    driver: Driver,
    cancel: Option<(JobCause, f64, PathBuf)>,
    control: Option<ControlRef>,
) -> Executed {
    let comparable = comparable_of(values);
    let cell_digest: Digest = comparable.digest();
    let experiment: Digest = format!("sha256:{}", "e".repeat(64))
        .parse()
        .expect("a digest");
    let mut paths: Vec<&str> = values.iter().map(|(path, _)| path.as_str()).collect();
    paths.extend(["harness", "treatment"]);
    paths.sort_unstable();
    let sources: Vec<candler_schemas::values::SourceEntry> = serde_json::from_value(Value::Array(
        paths
            .iter()
            .map(|path| json!({"path": path, "source": {"kind": "experiment_file"}}))
            .collect(),
    ))
    .expect("sources");
    let cell = CellRun::new(
        comparable,
        experiment,
        Vec::new(),
        sources,
        Vec::new(),
        None,
        Inputs::default(),
    );
    let cell = match control {
        Some(control) => cell.with_control(control),
        None => cell,
    };
    let key = RunKey::new(cell_digest.clone(), 1, 1).expect("a run key");
    let mut runtime = rig.runtime();
    runtime.driver = Arc::new(driver);
    runtime.isolation = Some(isolation);
    runtime.cells = BTreeMap::from([(cell_digest.clone(), Arc::new(cell))]);
    let plan = RunPlan {
        key: key.clone(),
        cell: cell_digest,
        stored: None,
        attempt: 1,
        deviations: Vec::new(),
    };
    let holder = Holder {
        job: runtime.job.to_string(),
        run: None,
    };
    let slot = runtime.slots.acquire(&holder).await.expect("a slot");
    let (tx, rx) = watch::channel::<Option<JobCause>>(None);
    let control = RunControl {
        cancel: rx,
        meter: Arc::new(JobMeter::new()),
    };
    let canceller = cancel.map(|(cause, after, trace)| {
        tokio::spawn(async move {
            let began = std::time::Instant::now();
            while !fs::read_to_string(&trace).is_ok_and(|text| text.contains(" opened ")) {
                assert!(
                    began.elapsed() < Duration::from_secs(20),
                    "the stub never opened"
                );
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            tokio::time::sleep(Duration::from_secs_f64(after)).await;
            tx.send_replace(Some(cause));
            tx
        })
    });
    let result = exec_run(&runtime, &plan, &slot, &control).await;
    if let Some(task) = canceller {
        task.abort();
    }
    let run_dir = rig.store.layout().root().join("runs").join(key.to_string());
    let manifest: Value = fs::read(run_dir.join("manifest.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    drop(slot);
    drop(runtime);
    (result, manifest, run_dir, rig)
}
