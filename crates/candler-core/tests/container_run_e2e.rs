//! The real-Docker guards of the container run: what the kernel says about the agent container.
//!
//! Every container and network a test makes carries `candler.job=<test id>` and
//! `candler.test=container_run`. The test reads the id from its own cidfile, checks that the
//! container's labels name the test, and only then allows it in the [`AllowlistDriver`]; the
//! driver refuses any other id. Every destroy goes through the [`ContainerGuard`]; nothing here
//! removes a container by name or prefix.
#![cfg(test)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use candler_core::isolation::container::expiry::wrap_expiry;
use candler_core::isolation::container::guard::{ContainerGuard, Destroyed, GuardError};
use candler_core::isolation::container::mounts::{MountMode, MountSpec};
use candler_core::isolation::container::run::run_argv;
use candler_core::isolation::docker::{ClientEnv, DockerCli, DockerDestroyer};
use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
use candler_core::isolation::model::{
    ContainerPlan, ContainerRef, ImageId, ImageRef, Labels, Limits, NetworkName, SweepScope,
    UserSpec,
};
use candler_core::isolation::testing::{AllowlistDriver, FakeJobView};
use candler_core::supervise::spawn::SpawnRequest;
use candler_core::supervise::stop::Captured;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::spec::CommandSpec;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{Digest, JobId, RunKey};
use candler_schemas::process::ExitStatus;
use serde_json::Value;

/// The testagent image, built from `assets/docker/testagent`.
const TESTAGENT: &str = "candler-testagent:test";

/// A debian-slim python image on the real coreutils `timeout`, pulled by its index digest.
const DEBIAN_SLIM: &str =
    "python@sha256:935c3c5b12357a7e10caa9c1e6ca5692344d97cf229832ef584be2cec9fc03df";

/// The label every test container and network carries, so a manual cleanup stays scoped.
const TEST_LABEL: &str = "candler.test=container_run";

/// The marker of the test children.
const MARKER: &str = "m2b-container-run-e2e";

/// The names the docker client reads from its environment.
const CLIENT_ENV: [&str; 3] = ["DOCKER_HOST", "DOCKER_CONTEXT", "HOME"];

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// Runs `docker <args>` and returns its output, which a test reads and never trusts.
fn docker(args: &[&str]) -> std::process::Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("the docker CLI runs")
}

/// Pulls `reference` by digest when the daemon lacks it.
fn ensure_image(reference: &str) {
    if !docker(&["image", "inspect", reference]).status.success() {
        let pulled = docker(&["pull", "--quiet", reference]);
        assert!(
            pulled.status.success(),
            "docker pull {reference}: {}",
            String::from_utf8_lossy(&pulled.stderr)
        );
    }
}

/// A number that differs between tests and between runs of a test.
fn unique() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let low = u64::try_from(nanos & u128::from(u64::MAX)).unwrap_or(0);
    low.rotate_left(17)
        ^ (u64::from(std::process::id()) << 32)
        ^ COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// One test's job id, network, labels, scratch directories and cleanup.
struct Rig {
    job: JobId,
    key: RunKey,
    network: NetworkName,
    base: PathBuf,
    _tmp: tempfile::TempDir,
    driver: Arc<AllowlistDriver<DockerDestroyer>>,
    guard: ContainerGuard,
    created: Mutex<Vec<String>>,
}

impl Rig {
    fn new() -> Self {
        let bits = unique();
        // The first twelve hex digits name the network, so they are the unique ones.
        let job: JobId = format!(
            "{:08x}-{:04x}-7000-8000-000000000000",
            bits >> 32,
            bits & 0xffff
        )
        .parse()
        .expect("a version 7 job id");
        let cell: Digest = format!("sha256:{}", "b".repeat(64))
            .parse()
            .expect("a digest");
        let tmp = tempfile::tempdir().expect("a scratch directory");
        let base = tmp.path().canonicalize().expect("a canonical path");
        let control = base.join("control");
        std::fs::create_dir_all(&control).expect("a control directory");
        // TRAIT_OBJECT: the supervisor takes its sink as a trait object
        let sink: Arc<dyn EventSink> = Arc::new(Silent);
        let cli = DockerCli::new(
            Arc::new(Supervisor::new(sink)),
            ClientEnv::from_process(),
            control,
        )
        .expect("a docker client");
        let driver = Arc::new(AllowlistDriver::new(cli.destroyer_for_tests()));
        // TRAIT_OBJECT: the guard holds its drivers as trait objects
        let destroy: Arc<dyn DestroyDriver> = driver.clone(); // BORROW: .clone() shares the allowlist
        // TRAIT_OBJECT: the guard holds its drivers as trait objects
        let reads: Arc<dyn DockerDriver> = Arc::new(cli);
        let guard = ContainerGuard::new(destroy, reads, FakeJobView::new(job.clone()).shared()); // BORROW: .clone() gives the view its own id
        let rig = Self {
            key: RunKey::new(cell, 1, 1).expect("a run key"),
            network: NetworkName::for_job(&job),
            job,
            base,
            _tmp: tmp,
            driver,
            guard,
            created: Mutex::new(Vec::new()),
        };
        let created = docker(&[
            "network",
            "create",
            "--internal",
            "--label",
            TEST_LABEL,
            "--label",
            &format!("candler.job={}", rig.job),
            // BORROW: `.as_str()` borrows the text of the network name
            rig.network.as_str(),
        ]);
        assert!(
            created.status.success(),
            "docker network create: {}",
            String::from_utf8_lossy(&created.stderr)
        );
        // BORROW: `.as_str()` borrows the text of the network name
        rig.driver.allow_network(rig.network.as_str());
        rig
    }

    /// A directory under the test's scratch space.
    fn dir(&self, name: &str) -> PathBuf {
        let path = self.base.join(name);
        std::fs::create_dir_all(&path).expect("a directory");
        path
    }

    /// A plan of the test's own: the image, the mounts and the environment are the test's.
    fn plan(
        &self,
        image: &str,
        mounts: Vec<MountSpec>,
        env: Vec<(String, String)>,
    ) -> ContainerPlan {
        ContainerPlan {
            image: ImageRef {
                // BORROW: `.to_owned()` makes the owned id the image reference holds
                id: ImageId(image.to_owned()),
                cc_version: None,
            },
            labels: Labels::new()
                .with("candler.job", &self.job.to_string())
                .with("candler.run", &self.key.to_string())
                .with("candler.role", "agent")
                .with("candler.test", "container_run"),
            mounts,
            env,
            // BORROW: the spec owns its network name
            network: self.network.clone(),
            egress: None,
            limits: Limits {
                pids_limit: 128,
                memory: String::from("256m"),
                cpus: 1.0,
            },
            cidfile: self.base.join("cids").join(format!("{:x}.cid", unique())),
            user: UserSpec::Engine,
            workdir: String::from("/work"),
            home: String::from("/home/agent"),
            expiry: Duration::from_secs(10),
            // BORROW: the spec owns its path
            project_root: self.base.clone(),
            // BORROW: the spec owns its path
            store_root: self.base.clone(),
        }
    }

    /// Runs `argv` (a docker argv from `run_argv`) to its end, under the deadline `deadline`.
    async fn run(
        &self,
        mut argv: Vec<String>,
        deadline: Duration,
        plan: &ContainerPlan,
    ) -> Captured {
        let cids = plan.cidfile.parent().expect("a cidfile directory");
        std::fs::create_dir_all(cids).expect("the cidfile directory");
        if let Some(first) = argv.first_mut() {
            *first = String::from("docker");
        }
        let env: Vec<(String, String)> = CLIENT_ENV
            .iter()
            // BORROW: `.to_owned()` makes the owned variable name the environment holds
            .filter_map(|name| Some(((*name).to_owned(), std::env::var(name).ok()?)))
            .chain([(
                String::from("PATH"),
                std::env::var("PATH").unwrap_or_default(),
            )])
            .collect();
        let request = SpawnRequest::builder()
            .argv(argv)
            .env(env)
            .marker(MARKER.parse().expect("a marker"))
            // BORROW: the run key moves into the request
            .run(self.key.clone())
            .deadline(deadline)
            .record_dir(self.dir("records"))
            .build()
            .expect("a request");
        let sink: Arc<dyn EventSink> = Arc::new(Silent);
        let captured = Supervisor::new(sink)
            .run_captured(request, 64 * 1024)
            .await
            .expect("docker run ends");
        self.adopt(&plan.cidfile);
        captured
    }

    /// Reads the container id from the cidfile, checks that its labels name this test and allows
    /// it in the driver. Returns the id.
    fn adopt(&self, cidfile: &Path) -> String {
        let id = std::fs::read_to_string(cidfile)
            .expect("docker wrote the cidfile")
            .trim()
            // BORROW: `.to_owned()` makes the owned id the test keeps
            .to_owned();
        assert!(!id.is_empty(), "the cidfile is empty");
        let labels = docker(&["inspect", "--format", "{{json .Config.Labels}}", &id]);
        assert!(labels.status.success(), "the container is inspectable");
        let labels: BTreeMap<String, String> =
            serde_json::from_slice(&labels.stdout).expect("a label map");
        assert_eq!(
            // BORROW: `.as_str()` borrows the text for the comparison
            labels.get("candler.job").map(String::as_str),
            // BORROW: `.as_str()` borrows the text of the job id
            Some(self.job.to_string().as_str()),
            "the container is not this test's"
        );
        assert_eq!(
            labels.get("candler.test").map(String::as_str),
            Some("container_run")
        );
        self.driver.allow_container(&id);
        // BORROW: the list owns its id
        self.created.lock().expect("a lock").push(id.clone());
        id
    }

    /// The `docker inspect` of an id, as JSON, or `None` when docker no longer knows it.
    fn inspect(id: &str) -> Option<Value> {
        let output = docker(&["inspect", id]);
        if !output.status.success() {
            return None;
        }
        let parsed: Value = serde_json::from_slice(&output.stdout).expect("inspect JSON");
        parsed.get(0).cloned()
    }
}

impl Rig {
    /// Destroys the container `id` through the guard, within the test's run: the guard asks a
    /// record of the current job's container unless its run is the scope's run, and the test
    /// writes no record.
    async fn destroy(&self, id: &str) -> Result<Destroyed, GuardError> {
        let seen = self
            .guard
            .reader()
            .inspect(id)
            .await
            .expect("the container is inspectable");
        let container = ContainerRef::new(seen.id, seen.labels, seen.created);
        self.guard
            .destroy(&container, &SweepScope::Run(self.key.clone())) // BORROW: .clone() gives the scope its own key
            .await
    }

    /// Removes the test's containers and its network, all through the guard.
    async fn clean(&self) {
        let ids: Vec<String> = self
            .created
            .lock()
            // BORROW: the copy of the list outlives the lock
            .map(|ids| ids.clone())
            .unwrap_or_default();
        for id in ids {
            if Self::inspect(&id).is_some() {
                let _ = self.destroy(&id).await;
            }
        }
        let _ = self
            .guard
            .destroy_network(&self.network, &SweepScope::CurrentJob)
            .await;
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(self.clean())),
            Err(_) => futures::executor::block_on(self.clean()),
        }
    }
}

fn command(argv: &[&str]) -> CommandSpec {
    CommandSpec::builder()
        // BORROW: `.to_owned()` makes each part an owned argument
        .argv(argv.iter().map(|part| (*part).to_owned()).collect())
        .build()
        .expect("a command")
}

/// Puts `--entrypoint ""` before the image, because the testagent image carries an entrypoint
/// of its own and the argv hands it the command to run.
fn without_entrypoint(argv: Vec<String>, image: &str) -> Vec<String> {
    let at = argv
        .iter()
        .position(|part| part == image)
        .expect("the image is in the argv");
    let mut out = argv;
    out.splice(at..at, [String::from("--entrypoint"), String::new()]);
    out
}

/// The user the container runs as: the host user on a Linux engine, the image's own elsewhere.
#[cfg(target_os = "linux")]
fn container_user() -> UserSpec {
    UserSpec::Host {
        uid: rustix::process::getuid().as_raw(),
        gid: rustix::process::getgid().as_raw(),
    }
}

/// The user the container runs as: the host user on a Linux engine, the image's own elsewhere.
#[cfg(not(target_os = "linux"))]
const fn container_user() -> UserSpec {
    UserSpec::Engine
}

/// Guards: B06 — the argv hardening holds in the kernel.
/// Oracle: the testagent probe JSON and the daemon's `HostConfig` of the container; the probe is
/// read from `/proc/self/status` and from real writes, and its literals are the kernel's: `CapEff`
/// is a 64-bit mask, so no capability is `0000000000000000` (capabilities(7)), `NoNewPrivs` is `1`
/// once `no_new_privs` is set (`Documentation/userspace-api/no_new_privs`), and a write to a
/// read-only mount fails with EROFS.
/// Bite: drop `--cap-drop ALL`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs a Docker daemon and the candler-testagent:test image"]
async fn b06_kernel_facts() {
    ensure_image(TESTAGENT);
    let rig = Rig::new();
    let workspace = rig.dir("workspace");
    let home = rig.dir("home");
    let input = rig.dir("input");
    // The sentinel sits in the host's scratch directory and is never mounted.
    let sentinel = rig.base.join("sentinel-never-mounted");
    std::fs::write(&sentinel, b"host only").expect("a sentinel");
    let mounts = vec![
        MountSpec {
            source: workspace,
            target: String::from("/work"),
            mode: MountMode::ReadWrite,
        },
        MountSpec {
            source: home,
            target: String::from("/home/agent"),
            mode: MountMode::ReadWrite,
        },
        MountSpec {
            source: input,
            target: String::from("/input"),
            mode: MountMode::ReadOnly,
        },
    ];
    let env = vec![
        (String::from("HOME"), String::from("/home/agent")),
        (String::from("PROBE_RO"), String::from("/input")),
        (
            String::from("PROBE_SENTINEL"),
            sentinel.to_string_lossy().into_owned(),
        ),
    ];
    let mut plan = rig.plan(TESTAGENT, mounts, env);
    plan.user = container_user();
    let wrapped = wrap_expiry(&command(&["/usr/local/bin/testagent", "probe"]), 60)
        .expect("a wrapped command");
    let argv = without_entrypoint(run_argv(&plan, &wrapped).expect("an argv"), TESTAGENT);
    let captured = rig.run(argv, Duration::from_secs(40), &plan).await;
    assert_eq!(
        captured.exit.status,
        ExitStatus::Code { code: 0 },
        "stderr: {}",
        String::from_utf8_lossy(&captured.stderr)
    );
    let text = String::from_utf8(captured.stdout).expect("UTF-8");
    let line = text.lines().last().expect("a probe line");
    let probe: Value = serde_json::from_str(line).expect("the probe JSON");

    // INDEX: the probe prints the keys read here
    assert_ne!(probe["euid"], 0, "the agent is not root: {probe}");
    assert_eq!(probe["cap_eff"], "0000000000000000", "{probe}");
    assert_eq!(probe["no_new_privs"], "1", "{probe}");
    assert_eq!(probe["write"]["root"], "erofs", "{probe}");
    assert_eq!(probe["write"]["ro"], "erofs", "{probe}");
    assert_eq!(probe["write"]["tmp"], "ok", "{probe}");
    assert_eq!(probe["write"]["home"], "ok", "{probe}");
    assert_eq!(probe["write"]["workspace"], "ok", "{probe}");
    assert_eq!(probe["sentinel_exists"], false, "{probe}");

    // A non-root process holds no effective capability whatever the argv says, so the probe
    // cannot see `--cap-drop`; the daemon's own record of the container does.
    let id = rig
        .created
        .lock()
        .expect("a lock")
        .first()
        .cloned()
        .expect("a container");
    let inspected = Rig::inspect(&id).expect("the container is inspectable");
    assert_eq!(
        // INDEX: `docker inspect` prints the keys read here
        inspected["HostConfig"]["CapDrop"],
        serde_json::json!(["ALL"])
    );
    assert_eq!(
        // INDEX: `docker inspect` prints the keys read here
        inspected["HostConfig"]["ReadonlyRootfs"],
        true,
        "{inspected}"
    );
}

/// Guards: B08 — the inner timeout expires a container the supervisor does not stop.
/// Oracle: exit code 124 of `docker run` on the real debian-slim base, the status GNU coreutils
/// `timeout` documents for a command it timed out; the container reads `exited` in
/// `docker inspect` until the allowlisted destroy removes it.
/// Bite: remove the timeout wrapper.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs a Docker daemon and the debian-slim image, pulled by digest"]
async fn b08_self_expiry() {
    ensure_image(DEBIAN_SLIM);
    let rig = Rig::new();
    let mut plan = rig.plan(DEBIAN_SLIM, Vec::new(), Vec::new());
    plan.expiry = Duration::from_secs(2);
    // The test margin is a literal 2 s, not the production margin, so the inner limit is 4 s.
    let wrapped = wrap_expiry(&command(&["sleep", "300"]), 4).expect("a wrapped command");
    let argv = run_argv(&plan, &wrapped).expect("an argv");
    // The supervisor's deadline is deliberately longer than the inner limit.
    let began = std::time::Instant::now();
    let captured = rig.run(argv, Duration::from_secs(60), &plan).await;
    let took = began.elapsed();
    assert_eq!(
        captured.exit.status,
        ExitStatus::Code { code: 124 },
        "stderr: {}",
        String::from_utf8_lossy(&captured.stderr)
    );
    assert!(took < Duration::from_secs(40), "the run took {took:?}");

    let id = rig
        .created
        .lock()
        .expect("a lock")
        .first()
        .cloned()
        .expect("a container");
    let before = Rig::inspect(&id).expect("the container outlives its process");
    // INDEX: `docker inspect` prints the keys read here
    assert_eq!(before["State"]["Status"], "exited", "{before}");
    assert_eq!(before["State"]["ExitCode"], 124, "{before}");

    rig.destroy(&id).await.expect("the destroy");
    assert!(
        Rig::inspect(&id).is_none(),
        "the container is gone after the destroy"
    );
}

/// Guards: B09 — files written in the container belong to the host uid.
/// Oracle: the stat uid of the files on the host, and a chmod of the run directory that succeeds.
/// Bite: omit `--user <uid>:<gid>`.
#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs a Linux Docker engine"]
async fn b09_host_uid_sealable() {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    ensure_image(DEBIAN_SLIM);
    let rig = Rig::new();
    let workspace = rig.dir("workspace");
    let mounts = vec![MountSpec {
        // BORROW: the spec owns its path
        source: workspace.clone(),
        target: String::from("/work"),
        mode: MountMode::ReadWrite,
    }];
    let mut plan = rig.plan(DEBIAN_SLIM, mounts, Vec::new());
    plan.user = container_user();
    let wrapped = wrap_expiry(
        &command(&[
            "sh",
            "-c",
            "touch /work/made && mkdir /work/sub && touch /work/sub/inner",
        ]),
        60,
    )
    .expect("a wrapped command");
    let argv = run_argv(&plan, &wrapped).expect("an argv");
    let captured = rig.run(argv, Duration::from_secs(40), &plan).await;
    assert_eq!(
        captured.exit.status,
        ExitStatus::Code { code: 0 },
        "stderr: {}",
        String::from_utf8_lossy(&captured.stderr)
    );
    let host_uid = rustix::process::getuid().as_raw();
    for name in ["made", "sub", "sub/inner"] {
        let meta = std::fs::metadata(workspace.join(name)).expect("a file the container made");
        assert_eq!(meta.uid(), host_uid, "{name}");
    }
    let mode = std::fs::metadata(&workspace)
        .expect("the run directory")
        .permissions()
        .mode();
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(mode & 0o555))
        .expect("the host can seal the run directory");
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(mode))
        .expect("the host can unseal it again");
}
