//! The session controls: whether the instrument works, checked before a job runs a session cell.
//!
//! Control C1 (the agent must wait for a background job) and control C2 (the agent must stop
//! once it has answered) run the real harness in the real base image, through the production
//! gateway sidecar, against the scripted fake model in `assets/docker/fakemodel`. A control
//! validates the instrument and never a model: the runner is given the base image and a control,
//! and nothing of any cell's model, so no model's behaviour can reach the verdict.
//!
//! A passing record is keyed by the content of the base image and the digest of the control (the
//! fake model's files, the control's parameters and the version of this crate, which holds the
//! verdict and the close rule), so a changed script, parameter or release is a new key. The record is written once, and a failing verdict writes nothing. The controls run the
//! close rule at the controls' own `idle_s` only: a defect that appears at a longer idle is for
//! the run loop's own guards.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use candler_schemas::execution::{Comparable, EventPayload, JobRecord, JobState, SecretPresence};
use candler_schemas::primitives::{Digest, RunKey};
use candler_schemas::values::SourceEntry;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::watch;

use super::session::{DockerHost, JobHost as _, JobSession, sidecar_gateway};
use crate::import::digest_of;
use crate::isolation::driver::DockerDriver;
use crate::isolation::gateway::{SecretError, SecretSource, SecretValue, UpstreamConfig};
use crate::isolation::image::{base_tag, context_digest};
use crate::isolation::model::{BuildRequest, Labels, NetworkName, SweepScope};
use crate::jobs::dir::JobDir;
use crate::jobs::model::{JobCause, JobControl, JobSpec};
use crate::jobs::runtime::launch::{own_identity, spec_digest};
use crate::jobs::slots::Holder;
use crate::runner::evidence::Inputs;
use crate::runner::evidence::session::ControlRef;
use crate::runner::exec::{CellRun, exec_run};
use crate::runner::plan::RunPlan;
use crate::runner::{JobMeter, RunControl, Runtime};
use crate::store::Store;
use crate::supervise::EventSink;
use crate::supervise::signal::control_marker;

use super::session::OrchestrateError;

/// The directory below the store root that holds the control records.
const CONTROLS_DIR: &str = "controls";

/// The schema line of a control record.
const RECORD_SCHEMA: &str = "session-controls/1";

/// The milliseconds past `idle_s` within which the session closes: the time for a late `Stop`
/// line and the harness's exit.
pub const CLOSE_TOLERANCE_MS: u64 = 1000;

/// The length of the background job the C1 script starts, in milliseconds.
pub const C1_JOB_MS: u64 = 20_000;

/// The name the fake model's certificate is for.
const FAKE_HOST: &str = "fake-model.test";

/// The port the fake model serves on.
const FAKE_PORT: u16 = 8443;

/// The directory of the fake model, as the container sees it.
const FAKE_MOUNT: &str = "/fake";

/// The seconds after which a fake model container ends on its own.
const FAKE_LIFETIME_S: &str = "900";

/// How long a docker call of the controls may run.
const CALL: Duration = Duration::from_secs(60);

/// How long the controls wait for the fake model to make its authority, or to end.
const WAIT: Duration = Duration::from_secs(60);

/// The wall cap of a control's run, in seconds.
const WALL_S: u64 = 240;

/// The two controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Control {
    /// Must wait: a background job is pending when the agent first yields.
    C1,
    /// Must stop: the agent answers once and the session closes `idle_s` later.
    C2,
}

impl Control {
    /// The controls, in the order they run.
    pub const ALL: [Self; 2] = [Self::C1, Self::C2];

    /// The control's name, which is also the fake model's script name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::C1 => "c1",
            Self::C2 => "c2",
        }
    }

    /// The control's own `runner.idle_s`.
    #[must_use]
    pub const fn idle_s(self) -> u64 {
        match self {
            Self::C1 => 3,
            Self::C2 => 2,
        }
    }

    /// The turns the script's answers make.
    #[must_use]
    pub const fn turns(self) -> u64 {
        match self {
            Self::C1 => 2,
            Self::C2 => 1,
        }
    }

    /// The least time the last turn ends after, in milliseconds.
    const fn least_yield_ms(self) -> u64 {
        match self {
            Self::C1 => C1_JOB_MS,
            Self::C2 => 0,
        }
    }
}

/// What a control's run measured, read from its sealed manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measures {
    /// Whether the run is valid.
    pub valid: bool,
    /// Whether `run.ended_by` is `completed`.
    pub completed: bool,
    /// `session.idle_s`.
    pub idle_s: Option<u64>,
    /// `session.turns`.
    pub turns: Option<u64>,
    /// Whether `session.pending_at_end` is the empty list.
    pub nothing_pending: Option<bool>,
    /// `session.end`.
    pub end: Option<String>,
    /// `session.close_forced`.
    pub close_forced: Option<bool>,
    /// `session.yielded_at_ms`.
    pub yielded_at_ms: Option<u64>,
    /// `run.duration_ms`.
    pub duration_ms: Option<u64>,
}

impl Measures {
    /// The measures of the sealed run whose manifest is `manifest`; `valid` says whether the run
    /// carried no invalid reason.
    #[must_use]
    pub fn from_manifest(manifest: &Value, valid: bool) -> Self {
        let at = |path: &str| {
            manifest["measured"]
                .as_array()?
                .iter()
                .find(|entry| entry["path"] == path)
                .map(|entry| entry["value"].clone())
        };
        Self {
            valid,
            completed: at("run.ended_by") == Some(json!({"kind": "completed"})),
            idle_s: at("session.idle_s").and_then(|value| value.as_u64()),
            turns: at("session.turns").and_then(|value| value.as_u64()),
            nothing_pending: at("session.pending_at_end")
                .map(|value| value.as_array().is_some_and(Vec::is_empty)),
            end: at("session.end").and_then(|value| value.as_str().map(str::to_owned)), // BORROW: the measures own their text
            close_forced: at("session.close_forced").and_then(|value| value.as_bool()),
            yielded_at_ms: at("session.yielded_at_ms").and_then(|value| value.as_u64()),
            duration_ms: at("run.duration_ms").and_then(|value| value.as_u64()),
        }
    }
}

/// Whether `measures` are what `control` requires of a working instrument; the reason when not.
///
/// The check reads the measures of the control's own run and nothing else.
///
/// # Errors
/// The first requirement the measures do not meet, in words.
pub fn verdict(control: Control, measures: &Measures) -> Result<(), String> {
    let name = control.name();
    if !measures.valid || !measures.completed {
        return Err(format!(
            "the run of {name} was not valid and completed (valid: {}, completed: {})",
            measures.valid, measures.completed
        ));
    }
    if measures.idle_s != Some(control.idle_s()) {
        return Err(format!(
            "{name} ran with idle_s {:?}, not {}",
            measures.idle_s,
            control.idle_s()
        ));
    }
    if measures.turns != Some(control.turns()) {
        return Err(format!(
            "{name} recorded {:?} turns, expected {}",
            measures.turns,
            control.turns()
        ));
    }
    if measures.nothing_pending != Some(true) {
        return Err(format!("{name} ended with work still pending"));
    }
    if measures.end.as_deref() != Some("closed") || measures.close_forced != Some(false) {
        return Err(format!(
            "{name} did not close on its own (end: {:?}, forced: {:?})",
            measures.end, measures.close_forced
        ));
    }
    let (Some(yielded), Some(duration)) = (measures.yielded_at_ms, measures.duration_ms) else {
        return Err(format!("{name} recorded no turn end or no duration"));
    };
    if yielded < control.least_yield_ms() {
        return Err(format!(
            "{name} yielded at {yielded} ms, before its background job ended"
        ));
    }
    let idle_ms = control.idle_s().saturating_mul(1000);
    let closing = duration.saturating_sub(yielded);
    if closing < idle_ms || closing > idle_ms.saturating_add(CLOSE_TOLERANCE_MS) {
        return Err(format!(
            "{name} closed {closing} ms after its last turn, not within {idle_ms} to {} ms",
            idle_ms.saturating_add(CLOSE_TOLERANCE_MS)
        ));
    }
    Ok(())
}

/// What a runner says of the instrument before it runs a control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// The first digits of the digest of the base image's sources: the content the record is
    /// keyed by.
    pub base_src: String,
    /// The base image's id.
    pub base_id: String,
    /// The digest of the controls: the fake model's files and the controls' parameters.
    pub control_digest: Digest,
}

/// The digest of the controls: the digest `fake_files` of the fake model's build context and the
/// parameters of both controls and of the verdict.
///
/// # Errors
/// [`OrchestrateError::Controls`] when the digest cannot be formed, which a SHA-256 never does.
pub fn control_digest(fake_files: &str) -> Result<Digest, OrchestrateError> {
    let controls: Vec<Value> = Control::ALL
        .iter()
        .map(|control| {
            json!({
                "script": control.name(),
                "idle_s": control.idle_s(),
                "turns": control.turns(),
                "least_yield_ms": control.least_yield_ms(),
            })
        })
        .collect();
    let parameters = json!({
        "fakemodel": fake_files,
        "candler_core": env!("CARGO_PKG_VERSION"),
        "controls": controls,
        "close_tolerance_ms": CLOSE_TOLERANCE_MS,
        "wall_s": WALL_S,
    });
    let bytes = serde_json::to_vec(&parameters).map_err(|error| setup(&error))?;
    digest_of(bytes.as_slice()).map_err(|error| setup(&error)) // BORROW: explicit .as_slice()
}

/// Runs a control and says what it measured.
pub trait ControlRunner: Send + Sync {
    /// The base image of the job's cells and the digest of the controls.
    ///
    /// # Errors
    /// The reason the base image or the fake model's files cannot be identified.
    fn identify<'a>(&'a self, rt: &'a Runtime) -> BoxFuture<'a, Result<Identity, String>>;

    /// Runs `control` against the fake model, in the base image `identity` names, with the
    /// harness `harness` (its `name` and `version`). A cause sent on `cancel` stops the run.
    ///
    /// # Errors
    /// The reason the run could not be made, or did not seal.
    fn run<'a>(
        &'a self,
        rt: &'a Runtime,
        identity: &'a Identity,
        harness: &'a Value,
        control: Control,
        cancel: watch::Receiver<Option<JobCause>>,
    ) -> BoxFuture<'a, Result<Measures, String>>;
}

/// A refusal of session mode for the harness named `harness`.
fn refused(harness: &str, control: &str, reason: String) -> OrchestrateError {
    OrchestrateError::Controls {
        harness: harness.to_owned(), // BORROW: the error owns its text
        control: control.to_owned(), // BORROW: the error owns its text
        reason,
    }
}

/// A refusal because the controls could not be set up.
fn setup(error: &impl ToString) -> OrchestrateError {
    refused("the harness", "setup", error.to_string())
}

/// What a record holds: the key it was written under and what each control measured.
#[derive(Debug, Serialize, Deserialize)]
struct Record {
    schema: String,
    base_id: String,
    harness: String,
    control_digest: String,
    results: Vec<Recorded>,
}

/// What one control measured.
#[derive(Debug, Serialize, Deserialize)]
struct Recorded {
    control: String,
    measures: Measures,
}

/// The `<name> <version>` of the harness value `harness`.
fn harness_text(harness: &Value) -> String {
    let field = |key: &str| harness.get(key).and_then(Value::as_str).unwrap_or("?");
    format!("{} {}", field("name"), field("version"))
}

/// The record's file name below `controls/`.
fn record_name(identity: &Identity) -> String {
    let hex = identity.control_digest.hex();
    format!(
        "{}-{}.json",
        identity.base_src,
        hex.get(..16).unwrap_or(hex)
    )
}

/// Writes `bytes` at `path` unless a file is there, atomically: the file appears whole or not at
/// all, and a file that is there is never replaced. Says whether this call wrote it.
fn write_once(path: &Path, bytes: &[u8]) -> std::io::Result<bool> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let temp = dir.join(format!(".{}-{}.tmp", std::process::id(), nanos()));
    std::fs::write(&temp, bytes)?;
    let linked = std::fs::hard_link(&temp, path);
    drop(std::fs::remove_file(&temp));
    match linked {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    }
}

/// Nanoseconds since the epoch, for names that must not repeat.
fn nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos())
}

/// The reference of the record at `path` below `root`, once its bytes are `bytes`.
fn reference(root: &Path, path: &Path, bytes: &[u8]) -> Result<ControlRef, OrchestrateError> {
    let digest = digest_of(bytes).map_err(|error| setup(&error))?;
    let relative = path
        .strip_prefix(root)
        .map_err(|error| setup(&error))?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(ControlRef {
        path: relative,
        digest,
    })
}

/// Whether the cell `comparable` is a session.
#[must_use]
pub fn is_session(comparable: &Comparable) -> bool {
    comparable.values().iter().any(|value| {
        value.path().as_str() == crate::experiment::paths::RUNNER_MODE // BORROW: compares the path
            && value
                .value()
                .is_some_and(|value| value.as_value() == "session")
    })
}

/// The control record the job relies on: the passing record of the base image under the current
/// controls, or the one made by running C1 and then C2.
///
/// A record that is there is reused and never re-run; a failing control writes nothing.
///
/// # Errors
/// [`OrchestrateError::Controls`] naming the harness version and the control that did not pass,
/// or `setup` when the controls could not be run at all.
pub async fn ensure(
    rt: &Runtime,
    spec: &JobSpec,
    runner: &dyn ControlRunner, // TRAIT_OBJECT: the runner is docker, or a fake in tests
    ctl: &JobControl,
) -> Result<ControlRef, OrchestrateError> {
    let harness = spec
        .resolution
        .values
        .iter()
        .find(|value| value.path().as_str() == crate::experiment::paths::HARNESS) // BORROW: compares the path
        .and_then(|value| value.value())
        .map(|value| value.as_value().clone()) // BORROW: the controls own the harness value
        .ok_or_else(|| setup(&"the resolution names no harness"))?;
    let text = harness_text(&harness);
    let identity = runner
        .identify(rt)
        .await
        .map_err(|reason| refused(&text, "setup", reason))?;
    let root = rt.store.layout().root();
    let path = root.join(CONTROLS_DIR).join(record_name(&identity));
    if let Ok(bytes) = std::fs::read(&path) {
        let record: Record = serde_json::from_slice(&bytes).map_err(|error| {
            refused(
                &text,
                "setup",
                format!("{} is not a record: {error}", path.display()),
            )
        })?;
        if record.control_digest != identity.control_digest.to_string() {
            return Err(refused(
                &text,
                "setup",
                format!("{} holds another key than its name says", path.display()),
            ));
        }
        return reference(root, &path, &bytes);
    }
    let mut results = Vec::new();
    for control in Control::ALL {
        let measures = runner
            .run(rt, &identity, &harness, control, ctl.cancel.subscribe())
            .await
            .map_err(|reason| refused(&text, control.name(), reason))?;
        verdict(control, &measures).map_err(|reason| refused(&text, control.name(), reason))?;
        results.push(Recorded {
            control: control.name().to_owned(), // BORROW: the record owns its text
            measures,
        });
    }
    let record = Record {
        schema: RECORD_SCHEMA.to_owned(), // BORROW: the record owns its text
        base_id: identity.base_id.clone(), // BORROW: the record owns its text
        harness: text.clone(),            // BORROW: the record owns its text
        control_digest: identity.control_digest.to_string(),
        results,
    };
    let bytes = serde_json::to_vec_pretty(&record).map_err(|error| setup(&error))?;
    let wrote = write_once(&path, &bytes).map_err(|error| setup(&error))?;
    // a record another job wrote first is the one both rely on
    let held = if wrote {
        bytes
    } else {
        std::fs::read(&path).map_err(|error| setup(&error))?
    };
    reference(root, &path, &held)
}

/// The build contexts the production controls need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlContexts {
    /// The build context of the base image the cells run on.
    pub base: PathBuf,
    /// The build context of the fake model's image.
    pub fakemodel: PathBuf,
}

/// Runs the controls in docker: the fake model in a container of its own, the production gateway
/// sidecar in front of it, and the harness in the base image behind the production isolation.
pub struct DockerControlRunner {
    contexts: ControlContexts,
}

impl DockerControlRunner {
    /// A runner over `contexts`.
    #[must_use]
    pub const fn new(contexts: ControlContexts) -> Self {
        Self { contexts }
    }
}

/// A secret source that holds the fake model's credential.
struct Fixed(String);

impl SecretSource for Fixed {
    fn resolve(&self, _name: &str) -> Result<SecretValue, SecretError> {
        Ok(SecretValue::new(self.0.clone())) // BORROW: the value owns its text
    }
}

/// Discards every event: a control belongs to no job's log.
struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// Polls `check` until it gives a value, for at most [`WAIT`]; none when it never does.
async fn wait_for<T, F, Fut>(mut check: F) -> Option<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let deadline = std::time::Instant::now() + WAIT;
    loop {
        if let Some(found) = check().await {
            return Some(found);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// The fake model's image: found by its content tag, or built from `context`.
async fn fake_image(
    docker: &dyn DockerDriver, // TRAIT_OBJECT: docker, or a fake in tests
    context: &Path,
    digest: &str,
) -> Result<String, String> {
    let tag = format!(
        "candler-fakemodel:src-{}",
        digest.get(..12).unwrap_or(digest)
    );
    if let Some(found) = docker
        .image_inspect(&tag)
        .await
        .map_err(|e| e.to_string())?
    {
        return Ok(found.id.0);
    }
    let request = BuildRequest {
        context: context.to_path_buf(), // BORROW: the request owns its context
        tag,
        build_args: Vec::new(),
        labels: Labels::new(),
    };
    docker
        .build(&request)
        .await
        .map(|built| built.id.0)
        .map_err(|e| e.to_string())
}

impl ControlRunner for DockerControlRunner {
    fn identify<'a>(&'a self, rt: &'a Runtime) -> BoxFuture<'a, Result<Identity, String>> {
        Box::pin(async move {
            let tag =
                base_tag(&self.contexts.base).map_err(|e| format!("the base sources: {e}"))?;
            let base = rt
                .docker
                .image_inspect(&tag)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("docker holds no image {tag}"))?;
            let src = tag.rsplit("src-").next().unwrap_or(&tag).to_owned(); // BORROW: the identity owns its text
            let fake = context_digest(&self.contexts.fakemodel)
                .map_err(|e| format!("the fake model's files: {e}"))?;
            let control_digest = control_digest(&fake).map_err(|e| e.to_string())?;
            Ok(Identity {
                base_src: src,
                base_id: base.id.0,
                control_digest,
            })
        })
    }

    fn run<'a>(
        &'a self,
        rt: &'a Runtime,
        identity: &'a Identity,
        harness: &'a Value,
        control: Control,
        cancel: watch::Receiver<Option<JobCause>>,
    ) -> BoxFuture<'a, Result<Measures, String>> {
        Box::pin(self.run_control(rt, identity, harness, control, cancel))
    }
}

/// What a control's world made, so that it can be ended whatever happens.
struct World {
    work: PathBuf,
    fake: Option<String>,
}

impl DockerControlRunner {
    async fn run_control(
        &self,
        rt: &Runtime,
        identity: &Identity,
        harness: &Value,
        control: Control,
        cancel: watch::Receiver<Option<JobCause>>,
    ) -> Result<Measures, String> {
        let work = rt.store.layout().root().join(CONTROLS_DIR).join(format!(
            "work-{}-{}",
            std::process::id(),
            nanos()
        ));
        let mut world = World { work, fake: None };
        let outcome = self
            .drive(rt, identity, harness, control, cancel, &mut world)
            .await;
        drop(std::fs::remove_dir_all(&world.work));
        outcome
    }

    async fn drive(
        &self,
        rt: &Runtime,
        identity: &Identity,
        harness: &Value,
        control: Control,
        cancel: watch::Receiver<Option<JobCause>>,
        world: &mut World,
    ) -> Result<Measures, String> {
        let err = |what: &str, e: &dyn std::fmt::Display| format!("{what}: {e}"); // TRAIT_OBJECT: any error is rendered
        let job = rt.job.clone(); // BORROW: the control's runtime owns its job id
        let store =
            Store::init(&world.work.join("store")).map_err(|e| err("the scratch store", &e))?;
        let record = JobRecord::builder()
            .id(job.clone()) // BORROW: the record owns its id
            .supervisor(own_identity().map_err(|e| err("the supervisor identity", &e))?)
            .spec_digest(spec_digest(b"controls").map_err(|e| err("the digest", &e))?)
            .state(JobState::Running)
            .build()
            .map_err(|e| err("the job record", &e))?;
        let job_dir = JobDir::create(store.layout(), job.clone(), &record) // BORROW: the directory owns its id
            .map_err(|e| err("the job directory", &e))?;
        let host = DockerHost::default();
        let docker_cli = host
            .docker_cli(Arc::clone(&rt.supervisor), job_dir.control_dir())
            .map_err(|e| err("the docker client", &e))?;
        let docker: Arc<dyn DockerDriver> = Arc::new(docker_cli); // TRAIT_OBJECT: the runtime reads docker through the client

        // the control's own runtime, over a scratch store
        let (cell, plan) = control_cell(identity, harness, control)?;
        let mut crt = Runtime {
            supervisor: Arc::clone(&rt.supervisor),
            store: store.clone(), // BORROW: the runtime owns its handle on the store
            sink: Arc::new(NullSink),
            slots: rt.slots.clone(), // BORROW: the control takes a slot of the job's own
            docker: Arc::clone(&docker),
            isolation: None,
            gateway: None,
            job: job.clone(), // BORROW: the runtime owns its job id
            driver: Arc::clone(&rt.driver),
            cells: std::collections::BTreeMap::new(),
            eval_infra: rt.eval_infra.clone(), // BORROW: the runtime owns its build identity
        };
        crt.cells.insert(plan.cell.clone(), Arc::new(cell)); // BORROW: the map owns the digest

        // the fake model
        let digest =
            context_digest(&self.contexts.fakemodel).map_err(|e| err("the fake's files", &e))?;
        let image = fake_image(&*docker, &self.contexts.fakemodel, &digest).await?;
        let token = format!("candler-control-{}-{}", std::process::id(), nanos());
        let started = docker
            .capture(fake_args(&image, &token), CALL)
            .await
            .map_err(|e| err("starting the fake model", &e))?;
        let fake = String::from_utf8_lossy(&started).trim().to_owned(); // BORROW: the world keeps the container id
        world.fake = Some(fake.clone()); // BORROW: the world keeps the id to stop it
        let ca = wait_for(|| authority(&*docker, &fake))
            .await
            .ok_or("the fake model made no authority")?;

        // the gateway sidecar in front of it
        let upstream = UpstreamConfig::builder()
            .host(FAKE_HOST)
            .port(FAKE_PORT)
            .ca_pem(ca)
            .build()
            .map_err(|e| err("the upstream", &e))?;
        let gateway = sidecar_gateway(&crt, upstream, 4, control_marker())
            .await
            .map_err(|e| e.to_string())?;

        // the fake joins the egress network once the gateway has made it
        let egress = NetworkName::egress_for_job(&job);
        let link = {
            let (docker, egress, fake) = (Arc::clone(&docker), egress.clone(), fake.clone()); // BORROW: the task owns its handles
            tokio::spawn(async move {
                wait_for(|| {
                    let (docker, egress) = (Arc::clone(&docker), egress.clone()); // BORROW: each probe owns its handles
                    async move { docker.network_inspect(&egress).await.ok().map(|_| ()) }
                })
                .await
                .ok_or_else(|| "the egress network never appeared".to_owned())?; // BORROW: the error owns its text
                docker
                    .capture(
                        [
                            "network",
                            "connect",
                            "--alias",
                            FAKE_HOST,
                            egress.as_str(),
                            fake.as_str(),
                        ]
                        .iter()
                        .map(|word| (*word).to_owned()) // BORROW: owned argv
                        .collect(),
                        CALL,
                    )
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            })
        };
        let secrets = Fixed(token);
        let experiment = identity.control_digest.clone(); // BORROW: the label owns its digest
        let opened =
            JobSession::open_service(&mut crt, "control", gateway, &experiment, &host, &secrets)
                .await;
        let connected = link.await.map_err(|e| err("the join", &e)).and_then(|r| r);
        let session = match (opened, connected) {
            (Ok(session), Ok(())) => session,
            (Ok(session), Err(reason)) => {
                session.close(&SweepScope::CurrentJob).await;
                self.stop_fake(&fake, &*docker).await;
                return Err(reason);
            }
            (Err(error), _) => {
                self.stop_fake(&fake, &*docker).await;
                return Err(error.to_string());
            }
        };

        // the run
        let measures = Self::run_cell(&crt, &plan, &store, cancel).await;
        // the fake leaves the egress network before the networks are removed
        self.stop_fake(&fake, &*docker).await;
        session.close(&SweepScope::CurrentJob).await;
        measures
    }

    /// Ends the fake model by its done file and waits until docker no longer has the container.
    async fn stop_fake(
        &self,
        fake: &str,
        docker: &dyn DockerDriver, // TRAIT_OBJECT: docker, or a fake in tests
    ) {
        let ended = docker
            .capture(
                argv(&["exec", fake, "touch", &format!("{FAKE_MOUNT}/done")]),
                CALL,
            )
            .await;
        drop(ended);
        let gone = wait_for(|| async {
            docker
                .capture(argv(&["container", "inspect", fake]), CALL)
                .await
                .err()
        })
        .await;
        drop(gone);
    }

    async fn run_cell(
        crt: &Runtime,
        plan: &RunPlan,
        store: &Store,
        cancel: watch::Receiver<Option<JobCause>>,
    ) -> Result<Measures, String> {
        let holder = Holder {
            job: crt.job.to_string(),
            run: None,
        };
        let slot = crt
            .slots
            .acquire(&holder)
            .await
            .map_err(|e| e.to_string())?;
        let ctl = RunControl {
            cancel,
            meter: Arc::new(JobMeter::new()),
        };
        let done = exec_run(crt, plan, &slot, &ctl)
            .await
            .map_err(|e| e.to_string())?;
        let path = store.layout().run_dir(&plan.key).join("manifest.json");
        let bytes = std::fs::read(&path).map_err(|e| format!("the run's manifest: {e}"))?;
        let manifest: Value =
            serde_json::from_slice(&bytes).map_err(|e| format!("the run's manifest: {e}"))?;
        Ok(Measures::from_manifest(&manifest, done.invalid.is_none()))
    }
}

/// The words of a docker command line.
fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect() // BORROW: owned argv
}

/// The fake model's certificate authority, when it has made one.
async fn authority(
    docker: &dyn DockerDriver, // TRAIT_OBJECT: docker, or a fake in tests
    fake: &str,
) -> Option<Vec<u8>> {
    let path = format!("{FAKE_MOUNT}/ca.pem");
    let found = docker
        .capture(argv(&["exec", fake, "cat", &path]), CALL)
        .await
        .ok()?;
    (!found.is_empty()).then_some(found)
}

/// The arguments of the `docker run` of the fake model: a control container that removes itself
/// when it ends and ends by itself after [`FAKE_LIFETIME_S`] seconds. Its working directory is a tmpfs.
fn fake_args(image: &str, token: &str) -> Vec<String> {
    let mut args: Vec<String> = [
        "run",
        "-d",
        "--rm",
        "--init",
        "--read-only",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "--pids-limit",
        "64",
        "--memory",
        "256m",
        "--label",
        "candler.role=control",
        "--entrypoint",
        "timeout",
    ]
    .iter()
    .map(|word| (*word).to_owned()) // BORROW: owned argv
    .collect();
    args.extend([
        "-e".to_owned(), // BORROW: owned argv
        format!("FAKE_TOKEN={token}"),
        "-e".to_owned(), // BORROW: owned argv
        format!("FAKE_HOST={FAKE_HOST}"),
        "-e".to_owned(), // BORROW: owned argv
        format!("FAKE_PORT={FAKE_PORT}"),
        "-e".to_owned(), // BORROW: owned argv
        format!("FAKE_DIR={FAKE_MOUNT}"),
        "--tmpfs".to_owned(), // BORROW: owned argv
        format!("{FAKE_MOUNT}:rw,mode=1777,size=1m,noexec,nosuid"),
        image.to_owned(),               // BORROW: owned argv
        FAKE_LIFETIME_S.to_owned(),     // BORROW: owned argv
        "python3".to_owned(),           // BORROW: owned argv
        "-u".to_owned(),                // BORROW: owned argv
        "/opt/fakemodel.py".to_owned(), // BORROW: owned argv
    ]);
    args
}

/// The cell of a control and its plan: a session over the base image with the fake model's
/// script `control` as the model, the control's own `idle_s` and the harness `harness`.
fn control_cell(
    identity: &Identity,
    harness: &Value,
    control: Control,
) -> Result<(CellRun, RunPlan), String> {
    let values: Vec<(&str, Value)> = vec![
        ("budget.per_run.wall_s", json!(WALL_S)),
        ("isolation.image", json!(identity.base_id)),
        ("model.id", json!(control.name())),
        ("permission_mode", json!("bypassPermissions")),
        ("prompt.template", json!("go")),
        ("runner.idle_s", json!(control.idle_s())),
        ("runner.mode", json!("session")),
    ];
    let entries: Vec<Value> = values
        .iter()
        .map(|(path, value)| json!({"path": path, "value": value}))
        .collect();
    let comparable: Comparable = serde_json::from_value(json!({
        "values": entries,
        "harness": harness,
        "treatment": {"name": "control"},
        "snapshots": [],
        "inputs": [],
        "environments": [],
    }))
    .map_err(|e| format!("the control's cell: {e}"))?;
    let mut paths: Vec<&str> = values.iter().map(|(path, _)| *path).collect();
    paths.extend(["harness", "treatment"]);
    paths.sort_unstable();
    let sources: Vec<SourceEntry> = serde_json::from_value(Value::Array(
        paths
            .iter()
            .map(|path| json!({"path": path, "source": {"kind": "experiment_file"}}))
            .collect(),
    ))
    .map_err(|e| format!("the control's sources: {e}"))?;
    let cell = comparable.digest();
    let experiment = identity.control_digest.clone(); // BORROW: the cell owns its digest
    let run = RunKey::new(cell.clone(), 1, 1) // BORROW: the key owns its digest
        .map_err(|e| format!("the control's run key: {e}"))?;
    let secrets: Vec<SecretPresence> = Vec::new();
    let run_cell = CellRun::new(
        comparable,
        experiment,
        Vec::new(),
        sources,
        secrets,
        None,
        Inputs::default(),
    );
    let plan = RunPlan {
        key: run,
        cell,
        stored: None,
        attempt: 1,
        deviations: Vec::new(),
    };
    Ok((run_cell, plan))
}
