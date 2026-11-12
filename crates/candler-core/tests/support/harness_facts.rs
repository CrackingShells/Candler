//! What the core produces for a Claude Code run, written as JSON values so a guard can set them
//! beside the values recorded from cf3de30: the `docker run` argv of a reference cell, the census
//! of every recorded stream, the control container that reads the harness version, and the
//! `comparable` of the repo's experiment files.
//!
//! Each producer goes through the one call the core offers for it. A change of that call changes
//! this file's adapter section and nothing else.
#![allow(dead_code, reason = "each guard binary uses a subset of the helpers")]

use std::fs::File;
use std::io::{BufRead, BufReader, Read as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::Arc;
use std::time::Duration;

use candler_core::isolation::IsolationError;
use candler_core::isolation::container::mounts::{MountMode, MountSpec};
use candler_core::isolation::container::run::{RunArgvError, agent_argv as agent_argv_of};
use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::image::{
    BuildSetting, DockerImageService, ImageAssets, ImageError, ImageService, LockDigest,
    LockRequest,
};
use candler_core::isolation::model::{
    ContainerPlan, ImageId, ImageRef, Labels, Limits, NetworkName, UserSpec,
};
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, ResolveInput};
use candler_core::resolve::preflight::{
    PreflightError, PreflightStep, ResolveContext, Resolver, StepReport,
};
use candler_core::resolve::steps::identity::HarnessIdentityStep;
use candler_core::resolve::steps::image::ImageIdStep;
use candler_core::runner::evidence::census::census;
use candler_core::runner::evidence::run::{RunFacts, run_values};
use candler_core::runner::exec::apply_harness_env;
use candler_core::runner::plan::{Force, PlanMode, plan_runs};
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::driver::HarnessDriver;
use candler_harness::reader::{FrameIn, TraceReader};
use candler_harness::spec::{Cell, CommandSpec, Workspace};
use candler_harness_claude_code::ClaudeCodeReader;
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::{EventPayload, ExperimentFile};
use candler_schemas::primitives::{Digest, FieldPath, JobId, RunKey};
use candler_schemas::raw::{FrameData, RawFrame, RawRef};
use candler_schemas::trace::EndedBy;
use candler_schemas::values::{JsonValue, ResolvedValue};
use flate2::read::MultiGzDecoder;
use futures::future::BoxFuture;
use serde_json::{Value, json};
use url::Url;

use crate::support;

/// The text the token variable carries in the reference argv.
pub const DUMMY_TOKEN: &str = "dummy-token-0001";

/// The base URL of the reference run.
pub const BASE_URL: &str = "http://gateway:8080/run/ref";

/// The image id the fake docker prints and answers for.
pub const FAKE_ID: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// What the fake docker answers to `docker image inspect`, with the label of the base image.
const INSPECT_ANSWER: &str = concat!(
    r#"{"Id":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","#,
    r#""RootFS":{"Layers":["sha256:aa","sha256:bb"]},"#,
    r#""Config":{"Labels":{"candler.cc_version":"2.1.311"}}}"#
);

/// The lines of the recorded streams the census guards read: every stream the Claude Code crate
/// keeps as a gzip file, and the two recorded session stdouts.
pub fn stream_paths() -> Vec<(String, PathBuf)> {
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../candler-harness-claude-code/tests/fixtures");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(fixtures.join("streams")).expect("the streams directory") {
        let dir = entry.expect("an entry").path();
        let file = dir.join("agent_stream.jsonl.gz");
        if file.is_file() {
            let name = dir
                .file_name()
                .expect("a name")
                .to_string_lossy()
                .into_owned();
            found.push((format!("streams/{name}"), file));
        }
    }
    for name in ["c1", "c2"] {
        found.push((
            format!("sessions/{name}"),
            fixtures.join("sessions").join(name).join("stdout.jsonl"),
        ));
    }
    found.sort();
    found
}

/// The lines of a recorded stream, gunzipped when it ends in `.gz`.
pub fn lines_of(path: &Path) -> Vec<String> {
    let file = File::open(path).expect("a fixture stream");
    let reader: Box<dyn std::io::Read> = if path.extension().is_some_and(|ext| ext == "gz") {
        Box::new(MultiGzDecoder::new(file))
    } else {
        Box::new(file)
    };
    BufReader::new(reader)
        .lines()
        .map(|line| line.expect("a line"))
        .collect()
}

fn frame_of(line: &str) -> RawFrame {
    RawFrame::new(None, FrameData::from_bytes(line.as_bytes()))
}

fn raw_ref(index: usize) -> RawRef {
    let at = u64::try_from(index).expect("a frame index");
    RawRef::new("agent_stream.jsonl".parse().expect("a stream name"), at)
}

// ---------------------------------------------------------------------------------------------
// The census.
// ---------------------------------------------------------------------------------------------

/// The three census values of the stream `lines`, by path, or `null` when the core records none.
pub fn census_value(lines: &[String]) -> Value {
    let mut reader = ClaudeCodeReader::new().expect("the reader");
    for (index, line) in lines.iter().enumerate() {
        if reader
            .feed(FrameIn::new(raw_ref(index), None, &frame_of(line)))
            .is_err()
        {
            return Value::Null;
        }
    }
    // ----- adapter: the call the core offers for the census -----
    let observed = reader.observed();
    let Ok(sessions) = Box::new(reader).finish() else {
        return Value::Null;
    };
    if sessions.is_empty() {
        return Value::Null;
    }
    let Ok(values) = census(observed.init.as_ref()) else {
        return Value::Null;
    };
    let mut map = serde_json::Map::new();
    for value in values {
        map.insert(
            value.path().to_string(),
            value.value().as_value().clone(), // BORROW: the map owns its values
        );
    }
    Value::Object(map)
}

/// The `run.usage`, `run.cost_usd` and `run.num_turns` values of the stream `lines`, by path, or
/// `null` when the core records no run. A value the stream's closing report lacks is absent.
pub fn run_totals_value(lines: &[String]) -> Value {
    let mut reader = ClaudeCodeReader::new().expect("the reader");
    for (index, line) in lines.iter().enumerate() {
        if reader
            .feed(FrameIn::new(raw_ref(index), None, &frame_of(line)))
            .is_err()
        {
            return Value::Null;
        }
    }
    // ----- adapter: the call the core offers for the run values -----
    let observed = reader.observed();
    let Ok(sessions) = Box::new(reader).finish() else {
        return Value::Null;
    };
    if sessions.is_empty() {
        return Value::Null;
    }
    let values = run_values(&RunFacts::new(
        &EndedBy::Completed,
        Some(0),
        0,
        &[],
        &observed.totals,
    ));
    let mut map = serde_json::Map::new();
    for value in values {
        let path = value.path().to_string();
        if ["run.usage", "run.cost_usd", "run.num_turns"].contains(&path.as_str()) {
            map.insert(path, value.value().as_value().clone()); // BORROW: the map owns its values
        }
    }
    Value::Object(map)
}

// ---------------------------------------------------------------------------------------------
// The argv of the agent container.
// ---------------------------------------------------------------------------------------------

/// A directory tree and a plan over it.
pub struct Scene {
    /// The canonical root every mount source and every root of the plan sits under.
    pub root: PathBuf,
    /// The host directory mounted as the agent's home.
    pub home: PathBuf,
    /// The plan, as the isolation makes it for a reference cell.
    pub plan: ContainerPlan,
    /// The run's key.
    pub key: RunKey,
}

/// The digest of `c` repeated.
pub fn digest_of(c: char) -> Digest {
    format!("sha256:{}", String::from(c).repeat(64))
        .parse()
        .expect("a digest")
}

/// A plan of the shape `DockerIsolation::plan` makes, over directories made under `dir`.
pub fn scene(dir: &Path) -> Scene {
    let root = std::fs::canonicalize(dir).expect("a canonical directory");
    let workspace = root.join("workspace");
    let home = root.join("home");
    for sub in [&workspace, &home] {
        std::fs::create_dir_all(sub).expect("a directory");
    }
    let input = workspace.join("in.txt");
    std::fs::write(&input, "x").expect("a file");
    let job = JobId::from_str("01900000-0000-7000-8000-000000000000").expect("a job id");
    let plan = ContainerPlan {
        image: ImageRef {
            id: ImageId(String::from("sha256:abc")),
            cc_version: None,
        },
        labels: Labels::new()
            .with("candler.job", "01900000-0000-7000-8000-000000000000")
            .with("candler.role", "agent"),
        mounts: vec![
            MountSpec {
                source: workspace,
                target: String::from("/work"),
                mode: MountMode::ReadWrite,
            },
            MountSpec {
                source: home.clone(), // BORROW: the scene keeps the home for the state paths
                target: String::from("/home/agent"),
                mode: MountMode::ReadWrite,
            },
            MountSpec {
                source: input,
                target: String::from("/work/data/in.txt"),
                mode: MountMode::ReadOnly,
            },
        ],
        env: vec![
            (String::from("PATH"), String::from("/opt/env/bin:/usr/bin")),
            (String::from("HOME"), String::from("/home/agent")),
            (String::from("VIRTUAL_ENV"), String::from("/opt/env")),
            (String::from("LANG"), String::from("C.UTF-8")),
        ],
        gateway: None,
        state: Vec::new(),
        network: NetworkName::for_job(&job),
        egress: None,
        limits: Limits {
            pids_limit: 512,
            memory: String::from("4g"),
            cpus: 2.0,
        },
        cidfile: root.join("agent.cid"),
        user: UserSpec::Host {
            uid: 1000,
            gid: 1000,
        },
        workdir: String::from("/work"),
        home: String::from("/home/agent"),
        expiry: Duration::from_secs(3600),
        project_root: root.clone(), // BORROW: the scene keeps the root
        store_root: root.clone(),   // BORROW: the scene keeps the root
    };
    Scene {
        root,
        home,
        plan,
        key: RunKey::new(digest_of('c'), 1, 1).expect("a run key"),
    }
}

fn resolved(path: &str, value: Value) -> ResolvedValue {
    ResolvedValue::recorded(
        FieldPath::from_str(path).expect("a field path"),
        JsonValue::new(value).expect("a JSON value"),
    )
}

/// The values of the reference cell: every flag of the headless command line is set.
pub fn reference_values() -> Vec<ResolvedValue> {
    vec![
        resolved("model.id", json!("claude-haiku-4-5")),
        resolved("model.effort", json!("medium")),
        resolved("permission_mode", json!("bypassPermissions")),
        resolved("prompt.template", json!("Build {thing} now")),
        resolved("prompt.values", json!({"thing": "a domain"})),
        resolved("budget.per_run.turns", json!(30)),
        resolved("budget.per_run.max_cost_usd", json!(1.0)),
    ]
}

/// The headless command the Claude Code driver gives the reference cell.
pub fn reference_command() -> CommandSpec {
    let values = reference_values();
    ClaudeCodeDriver::new()
        .expect("the driver")
        .command(&Cell::new(&values), &Workspace::new(Path::new("/work/run")))
        .expect("a command")
}

/// `argv` with the scene's root written as `<root>` and the client binary as `docker`.
pub fn normalised(scene: &Scene, mut argv: Vec<String>) -> Vec<String> {
    if let Some(first) = argv.first_mut() {
        first.clear();
        first.push_str("docker");
    }
    let root = scene.root.to_string_lossy().into_owned();
    argv.into_iter()
        .map(|word| word.replace(root.as_str(), "<root>"))
        .collect()
}

/// What refused the argv of an agent container.
#[derive(Debug)]
pub enum ArgvRefusal {
    /// The core refused the driver's gateway or state variables.
    Env(IsolationError),
    /// The `docker run` argv refused the plan, the overlay or the gateway variables.
    Run(RunArgvError),
}

/// The argv of the agent container that runs `command` in `plan`, which already holds the
/// driver's gateway and state variables, as the core builds it.
pub fn plan_argv(
    scene: &Scene,
    plan: &ContainerPlan,
    command: &CommandSpec,
) -> Result<Vec<String>, ArgvRefusal> {
    // ----- adapter: the call the core offers for the argv -----
    let base_url = Url::parse(BASE_URL).expect("a base url");
    let argv = agent_argv_of(plan, command, &scene.key, &base_url, DUMMY_TOKEN)
        .map_err(ArgvRefusal::Run)?;
    Ok(normalised(scene, argv))
}

/// The argv of the agent container that runs `command` under `driver` in `scene`, as the core
/// builds it: the driver's gateway and state variables, the base URL and the token.
pub fn agent_argv(
    scene: &Scene,
    driver: &dyn HarnessDriver,
    command: &CommandSpec,
) -> Result<Vec<String>, ArgvRefusal> {
    plan_argv_over(scene, scene.plan.clone(), driver, command) // BORROW: the scene keeps its plan
}

/// The argv of the agent container over `plan`, to which the core adds the variables of `driver`.
pub fn plan_argv_over(
    scene: &Scene,
    mut plan: ContainerPlan,
    driver: &dyn HarnessDriver,
    command: &CommandSpec,
) -> Result<Vec<String>, ArgvRefusal> {
    // ----- adapter: the call the core offers for the driver's variables -----
    apply_harness_env(&mut plan, driver, &scene.home).map_err(ArgvRefusal::Env)?;
    plan_argv(scene, &plan, command)
}

// ---------------------------------------------------------------------------------------------
// The control container that reads the harness version.
// ---------------------------------------------------------------------------------------------

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

fn write_script(path: &Path, text: &str) {
    std::fs::write(path, text).expect("a script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("a mode");
}

/// An image service over the real docker client and the PATH-fake docker, and the fake's log.
pub struct VersionRig {
    service: DockerImageService,
    log: PathBuf,
    _dir: tempfile::TempDir,
}

impl VersionRig {
    /// A rig whose control container prints `answer`, with `driver` as the service's harness.
    pub fn new(driver: Arc<dyn HarnessDriver>, answer: &str) -> Self {
        let dir = tempfile::tempdir().expect("a directory");
        let canned = dir.path().join("canned");
        let bin = dir.path().join("bin");
        let records = dir.path().join("records");
        for sub in [&canned, &bin, &records] {
            std::fs::create_dir_all(sub).expect("a directory");
        }
        let log = dir.path().join("docker.log");
        std::fs::write(canned.join("build.out"), format!("{FAKE_ID}\n")).expect("an answer");
        std::fs::write(canned.join("image-inspect.out"), INSPECT_ANSWER).expect("an answer");
        std::fs::write(canned.join("run.out"), answer).expect("an answer");
        let wrapper = bin.join("docker");
        write_script(
            &wrapper,
            &format!(
                "#!/bin/sh\nexport FAKE_DOCKER_LOG='{}' FAKE_DOCKER_DIR='{}'\nexec '{}' \"$@\"\n",
                log.display(),
                canned.display(),
                support::fake_docker().display()
            ),
        );
        // TRAIT_OBJECT: the supervisor takes its sink as a trait object
        let sink: Arc<dyn EventSink> = Arc::new(Silent);
        let supervisor = Arc::new(Supervisor::new(sink));
        let env = ClientEnv::from_process();
        let cli = DockerCli::with_binary(
            Arc::clone(&supervisor),
            wrapper,
            env.clone(), // BORROW: the service owns its environment
            records,
        );
        let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/docker");
        let assets = ImageAssets {
            base_context: sources.join("base"),
            env_context: sources.join("env"),
            store_root: dir.path().join("store"),
        };
        // ----- adapter: the service's constructor -----
        let service = DockerImageService::new(supervisor, cli, env, assets, driver);
        Self {
            service,
            log,
            _dir: dir,
        }
    }

    /// The version the service reads from the image.
    pub async fn version(&self) -> Result<semver::Version, ImageError> {
        let image = ImageRef {
            id: ImageId(String::from(FAKE_ID)),
            cc_version: None,
        };
        // ----- adapter: the call that reads the harness version -----
        self.service.harness_version(&image).await
    }

    /// The argv of every `docker run` the fake saw, as JSON arrays of words.
    pub fn runs(&self) -> Vec<Vec<String>> {
        // The fake writes its log on its first call: no log is no call.
        let mut text = String::new();
        if let Ok(mut log) = File::open(&self.log) {
            log.read_to_string(&mut text).expect("text");
        }
        text.lines()
            .map(|line| serde_json::from_str::<Vec<String>>(line).expect("a JSON array"))
            .filter(|argv| argv.first().is_some_and(|verb| verb == "run"))
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// The comparable of an experiment file.
// ---------------------------------------------------------------------------------------------

/// An image service that answers with one image and one harness version, `2.1.311`.
struct Fixed;

impl ImageService for Fixed {
    fn lock<'a>(&'a self, _request: &'a LockRequest) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        Box::pin(async { Ok(b"lock".to_vec()) })
    }

    fn ensure<'a>(
        &'a self,
        _lock: &'a LockDigest,
        _setting: &'a BuildSetting,
    ) -> BoxFuture<'a, Result<ImageRef, ImageError>> {
        Box::pin(async {
            Ok(ImageRef {
                id: ImageId(format!("sha256:{}", "7".repeat(64))),
                cc_version: None,
            })
        })
    }

    // ----- adapter: the method that reads the harness version -----
    fn harness_version<'a>(
        &'a self,
        _image: &'a ImageRef,
    ) -> BoxFuture<'a, Result<semver::Version, ImageError>> {
        Box::pin(async { Ok(semver::Version::new(2, 1, 311)) })
    }
}

/// Derives the typed roots the image and harness steps do not.
struct OtherRoots;

impl PreflightStep for OtherRoots {
    fn name<'a>(&'a self) -> &'a candler_schemas::primitives::NameToken {
        static NAME: std::sync::OnceLock<candler_schemas::primitives::NameToken> =
            std::sync::OnceLock::new();
        NAME.get_or_init(|| "roots@1".parse().expect("a name@token"))
    }

    fn run(
        &self,
        _ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        let digest = format!("sha256:{}", "a".repeat(64));
        let fp = |text: &str| FieldPath::from_str(text).expect("a field path");
        let js = |value: Value| JsonValue::new(value).expect("a JSON value");
        let report = StepReport::new("roots@1".parse().expect("a name@token"))
            .derive(
                fp("treatment"),
                js(json!({"name": "htn", "commit": "1".repeat(40)})),
            )
            .derive(
                fp("snapshots"),
                js(json!([{"kind": "catalog", "digest": digest}])),
            )
            .derive(
                fp("environments"),
                js(json!([{"plugin": "p@1.0.0", "lock_digest": digest}])),
            );
        Box::pin(async move { Ok(report) })
    }
}

/// The cells of `file`, as `candler run` resolves and plans them: the image step, the real
/// harness step over a service that reports `2.1.311`, and the roots a fake derives. Each cell is
/// the JSON of its `comparable`, in the order of the cell digests.
pub async fn comparables(file: &ExperimentFile, dir: &Path) -> Vec<Value> {
    let config = Arc::new(Config::new(
        dir.to_owned(),
        None,
        UserConfig::default(),
        Some(dir.join("state")),
    ));
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let supervisor = Arc::new(Supervisor::new(sink));
    // TRAIT_OBJECT: the image service is a fixed fake
    let images: Arc<dyn ImageService> = Arc::new(Fixed);
    let image = ImageIdStep::new(Arc::clone(&images), supervisor).expect("a step");
    let supported = ClaudeCodeDriver::new().expect("the driver").supported();
    let harness = HarnessIdentityStep::new(images, supported).expect("a step");
    let mut values: Vec<(FieldPath, JsonValue)> = file
        .fixed()
        .iter()
        .map(|(path, value)| (path.clone(), value.clone())) // BORROW: the layer owns its paths and values
        .collect();
    for (path, value) in [
        ("request.harness", serde_json::to_value(file.harness())),
        ("request.treatment", serde_json::to_value(file.treatment())),
    ] {
        values.push((
            FieldPath::from_str(path).expect("a field path"),
            JsonValue::new(value.expect("JSON")).expect("a JSON value"),
        ));
    }
    let layer = FileValues::new(values)
        .relative_to(dir)
        .expect("absolute paths");
    let resolution = Resolver::new(config)
        .register(Box::new(OtherRoots))
        .expect("registers")
        .register(Box::new(image))
        .expect("registers")
        .register(Box::new(harness))
        .expect("registers")
        .resolve(ResolveInput::default().file(layer))
        .await
        .expect("resolves");
    let (_keep, store, lock) = support::run_plan::fresh_store();
    let mut facts = support::run_plan::Facts::base();
    facts.secrets = vec![String::from("RIVAULT_LLM_INFERENCE")];
    let ctx = support::run_plan::plan_ctx(&store, &lock, &facts);
    let outcome =
        plan_runs(&ctx, file, &resolution, Force::No, PlanMode::Check).expect("the file plans");
    outcome
        .fresh
        .into_values()
        .map(|cell| serde_json::to_value(&cell).expect("JSON"))
        .collect()
}

/// The experiment files the resolution guards read: a name and the file's text, with the files
/// a setting names written beside it in `dir`.
pub fn experiments(dir: &Path) -> Vec<(String, ExperimentFile)> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |relative: &str| -> ExperimentFile {
        let text = std::fs::read_to_string(repo.join(relative)).expect("an experiment file");
        ExperimentFile::from_toml_str(text.as_str()).expect("an experiment file")
    };
    std::fs::write(dir.join("requirements.in"), "gtpyhop\n").expect("a requirements file");
    let settings = "schema = \"experiment/1\"\nreplicates = 1\n\n[fixed]\n\"prompt.template\" = \"say hello\"\n\
         \"isolation.build.prune\" = [\"a.py\", \"b.py\"]\n\"isolation.record_packages\" = [\"numpy\"]\n\
         \"permission_mode\" = \"acceptEdits\"\n\n\
         [[factors]]\npath = \"model.id\"\nlevels = [\"m1\", \"m2\"]\n\n\
         [budget.per_run]\nwall_s = 5\nturns = 4\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"htn\"\n";
    vec![
        (
            String::from("cont_train"),
            read("crates/candler-schemas/tests/vectors/experiment/cont_train.toml"),
        ),
        (
            String::from("smoke_v2"),
            read("__reports__/m2b-execution-parity/smoke_v2/experiment.toml"),
        ),
        (
            String::from("run_plan_experiment"),
            support::run_plan::experiment(&["m1", "m2"], 2),
        ),
        (
            String::from("settings_experiment"),
            ExperimentFile::from_toml_str(settings).expect("an experiment file"),
        ),
    ]
}
