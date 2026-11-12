#![cfg(test)]
//! Guards for the inputs an experiment declares: what is refused, how the build inputs and the
//! declared ones merge into the `inputs` root, and how the inputs reach the container and the
//! manifest.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use candler_core::experiment::inputs::resolve_inputs;
use candler_core::isolation::image::{
    BuildSetting, ImageError, ImageService, LockDigest, LockRequest,
};
use candler_core::isolation::model::{ImageId, ImageRef};
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, ResolveInput};
use candler_core::resolve::preflight::Resolver;
use candler_core::resolve::steps::image::ImageIdStep;
use candler_core::store::Store;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::{EventPayload, ExperimentInput, InputAudience};
use candler_schemas::primitives::{FieldPath, Name, RunKey};
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;
use serde_json::json;

fn name(text: &str) -> Name {
    Name::from_str(text).expect("a name")
}

fn input(kind: &str, path: &str, at: Option<&str>, audience: InputAudience) -> ExperimentInput {
    ExperimentInput::new(name(kind), path.to_owned(), at.map(str::to_owned), audience)
        .expect("a declaration the wire accepts")
}

/// A store in a temporary directory.
fn store(root: &Path) -> Store {
    Store::init(&root.join("store")).expect("a store")
}

/// An experiment directory holding `a.txt`, a directory `tree` with two files, and an empty
/// directory `empty`.
fn experiment_dir(root: &Path) -> std::path::PathBuf {
    let dir = root.join("exp");
    fs::create_dir_all(dir.join("tree/sub")).expect("directories");
    fs::create_dir_all(dir.join("empty")).expect("an empty directory");
    fs::write(dir.join("a.txt"), "alpha").expect("a file");
    fs::write(dir.join("tree/one.txt"), "one").expect("a file");
    fs::write(dir.join("tree/sub/two.txt"), "two").expect("a file");
    dir
}

/// Guards: I14 — an input the experiment declares wrongly is refused before any run, and the
/// refusal names the input's kind and the path at fault (authors).
/// Oracle: hand-written; each case is one way to declare an input that cannot be mounted or
/// hashed, with the words the message must hold.
#[test]
fn declared_input_refused_names_kind_and_path() {
    let root = tempfile::tempdir().expect("a directory");
    let dir = experiment_dir(root.path());
    symlink(dir.join("a.txt"), dir.join("tree/link.txt")).expect("a symlink");
    symlink(dir.join("a.txt"), dir.join("direct.txt")).expect("a symlink");
    fs::create_dir_all(dir.join("only_dirs/inner")).expect("nested empty directories");
    fs::create_dir_all(dir.join("dirs/d,ir")).expect("a directory");
    fs::write(dir.join("dirs/d,ir/inner.txt"), "x").expect("a file");
    fs::create_dir_all(dir.join("commas")).expect("a directory");
    fs::write(dir.join("commas/a,b.txt"), "x").expect("a file name with a comma");
    fs::create_dir_all(dir.join("lines")).expect("a directory");
    fs::write(dir.join("lines/nl\nfile.txt"), "x").expect("a file name with a newline");
    fs::write(dir.join("d,ir.txt"), "x").expect("a file input named with a comma");
    let store = store(root.path());
    let agent = InputAudience::Agent;
    let cases: Vec<(&str, Vec<ExperimentInput>, &str)> = vec![
        (
            "duplicate kind",
            vec![
                input("servers", "a.txt", None, agent),
                input("servers", "tree", None, agent),
            ],
            "servers",
        ),
        (
            "missing path",
            vec![input("gone", "nowhere.txt", None, agent)],
            "nowhere.txt",
        ),
        ("empty path", vec![input("blank", "", None, agent)], "blank"),
        (
            "absolute path",
            vec![input("abs", "/etc/hosts", None, agent)],
            "/etc/hosts",
        ),
        (
            "parent segment",
            vec![input("up", "../exp/a.txt", None, agent)],
            "../exp/a.txt",
        ),
        (
            "parent segment inside",
            vec![input(
                "up",
                "tree/../a.txt",
                None,
                InputAudience::Evaluators,
            )],
            "tree/../a.txt",
        ),
        (
            "symlink in a directory",
            vec![input("tree", "tree", None, agent)],
            "link.txt",
        ),
        (
            "symlink as the input",
            vec![input("direct", "direct.txt", None, agent)],
            "direct.txt",
        ),
        (
            "empty directory",
            vec![input("hollow", "empty", None, agent)],
            "empty",
        ),
        (
            "directory of empty directories",
            vec![input(
                "hollow",
                "only_dirs",
                None,
                InputAudience::Evaluators,
            )],
            "only_dirs",
        ),
        (
            "two files at one workspace path",
            vec![
                input("first", "a.txt", Some("data"), agent),
                input("second", "a.txt", Some("data"), agent),
            ],
            "data/a.txt",
        ),
        (
            "comma in a file name of a directory",
            vec![input("commas", "commas", None, agent)],
            "a,b.txt",
        ),
        (
            "comma in a directory name",
            vec![input("dirs", "dirs", None, agent)],
            "d,ir",
        ),
        (
            "newline in a file name",
            vec![input("lines", "lines", None, agent)],
            "nl",
        ),
        (
            "comma in the file name of a file input",
            vec![input("comma_file", "d,ir.txt", None, agent)],
            "d,ir.txt",
        ),
        (
            "comma in at",
            vec![input("where", "a.txt", Some("x,y"), agent)],
            "x,y",
        ),
        (
            "newline in at",
            vec![input(
                "where",
                "a.txt",
                Some("x\ny"),
                InputAudience::Evaluators,
            )],
            "x",
        ),
        (
            "a file inside another file's path",
            vec![
                input("file", "a.txt", Some("x"), agent),
                input("under", "tree/sub", Some("x/a.txt"), agent),
            ],
            "x/a.txt",
        ),
    ];
    for (label, declared, expect) in cases {
        let error = resolve_inputs(&store, &declared, &dir)
            .expect_err(label)
            .to_string();
        assert!(
            error.contains(expect),
            "{label}: `{error}` lacks `{expect}`"
        );
        let kind = declared.last().expect("an input").kind().to_string();
        assert!(
            error.contains(&kind),
            "{label}: `{error}` lacks the kind `{kind}`"
        );
    }
}

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// An image service that answers with one image and never builds.
struct OneImage;

impl ImageService for OneImage {
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

    fn harness_version<'a>(
        &'a self,
        _image: &'a ImageRef,
    ) -> BoxFuture<'a, Result<semver::Version, ImageError>> {
        Box::pin(async { Ok(semver::Version::new(2, 1, 311)) })
    }
}

fn fp(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

fn js(value: serde_json::Value) -> JsonValue {
    JsonValue::new(value).expect("a JSON value")
}

/// Resolves with the image step alone and returns the value of the `inputs` root.
async fn resolved_inputs(
    cwd: &Path,
    settings: Vec<(&str, serde_json::Value)>,
) -> serde_json::Value {
    let config = Arc::new(Config::new(
        cwd.to_owned(),
        None,
        UserConfig::default(),
        Some(cwd.join("state")),
    ));
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let step =
        ImageIdStep::new(Arc::new(OneImage), Arc::new(Supervisor::new(sink))).expect("a step");
    let values = settings
        .into_iter()
        .map(|(path, value)| (fp(path), js(value)))
        .collect();
    let resolution = Resolver::new(config)
        .register(Box::new(step))
        .expect("registers")
        .resolve(ResolveInput::default().file(FileValues::new(values)))
        .await
        .expect("resolves with no step deriving a path twice");
    resolution
        .values
        .iter()
        .find(|value| value.path().as_str() == "inputs")
        .and_then(|value| value.value())
        .expect("an inputs root")
        .as_value()
        .clone()
}

/// Guards: I14 — an experiment with a build extension and declared agent inputs resolves to one
/// `inputs` root that holds both, sorted by kind, and the one step that derives it derives it
/// once (the image step; agent environment; comparability).
/// Oracle: hand-written. The agent entries are given digests by hand; `build_inputs` is the
/// digest of the Dockerfile and context the step reads, so the test only checks its kind and
/// position, and that it is a `sha256:` digest.
#[tokio::test]
async fn declared_input_with_build_inputs_merge_into_one_sorted_root() {
    let root = tempfile::tempdir().expect("a directory");
    let cwd = root.path();
    fs::write(cwd.join("Dockerfile"), "FROM scratch\n").expect("a Dockerfile");
    fs::create_dir_all(cwd.join("ctx")).expect("a context");
    fs::write(cwd.join("ctx/f"), "x").expect("a file");
    fs::write(cwd.join("lock.txt"), "lock").expect("a lock");
    let agent = json!([
        {"kind": "zeta", "digest": format!("sha256:{}", "b".repeat(64))},
        {"kind": "alpha", "digest": format!("sha256:{}", "a".repeat(64))},
    ]);
    let settings = vec![
        ("isolation.build.lock", json!("lock.txt")),
        ("isolation.build.dockerfile", json!("Dockerfile")),
        ("isolation.build.context", json!("ctx")),
        ("request.inputs", agent.clone()),
    ];
    let merged = resolved_inputs(cwd, settings).await;
    let entries = merged.as_array().expect("a list");
    let kinds: Vec<&str> = entries
        .iter()
        .map(|entry| entry["kind"].as_str().expect("a kind"))
        .collect();
    assert_eq!(
        kinds,
        ["alpha", "build_inputs", "zeta"],
        "one sorted root: {merged}"
    );
    assert_eq!(
        entries[0], agent[1],
        "the declared digests pass through unchanged"
    );
    assert_eq!(
        entries[2], agent[0],
        "the declared digests pass through unchanged"
    );
    assert!(
        entries[1]["digest"]
            .as_str()
            .is_some_and(|d| d.starts_with("sha256:"))
    );
}

/// Guards: I14 — declared agent inputs reach the `inputs` root with no build extension too, and
/// an experiment that declares none has no `inputs` root.
/// Oracle: hand-written.
#[tokio::test]
async fn declared_input_without_build_inputs_stands_alone() {
    let root = tempfile::tempdir().expect("a directory");
    let cwd = root.path();
    fs::write(cwd.join("lock.txt"), "lock").expect("a lock");
    let agent = json!([{"kind": "alpha", "digest": format!("sha256:{}", "a".repeat(64))}]);
    let with = resolved_inputs(
        cwd,
        vec![
            ("isolation.build.lock", json!("lock.txt")),
            ("request.inputs", agent.clone()),
        ],
    )
    .await;
    assert_eq!(with, agent);
}

mod support;

use std::path::PathBuf;
use std::sync::Mutex;

use candler_core::isolation::container::DockerIsolation;
use candler_core::isolation::container::run::run_argv;
use candler_core::isolation::docker::{DockerCli, DockerError};
use candler_core::isolation::gateway::Gateway;
use candler_core::isolation::model::{
    ContainerOutcome, ContainerPlan, Inspect, InspectState, Labels,
};
use candler_core::isolation::{IsolatedRequest, Isolation, IsolationError, RunOutputs};
use candler_core::jobs::model::{JobCause, JobControl, JobOutcome, JobSpec};
use candler_core::jobs::orchestrate::orchestrate_with;
use candler_core::jobs::orchestrate::schedule::Clock;
use candler_core::jobs::orchestrate::session::{JobHost, OrchestrateError};
use candler_core::runner::Runtime;
use candler_core::supervise::stop::Exit;
use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::TraceReader;
use candler_harness::spec::{
    Capabilities, Cell, CollectSpec, CommandSpec, HarnessId, Observed, Prepared, ProcessExit,
    StopSpec, Workspace,
};
use candler_schemas::primitives::{ContainerId, Marker, Timestamp};
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};
use sha2::{Digest as _, Sha256};
use support::job_orchestrate::{FakeHost, Rig, spec};
use support::run_plan::Facts;
use tokio::sync::watch;

/// What a run's container was given, as the host saw it.
#[derive(Debug, Default, Clone)]
struct Seen {
    /// The full `docker run` argv of each run.
    argvs: Vec<Vec<String>>,
    /// The files of the workspace's host directory when the run began, by relative path.
    workspace: Vec<String>,
}

/// An isolation that records the argv it would run and the workspace it was given, and, when
/// `leak` names a file, copies it into the workspace as the run's output.
struct Recording {
    inner: Arc<dyn Isolation>,
    seen: Arc<Mutex<Seen>>,
    leak: Option<PathBuf>,
}

impl Isolation for Recording {
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
        command: CommandSpec,
        marker: Marker,
        key: RunKey,
        outputs: RunOutputs,
    ) -> Result<IsolatedRequest, IsolationError> {
        // The run's labels are added by the isolation that starts it; the argv is built from a
        // plan that carries them.
        let mut labelled = plan.clone();
        labelled.labels = plan
            .labels
            .clone()
            .with("candler.job", "job")
            .with("candler.run", "run")
            .with("candler.role", "agent");
        let argv = run_argv(&labelled, &command).expect("the real run argv");
        let host_workspace = plan
            .mounts
            .iter()
            .find(|mount| {
                mount.target == plan.workdir
                    && mount.mode
                        == candler_core::isolation::container::mounts::MountMode::ReadWrite
            })
            .map(|mount| mount.source.clone())
            .expect("the workspace mount");
        let mut files = Vec::new();
        let mut pending = vec![host_workspace.clone()];
        while let Some(dir) = pending.pop() {
            for entry in fs::read_dir(&dir).expect("a directory") {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push(
                        path.strip_prefix(&host_workspace)
                            .expect("inside")
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
        files.sort();
        {
            let mut seen = self.seen.lock().expect("a lock");
            seen.argvs.push(argv);
            seen.workspace = files;
        }
        if let Some(source) = &self.leak {
            fs::copy(source, host_workspace.join("copied.txt")).expect("a copy");
        }
        self.inner.run_request(plan, command, marker, key, outputs)
    }

    fn collect_and_destroy<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        workspace: &'a Workspace<'a>,
        exit: &'a Exit,
        key: &'a RunKey,
    ) -> BoxFuture<'a, Result<ContainerOutcome, IsolationError>> {
        let _ = (plan, workspace, exit, key);
        Box::pin(async {
            let cid: ContainerId = "0123456789ab".parse().expect("a container id");
            let created: Timestamp = "2026-01-01T00:00:00Z".parse().expect("a time");
            let state = InspectState::new("exited".to_owned(), false, 0, false, None, None);
            let inspect = Inspect::new(
                cid,
                ImageId("sha256:aa".to_owned()),
                created,
                Labels::new(),
                state,
            );
            Ok(ContainerOutcome {
                inspect: Some(inspect),
                ..ContainerOutcome::default()
            })
        })
    }

    fn record_container<'a>(
        &'a self,
        plan: &'a ContainerPlan,
        key: &'a RunKey,
    ) -> BoxFuture<'a, Result<Option<candler_schemas::primitives::ContainerId>, IsolationError>>
    {
        self.inner.record_container(plan, key)
    }
}

/// The fake host with the recording isolation.
struct RecordingHost {
    base: Arc<FakeHost>,
    seen: Arc<Mutex<Seen>>,
    leak: Option<PathBuf>,
}

impl JobHost for RecordingHost {
    fn docker_cli(
        &self,
        supervisor: Arc<Supervisor>,
        record_dir: PathBuf,
    ) -> Result<DockerCli, DockerError> {
        self.base.docker_cli(supervisor, record_dir)
    }

    fn gateway<'a>(
        &'a self,
        rt: &'a Runtime,
        job: &'a JobSpec,
    ) -> BoxFuture<'a, Result<Arc<dyn Gateway>, OrchestrateError>> {
        self.base.gateway(rt, job)
    }

    fn clock(&self) -> Clock {
        self.base.clock()
    }

    fn isolation(&self, isolation: DockerIsolation) -> Arc<dyn Isolation> {
        Arc::new(Recording {
            inner: self.base.isolation(isolation),
            seen: Arc::clone(&self.seen),
            leak: self.leak.clone(),
        })
    }
}

fn sha(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    Sha256::digest(bytes)
        .iter()
        .fold(String::from("sha256:"), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// The quiet harness, except that it collects every file of the workspace as an output.
struct Collecting(support::job_orchestrate::QuietDriver);

impl HarnessDriver for Collecting {
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
    fn gateway(&self) -> Option<candler_harness::spec::GatewayEnv> {
        self.0.gateway()
    }
    fn state_paths(&self) -> Vec<candler_harness::spec::StatePath> {
        self.0.state_paths()
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
    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        self.0.live_reader()
    }
    fn offline_reader(&self, stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        self.0.offline_reader(stream)
    }
    fn classify_exit(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy {
        self.0.classify_exit(exit, sessions)
    }
    fn collect(&self, _workspace: &Workspace<'_>, _observed: &Observed) -> Vec<CollectSpec> {
        vec![CollectSpec::file("**".to_owned())]
    }
}

/// A job over one model with the inputs `resolved`, run to its end on the recording host.
struct Ran {
    rig: Rig,
    outcome: JobOutcome,
    seen: Seen,
    spec: JobSpec,
}

fn run_with(
    rig: Rig,
    declared: &candler_core::experiment::inputs::ResolvedInputs,
    leak: Option<PathBuf>,
) -> Ran {
    let facts = Facts {
        secrets: Vec::new(),
        input: None,
        ..Facts::base()
    };
    let mut job = spec(&["m"], &facts, 60, 1);
    job.inputs = declared.mounts.clone();
    job.evaluation_inputs = declared.evaluators.clone();
    let mut resolution = serde_json::to_value(&job.resolution).expect("resolution JSON");
    resolution["values"].as_array_mut().expect("values").push(
        json!({"path": "inputs", "value": serde_json::to_value(&declared.agent).expect("refs")}),
    );
    job.resolution = serde_json::from_value(resolution).expect("a resolution");
    let (cancel, _keep) = watch::channel::<Option<JobCause>>(None);
    let ctl = JobControl {
        cancel: Arc::new(cancel),
    };
    let seen = Arc::new(Mutex::new(Seen::default()));
    let host = Arc::new(RecordingHost {
        base: rig.host(),
        seen: Arc::clone(&seen),
        leak,
    });
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let mut rt = rig.runtime();
    rt.driver = Arc::new(Collecting(support::job_orchestrate::QuietDriver));
    let outcome = runtime.block_on(orchestrate_with(rt, job.clone(), ctl, host));
    let seen = seen.lock().expect("a lock").clone();
    Ran {
        rig,
        outcome,
        seen,
        spec: job,
    }
}

fn manifest_of(ran: &Ran) -> serde_json::Value {
    let key = &ran.outcome.runs.first().expect("one run").key;
    let path = ran
        .rig
        .store
        .layout()
        .root()
        .join("runs")
        .join(key.to_string())
        .join("manifest.json");
    serde_json::from_slice(&fs::read(path).expect("a manifest")).expect("manifest JSON")
}

/// Guards: I14 — an agent input reaches `comparable.inputs` as one `{kind, digest}` per input, and
/// every file of it is mounted read-only under `at/` in the `docker run` argv (agent layout;
/// comparability). Oracle: the digests are worked by hand — sha256 of the RFC 8785 text of the
/// sorted `[{path, sha256}]` list, written out literally — and the argv expectations are written
/// by hand from the files of the input trees.
#[test]
fn declared_input_agent_files_are_mounted_readonly_and_digested() {
    let rig = Rig::new(0);
    let dir = tempfile::tempdir().expect("a directory");
    let exp = experiment_dir(dir.path());
    let declared = resolve_inputs(
        &rig.store,
        &[
            input("doc", "a.txt", None, InputAudience::Agent),
            input("servers", "tree", Some("srv"), InputAudience::Agent),
        ],
        &exp,
    )
    .expect("the inputs resolve");
    let ran = run_with(rig, &declared, None);
    let list = |entries: &[(&str, &str)]| {
        let items: Vec<String> = entries
            .iter()
            .map(|(path, body)| {
                format!(
                    "{{\"path\":\"{path}\",\"sha256\":\"{}\"}}",
                    sha(body.as_bytes())
                )
            })
            .collect();
        sha(format!("[{}]", items.join(",")).as_bytes())
    };
    let expected = json!([
        {"kind": "doc", "digest": list(&[("a.txt", "alpha")])},
        {"kind": "servers", "digest": list(&[("one.txt", "one"), ("sub/two.txt", "two")])},
    ]);
    assert_eq!(manifest_of(&ran)["comparable"]["inputs"], expected);

    let argv = ran.seen.argvs.first().expect("a recorded argv");
    let mounts: Vec<&String> = argv
        .windows(2)
        .filter(|pair| pair[0] == "--mount" && pair[1].starts_with("type=bind"))
        .map(|pair| &pair[1])
        .collect();
    for (target, body) in [
        ("a.txt", "alpha"),
        ("srv/one.txt", "one"),
        ("srv/sub/two.txt", "two"),
    ] {
        let found: Vec<&&String> = mounts
            .iter()
            .filter(|mount| mount.ends_with(&format!("/{target},readonly")))
            .collect();
        assert_eq!(
            found.len(),
            1,
            "`{target}` is mounted once, read-only: {mounts:?}"
        );
        let hex = sha(body.as_bytes()).replace("sha256:", "");
        assert!(
            found[0].contains(&hex),
            "`{target}` is mounted from its blob: {}",
            found[0]
        );
    }
    assert_eq!(
        mounts
            .iter()
            .filter(|mount| mount.ends_with(",readonly"))
            .count(),
        3,
        "no other read-only bind mount: {mounts:?}"
    );
}

/// Guards: I1, B24 — a run whose output is a copy of an input file is sealed `invalid` with the
/// reason `input_leak`; the same run without the copy is not (comparability).
/// Oracle: hand-written; the output is the input file's bytes under another name, the control
/// writes nothing.
#[test]
fn declared_input_copied_into_the_output_is_an_input_leak() {
    let dir = tempfile::tempdir().expect("a directory");
    let exp = experiment_dir(dir.path());
    let seal = |leak: bool| {
        let rig = Rig::new(0);
        let declared = resolve_inputs(
            &rig.store,
            &[input("doc", "a.txt", None, InputAudience::Agent)],
            &exp,
        )
        .expect("the input resolves");
        let ran = run_with(rig, &declared, leak.then(|| exp.join("a.txt")));
        manifest_of(&ran)["integrity"].clone()
    };
    assert_eq!(
        seal(true),
        json!({"kind": "invalid", "reason": "input_leak"})
    );
    assert_ne!(
        seal(false)["reason"],
        "input_leak",
        "the control run copies nothing"
    );
}

/// The canary the evaluator-only input holds; no file the agent could see may hold it.
const CANARY: &str = "CANARY-7f3a91c2-evaluator-only";

/// Guards: I8 — an evaluator-only input is never in `JobSpec.inputs`, the `docker run` argv, the
/// workspace, `tree.json`, the run directory or `comparable`; it is in the manifest's
/// `evaluation_inputs`, with its list blob and file blobs in the store (evaluators; I8).
/// Oracle: the canary string in the input's files, and the list's RFC 8785 text written out by
/// hand; the agent's own input is the control that the scans can see a mounted input.
#[test]
fn evaluator_only_input_never_reaches_the_agent() {
    let rig = Rig::new(0);
    let dir = tempfile::tempdir().expect("a directory");
    let exp = experiment_dir(dir.path());
    fs::create_dir_all(exp.join("secret/deep")).expect("a directory");
    fs::write(exp.join("secret/answer.txt"), CANARY).expect("a canary file");
    fs::write(exp.join("secret/deep/key.txt"), CANARY).expect("a canary file");
    let declared = resolve_inputs(
        &rig.store,
        &[
            input("doc", "a.txt", None, InputAudience::Agent),
            input("hidden", "secret", Some("h"), InputAudience::Evaluators),
        ],
        &exp,
    )
    .expect("the inputs resolve");
    assert_eq!(declared.evaluators.len(), 1);
    let ran = run_with(rig, &declared, None);
    let manifest = manifest_of(&ran);

    // the cell and the job spec
    let comparable = manifest["comparable"].to_string();
    assert!(!comparable.contains("hidden"), "comparable: {comparable}");
    assert_eq!(
        manifest["comparable"]["inputs"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(ran.spec.inputs.len(), 1, "only the agent's file is a mount");
    assert_eq!(ran.spec.inputs[0].path, "a.txt");
    assert!(
        !serde_json::to_string(&ran.spec)
            .expect("spec JSON")
            .contains(CANARY)
    );

    // the manifest lists the evaluator input, its blobs are in the store
    let digest = declared.evaluators[0].digest.to_string();
    assert_eq!(
        manifest["evaluation_inputs"],
        json!([{"kind": "hidden", "digest": digest}])
    );
    let store = &ran.rig.store;
    let list =
        fs::read_to_string(store.blob_path(&declared.evaluators[0].digest)).expect("the list blob");
    let wanted = format!(
        "[{{\"path\":\"answer.txt\",\"sha256\":\"{0}\"}},{{\"path\":\"deep/key.txt\",\"sha256\":\"{0}\"}}]",
        sha(CANARY.as_bytes())
    );
    assert_eq!(
        list, wanted,
        "the list blob is the RFC 8785 text of the sorted list"
    );
    assert_eq!(digest, sha(wanted.as_bytes()));
    let file_blob = store.blob_path(&sha(CANARY.as_bytes()).parse().expect("a digest"));
    assert_eq!(fs::read_to_string(file_blob).expect("a file blob"), CANARY);

    // the container's world
    let argv = ran.seen.argvs.first().expect("an argv").join(" ");
    for needle in [
        CANARY,
        "answer.txt",
        "hidden",
        digest.trim_start_matches("sha256:"),
        sha(CANARY.as_bytes()).trim_start_matches("sha256:"),
    ] {
        assert!(!argv.contains(needle), "argv holds `{needle}`: {argv}");
    }
    assert!(
        ran.seen.workspace.contains(&"a.txt".to_owned()),
        "the control: the agent's file is in the workspace"
    );
    assert!(
        ran.seen
            .workspace
            .iter()
            .all(|file| !file.contains("answer")
                && !file.contains("key.txt")
                && !file.starts_with("h/")),
        "workspace: {:?}",
        ran.seen.workspace
    );

    // the agent-visible places never name the list digest or the file digests, and no mount
    // comes from the job directory
    let hidden_digests = [
        digest.trim_start_matches("sha256:").to_owned(),
        sha(CANARY.as_bytes())
            .trim_start_matches("sha256:")
            .to_owned(),
    ];
    let job_dir = store.layout().root().join("jobs");
    for pair in ran.seen.argvs[0].windows(2) {
        if pair[0] == "--mount" {
            let source = pair[1]
                .split(',')
                .find_map(|part| part.strip_prefix("source="))
                .unwrap_or_default();
            assert!(
                !Path::new(source).starts_with(&job_dir),
                "a mount comes from the job directory: {}",
                pair[1]
            );
            for needle in &hidden_digests {
                assert!(
                    !pair[1].contains(needle.as_str()),
                    "mount holds `{needle}`: {}",
                    pair[1]
                );
            }
        }
    }
    let workspace_dir = store
        .layout()
        .root()
        .join("runs")
        .join(ran.outcome.runs[0].key.to_string())
        .join("workspace");
    for needle in &hidden_digests {
        let found = support::scan(&workspace_dir, needle).expect("a scan of the workspace");
        assert!(found.files > 0, "the scan read the workspace");
        assert!(
            found.hits.is_empty(),
            "the workspace holds `{needle}`: {:?}",
            found.hits
        );
        assert!(
            ran.seen
                .workspace
                .iter()
                .all(|file| !file.contains(needle.as_str())),
            "a workspace file is named by `{needle}`"
        );
    }

    // no file of the run directory or the job records holds the canary
    let root = store.layout().root();
    let runs = support::scan(&root.join("runs"), CANARY).expect("a scan of the runs");
    assert!(runs.files > 0, "the scan read the run directory");
    assert!(runs.hits.is_empty(), "the canary is in {:?}", runs.hits);
    let jobs = support::scan(&root.join("jobs"), CANARY).expect("a scan of the jobs");
    assert!(jobs.hits.is_empty(), "the canary is in {:?}", jobs.hits);
    let key = ran.outcome.runs[0].key.to_string();
    let tree =
        fs::read_to_string(root.join("runs").join(key).join("tree.json")).expect("tree.json");
    for needle in ["answer", "key.txt", "hidden"]
        .into_iter()
        .chain(hidden_digests.iter().map(String::as_str))
    {
        assert!(!tree.contains(needle), "tree.json holds `{needle}`: {tree}");
    }
    assert!(
        ran.seen.workspace.contains(&"a.txt".to_owned()),
        "the control: the agent's input is in the workspace the tree is read from"
    );
}

/// Guards: I11 — a resume whose declared input has one byte changed is refused without `--force`,
/// naming `inputs.<kind>` with the stored and the observed list digest; the same inputs resume
/// (authors; resume and deviation checks).
/// Oracle: the stored digest is the one the first resolution made, and the observed one is the
/// digest the changed tree resolves to; the path is the literal `inputs.doc`.
#[test]
fn declared_input_changed_byte_is_refused_on_resume() {
    use candler_core::runner::plan::{Force, PlanError, PlanMode, plan_runs};

    let dir = tempfile::tempdir().expect("a directory");
    let exp = experiment_dir(dir.path());
    let rig = Rig::new(0);
    let first = resolve_inputs(
        &rig.store,
        &[input("doc", "a.txt", None, InputAudience::Agent)],
        &exp,
    )
    .expect("the input resolves");
    let ran = run_with(rig, &first, None);
    let store = ran.rig.store.clone();
    let lock = dir.path().join("lock");
    let ctx = support::run_plan::plan_ctx(&store, &lock, &Facts::base());
    let file = support::run_plan::experiment(&["m"], 2);
    let resolution_of = |declared: &candler_core::experiment::inputs::ResolvedInputs| {
        let mut value = serde_json::to_value(&ran.spec.resolution).expect("resolution JSON");
        for entry in value["values"].as_array_mut().expect("values") {
            if entry["path"] == "inputs" {
                entry["value"] = serde_json::to_value(&declared.agent).expect("refs");
            }
        }
        serde_json::from_value::<candler_core::resolve::layers::Resolution>(value)
            .expect("a resolution")
    };
    plan_runs(
        &ctx,
        &file,
        &resolution_of(&first),
        Force::No,
        PlanMode::Hold,
    )
    .map_err(|error| error.to_string())
    .expect("the same input resumes");

    fs::write(exp.join("a.txt"), "alphA").expect("a changed byte");
    let second = resolve_inputs(
        &store,
        &[input("doc", "a.txt", None, InputAudience::Agent)],
        &exp,
    )
    .expect("the changed input resolves");
    let Err(PlanError::Changed(deviations)) = plan_runs(
        &ctx,
        &file,
        &resolution_of(&second),
        Force::No,
        PlanMode::Hold,
    ) else {
        panic!("a changed input byte must be refused");
    };
    assert_eq!(deviations.len(), 1, "{deviations:?}");
    assert_eq!(deviations[0].path.as_str(), "inputs.doc");
    let text = |digest: &candler_schemas::execution::DigestRef| digest.digest.to_string();
    assert_eq!(
        deviations[0].stored.as_ref().map(|v| v.as_value().clone()),
        Some(json!(text(&first.agent[0])))
    );
    assert_eq!(
        deviations[0]
            .observed
            .as_ref()
            .map(|v| v.as_value().clone()),
        Some(json!(text(&second.agent[0])))
    );
    assert_ne!(text(&first.agent[0]), text(&second.agent[0]));
}

/// Guards: I10 — an input tree that holds an empty file (such as `__init__.py`) is declarable and
/// runs clean: no run of it is an input leak (authors; the agent layout).
/// Oracle: hand-written trees; the verdict is the literal reason `input_leak`.
/// Bite: the mount points collected as outputs and an empty match counted (both are needed to
/// turn it red together with the refusal of empty files).
#[test]
fn declared_input_with_an_empty_file_runs_clean() {
    let dir = tempfile::tempdir().expect("a directory");
    let exp = experiment_dir(dir.path());
    fs::write(exp.join("zero.txt"), "").expect("an empty file");
    fs::create_dir_all(exp.join("pkg/inner")).expect("a directory");
    fs::write(exp.join("pkg/__init__.py"), "").expect("an empty file");
    fs::write(exp.join("pkg/inner/__init__.py"), "").expect("an empty file");
    fs::write(exp.join("pkg/mod.py"), "x = 1\n").expect("a file");
    for (label, path) in [
        ("an empty file", "zero.txt"),
        ("a tree with empty files", "pkg"),
    ] {
        let rig = Rig::new(0);
        let declared = resolve_inputs(
            &rig.store,
            &[input("pkg", path, None, InputAudience::Agent)],
            &exp,
        )
        .unwrap_or_else(|error| panic!("{label} is declarable: {error}"));
        let ran = run_with(rig, &declared, None);
        assert_ne!(
            manifest_of(&ran)["integrity"]["reason"],
            "input_leak",
            "{label}: a run that copies nothing is not a leak"
        );
    }
}

/// Guards: I10 — the mount point of an input file, an empty file in the workspace, is not an
/// output of the run: collection leaves out every path an input is mounted at and keeps the rest.
/// Oracle: a hand-built workspace listing; the expected outputs are written out.
/// Bite: the mount points collected again.
#[test]
fn declared_input_mount_points_are_not_outputs() {
    use candler_core::runner::evidence::{InputDigest, Inputs};
    use candler_core::runner::exec::gather::collect_outputs;

    let dir = tempfile::tempdir().expect("a directory");
    let ws = dir.path();
    fs::create_dir_all(ws.join("srv/sub")).expect("directories");
    fs::write(ws.join("a.txt"), "").expect("a mount point");
    fs::write(ws.join("srv/sub/two.txt"), "").expect("a mount point");
    fs::write(ws.join("srv/made.txt"), "kept").expect("an output");
    fs::write(ws.join("empty_output.txt"), "").expect("an output");
    fs::write(ws.join("a.txt.bak"), "kept").expect("an output named like a mount point");
    fs::write(ws.join("srv/sub/two.txt2"), "kept").expect("an output named like a mount point");
    fs::create_dir_all(ws.join("srv/sub/two.txtd")).expect("a directory");
    fs::write(ws.join("srv/sub/two.txtd/f"), "kept")
        .expect("an output under a lookalike directory");
    let at = |path: &str| InputDigest {
        path: path.to_owned(),
        digest: sha("x".as_bytes()).parse().expect("a digest"),
    };
    let inputs = Inputs {
        digests: vec![at("a.txt"), at("srv/sub/two.txt")],
    };
    let found =
        collect_outputs(ws, &[CollectSpec::file("**".to_owned())], &inputs).expect("a collection");
    let paths: Vec<&str> = found.iter().map(|out| out.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "a.txt.bak",
            "empty_output.txt",
            "srv/made.txt",
            "srv/sub/two.txt2",
            "srv/sub/two.txtd/f"
        ]
    );
}

/// Guards: I10 — copying an empty file leaks nothing, and copying a non-empty input still does.
/// Oracle: the sha256 of the empty string, `e3b0c442...b855` (`printf '' | shasum -a 256`), and
/// of `alpha`, both literal.
/// Bite: the empty match counted.
#[test]
fn declared_input_empty_match_is_no_leak_and_a_real_copy_is() {
    use candler_core::runner::evidence::leak::input_leak;
    use candler_core::runner::evidence::{Evidence, InputDigest, Inputs, OutputDigest};
    const EMPTY: &str = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let evidence = |sha: &str| Evidence {
        sessions: Vec::new(),
        events: Vec::new(),
        outputs: vec![OutputDigest {
            path: "copy.txt".to_owned(),
            digest: sha.parse().expect("a digest"),
        }],
        tree: candler_schemas::execution::RunTree::builder()
            .build()
            .expect("an empty tree"),
    };
    let inputs = |sha: &str| Inputs {
        digests: vec![InputDigest {
            path: "in.txt".to_owned(),
            digest: sha.parse().expect("a digest"),
        }],
    };
    assert_eq!(sha(b""), EMPTY, "the oracle constant is the empty digest");
    assert!(input_leak(&evidence(EMPTY), &inputs(EMPTY)).is_none());
    let alpha = sha(b"alpha");
    assert!(input_leak(&evidence(&alpha), &inputs(&alpha)).is_some());
}
