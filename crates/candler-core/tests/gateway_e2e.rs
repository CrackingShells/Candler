//! The real-Docker guards of the `gateway` leaf: the nginx sidecar, its TLS upstream, its log and
//! its cap, and B17 and B18 (the token stays out of the agent container, which has no route out).
//!
//! Every test builds its own world under a `candler.job=<id>` label, wraps its drivers in the
//! test-side allowlist, and tears down only what carries its own job label. The upstream is a
//! Python-stdlib TLS server (`fixtures/gateway/tls_upstream.py`) with a certificate made by the
//! `openssl` CLI; the only token is a random canary.
#![cfg(test)]

use std::fmt::Write as _;
use std::io::Read as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use candler_core::isolation::docker::DockerError;
use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
use candler_core::isolation::gateway::{
    ChildRecorder, DockerCli, DockerGateway, Gateway, GatewayError, GatewaySignal, GatewayStart,
    GatewayStatus, NotReadyCause, SecretError, SecretSource, SecretValue, UpstreamConfig,
};
use candler_core::isolation::model::{
    BuildRequest, ContainerRef, GatewayHandle, ImageId, ImageInfo, ImageRef, Inspect, InspectState,
    LabelSel, Labels, NetworkInfo, NetworkName,
};
use candler_core::isolation::testing::AllowlistDriver;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::container::ContainerRecord;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{Digest, JobId, RunKey, Timestamp};
use futures::future::BoxFuture;

/// The directory of the sidecar image's build context.
const GATEWAY_CONTEXT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/gateway");

/// The upstream fake, written into the fake's own directory at test time.
const FAKE_SCRIPT: &str = include_str!("fixtures/gateway/tls_upstream.py");

/// The pinned Python image the fake runs in (3.12, slim-bookworm), by digest.
const PYTHON_IMAGE: &str =
    "python@sha256:935c3c5b12357a7e10caa9c1e6ca5692344d97cf229832ef584be2cec9fc03df";

/// The test agent image: busybox with `nc`, `wget` and `sh`.
const AGENT_IMAGE: &str = "candler-testagent:test";

/// The host name the fake's certificate is valid for and the sidecar forwards to.
const UPSTREAM_HOST: &str = "upstream.test";

/// The port the fake serves TLS on.
const UPSTREAM_PORT: u16 = 8443;

/// The name the gateway's token is resolved under.
const SECRET_NAME: &str = "model.token_secret";

/// The cell digest of the run keys these tests use.
const CELL_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

// ---------------------------------------------------------------------------------------------
// The docker command line, as a test uses it.
// ---------------------------------------------------------------------------------------------

/// What a command printed and how it ended.
struct Ran {
    code: Option<i32>,
    out: String,
    err: String,
}

impl Ran {
    fn ok(&self) -> bool {
        self.code == Some(0)
    }

    /// Standard output of a command that must have succeeded.
    fn must(self, what: &str) -> String {
        assert!(
            self.ok(),
            "{what} failed (exit {:?}): {}",
            self.code,
            self.err.trim()
        );
        self.out
    }

    /// Both streams as one text.
    fn both(&self) -> String {
        format!("{}\n{}", self.out, self.err)
    }
}

/// The environment of the docker client: what it needs to find its daemon, and nothing else.
fn client_env() -> Vec<(String, String)> {
    [
        "PATH",
        "HOME",
        "DOCKER_HOST",
        "DOCKER_CONFIG",
        "DOCKER_CONTEXT",
    ]
    .into_iter()
    .filter_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| (String::from(key), value))
    })
    .collect()
}

/// Runs `program` with `args` and `stdin`, and waits for it.
fn run(program: &str, args: &[&str], stdin: &[u8]) -> Ran {
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .envs(client_env())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("`{program}` did not start (is it installed?): {error}"));
    if let Some(mut pipe) = child.stdin.take() {
        // A command that exits without reading its stdin closes the pipe; that is not an error.
        let _ = pipe.write_all(stdin);
    }
    let done = child.wait_with_output().expect("a command to finish");
    Ran {
        code: done.status.code(),
        out: String::from_utf8_lossy(&done.stdout).into_owned(),
        err: String::from_utf8_lossy(&done.stderr).into_owned(),
    }
}

fn docker(args: &[&str]) -> Ran {
    run("docker", args, &[])
}

/// `docker exec <cid> <args>`.
fn exec(cid: &str, args: &[&str]) -> Ran {
    let mut all = vec!["exec", cid];
    all.extend_from_slice(args);
    docker(&all)
}

/// 128 random bits as lowercase hex, from the operating system.
fn random_hex() -> String {
    let mut bytes = [0_u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .expect("random bytes");
    let mut text = String::with_capacity(32);
    for byte in bytes {
        write!(text, "{byte:02x}").expect("a string write");
    }
    text
}

/// A job id with random leading digits: the network name is made from them.
fn random_job() -> JobId {
    let hex = random_hex();
    let at = |from: usize, to: usize| hex.get(from..to).expect("32 hex digits");
    format!(
        "{}-{}-7{}-8{}-{}",
        at(0, 8),
        at(8, 12),
        at(12, 15),
        at(15, 18),
        at(18, 30)
    )
    .parse()
    .expect("a job id")
}

// ---------------------------------------------------------------------------------------------
// The two drivers the tests supply, until the real ones land with container_stop.
// ---------------------------------------------------------------------------------------------

/// The read half of docker, on the command line.
struct CliDocker;

fn failed(what: &str, ran: &Ran) -> DockerError {
    DockerError::Failed {
        detail: format!("{what}: {}", ran.err.trim()),
    }
}

fn unreadable(detail: &str) -> DockerError {
    DockerError::Unreadable {
        detail: String::from(detail),
    }
}

/// The labels of a `Config.Labels` or `Labels` object.
fn labels_of(object: &serde_json::Value) -> Labels {
    let mut labels = Labels::new();
    if let Some(map) = object.as_object() {
        for (key, value) in map {
            // BORROW: explicit .as_str() instead of Deref coercion
            if let Some(text) = value.as_str() {
                labels = labels.with(key, text);
            }
        }
    }
    labels
}

/// A timestamp from docker's text: its fraction is cut to what the schema reads.
fn timestamp_of(text: &str) -> Result<Timestamp, DockerError> {
    let cut = text.split('.').next().unwrap_or(text).trim_end_matches('Z');
    format!("{cut}Z")
        .parse()
        .map_err(|_| unreadable("a timestamp"))
}

fn inspect_blocking(id: &str) -> Result<Inspect, DockerError> {
    let ran = docker(&["inspect", id]);
    if !ran.ok() {
        return Err(failed("docker inspect", &ran));
    }
    let all: serde_json::Value =
        serde_json::from_str(&ran.out).map_err(|_| unreadable("inspect json"))?;
    let first = all.get(0).ok_or_else(|| unreadable("an empty inspect"))?;
    let text = |pointer: &str| {
        first
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .map(String::from)
            .ok_or_else(|| unreadable(pointer))
    };
    let flag = |pointer: &str| first.pointer(pointer).and_then(serde_json::Value::as_bool);
    let state = InspectState::new(
        text("/State/Status")?,
        flag("/State/Running").unwrap_or(false),
        first
            .pointer("/State/ExitCode")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
        flag("/State/OOMKilled").unwrap_or(false),
        None,
        None,
    );
    Ok(Inspect::new(
        text("/Id")?.parse().map_err(|_| unreadable("an id"))?,
        ImageId(text("/Image")?),
        timestamp_of(&text("/Created")?)?,
        labels_of(
            first
                .pointer("/Config/Labels")
                .unwrap_or(&serde_json::Value::Null),
        ),
        state,
    ))
}

impl DockerDriver for CliDocker {
    fn inspect<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Inspect, DockerError>> {
        Box::pin(async move { inspect_blocking(id) })
    }

    fn ps_by_label<'a>(
        &'a self,
        sel: &'a LabelSel,
    ) -> BoxFuture<'a, Result<Vec<ContainerRef>, DockerError>> {
        Box::pin(async move {
            let filter = format!("label={}={}", sel.key, sel.value);
            let ran = docker(&["ps", "-a", "-q", "--no-trunc", "--filter", &filter]);
            if !ran.ok() {
                return Err(failed("docker ps", &ran));
            }
            let mut found = Vec::new();
            for id in ran.out.split_whitespace() {
                let seen = inspect_blocking(id)?;
                found.push(ContainerRef::new(seen.id, seen.labels, seen.created));
            }
            Ok(found)
        })
    }

    fn network_create<'a>(
        &'a self,
        name: &'a NetworkName,
        internal: bool,
        labels: &'a Labels,
    ) -> BoxFuture<'a, Result<(), DockerError>> {
        Box::pin(async move {
            let mut args = vec![String::from("network"), String::from("create")];
            if internal {
                args.push(String::from("--internal"));
            }
            for (key, value) in labels.iter() {
                args.push(String::from("--label"));
                args.push(format!("{key}={value}"));
            }
            args.push(String::from("--label"));
            args.push(String::from("candler.test=gateway"));
            // BORROW: explicit .as_str() instead of Deref coercion
            args.push(String::from(name.as_str()));
            let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
            let ran = docker(&borrowed);
            if ran.ok() {
                Ok(())
            } else {
                Err(failed("docker network create", &ran))
            }
        })
    }

    fn network_inspect<'a>(
        &'a self,
        name: &'a NetworkName,
    ) -> BoxFuture<'a, Result<NetworkInfo, DockerError>> {
        Box::pin(async move {
            // BORROW: explicit .as_str() instead of Deref coercion
            let ran = docker(&["network", "inspect", name.as_str()]);
            if !ran.ok() {
                return Err(failed("docker network inspect", &ran));
            }
            let all: serde_json::Value =
                serde_json::from_str(&ran.out).map_err(|_| unreadable("network json"))?;
            let first = all.get(0).ok_or_else(|| unreadable("an empty network"))?;
            Ok(NetworkInfo {
                // BORROW: .clone() returns the name the caller asked about
                name: name.clone(),
                internal: first
                    .get("Internal")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                labels: labels_of(first.get("Labels").unwrap_or(&serde_json::Value::Null)),
            })
        })
    }

    fn image_inspect<'a>(
        &'a self,
        _reference: &'a str,
    ) -> BoxFuture<'a, Result<Option<ImageInfo>, DockerError>> {
        Box::pin(async {
            Err(DockerError::Unavailable {
                detail: String::from("image_inspect"),
            })
        })
    }

    fn build<'a>(
        &'a self,
        _request: &'a BuildRequest,
    ) -> BoxFuture<'a, Result<ImageInfo, DockerError>> {
        Box::pin(async {
            Err(DockerError::Unavailable {
                detail: String::from("build"),
            })
        })
    }
}

/// The destroy half of docker for these tests. Before it stops or removes anything it reads the
/// object's `candler.job` label and panics unless it is this test's job: a second fence behind
/// the allowlist.
struct CliDestroyer {
    job: String,
}

impl CliDestroyer {
    fn fence(&self, inspect: &[&str], id: &str, format: &str) {
        let mut args = Vec::from(inspect);
        args.extend(["inspect", "--format", format, id]);
        let ran = docker(&args);
        assert_eq!(
            ran.out.trim(),
            self.job,
            "`{id}` does not carry this test's candler.job label"
        );
    }
}

impl DestroyDriver for CliDestroyer {
    fn stop<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        Box::pin(async move {
            self.fence(&[], id, "{{index .Config.Labels \"candler.job\"}}");
            let ran = docker(&["stop", "-t", "1", id]);
            if ran.ok() {
                Ok(())
            } else {
                Err(failed("docker stop", &ran))
            }
        })
    }

    fn rm<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        Box::pin(async move {
            self.fence(&[], id, "{{index .Config.Labels \"candler.job\"}}");
            let ran = docker(&["rm", id]);
            if ran.ok() {
                Ok(())
            } else {
                Err(failed("docker rm", &ran))
            }
        })
    }

    fn network_rm<'a>(&'a self, name: &'a NetworkName) -> BoxFuture<'a, Result<(), DockerError>> {
        Box::pin(async move {
            self.fence(
                &["network"],
                // BORROW: explicit .as_str() instead of Deref coercion
                name.as_str(),
                "{{index .Labels \"candler.job\"}}",
            );
            // BORROW: explicit .as_str() instead of Deref coercion
            let ran = docker(&["network", "rm", name.as_str()]);
            if ran.ok() {
                Ok(())
            } else {
                Err(failed("docker network rm", &ran))
            }
        })
    }
}

// ---------------------------------------------------------------------------------------------
// One test's world.
// ---------------------------------------------------------------------------------------------

/// The sidecar image, built from the repository's build context, tagged `candler-gateway:test`
/// and read back by id.
fn sidecar_image() -> ImageRef {
    static BUILD: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _held = BUILD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    docker(&[
        "build",
        "--quiet",
        "--label",
        "candler.test=gateway",
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
    ImageRef {
        id: ImageId(String::from(id.trim())),
        cc_version: None,
    }
}

/// The test agent image must exist; the tests never pull a tag.
fn require_agent_image() {
    assert!(
        docker(&["image", "inspect", AGENT_IMAGE]).ok(),
        "build it: docker build --label candler.test=gateway -t {AGENT_IMAGE} assets/docker/testagent"
    );
}

/// The Python image, by digest: pulled when absent.
fn require_python_image() {
    if !docker(&["image", "inspect", PYTHON_IMAGE]).ok() {
        docker(&["pull", "--quiet", PYTHON_IMAGE]).must("docker pull of the pinned python image");
    }
}

/// A test's labels, drivers and temporary directory.
struct Rig {
    job: JobId,
    dir: tempfile::TempDir,
    driver: AllowlistDriver<CliDocker>,
    destroyer: AllowlistDriver<CliDestroyer>,
    /// The bridge network the sidecar and the fake share. It is labelled with this job; only its
    /// name comes from another id, because a network name is made from a job id.
    egress: NetworkName,
}

impl Rig {
    fn new() -> Self {
        require_agent_image();
        require_python_image();
        let job = random_job();
        let rig = Self {
            dir: tempfile::tempdir().expect("a temporary directory"),
            driver: AllowlistDriver::new(CliDocker),
            destroyer: AllowlistDriver::new(CliDestroyer {
                // BORROW: explicit .as_str() instead of Deref coercion
                job: String::from(job.as_str()),
            }),
            egress: NetworkName::for_job(&random_job()),
            job,
        };
        docker(&[
            "network",
            "create",
            "--label",
            &format!("candler.job={}", rig.job),
            "--label",
            "candler.test=gateway",
            // BORROW: explicit .as_str() instead of Deref coercion
            rig.egress.as_str(),
        ])
        .must("creating the egress network");
        // BORROW: explicit .as_str() instead of Deref coercion
        rig.destroyer.allow_network(rig.egress.as_str());
        rig
    }

    /// The job's own network, the one the agent joins.
    fn job_network(&self) -> NetworkName {
        NetworkName::for_job(&self.job)
    }

    /// The ids of every container that carries this job's label.
    fn containers(&self) -> Vec<String> {
        let filter = format!("label=candler.job={}", self.job);
        docker(&["ps", "-a", "-q", "--no-trunc", "--filter", &filter])
            .must("docker ps")
            .split_whitespace()
            .map(String::from)
            .collect()
    }

    fn work_dir(&self, name: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        std::fs::create_dir_all(&path).expect("a directory");
        path
    }

    /// Stops and removes what carries this job's label, then the two networks, each through the
    /// allowlisted destroyer.
    fn teardown(&self) {
        for id in self.containers() {
            self.destroyer.allow_container(&id);
            let _ = futures::executor::block_on(self.destroyer.stop(&id));
            let _ = futures::executor::block_on(self.destroyer.rm(&id));
        }
        let job_net = self.job_network();
        for net in [&job_net, &self.egress] {
            // BORROW: explicit .as_str() instead of Deref coercion
            if docker(&["network", "inspect", net.as_str()]).ok() {
                // BORROW: explicit .as_str() instead of Deref coercion
                self.destroyer.allow_network(net.as_str());
                let _ = futures::executor::block_on(self.destroyer.network_rm(net));
            }
        }
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.teardown();
    }
}

/// Discards every event.
struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// A secret source that holds one value.
struct Fixed(String);

impl SecretSource for Fixed {
    fn resolve(&self, _name: &str) -> Result<SecretValue, SecretError> {
        // BORROW: explicit .as_str() instead of Deref coercion
        Ok(SecretValue::new(String::from(self.0.as_str())))
    }
}

/// A random canary token: it stands in for the real one.
fn canary() -> String {
    format!("cnd-canary-{}", random_hex())
}

/// A run key of the shape the agent's base url carries.
fn run_key(attempt: u32) -> RunKey {
    format!("{CELL_HEX}-r1-a{attempt}")
        .parse()
        .expect("a run key")
}

// ---------------------------------------------------------------------------------------------
// The TLS upstream.
// ---------------------------------------------------------------------------------------------

fn path_text(path: &Path) -> String {
    String::from(path.to_str().expect("a utf-8 path"))
}

/// Makes a certificate authority `<name>.pem` with its key `<name>.key` in `dir`.
fn make_ca(dir: &Path, name: &str) {
    run(
        "openssl",
        &[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            &path_text(&dir.join(format!("{name}.key"))),
            "-out",
            &path_text(&dir.join(format!("{name}.pem"))),
            "-days",
            "2",
            "-subj",
            &format!("/CN=candler-gateway-test-{name}"),
            "-addext",
            "basicConstraints=critical,CA:TRUE",
            "-addext",
            "keyUsage=critical,keyCertSign,cRLSign",
        ],
        &[],
    )
    .must("openssl req of a certificate authority");
}

/// Makes the server's key and a certificate for `UPSTREAM_HOST`, signed by the authority `ca`.
fn make_server_cert(dir: &Path, ca: &str) {
    let path = |name: &str| path_text(&dir.join(name));
    run(
        "openssl",
        &[
            "req",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            &path("key.pem"),
            "-out",
            &path("server.csr"),
            "-subj",
            &format!("/CN={UPSTREAM_HOST}"),
        ],
        &[],
    )
    .must("openssl req of the server key");
    std::fs::write(
        dir.join("server.ext"),
        format!(
            "subjectAltName=DNS:{UPSTREAM_HOST}\nbasicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n"
        ),
    )
    .expect("an extension file");
    run(
        "openssl",
        &[
            "x509",
            "-req",
            "-in",
            &path("server.csr"),
            "-CA",
            &path(&format!("{ca}.pem")),
            "-CAkey",
            &path(&format!("{ca}.key")),
            "-CAcreateserial",
            "-out",
            &path("cert.pem"),
            "-days",
            "2",
            "-extfile",
            &path("server.ext"),
        ],
        &[],
    )
    .must("openssl x509 of the server certificate");
}

/// The running upstream fake.
struct Fake {
    cid: String,
    /// The authority the server's certificate chains to.
    ca_pem: Vec<u8>,
    /// Another authority, which signed nothing the fake serves.
    rogue_ca_pem: Vec<u8>,
}

/// Starts the TLS upstream on the job's egress network as `upstream.test`, expecting `token`.
fn start_fake(rig: &Rig, token: &str) -> Fake {
    let dir = rig.work_dir("fake");
    make_ca(&dir, "ca");
    make_ca(&dir, "rogue");
    make_server_cert(&dir, "ca");
    std::fs::write(dir.join("token"), format!("{token}\n")).expect("a token file");
    std::fs::write(dir.join("tls_upstream.py"), FAKE_SCRIPT).expect("the fake script");
    let mount = format!("{}:/fake:ro", dir.display());
    let port = UPSTREAM_PORT.to_string();
    let cid = docker(&[
        "run",
        "-d",
        "--label",
        &format!("candler.job={}", rig.job),
        "--label",
        "candler.role=upstream",
        "--label",
        "candler.test=gateway",
        "--network",
        // BORROW: explicit .as_str() instead of Deref coercion
        rig.egress.as_str(),
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
    let cid = String::from(cid.trim());
    let deadline = Instant::now() + Duration::from_secs(30);
    let probe =
        format!("import socket; socket.create_connection(('127.0.0.1', {UPSTREAM_PORT}), 2)");
    while !exec(&cid, &["python3", "-c", &probe]).ok() {
        assert!(
            Instant::now() < deadline,
            "the upstream fake did not listen"
        );
        std::thread::sleep(Duration::from_millis(300));
    }
    Fake {
        cid,
        ca_pem: std::fs::read(dir.join("ca.pem")).expect("the authority"),
        rogue_ca_pem: std::fs::read(dir.join("rogue.pem")).expect("the other authority"),
    }
}

/// Every request the fake has seen, oldest first.
fn records(fake: &Fake) -> Vec<serde_json::Value> {
    let ran = exec(
        &fake.cid,
        &["sh", "-c", "cat /tmp/records.jsonl 2>/dev/null; true"],
    );
    ran.out
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn field(record: &serde_json::Value, key: &str) -> String {
    record
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(String::from)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// The gateway under test.
// ---------------------------------------------------------------------------------------------

/// A gateway to the fake: `ca` is the authority it trusts, `max_parallel` its cap.
fn gateway(rig: &Rig, ca: &[u8], max_parallel: u32) -> (DockerGateway, Arc<Supervisor>) {
    let sup = Arc::new(Supervisor::new(Arc::new(NullSink)));
    let cli = DockerCli::new(
        client_env(),
        "gateway".parse().expect("a marker"),
        rig.work_dir("records"),
        rig.work_dir("cids"),
    );
    let upstream = UpstreamConfig::builder()
        .host(UPSTREAM_HOST)
        .port(UPSTREAM_PORT)
        .ca_pem(Vec::from(ca))
        .build()
        .expect("an upstream");
    (
        DockerGateway::new(
            sidecar_image(),
            upstream,
            max_parallel,
            cli,
            Arc::clone(&sup),
        ),
        sup,
    )
}

/// Starts the sidecar on the job's internal network with `token`.
async fn start(
    rig: &Rig,
    gw: &DockerGateway,
    sup: &Supervisor,
    token: &str,
) -> Result<GatewayHandle, GatewayError> {
    struct NoRecord;
    impl ChildRecorder for NoRecord {
        fn record_child(&self, _record: &ContainerRecord) -> std::io::Result<()> {
            Ok(())
        }
    }
    let experiment: Digest = format!("sha256:{}", "c".repeat(64))
        .parse()
        .expect("a digest");
    gw.start(
        sup,
        &rig.driver,
        GatewayStart {
            job: &rig.job,
            experiment: &experiment,
            recorder: &NoRecord,
            token: &Fixed(String::from(token)),
            secret: SECRET_NAME,
        },
    )
    .await
}

/// A GET, or a POST of `post`, from inside the sidecar to its own listener, headers included.
fn from_sidecar(handle: &GatewayHandle, path: &str, post: Option<&str>) -> Ran {
    let cid = handle.container.cid.to_string();
    let url = format!("http://127.0.0.1:8080{path}");
    let mut args = vec!["wget", "-q", "-S", "-T", "10", "-O-"];
    if let Some(body) = post {
        args.extend([
            "--header",
            "Content-Type: application/json",
            "--post-data",
            body,
        ]);
    }
    args.push(&url);
    exec(&cid, &args)
}

// ---------------------------------------------------------------------------------------------
// The sidecar against the TLS fake.
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
async fn gateway_tls_sends_the_real_token_with_host_and_sni() {
    let rig = Rig::new();
    let token = canary();
    let fake = start_fake(&rig, &token);
    let (gw, sup) = gateway(&rig, &fake.ca_pem, 4);
    let handle = start(&rig, &gw, &sup, &token)
        .await
        .expect("a started gateway");
    assert_eq!(gw.probe(&handle).await, GatewayStatus::Ready);

    let key = run_key(1);
    let echo = from_sidecar(&handle, &format!("/run/{key}/echo"), None);
    assert!(echo.ok(), "echo: {}", echo.both());
    // The fake echoed `Authorization`; the sidecar's body filter replaced the token.
    assert!(echo.out.contains("Bearer <redacted>"), "body: {}", echo.out);
    assert!(!echo.both().contains(&token));

    let seen = records(&fake);
    let last = seen
        .iter()
        .rev()
        .find(|record| field(record, "path") == "/echo")
        .expect("the echo request reached the fake");
    // Oracle: the fake's own record of the TLS handshake and of the request it received.
    assert_eq!(field(last, "sni"), UPSTREAM_HOST);
    assert_eq!(
        field(last, "host"),
        format!("{UPSTREAM_HOST}:{UPSTREAM_PORT}")
    );
    assert_eq!(field(last, "authorization"), format!("Bearer {token}"));
    assert_eq!(field(last, "x_api_key"), token);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
async fn gateway_tls_untrusted_certificate_fails_closed() {
    let rig = Rig::new();
    let token = canary();
    let fake = start_fake(&rig, &token);
    // The sidecar trusts an authority that signed nothing the fake serves.
    let (gw, sup) = gateway(&rig, &fake.rogue_ca_pem, 4);
    let started = start(&rig, &gw, &sup, &token).await;
    assert!(
        matches!(
            started,
            Err(GatewayError::NotReady {
                cause: NotReadyCause::Upstream(502)
            })
        ),
        "an untrusted certificate must not serve: {started:?}"
    );
    // The handshake failed, so no request reached the fake.
    assert!(records(&fake).is_empty(), "{:?}", records(&fake));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
async fn gateway_tls_serves_nothing_until_the_credential_arrives() {
    let rig = Rig::new();
    let token = canary();
    let fake = start_fake(&rig, &token);
    // The sidecar as `start` makes it, minus the injection: it must wait, and serve nothing.
    let image = sidecar_image();
    let cid = docker(&[
        "run",
        "-d",
        "--init",
        "--read-only",
        "--label",
        &format!("candler.job={}", rig.job),
        "--label",
        "candler.role=gateway",
        "--label",
        "candler.test=gateway",
        "--network",
        // BORROW: explicit .as_str() instead of Deref coercion
        rig.egress.as_str(),
        "--tmpfs",
        "/run/gw:rw,mode=0700,uid=101,gid=101,size=64k,noexec,nosuid",
        "--tmpfs",
        "/tmp:rw,mode=1777,uid=101,gid=101,size=8m,noexec,nosuid",
        "--tmpfs",
        "/var/cache/nginx:rw,mode=0700,uid=101,gid=101,size=64m,noexec,nosuid",
        "-e",
        &format!("GW_UPSTREAM_HOST={UPSTREAM_HOST}"),
        "-e",
        &format!("GW_UPSTREAM_AUTHORITY={UPSTREAM_HOST}:{UPSTREAM_PORT}"),
        "-e",
        "GW_MAX_PARALLEL=4",
        &image.id.0,
    ])
    .must("starting a sidecar without a credential");
    let cid = String::from(cid.trim());
    std::thread::sleep(Duration::from_secs(4));
    let state = docker(&["inspect", "--format", "{{.State.Running}}", &cid]).must("inspect");
    assert_eq!(
        state.trim(),
        "true",
        "the entrypoint waits, it does not exit early"
    );
    let models = exec(
        &cid,
        &[
            "wget",
            "-q",
            "-T",
            "2",
            "-O-",
            "http://127.0.0.1:8080/v1/models",
        ],
    );
    let health = exec(
        &cid,
        &[
            "wget",
            "-q",
            "-T",
            "2",
            "-O-",
            "http://127.0.0.1:8080/healthz",
        ],
    );
    assert!(
        !models.ok() && !health.ok(),
        "nothing may listen before the credential"
    );
    assert_eq!(records(&fake).len(), 0, "the fake saw a request");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
async fn gateway_tls_restart_empties_the_credential_and_reinject_restores_it() {
    let rig = Rig::new();
    let token = canary();
    let fake = start_fake(&rig, &token);
    let (gw, sup) = gateway(&rig, &fake.ca_pem, 4);
    let handle = start(&rig, &gw, &sup, &token)
        .await
        .expect("a started gateway");
    let served = records(&fake).len();
    assert!(served >= 1, "the readiness probe reached the fake");

    let cid = handle.container.cid.to_string();
    docker(&["restart", "-t", "1", &cid]).must("docker restart");
    // The tmpfs is empty again: the sidecar waits for its credential and forwards nothing.
    let down = from_sidecar(&handle, "/v1/models", None);
    assert!(!down.ok(), "served without a credential: {}", down.both());
    assert_eq!(records(&fake).len(), served, "the fake saw a request");

    // BORROW: explicit .as_str() instead of Deref coercion
    gw.reinject(&handle, &Fixed(String::from(token.as_str())), SECRET_NAME)
        .await
        .expect("a re-injected credential");
    let up = from_sidecar(&handle, "/v1/models", None);
    assert!(up.ok(), "{}", up.both());
    assert!(records(&fake).len() > served);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
async fn gateway_log_line_carries_the_model_and_never_the_credential() {
    let rig = Rig::new();
    let token = canary();
    let fake = start_fake(&rig, &token);
    let (gw, sup) = gateway(&rig, &fake.ca_pem, 4);
    let handle = start(&rig, &gw, &sup, &token)
        .await
        .expect("a started gateway");
    let key = run_key(1);
    let model = format!("m-{}", random_hex());
    let body = format!("{{\"model\":\"{model}\"}}");
    let sent = from_sidecar(&handle, &format!("/run/{key}/v1/messages"), Some(&body));
    assert!(sent.ok(), "{}", sent.both());

    let log = gw.logs().read(&handle).await.expect("the logs");
    // Oracle: the access log is the only place the model can be read from. The quoted-tilde form
    // of the map passes `nginx -t` and matches nothing, so `model=-` here would be that defect.
    let line = log
        .lines()
        .find(|line| line.contains(&format!("model={model}")))
        .unwrap_or_else(|| panic!("no log line carries the model; log:\n{log}"));
    assert!(line.contains(&format!("run={key}")), "{line}");
    assert!(
        line.contains("method=POST") && line.contains("status=200"),
        "{line}"
    );
    assert!(!log.contains(&token) && !log.contains("Bearer") && !log.contains("Authorization"));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
async fn gateway_log_cap_answers_429_once_and_the_run_signal_reports_it() {
    let rig = Rig::new();
    let token = canary();
    let fake = start_fake(&rig, &token);
    let (gw, sup) = gateway(&rig, &fake.ca_pem, 2);
    let handle = start(&rig, &gw, &sup, &token)
        .await
        .expect("a started gateway");
    let key = run_key(7);
    let path = format!("/run/{key}/slow");
    // N + 1 simultaneous calls at a cap of N = 2: the fake holds each for three seconds.
    let answers: Vec<Ran> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..3)
            .map(|_| scope.spawn(|| from_sidecar(&handle, &path, None)))
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().expect("a worker"))
            .collect()
    });
    let capped = answers
        .iter()
        .filter(|ran| ran.both().contains("429"))
        .count();
    let served = answers.iter().filter(|ran| ran.ok()).count();
    assert_eq!(
        (capped, served),
        (1, 2),
        "{:?}",
        answers.iter().map(Ran::both).collect::<Vec<_>>()
    );

    let signals = gw.run_signals(&handle, &key).await.expect("the signals");
    assert_eq!(signals, vec![GatewaySignal::UpstreamStatus(429)]);
    // The other run saw nothing.
    let other = gw
        .run_signals(&handle, &run_key(8))
        .await
        .expect("the signals");
    assert_eq!(other.len(), 0);
}
