//! A job over the `OpenCode` driver and the fakes: the fake agent prints a recorded live stream,
//! the PATH-fake docker client answers a collect with a recorded export and logs every call, and
//! the fake gateway answers with the census lines the real gateway logged.
//!
//! Helpers only: builders and fixtures, never expected values.
#![allow(dead_code, reason = "each guard binary uses a subset of the helpers")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use candler_core::isolation::container::DockerIsolation;
use candler_core::isolation::docker::{DockerCli, DockerError};
use candler_core::isolation::gateway::{
    Gateway, GatewayError, GatewaySignal, GatewayStart, GatewayStatus, census_of,
};
use candler_core::isolation::model::{GatewayHandle, ImageRef};
use candler_core::isolation::{Isolation, driver::DockerDriver};
use candler_core::jobs::model::{JobCause, JobControl, JobSpec};
use candler_core::jobs::orchestrate::orchestrate_with;
use candler_core::jobs::orchestrate::schedule::Clock;
use candler_core::jobs::orchestrate::session::{JobHost, OrchestrateError};
use candler_core::runner::Runtime;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::TraceReader;
use candler_harness::spec::{
    Capabilities, Cell, CollectSpec, CommandSpec, EnvEntry, GatewayEnv, HarnessId, Observed,
    Prepared, PreparedFile, ProcessExit, StatePath, StopSpec, Workspace,
};
use candler_harness_opencode::driver::OpenCodeDriver;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{ContainerId, RunKey};
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::support::container_stop::{cid, digest};
use crate::support::job_orchestrate::{FakeAgent, FakeGateway, FakeHost, Rig, spec};
use crate::support::job_runtime::Recorder;
use crate::support::run_plan::{Facts, Run, manifest};

/// The image id the scripted inspect gives the agent.
const IMAGE_ID: &str = "sha256:5555555555555555555555555555555555555555555555555555555555555555";

/// The creation time the scripted inspect gives.
const CREATED: &str = "2026-03-04T05:06:07.123456789Z";

/// The run key and the tag the recorded gateway log was written under.
pub const LOG_RUN: &str = "1111111111111111111111111111111111111111111111111111111111111111-r1-a1";
/// The tag of the recorded gateway log.
pub const LOG_TAG: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
/// A value that stands for a literal in the overlay of the prepared command: it is recorded as a
/// name only.
pub const LITERAL_CANARY: &str = "LITERAL-CANARY-5d1c";

/// The directory of the recorded `OpenCode` streams.
pub fn streams() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../candler-harness-opencode/tests/fixtures/streams")
}

/// The text of `file` of the recorded run `case`.
pub fn recorded(case: &str, file: &str) -> String {
    fs::read_to_string(streams().join(case).join(file)).expect("a recorded file")
}

/// The session id the hand-written `expected.toml` of `case` names.
pub fn session_of(case: &str) -> String {
    recorded(case, "expected.toml")
        .lines()
        .find_map(|line| line.strip_prefix("session_id = \""))
        .and_then(|rest| rest.split('"').next())
        .expect("a session id")
        .to_owned()
}

/// The recorded census log of the real gateway.
pub fn census_log() -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gateway/census_log.txt"),
    )
    .expect("the recorded gateway log")
}

/// The run key of the recorded log of a run that denied the shell tool.
pub const DENIED_RUN: &str =
    "3333333333333333333333333333333333333333333333333333333333333333-r1-a1";

/// The recorded census log of a run whose cell was `bypassPermissions` with `tools.disallowed`
/// holding `Bash`, under the key [`DENIED_RUN`] and the tag [`LOG_TAG`].
pub fn census_log_deny_shell() -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/gateway/census_log_deny_shell.txt"),
    )
    .expect("the recorded gateway log of the denied run")
}

/// The `OpenCode` driver with a fixed command and a fixed preparation: the readers, the stop, the
/// collect and the census are the driver's own.
pub struct WiringDriver(pub OpenCodeDriver);

/// The file the prepared record shows.
pub const CONFIG_PATH: &str = ".xdg/config/opencode/opencode.json";

/// The text of the prepared file: the provider refers to the gateway by variable names.
pub const CONFIG_TEXT: &str =
    r#"{"providers":{"candler":{"settings":{"apiKey":"{env:CANDLER_GATEWAY_TOKEN}"}}}}"#;

impl HarnessDriver for WiringDriver {
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

    fn prepare(
        &self,
        _cell: &Cell<'_>,
        _workspace: &Workspace<'_>,
    ) -> Result<Prepared, DriverError> {
        let path = CONFIG_PATH.parse().expect("a path");
        Ok(Prepared::new(
            vec![PreparedFile::in_home(
                path,
                CONFIG_TEXT.as_bytes().to_vec(),
                false,
            )],
            vec![EnvEntry::literal(
                String::from("OC_LITERAL"),
                String::from(LITERAL_CANARY),
            )],
        ))
    }

    fn command(
        &self,
        _cell: &Cell<'_>,
        _workspace: &Workspace<'_>,
    ) -> Result<CommandSpec, DriverError> {
        Ok(CommandSpec::builder()
            .argv(vec![
                String::from("opencode"),
                String::from("run"),
                String::from("--"),
                String::from("the prompt"),
            ])
            .build()?)
    }

    fn gateway(&self) -> Option<GatewayEnv> {
        self.0.gateway()
    }

    fn state_paths(&self) -> Vec<StatePath> {
        self.0.state_paths()
    }

    fn census_stream(&self) -> Option<StreamName> {
        self.0.census_stream()
    }

    fn stop(&self) -> StopSpec {
        self.0.stop()
    }

    // TRAIT_OBJECT: the host hands out its trace reader as a trait object
    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        self.0.live_reader()
    }

    // TRAIT_OBJECT: the host hands out its trace reader as a trait object
    fn offline_reader(&self, stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        self.0.offline_reader(stream)
    }

    fn classify_exit(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy {
        self.0.classify_exit(exit, sessions)
    }

    fn classify_end(
        &self,
        exit: &ProcessExit,
        sessions: &[Session],
        observed: &Observed,
    ) -> EndedBy {
        self.0.classify_end(exit, sessions, observed)
    }

    fn collect(&self, workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec> {
        self.0.collect(workspace, observed)
    }
}

/// A gateway that serves nothing and answers a census with the recorded log, rewritten to the
/// run and tag it is asked about, through the real [`census_of`].
pub struct CensusGateway {
    /// The log the gateway pretends to have written, under [`LOG_RUN`] and [`LOG_TAG`].
    pub log: String,
}

impl Gateway for CensusGateway {
    fn start<'a>(
        &'a self,
        sup: &'a Supervisor,
        docker: &'a dyn DockerDriver, // TRAIT_OBJECT: the driver is docker, or a fake in tests
        request: GatewayStart<'a>,
    ) -> BoxFuture<'a, Result<GatewayHandle, GatewayError>> {
        FakeGateway.start(sup, docker, request)
    }

    fn probe<'a>(&'a self, handle: &'a GatewayHandle) -> BoxFuture<'a, GatewayStatus> {
        FakeGateway.probe(handle)
    }

    fn run_signals<'a>(
        &'a self,
        _handle: &'a GatewayHandle,
        _run: &'a RunKey,
    ) -> BoxFuture<'a, Result<Vec<GatewaySignal>, GatewayError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn run_census<'a>(
        &'a self,
        handle: &'a GatewayHandle,
        run: &'a RunKey,
        _since: std::time::SystemTime,
    ) -> BoxFuture<'a, Result<Vec<u8>, GatewayError>> {
        Box::pin(async move {
            let tag = handle.run_tag(run);
            let log = self
                .log
                .replace(LOG_RUN, &run.to_string())
                .replace(LOG_TAG, &tag);
            Ok(census_of(&log, run, &tag))
        })
    }
}

/// The host of the job: the fake host of the support, with the census gateway.
pub struct WiringHost {
    /// The fake host the runs are placed through.
    pub inner: Arc<FakeHost>,
    /// The log the gateway answers a census from.
    pub log: String,
}

impl JobHost for WiringHost {
    fn docker_cli(
        &self,
        supervisor: Arc<Supervisor>,
        record_dir: PathBuf,
    ) -> Result<DockerCli, DockerError> {
        self.inner.docker_cli(supervisor, record_dir)
    }

    fn gateway<'a>(
        &'a self,
        _rt: &'a Runtime,
        _spec: &'a JobSpec,
        // TRAIT_OBJECT: the gateway is a fake, as the session takes it from the host
    ) -> BoxFuture<'a, Result<Arc<dyn Gateway>, OrchestrateError>> {
        let log = self.log.clone(); // BORROW: the gateway owns the log
        Box::pin(async move {
            // TRAIT_OBJECT: the session takes the gateway as a trait object
            let gateway: Arc<dyn Gateway> = Arc::new(CensusGateway { log });
            Ok(gateway)
        })
    }

    // TRAIT_OBJECT: the host hands the runs an isolation as a trait object
    fn isolation(&self, isolation: DockerIsolation) -> Arc<dyn Isolation> {
        self.inner.isolation(isolation)
    }

    fn clock(&self) -> Clock {
        self.inner.clock()
    }

    fn packages<'a>(
        &'a self,
        rt: &'a Runtime,
        image: &'a ImageRef,
        names: &'a [String],
    ) -> BoxFuture<'a, Option<candler_core::isolation::image::PackageReadings>> {
        self.inner.packages(rt, image, names)
    }
}

/// What a scenario sets: what the fake agent prints, what a collect prints, the gateway's log and
/// how long the agent lives.
pub struct Scenario {
    /// What the agent prints on standard output.
    pub stdout: String,
    /// What a collect command's container prints, or nothing.
    pub export: Option<String>,
    /// The log the gateway answers a census from.
    pub log: String,
    /// How long the agent lives, in seconds.
    pub sleep_s: u32,
}

impl Scenario {
    /// A run of the recorded case `case` that exits at once, with the gateway log of the
    /// recorded census.
    pub fn exited(case: &str) -> Self {
        Self {
            stdout: recorded(case, "stdout.jsonl"),
            export: Some(recorded(case, "export.json")),
            log: census_log(),
            sleep_s: 0,
        }
    }
}

/// What a run left.
pub struct Ran {
    /// The world the run ran in.
    pub rig: Rig,
    /// The run's key.
    pub key: RunKey,
    /// The run's directory in the store.
    pub dir: PathBuf,
    /// The manifest of the sealed run.
    pub manifest: Value,
    /// Every docker call of the fake client, in order.
    pub docker: Vec<Vec<String>>,
    /// The events of the job.
    pub events: Vec<(Option<RunKey>, EventPayload)>,
}

impl Ran {
    /// The value measured at `path`.
    pub fn measured(&self, path: &str) -> Option<Value> {
        self.manifest["measured"]
            .as_array()?
            .iter()
            .find(|entry| entry["path"] == path)
            .map(|entry| entry["value"].clone()) // BORROW: the answer owns its value
    }

    /// The text of the raw stream `name`, gunzipped, or none when the run holds none.
    pub fn raw(&self, name: &str) -> Option<String> {
        let path = self.dir.join("raw").join(format!("{name}.jsonl.gz"));
        let bytes = fs::read(path).ok()?;
        let mut text = String::new();
        std::io::Read::read_to_string(
            &mut flate2::read::MultiGzDecoder::new(bytes.as_slice()),
            &mut text,
        )
        .ok()?;
        Some(text)
    }

    /// The outcome text of each pre-stop step the run's stop ladder took.
    pub fn pre_stops(&self) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|(_, payload)| {
                if let EventPayload::ProcessStopStep { step, outcome, .. } = payload
                    && format!("{step:?}").contains("PreStop")
                {
                    return outcome.clone();
                }
                None
            })
            .collect()
    }

    /// The steps the stop ladder took, by name, in order.
    pub fn steps(&self) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|(_, payload)| {
                if let EventPayload::ProcessStopStep { step, .. } = payload {
                    return Some(format!("{step:?}"));
                }
                None
            })
            .collect()
    }
}

/// The agent container of the scenario.
pub fn agent_cid() -> ContainerId {
    cid(0xabc)
}

fn facts() -> Facts {
    Facts {
        secrets: Vec::new(),
        ..Facts::base()
    }
}

/// The key of the run a job over the one-cell experiment plans.
pub fn planned_key() -> RunKey {
    let facts = facts();
    manifest(&Run {
        model: "m1",
        replicate: 1,
        attempt: 1,
        facts: &facts,
        integrity: json!({"kind": "valid"}),
        sealed: true,
        job: None,
    })
    .run()
    .clone() // BORROW: the key outlives the manifest
}

/// Scripts the fake client's `inspect` and `ls` answers for the agent of the run `key`, and the
/// answer of a collect container: `export`.
fn script(rig: &Rig, key: &RunKey, export: Option<&str>) {
    let id = agent_cid();
    let labels = json!({
        "candler.job": rig.job.to_string(),
        "candler.run": key.to_string(),
        "candler.role": "agent",
        "candler.experiment": digest(9).to_string(),
    });
    let answer = json!({
        "Id": id.as_str(),
        "Created": CREATED,
        "Image": IMAGE_ID,
        "Config": {"Labels": labels},
        "State": {"Status": "running", "Running": true, "ExitCode": 0, "OOMKilled": false,
                  "StartedAt": null, "FinishedAt": null},
    });
    let canned = rig.canned();
    fs::write(canned.join("container-inspect.out"), answer.to_string()).expect("an inspect");
    fs::write(canned.join("container-ls.out"), id.as_str()).expect("a listing");
    if let Some(text) = export {
        fs::write(canned.join("run.out"), text).expect("an export");
    }
}

/// Every docker call the fake client logged, in order. The client writes one JSON array of argv
/// per call and does not escape a newline inside an argument, so a record runs from a line that
/// starts with `[` to the next such line.
pub fn docker_calls(rig: &Rig) -> Vec<Vec<String>> {
    let text = fs::read_to_string(&rig.log).unwrap_or_default();
    let mut records: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.starts_with('[') {
            records.push(line.to_owned());
        } else if let Some(last) = records.last_mut() {
            last.push_str("\\n");
            last.push_str(line);
        }
    }
    records
        .iter()
        .map(|record| serde_json::from_str::<Vec<String>>(record).expect("a logged call"))
        .collect()
}

/// Runs the one-cell job of `scenario` over the `OpenCode` driver to its end; `during` runs on
/// another thread with the job's control while the job runs.
pub fn run(scenario: &Scenario, during: impl FnOnce(&Rig, &JobControl) + Send) -> Ran {
    let facts = facts();
    let key = planned_key();
    let rig = Rig::new(0);
    script(&rig, &key, scenario.export.as_deref());
    let inner = rig.host_with_agent(FakeAgent {
        cid: agent_cid(),
        sleep_s: scenario.sleep_s,
        // TRAIT_OBJECT: the hook sweeps through a trait object
        sweeper: Some(Arc::new(NoSweep)),
        stdout: scenario.stdout.clone(), // BORROW: the agent owns the text
    });
    let host: Arc<dyn JobHost> = Arc::new(WiringHost {
        inner,
        log: scenario.log.clone(), // BORROW: the host owns the log
    });
    let job = spec(&["m1"], &facts, 3600, 1);
    let (cancel, _keep) = watch::channel::<Option<JobCause>>(None);
    let ctl = JobControl {
        cancel: Arc::new(cancel),
    };
    let sink = Arc::new(Recorder::default());
    // TRAIT_OBJECT: the job takes its sink as a trait object
    let events: Arc<dyn EventSink> = sink.clone(); // BORROW: the test reads the sink after the job
    let mut runtime = rig.runtime_with_sink(events);
    runtime.driver = Arc::new(WiringDriver(OpenCodeDriver::new().expect("the driver")));
    let tokio = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let body = orchestrate_with(runtime, job, ctl.clone(), host); // BORROW: the helper thread holds the control too
    let outcome = std::thread::scope(|scope| {
        scope.spawn(|| during(&rig, &ctl));
        tokio.block_on(body)
    });
    assert_eq!(outcome.runs.len(), 1, "the run ended");
    let dir = rig.store.layout().root().join("runs").join(key.to_string());
    assert!(dir.join("run.done").is_file(), "the run is sealed");
    let manifest: Value =
        serde_json::from_slice(&fs::read(dir.join("manifest.json")).expect("a manifest"))
            .expect("manifest JSON");
    let docker = docker_calls(&rig);
    Ran {
        manifest,
        docker,
        events: sink.events(),
        key,
        dir,
        rig,
    }
}

/// A sweeper that destroys nothing, so a stop leaves the container to the guard of the run.
struct NoSweep;

impl candler_core::isolation::container::sweeper::ContainerSweeper for NoSweep {
    fn sweep<'a>(
        &'a self,
        _scope: &'a candler_core::isolation::model::SweepScope,
    ) -> BoxFuture<
        'a,
        Result<
            candler_core::isolation::model::ContainerSweep,
            candler_core::isolation::container::sweeper::SweepError,
        >,
    > {
        Box::pin(async { Ok(candler_core::isolation::model::ContainerSweep::default()) })
    }
}

/// Waits until `path` exists, up to ten seconds.
pub fn wait_for(path: &Path) {
    let began = Instant::now();
    while !path.exists() {
        assert!(
            began.elapsed() < Duration::from_secs(10),
            "{} did not appear",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
