//! The real-Docker guard of the agent's create-then-attach path: the real testagent image runs
//! through `DockerIsolation::run_request` (`docker create`, the record, `docker start -a`), and
//! what the run observes equals what `docker run` gives for the same script.
//!
//! Every container carries `candler.test=create_attach` and the test's own job label; each
//! one is destroyed by its id through the guard, and the network by its exact name.
#![cfg(test)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use candler_core::frames::FrameReader;
use candler_core::isolation::container::DockerIsolation;
use candler_core::isolation::container::expiry::wrap_expiry;
use candler_core::isolation::container::guard::ContainerGuard;
use candler_core::isolation::container::mounts::{MountMode, MountSpec};
use candler_core::isolation::container::run::{build_start_request, create_argv};
use candler_core::isolation::container::sweeper::LabelSweeper;
use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
use candler_core::isolation::gateway::ChildRecorder;
use candler_core::isolation::model::{
    ContainerPlan, GatewayHandle, ImageId, ImageRef, Labels, Limits, NetworkName, UserSpec,
};
use candler_core::isolation::testing::AllowlistDriver;
use candler_core::isolation::{EndFacts, Isolation as _, RunOutputs, StartProbe};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::view::JobView;
use candler_core::runner::exec::spawn::RunStopper;
use candler_core::supervise::spawn::{OutputSpec, SpawnRequest};
use candler_core::supervise::stop::{Ended, Exit};
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::spec::{CommandSpec, Framing, Workspace};
use candler_schemas::execution::{EventPayload, JobState};
use candler_schemas::primitives::{ContainerId, Digest, JobId, Marker, RunKey};
use candler_schemas::process::ExitStatus;
use tokio::io::AsyncWriteExt as _;

use crate::support::container_stop::{cid, container_ref, inspect, labels};
use crate::support::job_orchestrate::quiet_gateway;
use crate::support::job_store::job_record;
use crate::support::run_plan::fresh_store;
use crate::support::test_image::{TESTAGENT_CONTEXT, TestImage};

/// The label every container and network of this test carries.
const TEST_LABEL: &str = "candler.test=create_attach";

/// How long the test waits for a container to be running or gone.
const PATIENCE: Duration = Duration::from_secs(30);

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// Runs `docker <args>` and returns its output, which the test reads and never trusts.
fn docker(args: &[&str]) -> std::process::Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("the docker CLI runs")
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

/// What the world of one case holds.
struct World {
    /// The scratch directory under `$HOME`, which docker shares.
    _scratch: tempfile::TempDir,
    root: PathBuf,
    _store: tempfile::TempDir,
    key: RunKey,
    network: NetworkName,
    isolation: DockerIsolation,
    supervisor: Supervisor,
    destroy: Arc<AllowlistDriver<candler_core::isolation::docker::DockerDestroyer>>,
    plan: ContainerPlan,
}

impl World {
    /// A world for a run of `image`, interactive or not.
    fn new(image: &TestImage, interactive: bool) -> Self {
        let bits = unique();
        let job: JobId = format!(
            "{:08x}-{:04x}-7000-8000-000000000000",
            bits >> 32,
            bits & 0xffff
        )
        .parse()
        .expect("a version 7 job id");
        let home = std::env::var("HOME").expect("HOME");
        let scratch = tempfile::tempdir_in(home).expect("a scratch directory under HOME");
        let root = scratch.path().canonicalize().expect("a canonical path");
        for sub in ["work", "home", "records", "frames"] {
            std::fs::create_dir_all(root.join(sub)).expect("a directory");
        }
        let (store_dir, store, _lock) = fresh_store();
        let dir = JobDir::create(
            store.layout(),
            job.clone(), // BORROW: the directory owns a copy of the id
            &job_record(&job, JobState::Running),
        )
        .expect("a job directory");
        let network = NetworkName::for_job(&job);
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
        let sink: Arc<dyn EventSink> = Arc::new(Silent);
        let cli = DockerCli::new(
            Arc::new(Supervisor::new(Arc::clone(&sink))), // BORROW: the client shares the sink
            ClientEnv::from_process(),
            root.join("records"),
        )
        .expect("a docker client");
        let destroy = Arc::new(AllowlistDriver::new(cli.destroyer_for_tests()));
        let kill: Arc<dyn DestroyDriver> = destroy.clone(); // BORROW: the test allows ids in the same driver; TRAIT_OBJECT: the guard holds a trait object
        let read: Arc<dyn DockerDriver> = Arc::new(cli); // TRAIT_OBJECT: the isolation reads through the driver trait
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
            PATIENCE,
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
        let experiment: Digest = format!("sha256:{}", "b".repeat(64))
            .parse()
            .expect("a digest");
        let key = RunKey::new(experiment.clone(), 1, 1).expect("a run key"); // BORROW: the label below reads the digest
        let plan = ContainerPlan {
            image: ImageRef {
                id: ImageId(image.id().to_owned()), // BORROW: the plan owns the id
                harness_version: None,
            },
            labels: Labels::new()
                .with("candler.job", &job.to_string())
                .with("candler.role", "agent")
                .with("candler.experiment", &experiment.to_string())
                .with("candler.test", "create_attach"),
            mounts: ["work", "home"]
                .map(|sub| MountSpec {
                    source: root.join(sub),
                    target: format!("/{}", if sub == "work" { "work" } else { "home/agent" }),
                    mode: MountMode::ReadWrite,
                })
                .to_vec(),
            env: Vec::new(),
            gateway: Some(quiet_gateway()),
            state: Vec::new(),
            network: network.clone(), // BORROW: the plan owns its network name
            egress: None,
            add_hosts: Vec::new(),
            limits: Limits {
                pids_limit: 128,
                memory: String::from("256m"),
                cpus: 1.0,
            },
            record_dir: root.join("records"),
            user: UserSpec::Engine,
            workdir: String::from("/work"),
            home: String::from("/home/agent"),
            expiry: Duration::from_secs(30),
            project_root: root.clone(), // BORROW: the plan owns its path
            store_root: root.clone(),   // BORROW: the plan owns its path
            interactive,
        };
        Self {
            _scratch: scratch,
            root,
            _store: store_dir,
            key,
            network,
            isolation,
            supervisor: Supervisor::new(sink),
            destroy,
            plan,
        }
    }

    /// The outputs under the world's `frames` directory, named by `tag`.
    fn outputs(&self, tag: &str) -> RunOutputs {
        let framed = |stream: &str| {
            OutputSpec::framed(
                self.root.join("frames"),
                format!("{tag}-{stream}").parse().expect("a stream name"),
                Framing::Lines,
            )
        };
        RunOutputs::new(framed("stdout"), framed("stderr"))
    }

    /// The command: the testagent in `mode`, the way the harness's command reaches the image.
    fn command(mode: &str) -> CommandSpec {
        CommandSpec::builder()
            .argv(vec![
                String::from("/usr/local/bin/testagent"),
                mode.to_owned(), // BORROW: the command owns its words
            ])
            .build()
            .expect("a command")
    }

    /// The created container's request through the isolation, and its id, allowed in the driver.
    async fn made(&self, mode: &str) -> (SpawnRequest, ContainerId) {
        let (request, id, _facts) = self.made_stoppable(mode).await;
        (request, id)
    }

    /// [`Self::made`] with the run's end facts, which stop the container through the daemon.
    async fn made_stoppable(&self, mode: &str) -> (SpawnRequest, ContainerId, Arc<dyn EndFacts>) {
        let isolated = self
            .isolation
            .run_request(
                &self.plan,
                Self::command(mode),
                "create-attach".parse().expect("a marker"),
                self.key.clone(), // BORROW: the request owns its key
                self.outputs("run"),
            )
            .await
            .expect("a request");
        let id = isolated.container.expect("a created container");
        self.destroy.allow_container(id.as_str()); // BORROW: explicit .as_str()
        (isolated.request, id, isolated.end_facts)
    }

    /// Destroys the run's container through the isolation, as the collect does after the exit. A
    /// container the stop hook already destroyed is reported unknown, and no more.
    async fn collect(&self, exit: &Exit) {
        let root = self.root.join("work");
        let workspace = Workspace::new(&root);
        let outcome = self
            .isolation
            .collect_and_destroy(&self.plan, &workspace, exit, &self.key)
            .await
            .expect("an outcome");
        assert!(outcome.sweep.refused.is_empty(), "{:?}", outcome.sweep);
    }

    /// The data of every frame of the first raw file of `exit`, without the frame times.
    fn frames_of(exit: &Exit, which: usize) -> Vec<String> {
        let info = exit.raw.get(which).expect("a raw file");
        FrameReader::open(&info.path)
            .expect("a frame file")
            .map(|frame| format!("{:?}", frame.expect("a frame").data))
            .collect()
    }

    /// What `docker run --rm` of the same script gives: the exit and the stdout frames.
    async fn run_oracle(&self, mode: &str) -> Exit {
        let wrapped = wrap_expiry(&Self::command(mode), 60).expect("a wrapped command");
        let mut planned = self.plan.clone(); // BORROW: the oracle adds the run's labels
        planned.labels = planned
            .labels
            .with("candler.run", &Marker::for_run(&self.key).to_string());
        let mut argv = create_argv(&planned, &wrapped).expect("an argv");
        // The oracle is the same argv through `run`, removing itself when done.
        if let Some(verb) = argv.get_mut(1) {
            *verb = String::from("run");
        }
        argv.insert(2, String::from("--rm"));
        if let Some(first) = argv.first_mut() {
            *first = String::from("docker");
        }
        let env: Vec<(String, String)> = ["DOCKER_HOST", "DOCKER_CONTEXT", "HOME", "PATH"]
            .iter()
            .filter_map(|name| Some(((*name).to_owned(), std::env::var(name).ok()?))) // BORROW: the env owns its names
            .collect();
        let outputs = self.outputs("oracle");
        let request = SpawnRequest::builder()
            .argv(argv)
            .env(env)
            .marker("create-attach-oracle".parse().expect("a marker"))
            .run(self.key.clone()) // BORROW: the request owns its key
            .stdout(outputs.stdout)
            .stderr(outputs.stderr)
            .deadline(Duration::from_secs(60))
            .record_dir(self.root.join("records"))
            .build()
            .expect("a request");
        self.supervisor
            .spawn(request)
            .expect("docker run starts")
            .wait()
            .await
            .expect("docker run ends")
    }

    /// Waits until the container `id` is running.
    fn until_running(id: &ContainerId) {
        let began = Instant::now();
        loop {
            if Self::running(id) {
                return;
            }
            assert!(began.elapsed() < PATIENCE, "the container never ran");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Whether the container `id` is running.
    fn running(id: &ContainerId) -> bool {
        let out = docker(&["inspect", "--format", "{{.State.Running}}", id.as_str()]); // BORROW: explicit .as_str()
        String::from_utf8_lossy(&out.stdout).trim() == "true"
    }

    /// Whether docker still knows the container `id`.
    fn known(id: &ContainerId) -> bool {
        docker(&["inspect", id.as_str()]).status.success() // BORROW: explicit .as_str()
    }

    /// Removes the world's network by its exact name.
    fn clean(&self) {
        let _ = docker(&["network", "rm", self.network.as_str()]); // BORROW: explicit .as_str()
    }
}

/// Whether `bytes` holds `needle`.
fn holds(bytes: &[u8], needle: &str) -> bool {
    String::from_utf8_lossy(bytes).contains(needle)
}

/// The text of a raw frame file, for a containment check.
fn frames_text(path: &Path) -> String {
    FrameReader::open(path)
        .expect("a frame file")
        .map(|frame| format!("{:?}", frame.expect("a frame").data))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Guards: R01 §Process supervision — the agent's container is created, recorded and attached with
/// `docker start -a`: its exit code (3), its byte-exact standard output and error, its standard
/// input (`-i`), and a signal that reaches the agent's process through the attached client all
/// behave as they do under `docker run`; a stop is one `docker stop` of the container, so the
/// agent's last line after the TERM is in the recorded output and the recorded exit is the
/// container's (143 under a TERM trap), and the container is destroyed by id.
/// Oracle: the frames of `docker run --rm` of the same script on the same image (exit code and
/// frame data without times), the script's own output written by hand, `docker inspect` of the
/// container id after the destroy, and the coordinator's probe table (Engine 29.9.2: trap TERM,
/// stop 0.23 s, client and container exit 143, final line relayed).
/// Bite: `-i` dropped from the interactive start (the stdin row ends with exit code 143 and no
/// echo); the stop signalling the client instead of running `docker stop` (the stop row).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs a docker daemon and the testagent image; run by the container-e2e group"]
async fn agent_create_then_attach() {
    let image = TestImage::new(TESTAGENT_CONTEXT);
    let mut failures = Vec::new();

    // Exit code 3, and byte-exact frames equal to `docker run`'s of the same script.
    for mode in ["exit3", "stream-bytes"] {
        let world = World::new(&image, false);
        let (request, id) = world.made(mode).await;
        let exit = world
            .supervisor
            .spawn(request)
            .expect("the client starts")
            .wait()
            .await
            .expect("the client ends");
        let oracle = world.run_oracle(mode).await;
        let want = if mode == "exit3" { 3 } else { 0 };
        if exit.status != (ExitStatus::Code { code: want }) || oracle.status != exit.status {
            failures.push(format!(
                "{mode}: {:?} against {:?}",
                exit.status, oracle.status
            ));
        }
        for which in 0..exit.raw.len().min(2) {
            if World::frames_of(&exit, which) != World::frames_of(&oracle, which) {
                failures.push(format!("{mode}: stream {which} differs from docker run"));
            }
        }
        world.collect(&exit).await;
        if World::known(&id) {
            failures.push(format!("{mode}: the container is left"));
        }
        world.clean();
    }

    // A session's standard input reaches the agent through `start -a -i`.
    {
        let world = World::new(&image, true);
        let (request, id) = world.made("echo-stdin").await;
        let mut child = world.supervisor.spawn(request).expect("the client starts");
        let mut stdin = child.take_stdin().expect("a stdin pipe");
        stdin.write_all(b"ping\npong\n").await.expect("input");
        drop(stdin);
        let exit = child.wait().await.expect("the client ends");
        let shown = frames_text(&exit.raw.first().expect("a stdout file").path);
        if exit.status != (ExitStatus::Code { code: 0 })
            || !holds(shown.as_bytes(), "echo:ping")
            || !holds(shown.as_bytes(), "echo:pong")
        {
            failures.push(format!("stdin: {:?} {shown}", exit.status));
        }
        world.collect(&exit).await;
        if World::known(&id) {
            failures.push(String::from("stdin: the container is left"));
        }
        world.clean();
    }

    // A stop mid-run is one `docker stop` of the container: the client is not signalled, so it
    // relays what the agent printed after the TERM and exits with the container's code.
    {
        let world = World::new(&image, false);
        let (request, id, facts) = world.made_stoppable("term-echo").await;
        let child = world.supervisor.spawn(request).expect("the client starts");
        World::until_running(&id);
        RunStopper::new(
            child.stop_handle(),
            facts,
            world.key.clone(), // BORROW: the stopper owns its key
            PATIENCE,
        )
        .request();
        let exit = child.wait().await.expect("the client ends");
        let shown = frames_text(&exit.raw.first().expect("a stdout file").path);
        if exit.ended != Ended::Exited
            || exit.status != (ExitStatus::Code { code: 143 })
            || !holds(shown.as_bytes(), "got-term")
        {
            failures.push(format!(
                "stop: ended {:?}, {:?}, output {shown}",
                exit.ended, exit.status
            ));
        }
        world.collect(&exit).await;
        if World::known(&id) {
            failures.push(String::from("stop: the container is left"));
        }
        world.clean();
    }

    // A signal sent to the attached client (no hook) reaches the agent's process, which says so,
    // and the exit fact is the client's own.
    {
        let world = World::new(&image, false);
        let (hooked, id) = world.made("term-echo").await;
        drop(hooked);
        let bare = build_start_request(
            world.isolation.docker().as_ref(),
            &world.plan,
            &id,
            "create-attach-bare".parse().expect("a marker"),
            &world.key,
            None,
            world.outputs("bare"),
        )
        .expect("a request");
        let child = world.supervisor.spawn(bare).expect("the client starts");
        World::until_running(&id);
        let exit = child.stop().await.expect("the stop ends");
        // The client leaves at once on the signal and may not print the agent's last words; the
        // container's own log and exit code say whether the signal reached the agent's process.
        let began = Instant::now();
        while World::running(&id) {
            assert!(began.elapsed() < PATIENCE, "the agent never ended");
            std::thread::sleep(Duration::from_millis(50));
        }
        let logs = docker(&["logs", id.as_str()]); // BORROW: explicit .as_str()
        if !holds(&logs.stdout, "got-term") || exit.ended != Ended::Stopped {
            failures.push(format!("signal: {:?}, log {:?}", exit.status, logs.stdout));
        }
        world.collect(&exit).await;
        if World::known(&id) {
            failures.push(String::from("signal: the container is left"));
        }
        world.clean();
    }

    assert!(failures.is_empty(), "{failures:#?}");
}

/// Guards: a declared server whose start command exits is seen as exited with what it printed,
/// through a real container set up as a run's (the plan's image, mounts, network and `-i`), and
/// the probe's container is gone by id afterwards.
/// Oracle: the literal `server-boom` the command writes (and `sleep 30` running for the other case) to its error stream; `docker inspect` of
/// each container id that carried the test's labels while the probe ran.
/// Bite: skip the destroy after the reads (the container is left).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs a docker daemon and the testagent image; run by the container-e2e group"]
async fn server_that_exits_refuses_the_job_in_docker() {
    let image = TestImage::new(TESTAGENT_CONTEXT);
    let world = World::new(&image, true);
    let command = CommandSpec::builder()
        .argv(
            ["sh", "-c", "echo server-boom >&2; exit 7"]
                .map(String::from)
                .to_vec(),
        )
        .build()
        .expect("a command");
    let done = std::sync::atomic::AtomicBool::new(false);
    let probe = async {
        let first = world
            .isolation
            .start_probe(
                &world.plan,
                &command,
                world.key.clone(), // BORROW: the container is labelled with the key
                Duration::from_secs(2),
            )
            .await;
        let staying = CommandSpec::builder()
            .argv(["sleep", "30"].map(String::from).to_vec())
            .build()
            .expect("a command");
        let second = world
            .isolation
            .start_probe(
                &world.plan,
                &staying,
                world.key.clone(), // BORROW: the container is labelled with the key
                Duration::from_secs(2),
            )
            .await;
        done.store(true, std::sync::atomic::Ordering::SeqCst);
        (first, second)
    };
    // The test allows each container the probe creates, by the id docker lists for its label.
    let allow = async {
        let mut seen: Vec<String> = Vec::new();
        while !done.load(std::sync::atomic::Ordering::SeqCst) {
            let listed = docker(&[
                "ps",
                "-aq",
                "--no-trunc",
                "--filter",
                &format!("label={TEST_LABEL}"),
            ]);
            for id in String::from_utf8_lossy(&listed.stdout).split_whitespace() {
                world.destroy.allow_container(id);
                if !seen.iter().any(|known| known == id) {
                    seen.push(id.to_owned()); // BORROW: the list owns its ids
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        seen
    };
    let ((exited, staying), seen) = tokio::join!(probe, allow);
    world.clean();
    assert_eq!(
        staying.expect("a probe"),
        StartProbe::Running,
        "a command that keeps running, with stdin held open, is reported running"
    );
    let StartProbe::Exited { output } = exited.expect("a probe") else {
        panic!("the server was reported running");
    };
    assert!(output.contains("server-boom"), "the output: {output}");
    assert_eq!(seen.len(), 2, "each probe made one container: {seen:?}");
    for id in &seen {
        assert!(
            !World::known(&id.parse().expect("a container id")),
            "{id} is left"
        );
    }
}
