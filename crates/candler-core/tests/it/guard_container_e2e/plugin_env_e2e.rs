//! The real-Docker guard of a plug-in's agent environment: the image chain the environment names
//! (harness base, env layer, the environment's own extension) is built through the core, its
//! inputs are mounted the way a run mounts them, and the agent's process lists the tools of the
//! environment's MCP server from inside the container.
//!
//! The test builds with [`DockerImageService`] and runs through `DockerIsolation::run_request`.
//! Every image it built is removed by exact id, and the container and the network by theirs, by
//! [`Cleanup`] and, when the test panics first, by its `Drop`.
#![cfg(test)]

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use candler_core::experiment::inputs::resolve_input_groups;
use candler_core::frames::FrameReader;
use candler_core::isolation::container::DockerIsolation;
use candler_core::isolation::container::guard::ContainerGuard;
use candler_core::isolation::container::mounts::{MountMode, MountSpec};
use candler_core::isolation::container::sweeper::LabelSweeper;
use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
use candler_core::isolation::gateway::ChildRecorder;
use candler_core::isolation::image::{
    BuildSetting, DockerImageService, Extension, ImageAssets, ImageService, LockDigest, LockRequest,
};
use candler_core::isolation::model::{
    ContainerPlan, GatewayHandle, ImageId, ImageRef, Labels, Limits, NetworkName, UserSpec,
};
use candler_core::isolation::testing::AllowlistDriver;
use candler_core::isolation::{Isolation as _, RunOutputs};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::view::JobView;
use candler_core::resolve::steps::plugin_environment::Fragment;
use candler_core::store::Store;
use candler_core::supervise::spawn::OutputSpec;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::spec::{CommandSpec, Framing, Workspace};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::{EventPayload, ExperimentInput, InputAudience};
use candler_schemas::primitives::{Digest, JobId, Name, RunKey};
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::FrameData;
use serde_json::{Value, json};
use sha2::Digest as _;

use crate::support::container_stop::{cid, container_ref, inspect, labels};
use crate::support::job_orchestrate::quiet_gateway;
use crate::support::job_store::job_record;
use crate::support::run_plan::fresh_store;

/// The label every container and network of this test carries.
const TEST_LABEL: &str = "candler.test=plugin_environment";

/// The plug-in's directory name and the environment the test selects.
const SELECTOR: &str = "agent-env/tiny";

/// The evaluator-only file of the fixture, by name and by a line of its bytes.
const CANARY_NAME: &str = "canary.txt";
const CANARY_BYTES: &str = "EVALUATOR-ONLY-CANARY-5d1c0e7a-do-not-mount";

/// Runs `docker <args>` and returns its output, which the test reads and never trusts.
fn docker(args: &[&str]) -> std::process::Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("the docker CLI runs")
}

/// The lines `docker <args>` prints.
fn docker_lines(args: &[&str]) -> BTreeSet<String> {
    String::from_utf8_lossy(&docker(args).stdout)
        .lines()
        .map(str::to_owned) // BORROW: the set owns its ids
        .collect()
}

/// The lower-case hex sha256 of `bytes`.
fn hex_sha256(bytes: &[u8]) -> String {
    sha2::Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// A number that differs between runs of the test.
fn unique() -> u64 {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("a clock")
            .as_nanos()
            % u128::from(u64::MAX),
    )
    .expect("fits")
}

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// What the test created in docker. Nothing outside these lists is ever removed.
struct Cleanup {
    /// The image ids that were there before the test.
    before: BTreeSet<String>,
    lock_hex: Option<String>,
    images: Vec<String>,
    containers: Vec<String>,
    network: Option<String>,
}

impl Cleanup {
    fn new() -> Self {
        Self {
            before: docker_lines(&["images", "-aq", "--no-trunc"]),
            lock_hex: None,
            images: Vec::new(),
            containers: Vec::new(),
            network: None,
        }
    }

    /// Adds the env image of the test's lock, the base image of the shared tag and `extra`, each
    /// when it was not there before the test.
    fn adopt(&mut self, extra: Option<&str>) {
        let mut found = BTreeSet::new();
        if let Some(hex) = &self.lock_hex {
            let filter = format!("label=candler.lock={hex}");
            found.extend(docker_lines(&[
                "images",
                "-aq",
                "--no-trunc",
                "--filter",
                &filter,
            ]));
        }
        found.extend(docker_lines(&[
            "images",
            "-aq",
            "--no-trunc",
            "--filter",
            "reference=candler-base:src-*",
        ]));
        found.extend(extra.map(str::to_owned)); // BORROW: the set owns its ids
        for id in found {
            if !self.before.contains(&id) && !self.images.contains(&id) {
                self.images.push(id);
            }
        }
    }

    /// Removes the containers, the network and the images, children before parents, by exact
    /// id and name.
    fn remove_all(&mut self) {
        self.adopt(None);
        for id in self.containers.drain(..) {
            let _ = docker(&["rm", &id]);
        }
        if let Some(name) = self.network.take() {
            let _ = docker(&["network", "rm", &name]);
        }
        // The extension is a child of the env image, which is a child of the base.
        for pass in 0..3 {
            let _ = pass;
            self.images
                .retain(|id| !docker(&["rmi", id]).status.success());
        }
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        self.remove_all();
    }
}

/// The text a framed output file holds.
fn frames_text(path: &Path) -> String {
    FrameReader::open(path)
        .expect("a frame file")
        .map(|frame| match frame.expect("a frame").data {
            FrameData::Text { text } => text,
            // EXHAUSTIVE: the wire's enum is non-exhaustive; bytes and unknown kinds print as Debug
            other @ (FrameData::Bytes { .. } | _) => format!("{other:?}"),
        })
        .collect::<String>()
}

/// The fixture plug-in's environment, read through the core's reader.
fn fragment() -> Fragment {
    let dirs = [Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugins/agent-env")];
    Fragment::load(&dirs, SELECTOR).expect("the fixture environment loads")
}

/// The text of the fragment's value at `path`.
fn value_of(fragment: &Fragment, path: &str) -> Value {
    fragment
        .values
        .iter()
        .find(|(own, _)| own.as_str() == path)
        .map_or_else(
            || panic!("the environment sets no {path}"),
            |(_, value)| value.as_value().clone(), // BORROW: the caller owns the value
        )
}

/// The file the client reads: the resolved `mcp.servers.tools.*` values of the environment, as
/// a driver renders a server into its own configuration.
fn render_mcp(fragment: &Fragment) -> Value {
    json!({
        "command": value_of(fragment, "mcp.servers.tools.command"),
        "args": value_of(fragment, "mcp.servers.tools.args"),
    })
}

/// Guards: R06 D4, R01 §Agent environment — a real container built from a plug-in environment
/// (the harness base, the env layer, the environment's own Dockerfile and context) runs the
/// environment's MCP server from the environment's inputs, and the agent's process lists
/// exactly the server's tools from inside the container, while an evaluator-only file of the
/// same plug-in, declared as an evaluator-only input of the experiment, is in the container
/// neither by name nor by bytes. Everything the test built is
/// removed by id.
/// Oracle: the tools the fixture's `agent/server.py` declares, written by hand below (names,
/// the `add` schema, and the dependency's tag that only the wheel built into the extension
/// can supply); the fixture's `evaluator/canary.txt`, read on the host to prove it exists;
/// `docker inspect` of every id after the removal.
/// Bite: the extension left out of the build (the server fails on its import), the plug-in's
/// inputs not mounted (the client is not there), or the resolved `mcp.*` values dropped from the
/// file the client reads (no server to launch), or the evaluator-only input declared for the agent
/// (`InputAudience::Agent`: it is mounted, and the client finds the canary by name and bytes).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs a Docker daemon, the harness base image and PyPI for the env venv"]
async fn plugin_environment_serves_live_mcp_tools() {
    let mut cleanup = Cleanup::new();
    let fragment = fragment();
    let canary_file = fragment.dir.join("evaluator").join(CANARY_NAME);
    let canary_text = std::fs::read_to_string(&canary_file).expect("the fixture canary");
    assert!(
        canary_text.contains(CANARY_BYTES),
        "the fixture canary does not hold the bytes the test looks for"
    );

    let home = std::env::var("HOME").expect("HOME");
    let scratch = tempfile::tempdir_in(home).expect("a scratch directory under HOME");
    let root = scratch.path().canonicalize().expect("a canonical path");
    for sub in ["work", "home", "records", "frames", "control"] {
        std::fs::create_dir_all(root.join(sub)).expect("a directory");
    }

    // The image chain, through the core.
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/docker");
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let sup = Arc::new(Supervisor::new(Arc::clone(&sink))); // BORROW: the client shares the sink
    let env = ClientEnv::from_process();
    let cli = DockerCli::new(Arc::clone(&sup), env.clone(), root.join("control"))
        .expect("a docker client"); // BORROW: .clone() gives the service its own environment
    let service = DockerImageService::new(
        Arc::clone(&sup), // BORROW: the service shares the supervisor
        cli,
        env,
        ImageAssets {
            base_context: assets.join("base"),
            env_context: assets.join("env"),
            store_root: root.join("images"),
        },
        Arc::new(ClaudeCodeDriver::new()),
    );
    let lock = service
        .lock(&LockRequest {
            sources: Vec::new(),
        })
        .await
        .expect("the lock compiles");
    let hex = hex_sha256(&lock);
    cleanup.lock_hex = Some(hex.clone()); // BORROW: .clone() keeps the hex for the label filter
    let extension = Extension {
        dockerfile: PathBuf::from(
            value_of(&fragment, "isolation.build.dockerfile")
                .as_str()
                .expect("a path"),
        ),
        context: PathBuf::from(
            value_of(&fragment, "isolation.build.context")
                .as_str()
                .expect("a path"),
        ),
    };
    let built = service
        .ensure(
            &LockDigest(hex),
            &BuildSetting {
                lock,
                extension: Some(extension),
                prune: Vec::new(),
            },
        )
        .await;
    cleanup.adopt(built.as_ref().ok().map(|image| image.id.0.as_str())); // BORROW: explicit .as_str()
    let image = built.expect("the image chain builds");

    // The world of the run.
    let bits = unique();
    let job: JobId = format!(
        "{:08x}-{:04x}-7000-8000-000000000000",
        bits >> 32,
        bits & 0xffff
    )
    .parse()
    .expect("a version 7 job id");
    let (store_dir, jobs_store, _lock) = fresh_store();
    let dir = JobDir::create(
        jobs_store.layout(),
        job.clone(), // BORROW: the directory owns a copy of the id
        &job_record(&job, candler_schemas::execution::JobState::Running),
    )
    .expect("a job directory");
    let network = NetworkName::for_job(&job);
    cleanup.network = Some(network.as_str().to_owned()); // BORROW: the cleanup owns the name
    let created = docker(&[
        "network",
        "create",
        "--internal",
        "--label",
        TEST_LABEL,
        "--label",
        &format!("candler.job={job}"),
        network.as_str(), // BORROW: explicit .as_str()
    ]);
    assert!(
        created.status.success(),
        "docker network create: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let run_cli = DockerCli::new(
        Arc::new(Supervisor::new(Arc::clone(&sink))), // BORROW: the client shares the sink
        ClientEnv::from_process(),
        root.join("records"),
    )
    .expect("a docker client");
    let destroy = Arc::new(AllowlistDriver::new(run_cli.destroyer_for_tests()));
    let kill: Arc<dyn DestroyDriver> = destroy.clone(); // BORROW: the test allows ids in the same driver; TRAIT_OBJECT: the guard holds a trait object
    let read: Arc<dyn DockerDriver> = Arc::new(run_cli); // TRAIT_OBJECT: the isolation reads through the driver trait
    let view: Arc<dyn JobView> = Arc::new(dir.clone()); // BORROW: the view owns its directory handle; TRAIT_OBJECT: the guard reads records through a trait object
    let guard = Arc::new(ContainerGuard::new(
        kill,
        Arc::clone(&read), // BORROW: the sweeper reads through the same driver
        Arc::clone(&view), // BORROW: the sweeper shares the view
    ));
    let sweeper = LabelSweeper::new(
        Arc::clone(&read),  // BORROW: the sweeper reads through the same driver
        Arc::clone(&guard), // BORROW: the sweeper destroys through the same guard
        view,
        Duration::from_secs(30),
    );
    let gateway = GatewayHandle::new(
        container_ref(&inspect(&cid(0xfff), labels(&job, "gateway", None))),
        "http://127.0.0.1:1".parse().expect("a url"),
        network.clone(), // BORROW: the handle owns its network name
        NetworkName::egress_for_job(&job),
        String::from("dummy"),
    )
    .expect("a handle");
    let recorder: Arc<dyn ChildRecorder> = Arc::new(dir); // TRAIT_OBJECT: the isolation takes the recorder as a trait object
    let isolation = DockerIsolation::new(gateway, read, guard)
        .with_recorder(recorder)
        .with_sweeper(Arc::new(sweeper));

    // The environment's inputs, resolved against the plug-in directory into a store under HOME
    // and mounted read-only at their workspace paths, the way a run mounts them.
    let store = Store::init(&root.join("store")).expect("a store");
    // The experiment's own evaluator-only input is the plug-in's `evaluator` directory: a plug-in
    // fragment declares agent inputs only, so the audience is the experiment's to set.
    let evaluator_only = [ExperimentInput::new(
        "evaluator_only".parse::<Name>().expect("a name"),
        String::from("evaluator"),
        None,
        InputAudience::Evaluators,
    )
    .expect("an evaluator input")];
    let resolved = resolve_input_groups(
        &store,
        &[
            (fragment.inputs.as_slice(), fragment.dir.as_path()), // BORROW: explicit .as_slice()
            (evaluator_only.as_slice(), fragment.dir.as_path()),  // BORROW: explicit .as_slice()
        ],
    )
    .expect("the environment's inputs resolve");
    assert_eq!(
        resolved.evaluators.len(),
        1,
        "the evaluator-only input is declared and resolved"
    );
    let work = root.join("work");
    let mut mounts: Vec<MountSpec> = ["work", "home"]
        .map(|sub| MountSpec {
            source: root.join(sub),
            target: format!("/{}", if sub == "work" { "work" } else { "home/agent" }),
            mode: MountMode::ReadWrite,
        })
        .to_vec();
    for input in &resolved.mounts {
        let point = work.join(&input.path);
        std::fs::create_dir_all(point.parent().expect("a parent")).expect("a directory");
        std::fs::write(&point, b"").expect("a mount point");
        mounts.push(MountSpec {
            source: store.layout().blob(&input.digest),
            target: format!("/work/{}", input.path),
            mode: MountMode::ReadOnly,
        });
    }
    std::fs::write(
        work.join("mcp.json"),
        serde_json::to_vec(&render_mcp(&fragment)).expect("a config"),
    )
    .expect("the config file");

    let experiment: Digest = format!("sha256:{}", "c".repeat(64))
        .parse()
        .expect("a digest");
    let key = RunKey::new(experiment.clone(), 1, 1).expect("a run key"); // BORROW: the label below reads the digest
    let plan = ContainerPlan {
        image: ImageRef {
            id: ImageId(image.id.0.clone()), // BORROW: the plan owns its id
            harness_version: None,
        },
        labels: Labels::new()
            .with("candler.job", &job.to_string())
            .with("candler.role", "agent")
            .with("candler.experiment", &experiment.to_string())
            .with("candler.test", "plugin_environment"),
        mounts,
        env: Vec::new(),
        gateway: Some(quiet_gateway()),
        state: Vec::new(),
        network: network.clone(), // BORROW: the plan owns its network name
        egress: None,
        add_hosts: Vec::new(),
        limits: Limits {
            pids_limit: 256,
            memory: String::from("512m"),
            cpus: 1.0,
        },
        record_dir: root.join("records"),
        user: UserSpec::Engine,
        workdir: String::from("/work"),
        home: String::from("/home/agent"),
        expiry: Duration::from_secs(120),
        project_root: root.clone(), // BORROW: the plan owns its path
        store_root: root.clone(),   // BORROW: the plan owns its path
        interactive: false,
    };

    // The client script is the agent's command.
    let framed = |stream: &str| {
        OutputSpec::framed(
            root.join("frames"),
            stream.parse().expect("a stream name"),
            Framing::Lines,
        )
    };
    let command = CommandSpec::builder()
        .argv(vec![
            String::from("python"),
            String::from("/work/env/client.py"),
            String::from("/work/mcp.json"),
            CANARY_NAME.to_owned(),  // BORROW: the command owns its words
            CANARY_BYTES.to_owned(), // BORROW: the command owns its words
        ])
        .build()
        .expect("a command");
    let isolated = isolation
        .run_request(
            &plan,
            command,
            "plugin-env".parse().expect("a marker"),
            key.clone(), // BORROW: the request owns its key
            RunOutputs::new(framed("stdout"), framed("stderr")),
        )
        .await
        .expect("a request");
    let id = isolated.container.expect("a created container");
    destroy.allow_container(id.as_str()); // BORROW: explicit .as_str()
    cleanup.containers.push(id.as_str().to_owned()); // BORROW: the cleanup owns the id
    let exit = Supervisor::new(Arc::clone(&sink)) // BORROW: the supervisor shares the sink
        .spawn(isolated.request)
        .expect("the client starts")
        .wait()
        .await
        .expect("the client ends");
    let stdout = frames_text(&exit.raw.first().expect("a stdout file").path);
    let stderr = frames_text(&exit.raw.get(1).expect("a stderr file").path);
    let outcome = isolation
        .collect_and_destroy(&plan, &Workspace::new(&work), &exit, &key)
        .await
        .expect("an outcome");
    assert!(outcome.sweep.refused.is_empty(), "{:?}", outcome.sweep);

    assert_eq!(
        exit.status,
        ExitStatus::Code { code: 0 },
        "the client failed: stdout {stdout:?} stderr {stderr:?}"
    );
    let seen: Value = serde_json::from_str(stdout.lines().last().expect("a line of output"))
        .unwrap_or_else(|e| panic!("the client printed no JSON ({e}): {stdout:?} {stderr:?}"));
    // The tools of `agent/server.py`, written by hand.
    let expected = json!([
        {
            "name": "echo",
            "description": "Returns its text. tinydep-1.0",
            "inputSchema": {
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"],
            },
        },
        {
            "name": "add",
            "description": "Adds two integers.",
            "inputSchema": {
                "type": "object",
                "properties": {"a": {"type": "integer"}, "b": {"type": "integer"}},
                "required": ["a", "b"],
            },
        },
    ]);
    assert_eq!(seen["tools"], expected, "the tools listed in the container");
    assert_eq!(
        seen["canary"],
        json!({"by_name": [], "by_bytes": []}),
        "the evaluator-only file is in the container"
    );

    // Everything built is removed by id, and docker no longer knows any of it.
    let ids = (cleanup.images.clone(), cleanup.containers.clone()); // BORROW: .clone() keeps the ids for the check
    cleanup.remove_all();
    for id in ids.0.iter().chain(ids.1.iter()) {
        assert!(
            !docker(&["inspect", id]).status.success(),
            "{id} is still there after the test"
        );
    }
    drop(store_dir);
}
