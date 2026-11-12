//! The real-Docker guards of session mode: a real Claude Code, in the agent base image under the
//! production container hardening, behind the production gateway sidecar, against the scripted
//! fake model (`assets/docker/fakemodel`), driven by the real run loop. No token is spent and no
//! real model is called.
//!
//! The world is the job's: a gateway sidecar on the job's networks, the fake model as the
//! upstream on the egress network, and a `DockerIsolation` over them. Each test removes, by exact
//! id or name, the containers and networks it made.
#![cfg(test)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use candler_core::isolation::Isolation;
use candler_core::isolation::container::guard::ContainerGuard;
use candler_core::isolation::container::sweeper::LabelSweeper;
use candler_core::isolation::container::{DockerDirs, DockerIsolation};
use candler_core::isolation::docker::{ClientEnv, DockerCli as DockerClient};
use candler_core::isolation::driver::DockerDriver;
use candler_core::isolation::gateway::{
    DockerCli, DockerGateway, Gateway, GatewayStart, SecretError, SecretSource, SecretValue,
    UpstreamConfig,
};
use candler_core::isolation::model::NetworkName;
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::view::JobView;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{Digest, RunKey};
use serde_json::{Value, json};

#[path = "../support/gateway.rs"]
mod gateway_support;
#[path = "../support/session_rig.rs"]
mod rig;

use crate::support::job_orchestrate::Rig;
use gateway_support::{GATEWAY_CONTEXT, SECRET_NAME, UPSTREAM_PORT, canary, client_env, docker};

/// The context of the fake model image.
const FAKE_CONTEXT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/fakemodel");

/// The reference of the agent base image the harness runs in: the build of `assets/docker/base`
/// tags it `candler-base:src-<digest of its sources>`. The digest is private to the image service,
/// and building the base downloads the harness binary, so the test uses the base the machine
/// already holds and says how to make one when it holds none.
const BASE_REFERENCE: &str = "candler-base:src-*";

/// The name the fake model's certificate is for.
const FAKE_HOST: &str = "fake-anthropic.test";

/// Discards every event.
struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// A secret source that holds one value.
struct Fixed(String);

impl SecretSource for Fixed {
    fn resolve(&self, _name: &str) -> Result<SecretValue, SecretError> {
        Ok(SecretValue::new(self.0.clone()))
    }
}

/// The id of the image built from `context` as `tag`.
fn build(context: &str, tag: &str) -> String {
    docker(&[
        "build",
        "--quiet",
        "--label",
        "candler.test=session",
        "-t",
        tag,
        context,
    ])
    .must("docker build");
    String::from(
        docker(&["image", "inspect", "--format", "{{.Id}}", tag])
            .must("docker image inspect")
            .trim(),
    )
}

/// What a test made in docker besides the agent container, which the isolation destroys.
struct Made {
    containers: Vec<String>,
    networks: Vec<NetworkName>,
}

impl Drop for Made {
    fn drop(&mut self) {
        for cid in &self.containers {
            let _ = docker(&["rm", "-f", cid]);
        }
        for net in &self.networks {
            let _ = docker(&["network", "rm", net.as_str()]);
        }
    }
}

/// Waits until `check` holds, or fails the test with `what`.
fn wait_for(what: &str, limit: Duration, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + limit;
    while !check() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Starts the fake model; its certificate authority is `<dir>/ca.pem` on the host once it listens.
fn run_fake(dir: &Path, token: &str, image: &str, job: &str) -> String {
    let mount = format!("{}:/fake", dir.display());
    let cid = docker(&[
        "run",
        "-d",
        "--label",
        "candler.test=session",
        "--label",
        &format!("candler.session_test={job}"),
        "-e",
        &format!("FAKE_TOKEN={token}"),
        "-e",
        &format!("FAKE_HOST={FAKE_HOST}"),
        "-e",
        "FAKE_DIR=/fake",
        "-v",
        &mount,
        image,
    ])
    .must("starting the fake model");
    String::from(cid.trim())
}

/// The world of one test: the job's gateway, the fake model and the isolation over them.
struct World {
    rig: Rig,
    image: String,
    isolation: Arc<dyn Isolation>,
    _made: Made,
}

/// A directory of the test's own, made under its temporary directory.
fn work_dir(rig: &Rig, name: &str) -> PathBuf {
    let path = rig.dir.path().join(name);
    std::fs::create_dir_all(&path).expect("a directory");
    path
}

impl World {
    async fn new() -> Self {
        let rigged = Rig::new(0);
        let job = rigged.job.clone();
        let base = String::from(
            docker(&[
                "images",
                "--no-trunc",
                "--format",
                "{{.ID}}",
                "--filter",
                &format!("reference={BASE_REFERENCE}"),
            ])
            .must("docker images")
            .lines()
            .next()
            .expect(
                "an agent base image: run `candler preflight` once to build assets/docker/base",
            ),
        );
        let gateway_image = build(GATEWAY_CONTEXT, "candler-gateway:test");
        let fake_image = build(FAKE_CONTEXT, "candler-fakemodel:test");
        let dir = work_dir(&rigged, "fake");
        std::fs::set_permissions(&dir, std::os::unix::fs::PermissionsExt::from_mode(0o777))
            .expect("a writable directory");
        let token = canary();
        let fake = run_fake(&dir, &token, &fake_image, &job.to_string());
        let mut made = Made {
            containers: vec![fake.clone()],
            networks: vec![
                NetworkName::for_job(&job),
                NetworkName::egress_for_job(&job),
            ],
        };
        wait_for(
            "the fake model made no authority",
            Duration::from_secs(60),
            || dir.join("ca.pem").is_file(),
        );
        let ca = std::fs::read(dir.join("ca.pem")).expect("the authority");
        let sink: Arc<dyn EventSink> = Arc::new(NullSink);
        let sup = Arc::new(Supervisor::new(sink));
        let job_dir = JobDir::open(rigged.store.layout(), &job).expect("the job directory");
        let cli = Arc::new(
            DockerClient::new(
                Arc::clone(&sup),
                ClientEnv::from_process(),
                job_dir.control_dir(),
            )
            .expect("a docker client"),
        );
        let view: Arc<dyn JobView> = Arc::new(job_dir.clone());
        let guard = Arc::new(ContainerGuard::over_docker(&cli, Arc::clone(&view)));
        let driver: Arc<dyn DockerDriver> = cli.clone();
        let sweeper = Arc::new(LabelSweeper::new(
            Arc::clone(&driver),
            Arc::clone(&guard),
            view,
            Duration::from_secs(60),
        ));
        let gateway_cli = DockerCli::new(
            client_env(),
            "session".parse().expect("a marker"),
            work_dir(&rigged, "records"),
            work_dir(&rigged, "cids"),
        );
        let upstream = UpstreamConfig::builder()
            .host(FAKE_HOST)
            .port(UPSTREAM_PORT)
            .ca_pem(ca)
            .build()
            .expect("an upstream");
        let gw = DockerGateway::new(
            candler_core::isolation::model::ImageRef {
                id: candler_core::isolation::model::ImageId(gateway_image),
                harness_version: None,
            },
            upstream,
            4,
            gateway_cli,
            Arc::clone(&sup),
        );
        // The fake joins the egress network, as the upstream, once the gateway has made it.
        let egress = NetworkName::egress_for_job(&job);
        let joiner = {
            let (egress, fake) = (egress.clone(), fake.clone());
            tokio::task::spawn_blocking(move || {
                wait_for(
                    "the egress network never appeared",
                    Duration::from_secs(60),
                    || docker(&["network", "inspect", egress.as_str()]).ok(),
                );
                docker(&[
                    "network",
                    "connect",
                    "--alias",
                    FAKE_HOST,
                    egress.as_str(),
                    &fake,
                ])
                .must("joining the fake to the egress network");
            })
        };
        let experiment: Digest = format!("sha256:{}", "cd".repeat(32))
            .parse()
            .expect("a digest");
        let started = gw
            .start(
                &sup,
                cli.as_ref(),
                GatewayStart {
                    job: &job,
                    experiment: &experiment,
                    recorder: &job_dir,
                    token: &Fixed(token),
                    secret: SECRET_NAME,
                },
            )
            .await;
        joiner.await.expect("the join's thread");
        let handle = started.expect("a started gateway");
        made.containers.push(handle.container.cid.to_string());
        let store_root = rigged.store.layout().root().to_path_buf();
        let isolation: Arc<dyn Isolation> = Arc::new(
            DockerIsolation::new(handle.clone(), driver, guard)
                .with_sweeper(sweeper)
                .with_recorder(Arc::new(job_dir.clone()))
                .with_dirs(DockerDirs {
                    children: job_dir.children_dir(),
                    project_root: store_root.clone(),
                    store_root,
                }),
        );
        Self {
            rig: rigged,
            image: base,
            isolation,
            _made: made,
        }
    }
}

/// The cell of a session over the fake model's script `model`.
fn cell(image: &str, model: &str, idle_s: u64) -> Vec<(String, Value)> {
    let mut values = rig::cell_values(&[
        ("isolation.image", json!(image)),
        ("model.id", json!(model)),
        ("runner.idle_s", json!(idle_s)),
        ("budget.per_run.wall_s", json!(240)),
    ]);
    values.retain(|(path, _)| path != "budget.per_run.turns");
    values
}

/// A sealed run: what the run loop returned, its manifest and its directory.
struct Done {
    result: Result<candler_core::runner::RunOutcome, candler_core::runner::exec::ExecError>,
    manifest: Value,
}

impl Done {
    fn outcome(&self) -> &candler_core::runner::RunOutcome {
        self.result
            .as_ref()
            .unwrap_or_else(|error| panic!("the run failed: {error}"))
    }

    fn measured(&self, path: &str) -> Option<Value> {
        self.manifest["measured"]
            .as_array()?
            .iter()
            .find(|entry| entry["path"] == path)
            .map(|entry| entry["value"].clone())
    }

    fn number(&self, path: &str) -> f64 {
        self.measured(path)
            .and_then(|value| value.as_f64())
            .unwrap_or_else(|| panic!("{path} is recorded"))
    }
}

/// Runs the cell of the fake model's script `model` on a fresh world.
async fn run_session(model: &str, idle_s: u64) -> Done {
    let world = World::new().await;
    let values = cell(&world.image, model, idle_s);
    let (result, manifest, _run_dir, _rig) = rig::execute(
        world.rig,
        world.isolation,
        &values,
        rig::Driver::new(true),
        None,
    )
    .await;
    Done { result, manifest }
}

/// Guards: R01 §Session runner, M4 D2 and D3 — a real Claude Code that has answered once, with
/// nothing pending, is closed `idle_s` after its turn's end: the run is valid and completed,
/// records one turn, no pending task, `session.end = closed` with no forced close, and lasted at
/// least the time to yield plus `idle_s`, and not many seconds more.
/// Oracle: the fake model's script `c2` ("hello" to every request), the `idle_s` the cell sets and
/// the run's own `run.duration_ms` and `session.yielded_at_ms`.
/// Bite: leave `-i` out of the container's command line (the harness reads its end of input at
/// once, and the run records no turn).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
async fn session_e2e_c2_closes_at_idle() {
    let idle = 2_u64;
    let done = run_session("c2", idle).await;
    let outcome = done.outcome();
    assert!(
        outcome.invalid.is_none(),
        "the run is valid: {:?}",
        outcome.invalid
    );
    assert_eq!(
        done.measured("run.ended_by"),
        Some(json!({"kind": "completed"}))
    );
    assert_eq!(done.measured("session.turns"), Some(json!(1)));
    assert_eq!(done.measured("session.pending_at_end"), Some(json!([])));
    assert_eq!(done.measured("session.end"), Some(json!("closed")));
    assert_eq!(done.measured("session.close_forced"), Some(json!(false)));
    let (yielded, duration) = (
        done.number("session.yielded_at_ms"),
        done.number("run.duration_ms"),
    );
    let idle_ms = f64::from(u32::try_from(idle).expect("a small idle")) * 1000.0;
    assert!(
        duration >= yielded + idle_ms && duration < yielded + idle_ms + 8000.0,
        "the run lasted {duration} ms: yielded at {yielded} ms, idle {idle_ms} ms"
    );
}

/// Guards: R01 §Session runner, M4 D1 and D2 — a real Claude Code that started a background job
/// and waits for its completion is not closed while the job is pending: the run records two turns
/// (the one that started the job and the one the completion woke), an empty pending list and
/// `session.end = closed`; the last turn ends after the 20 s job and the input closes `idle_s` later.
/// Oracle: the fake model's script `c1` (a Bash call starting a 20 s background job that writes
/// `job.done`, a text, then a second text when the job's completion arrives).
/// Bite: close at the first yield (one turn, a pending task in the list, the run lasts about 5 s).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
async fn session_e2e_c1_waits_then_closes() {
    let done = run_session("c1", 3).await;
    let outcome = done.outcome();
    assert!(
        outcome.invalid.is_none(),
        "the run is valid: {:?}",
        outcome.invalid
    );
    assert_eq!(
        done.measured("run.ended_by"),
        Some(json!({"kind": "completed"}))
    );
    assert_eq!(done.measured("session.turns"), Some(json!(2)));
    assert_eq!(done.measured("session.pending_at_end"), Some(json!([])));
    assert_eq!(done.measured("session.end"), Some(json!("closed")));
    assert_eq!(done.measured("session.close_forced"), Some(json!(false)));
    let (yielded, duration) = (
        done.number("session.yielded_at_ms"),
        done.number("run.duration_ms"),
    );
    assert!(
        yielded >= 20_000.0,
        "the last turn ended at {yielded} ms, after the 20 s job"
    );
    assert!(
        duration >= yielded + 3000.0 && duration < yielded + 3000.0 + 8000.0,
        "the run lasted {duration} ms: yielded at {yielded} ms, idle 3000 ms"
    );
}
