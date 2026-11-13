//! Helpers for the guards of the supervision gates: builders and fixtures, never expected values.
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use candler_core::isolation::docker::ClientEnv;
use candler_core::isolation::driver::DockerDriver;
use candler_core::isolation::gateway::{
    DockerCli as GatewayCli, DockerGateway, Gateway, GatewayStart, GatewayStatus, SecretError,
    SecretSource, SecretValue, UpstreamConfig,
};
use candler_core::isolation::model::{ImageId, ImageRef, NetworkName};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::model::{JobCause, JobControl, JobOutcome, JobSpec};
use candler_core::runner::Runtime;
use candler_core::supervise::spawn::SpawnRequest;
use candler_schemas::execution::JobEnd;
use candler_schemas::primitives::{Digest, JobId, Marker, RunKey};
use candler_schemas::process::ProcessStart;
use futures::future::BoxFuture;

use super::gateway::{
    AGENT_IMAGE, CELL_HEX, EXPERIMENT, FAKE_SCRIPT, GATEWAY_CONTEXT, PYTHON_IMAGE, SECRET_NAME,
    UPSTREAM_HOST, UPSTREAM_PORT, docker, make_ca, make_server_cert,
};

/// The job the re-executed supervisor serves.
pub const ENV_JOB: &str = "CANDLER_TEST_GATES_JOB";
/// The store the supervisor child opens.
pub const ENV_STORE: &str = "CANDLER_TEST_GATES_STORE";
/// The directory that holds the child's files: the upstream's certificates and the ready file.
pub const ENV_WORK: &str = "CANDLER_TEST_GATES_WORK";
/// The id of the gateway's sidecar image.
pub const ENV_SIDECAR: &str = "CANDLER_TEST_GATES_SIDECAR";
/// The token the gateway presents upstream: the canary of the test.
pub const ENV_TOKEN: &str = "CANDLER_TEST_GATES_TOKEN";

/// The label every container the test makes carries, so a manual cleanup stays scoped.
pub const TEST_LABEL: &str = "candler.test=supervision_gates";
/// The run key the agent container carries.
pub fn run_key() -> RunKey {
    format!("{CELL_HEX}-r1-a1").parse().expect("a run key")
}

/// Whether the daemon answers.
pub fn daemon_up() -> bool {
    docker(&["info", "--format", "{{.ServerVersion}}"]).ok()
}

/// The images the test needs must exist; it never pulls a tag, and pulls the pinned python image
/// by digest when it is absent.
pub fn require_images() {
    assert!(
        docker(&["image", "inspect", AGENT_IMAGE]).ok(),
        "build it: docker build --label candler.test=supervision_gates -t {AGENT_IMAGE} assets/docker/testagent"
    );
    if !docker(&["image", "inspect", PYTHON_IMAGE]).ok() {
        docker(&["pull", "--quiet", PYTHON_IMAGE]).must("docker pull of the pinned python image");
    }
}

/// The id of the sidecar image, built from the repository's build context.
pub fn sidecar_image_id() -> String {
    docker(&[
        "build",
        "--quiet",
        "--label",
        TEST_LABEL,
        "-t",
        "candler-gateway:test",
        GATEWAY_CONTEXT,
    ])
    .must("docker build of the gateway image");
    let id = docker(&[
        "image",
        "inspect",
        "--format",
        "{{.Id}}",
        "candler-gateway:test",
    ])
    .must("docker image inspect");
    String::from(id.trim())
}

/// The ids of every container, running or not, that carries the label of `job`.
pub fn labelled(job: &JobId) -> Vec<String> {
    let filter = format!("label=candler.job={job}");
    docker(&["ps", "-a", "-q", "--no-trunc", "--filter", &filter])
        .must("docker ps")
        .split_whitespace()
        .map(String::from)
        .collect()
}

/// The ids of the containers of `job` that are running now.
pub fn running(job: &JobId) -> Vec<String> {
    let filter = format!("label=candler.job={job}");
    docker(&["ps", "-q", "--no-trunc", "--filter", &filter])
        .must("docker ps")
        .split_whitespace()
        .map(String::from)
        .collect()
}

/// The value of the label `key` that docker reports on `cid`, if the container exists.
pub fn label_of(cid: &str, key: &str) -> Option<String> {
    let format = format!("{{{{index .Config.Labels \"{key}\"}}}}");
    let ran = docker(&["inspect", "--format", &format, cid]);
    ran.ok().then(|| String::from(ran.out.trim()))
}

/// Whether docker knows a network called `name`.
pub fn network_exists(name: &NetworkName) -> bool {
    // BORROW: explicit .as_str() instead of Deref coercion
    docker(&["network", "inspect", name.as_str()]).ok()
}

/// Makes the upstream's certificate authority `ca.pem` and a server certificate `cert.pem` for
/// `UPSTREAM_HOST` in `dir`, with the `openssl` CLI.
pub fn make_certs(dir: &Path) {
    std::fs::create_dir_all(dir).expect("a certificate directory");
    make_ca(dir, "ca");
    make_server_cert(dir, "ca");
}

/// Starts the TLS upstream as `upstream.test` on the job's egress network, expecting `token`,
/// carrying the label of `job`, so that the lost-job sweep reaches it. The network does not exist
/// until the gateway's `start` creates it, so this waits for it; run it beside the supervisor.
/// Returns the container's id.
pub fn run_fake(job: &JobId, dir: &Path, token: &str) -> String {
    std::fs::write(dir.join("token"), format!("{token}\n")).expect("a token file");
    std::fs::write(dir.join("tls_upstream.py"), FAKE_SCRIPT).expect("the fake script");
    let egress = NetworkName::egress_for_job(job);
    let deadline = Instant::now() + Duration::from_secs(90);
    while !network_exists(&egress) {
        assert!(
            Instant::now() < deadline,
            "the egress network never appeared"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let mount = format!("{}:/fake:ro", dir.display());
    let port = UPSTREAM_PORT.to_string();
    let cid = docker(&[
        "run",
        "-d",
        "--label",
        &format!("candler.job={job}"),
        "--label",
        // The guard knows two roles; the fake stands in for the agent's role.
        "candler.role=agent",
        "--label",
        &format!("candler.run={}", run_key()),
        "--label",
        TEST_LABEL,
        "--network",
        // BORROW: explicit .as_str() instead of Deref coercion
        egress.as_str(),
        "--network-alias",
        UPSTREAM_HOST,
        "-v",
        &mount,
        PYTHON_IMAGE,
        "python3",
        "/fake/tls_upstream.py",
        "/fake/cert.pem",
        "/fake/key.pem",
        "/fake/token",
        &port,
        "/tmp/records.jsonl",
    ])
    .must("starting the upstream fake");
    String::from(cid.trim())
}

/// A secret source that holds the token the supervisor child was given.
struct EnvToken;

impl SecretSource for EnvToken {
    fn resolve(&self, _name: &str) -> Result<SecretValue, SecretError> {
        let value = std::env::var(ENV_TOKEN).map_err(|_| SecretError::Missing {
            // BORROW: .to_owned() gives the error the name, never a value
            name: SECRET_NAME.to_owned(),
        })?;
        Ok(SecretValue::new(value))
    }
}

/// The body of the supervisor child of B36. It starts the job's gateway on the job's own networks
/// through the real service, then starts an agent container through a docker client that the
/// supervisor spawns, marked with the run and recorded in the job's `children/`, and writes
/// `ready` once both serve. It then waits for a cancel it never gets: the test SIGKILLs it.
pub fn gates_body(
    rt: Runtime,
    _spec: JobSpec,
    control: JobControl,
) -> BoxFuture<'static, JobOutcome> {
    Box::pin(async move {
        let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is unset"));
        let work = PathBuf::from(var(ENV_WORK));
        let dir = JobDir::open(rt.store.layout(), &rt.job).expect("the job directory");
        let mut env: Vec<(String, String)> = ClientEnv::from_process().pairs().to_vec();
        if let Some(path) = std::env::var_os("PATH") {
            // BORROW: the child's environment owns its pair
            env.push(("PATH".to_owned(), path.to_string_lossy().into_owned()));
        }
        let marker: Marker = "gateway".parse().expect("a marker");
        let cli = GatewayCli::new(
            env.clone(), // BORROW: the gateway's client owns its environment
            marker,
            dir.control_dir(),
            dir.children_dir(),
        );
        let upstream = UpstreamConfig::builder()
            .host(UPSTREAM_HOST)
            .port(UPSTREAM_PORT)
            .ca_pem(std::fs::read(work.join("certs").join("ca.pem")).expect("the authority"))
            .build()
            .expect("an upstream");
        let image = ImageRef {
            id: ImageId(var(ENV_SIDECAR)),
            harness_version: None,
        };
        let service = DockerGateway::new(image, upstream, 2, cli, Arc::clone(&rt.supervisor));
        let experiment: Digest = EXPERIMENT.parse().expect("a digest");
        let docker: &dyn DockerDriver = &*rt.docker; // TRAIT_OBJECT: the driver is docker
        let handle = service
            .start(
                &rt.supervisor,
                docker,
                GatewayStart {
                    job: &rt.job,
                    experiment: &experiment,
                    recorder: &dir,
                    token: &EnvToken,
                    secret: SECRET_NAME,
                },
            )
            .await
            .expect("a started gateway");
        assert!(
            matches!(service.probe(&handle).await, GatewayStatus::Ready),
            "the gateway does not serve"
        );
        let key = run_key();
        let argv: Vec<String> = [
            "docker",
            "run",
            "--init",
            "--label",
            &format!("candler.job={}", rt.job),
            "--label",
            "candler.role=agent",
            "--label",
            &format!("candler.run={key}"),
            "--label",
            TEST_LABEL,
            "--network",
            // BORROW: explicit .as_str() instead of Deref coercion
            handle.network.as_str(),
            AGENT_IMAGE,
            "sleep-forever",
        ]
        .into_iter()
        // BORROW: .to_owned() builds the owned argv
        .map(str::to_owned)
        .collect();
        let request = SpawnRequest::builder()
            .argv(argv)
            .env(env)
            // BORROW: the marker is the run key, as a run's own processes carry it
            .marker(key.to_string().parse().expect("a marker"))
            .run(key)
            .record_dir(dir.children_dir())
            .build()
            .expect("a request");
        let agent = rt.supervisor.spawn(request).expect("the agent's client");
        let part = work.join("ready.part");
        std::fs::write(&part, format!("{}\n", agent.record().pid())).expect("the ready file");
        std::fs::rename(&part, work.join("ready")).expect("the ready file in place");
        let mut cancel = control.cancel.subscribe();
        loop {
            if matches!(*cancel.borrow_and_update(), Some(JobCause::Killed)) {
                break;
            }
            if cancel.changed().await.is_err() {
                break;
            }
        }
        JobOutcome {
            end: JobEnd::Completed,
            runs: Vec::new(),
        }
    })
}

/// The process ids that carry `CANDLER_RUN=<marker>` and are alive now. On Linux they are read
/// from `/proc/<pid>/environ`, every process of the user. Elsewhere the operating system gives no
/// process environment to a test, so the ids of `known`, the processes a test saw carry the
/// marker, are the answer when they are still the process that started at the recorded time.
/// Returns an error when the listing cannot be read, so an empty answer never stands for a failed
/// look.
#[allow(
    clippy::unnecessary_wraps,
    reason = "the Linux listing can fail, so an `expect` would be unfulfilled on Linux"
)]
pub fn marked(marker: &str, known: &[(u32, ProcessStart)]) -> Result<Vec<u32>, String> {
    #[cfg(target_os = "linux")]
    {
        let wanted = format!("CANDLER_RUN={marker}");
        let _ = known;
        let mut found = Vec::new();
        let listing = std::fs::read_dir("/proc").map_err(|error| error.to_string())?;
        for entry in listing.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u32>().ok())
            else {
                continue;
            };
            // A process of another user, or one that just ended, has no readable environment.
            let Ok(bytes) = std::fs::read(entry.path().join("environ")) else {
                continue;
            };
            if bytes
                .split(|byte| *byte == 0)
                .any(|var| var == wanted.as_bytes())
            {
                found.push(pid);
            }
        }
        Ok(found)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = marker;
        Ok(known
            .iter()
            .filter(|(pid, start)| !super::job_runtime::gone(*pid, start))
            .map(|(pid, _)| *pid)
            .collect())
    }
}
