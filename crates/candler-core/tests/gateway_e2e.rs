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
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use candler_core::isolation::container::guard::ContainerGuard;
use candler_core::isolation::docker::{ClientEnv, DockerCli as DockerClient, DockerDestroyer};
use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
use candler_core::isolation::gateway::{
    ChildRecorder, DockerCli, DockerGateway, Gateway, GatewayStart, SecretError, SecretSource,
    SecretValue, UpstreamConfig, assert_clean,
};
use candler_core::isolation::model::{
    GatewayHandle, ImageId, ImageRef, LabelSel, NetworkName, SweepScope,
};
use candler_core::isolation::testing::{AllowlistDriver, FakeJobView};
use candler_core::jobs::view::ChildEntry;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::container::{ContainerRecord, ContainerRole};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{Digest, JobId, RunKey};

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

/// Takes the records `start` writes, as the job directory would.
#[derive(Default)]
struct Recorder {
    seen: std::sync::Mutex<Vec<ContainerRecord>>,
}

impl ChildRecorder for Recorder {
    fn record_child(&self, record: &ContainerRecord) -> std::io::Result<()> {
        self.seen
            .lock()
            .expect("a lock")
            // BORROW: .clone() keeps the record for the assertions
            .push(record.clone());
        Ok(())
    }
}

/// A test's job label, the real docker client, the allowlisted destroy half behind the guard,
/// and a temporary directory. Nothing is removed but through the guard.
struct Rig {
    job: JobId,
    /// The run the test's agent, fake and control carry in `candler.run`.
    run: RunKey,
    dir: tempfile::TempDir,
    sup: Arc<Supervisor>,
    cli: Arc<DockerClient>,
    destroyer: Arc<AllowlistDriver<DockerDestroyer>>,
    /// What the gateway's `start` recorded: the job directory's children.
    recorder: Recorder,
}

impl Rig {
    fn new() -> Self {
        require_agent_image();
        require_python_image();
        let dir = tempfile::tempdir().expect("a temporary directory");
        std::fs::create_dir_all(dir.path().join("control")).expect("a control directory");
        // TRAIT_OBJECT: the supervisor takes its sink as a trait object
        let sink: Arc<dyn EventSink> = Arc::new(NullSink);
        let sup = Arc::new(Supervisor::new(sink));
        let cli = DockerClient::new(
            Arc::clone(&sup),
            ClientEnv::from_process(),
            dir.path().join("control"),
        )
        .expect("a docker client");
        let destroyer = Arc::new(AllowlistDriver::new(cli.destroyer_for_tests()));
        Self {
            job: random_job(),
            run: run_key(1),
            dir,
            sup,
            cli: Arc::new(cli),
            destroyer,
            recorder: Recorder::default(),
        }
    }

    /// The job's egress network, which `start` creates for the sidecar and the fake.
    fn egress_network(&self) -> NetworkName {
        NetworkName::egress_for_job(&self.job)
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

    /// The guard over the real client, deciding from the records `start` wrote.
    fn guard(&self) -> ContainerGuard {
        let children = self
            .recorder
            .seen
            .lock()
            .expect("a lock")
            .iter()
            // BORROW: .clone() gives the view its own records
            .map(|record| ChildEntry::Container(record.clone()))
            .collect();
        // BORROW: .clone() gives the view its own job
        let view = FakeJobView::new(self.job.clone())
            .with_children(children)
            .shared();
        // TRAIT_OBJECT: the guard holds its drivers as trait objects
        let destroy: Arc<dyn DestroyDriver> = self.destroyer.clone(); // BORROW: .clone() shares the allowlist
        // TRAIT_OBJECT: the guard holds its drivers as trait objects
        let docker: Arc<dyn DockerDriver> = self.cli.clone(); // BORROW: .clone() shares the client
        ContainerGuard::new(destroy, docker, view)
    }

    /// Destroys every container that carries this job's label, then the two networks, each
    /// through the guard. The gateway is recorded, so the guard finds it in the current job; the
    /// others carry the test's run. A failure is printed and does not stop the rest.
    async fn teardown(&self) {
        let guard = self.guard();
        let sel = LabelSel {
            key: String::from("candler.job"),
            value: self.job.to_string(),
        };
        let listed = self.cli.ps_by_label(&sel).await.unwrap_or_default();
        for container in listed {
            self.destroyer.allow_container(container.cid.as_str()); // BORROW: .as_str() names the container
            let scope = if container.labels.get("candler.role") == Some("gateway") {
                SweepScope::CurrentJob
            } else {
                SweepScope::Run(self.run.clone()) // BORROW: .clone() gives the scope its own key
            };
            if let Err(error) = guard.destroy(&container, &scope).await {
                eprintln!("teardown: {} was not destroyed: {error}", container.cid);
            }
        }
        for net in [self.job_network(), self.egress_network()] {
            self.destroyer.allow_network(net.as_str()); // BORROW: .as_str() names the network
            if let Err(error) = guard.destroy_network(&net, &SweepScope::CurrentJob).await {
                eprintln!("teardown: {} was not destroyed: {error}", net.as_str());
            }
        }
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.teardown());
        });
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

/// The upstream's certificates: the authority the server's certificate chains to.
struct Certs {
    dir: PathBuf,
    ca_pem: Vec<u8>,
}

/// Makes the fake's authority and server certificate with the `openssl` CLI.
fn make_certs(rig: &Rig) -> Certs {
    let dir = rig.work_dir("fake");
    make_ca(&dir, "ca");
    make_server_cert(&dir, "ca");
    Certs {
        ca_pem: std::fs::read(dir.join("ca.pem")).expect("the authority"),
        dir,
    }
}

/// The running upstream fake.
struct Fake {
    cid: String,
}

/// Starts the TLS upstream as `upstream.test` on the job's egress network, expecting `token`.
/// The network does not exist until the gateway's `start` creates it, so this waits for it; call
/// it beside `start`.
fn run_fake(job: &JobId, run: &RunKey, egress: &NetworkName, dir: &Path, token: &str) -> Fake {
    std::fs::write(dir.join("token"), format!("{token}\n")).expect("a token file");
    std::fs::write(dir.join("tls_upstream.py"), FAKE_SCRIPT).expect("the fake script");
    let deadline = Instant::now() + Duration::from_secs(60);
    // BORROW: explicit .as_str() instead of Deref coercion
    while !docker(&["network", "inspect", egress.as_str()]).ok() {
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
        // The guard knows two roles; the fake stands in for the agent's role and carries the
        // test's run, so a sweep of that run reaches it.
        "candler.role=agent",
        "--label",
        &format!("candler.run={run}"),
        "--label",
        "candler.test=gateway",
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
    Fake { cid }
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
fn gateway(rig: &Rig, ca: &[u8], max_parallel: u32) -> DockerGateway {
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
    DockerGateway::new(
        sidecar_image(),
        upstream,
        max_parallel,
        cli,
        Arc::clone(&rig.sup),
    )
}

/// The experiment digest the sidecar is labelled with.
const EXPERIMENT: &str = "sha256:cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";

/// A world with a gateway up: the upstream fake, the gateway, its handle and what it recorded.
struct World {
    fake: Fake,
    gw: DockerGateway,
    handle: GatewayHandle,
}

/// Starts the gateway with `token` and, beside it, the fake upstream the gateway's readiness
/// probe needs. `start` creates both networks and the sidecar.
async fn up(rig: &Rig, token: &str) -> World {
    let certs = make_certs(rig);
    let gw = gateway(rig, &certs.ca_pem, 4);
    let experiment: Digest = EXPERIMENT.parse().expect("a digest");
    let fake_task = {
        // BORROW: .clone() moves owned copies to the blocking thread
        let (job, run, egress, dir, secret) = (
            rig.job.clone(),
            rig.run.clone(),
            rig.egress_network(),
            certs.dir.clone(),
            String::from(token),
        );
        tokio::task::spawn_blocking(move || run_fake(&job, &run, &egress, &dir, &secret))
    };
    let started = gw
        .start(
            &rig.sup,
            rig.cli.as_ref(),
            GatewayStart {
                job: &rig.job,
                experiment: &experiment,
                recorder: &rig.recorder,
                token: &Fixed(String::from(token)),
                secret: SECRET_NAME,
            },
        )
        .await;
    let fake = fake_task.await.expect("the fake's thread");
    World {
        fake,
        gw,
        handle: started.expect("a started gateway"),
    }
}

// ---------------------------------------------------------------------------------------------
// The agent container.
// ---------------------------------------------------------------------------------------------

/// The agent: the test agent image on the job's internal network.
struct Agent {
    cid: String,
    workspace: PathBuf,
}

/// Starts an agent that runs `script` under `sh`, holding `api_key` as `ANTHROPIC_API_KEY` and
/// the run's base url as `ANTHROPIC_BASE_URL`, with a bind-mounted workspace and a tmpfs HOME.
fn start_agent(
    rig: &Rig,
    handle: &GatewayHandle,
    run: &RunKey,
    api_key: &str,
    script: &str,
) -> Agent {
    let workspace = rig.work_dir("workspace");
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o777))
        .expect("a world-writable workspace");
    let base_url = handle.run_base_url(run);
    let cid = docker(&[
        "run",
        "-d",
        "--init",
        "--label",
        &format!("candler.job={}", rig.job),
        "--label",
        "candler.role=agent",
        "--label",
        &format!("candler.run={}", rig.run),
        "--label",
        "candler.test=gateway",
        "--network",
        // BORROW: explicit .as_str() instead of Deref coercion
        handle.network.as_str(),
        "--add-host",
        "host.docker.internal:host-gateway",
        "--user",
        "1000:1000",
        "-e",
        &format!("ANTHROPIC_API_KEY={api_key}"),
        "-e",
        &format!("ANTHROPIC_BASE_URL={base_url}"),
        "-e",
        "HOME=/home/agent",
        "--tmpfs",
        "/home/agent:rw,mode=1777",
        "-v",
        &format!("{}:/workspace", workspace.display()),
        "-w",
        "/workspace",
        "--entrypoint",
        "sh",
        AGENT_IMAGE,
        "-c",
        script,
    ])
    .must("starting the agent");
    Agent {
        cid: String::from(cid.trim()),
        workspace,
    }
}

/// Waits until the agent's script has written `/workspace/done`.
fn wait_done(agent: &Agent) {
    let marker = agent.workspace.join("done");
    let deadline = Instant::now() + Duration::from_secs(60);
    while !marker.exists() {
        assert!(Instant::now() < deadline, "the agent did not finish");
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// The bytes of every file under `dir`, one after the other.
fn read_tree(dir: &Path) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut pending = vec![PathBuf::from(dir)];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).expect("a directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                bytes.extend(std::fs::read(&path).expect("a file"));
            }
        }
    }
    bytes
}

/// The agent's side of one HTTP/1.0 call through the gateway, written to `$3`.
const AGENT_REQUEST: &str = r#"
P=${ANTHROPIC_BASE_URL#http://gateway:8080}
req() {
  { printf '%s %s HTTP/1.0\r\nHost: gateway\r\nAuthorization: Bearer %s\r\nx-api-key: %s\r\nContent-Length: 0\r\n\r\n' "$1" "$P$2" "$ANTHROPIC_API_KEY" "$ANTHROPIC_API_KEY"; sleep 2; } | nc -w 5 gateway 8080 > "$3"
}
"#;

/// The python that calls the fake directly, as a client on the bridge network would, with the
/// credential given as argument one. It prints the status.
const DIRECT_CALL: &str = "import ssl, http.client, sys\n\
c = ssl.create_default_context()\n\
c.check_hostname = False\n\
c.verify_mode = ssl.CERT_NONE\n\
h = http.client.HTTPSConnection('127.0.0.1', 8443, context=c)\n\
h.request('GET', '/v1/models', headers={'Authorization': 'Bearer ' + sys.argv[1], 'x-api-key': sys.argv[1]})\n\
print(h.getresponse().status)\n";

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
/// Guards: B17 — the real token never enters the agent container.
/// Oracle: a random canary with `assert_clean` over the agent's env, `/proc/1/environ`, every
/// process's environ and command line, the workspace, HOME, every labelled container's
/// `docker inspect` and the gateway logs, while the upstream echoes `Authorization` in a 200 and in
/// an error page and the canary demonstrably reached the upstream.
/// Bite: put the real token in the agent's `-e ANTHROPIC_API_KEY`.
async fn b17_canary_absent() {
    let rig = Rig::new();
    let token = canary();
    let world = up(&rig, &token).await;
    let (fake, handle) = (&world.fake, &world.handle);
    let run = run_key(1);

    // The sidecar was recorded once, with its labels, before it held the token.
    let record = {
        let recorded = rig.recorder.seen.lock().expect("a lock");
        assert_eq!(recorded.len(), 1, "one container record");
        // BORROW: .clone() releases the lock before the awaits below
        recorded.first().expect("the record").clone()
    };
    assert_eq!(record.cid(), &handle.container.cid);
    assert_eq!(record.role(), ContainerRole::Gateway);
    assert_eq!(record.labels().job, rig.job);
    assert_eq!(record.labels().run.to_string(), "gateway");
    assert_eq!(record.labels().experiment.to_string(), EXPERIMENT);
    let sidecar_labels = docker(&[
        "inspect",
        "--format",
        "{{json .Config.Labels}}",
        &handle.container.cid.to_string(),
    ])
    .must("docker inspect of the sidecar");
    let sidecar_labels: serde_json::Value =
        serde_json::from_str(sidecar_labels.trim()).expect("labels json");
    for (key, want) in [
        ("candler.job", rig.job.to_string()),
        ("candler.role", String::from("gateway")),
        ("candler.run", String::from("gateway")),
        ("candler.experiment", String::from(EXPERIMENT)),
    ] {
        assert_eq!(field(&sidecar_labels, key), want, "label {key}");
    }
    let egress = rig
        .cli
        .network_inspect(&rig.egress_network())
        .await
        .expect("the egress network");
    assert!(!egress.internal, "the egress network is not internal");

    let script = format!(
        "{AGENT_REQUEST}
req GET /echo /workspace/echo.txt
req GET /error-page /workspace/error.txt
req GET /v1/models /workspace/models.txt
echo profile > \"$HOME/.agent-profile\"
echo agent-ready
echo done > /workspace/done
exec sleep 600
"
    );
    let agent_key = &handle.dummy_token;
    let agent = start_agent(&rig, handle, &run, agent_key, &script);
    wait_done(&agent);

    // The agent holds the dummy and what the gateway let back through.
    let env = exec(&agent.cid, &["env"]).must("env");
    assert!(
        env.contains(&handle.dummy_token),
        "the dummy is the agent's token"
    );

    // The canary really travelled: the upstream got it, so its absence below is not vacuous.
    let seen = records(fake);
    assert!(
        seen.iter().any(|r| field(r, "path").ends_with("/echo")
            && field(r, "authorization") == format!("Bearer {token}")),
        "the real token never reached the upstream: {seen:?}"
    );
    let echo =
        String::from_utf8_lossy(&std::fs::read(agent.workspace.join("echo.txt")).expect("echo"))
            .into_owned();
    let error =
        String::from_utf8_lossy(&std::fs::read(agent.workspace.join("error.txt")).expect("error"))
            .into_owned();
    assert!(
        echo.contains("200") && echo.contains("Bearer <redacted>"),
        "{echo}"
    );
    assert!(
        error.contains("500") && error.contains("<redacted>"),
        "{error}"
    );

    // The dummy opens nothing at the upstream; a direct call with it is refused.
    let direct = exec(
        &fake.cid,
        &["python3", "-c", DIRECT_CALL, &handle.dummy_token],
    )
    .must("direct call");
    assert_eq!(direct.trim(), "401", "the dummy token authenticated");

    let proc_files = exec(
        &agent.cid,
        &[
            "sh",
            "-c",
            "cat /proc/[0-9]*/environ /proc/[0-9]*/cmdline 2>/dev/null | tr '\\0' '\\n'",
        ],
    )
    .out;
    let pid1 = exec(
        &agent.cid,
        &["sh", "-c", "tr '\\0' '\\n' < /proc/1/environ"],
    )
    .out;
    let home = exec(
        &agent.cid,
        &[
            "sh",
            "-c",
            "cat /home/agent/.agent-profile; ls -la /home/agent",
        ],
    )
    .out;
    let agent_log = docker(&["logs", &agent.cid]).both();
    let workspace = read_tree(&agent.workspace);
    let logs = world
        .gw
        .logs()
        .read(handle)
        .await
        .expect("the gateway logs");
    let inspects: Vec<(String, String)> = rig
        .containers()
        .into_iter()
        .map(|id| {
            let text = docker(&["inspect", &id]).must("docker inspect");
            (format!("docker inspect {id}"), text)
        })
        .collect();

    let mut haystacks: Vec<(&str, &[u8])> = vec![
        ("agent env", env.as_bytes()),
        ("agent /proc/1/environ", pid1.as_bytes()),
        (
            "agent every process environ and cmdline",
            proc_files.as_bytes(),
        ),
        ("agent workspace", &workspace),
        ("agent HOME", home.as_bytes()),
        ("agent docker logs", agent_log.as_bytes()),
        ("gateway logs", logs.as_bytes()),
    ];
    for (name, text) in &inspects {
        // BORROW: explicit .as_str() instead of Deref coercion
        haystacks.push((name.as_str(), text.as_bytes()));
    }
    assert!(
        inspects.len() >= 3,
        "agent, gateway and upstream are labelled"
    );
    if let Err(leak) = assert_clean(&token, &haystacks) {
        panic!("the real token leaked: {leak:?}");
    }
}

/// A TCP listener on the host that counts the connections it accepts.
struct HostListener {
    port: u16,
    accepted: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl HostListener {
    fn start() -> Self {
        let listener = TcpListener::bind("0.0.0.0:0").expect("a host listener");
        listener
            .set_nonblocking(true)
            .expect("a non-blocking listener");
        let port = listener.local_addr().expect("an address").port();
        let accepted = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (count, halt) = (Arc::clone(&accepted), Arc::clone(&stop));
        let thread = std::thread::spawn(move || {
            while !halt.load(Ordering::SeqCst) {
                if listener.accept().is_ok() {
                    count.fetch_add(1, Ordering::SeqCst);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        Self {
            port,
            accepted,
            stop,
            thread: Some(thread),
        }
    }

    fn accepted(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }
}

impl Drop for HostListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The shell function both the agent and the control use: `probe <host> <port> <name>` prints
/// `<name>=open` or `<name>=fail`.
const PROBE_FN: &str = "probe() { if nc -w 3 \"$1\" \"$2\" </dev/null >/dev/null 2>&1; then echo \"$3=open\"; else echo \"$3=fail\"; fi; }\n";

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
/// Guards: B18 — the agent has no route to the host or the internet.
/// Oracle: the agent on the job's network reaches the sidecar and fails to reach a host listener,
/// the internet and a public resolver, while a control container on the default bridge reaches the
/// same host listener and the listener counts exactly that one connection.
/// Bite: drop `--internal` from the network the driver creates.
async fn b18_internal_network_no_route() {
    let rig = Rig::new();
    let token = canary();
    let world = up(&rig, &token).await;
    let handle = &world.handle;
    let net = rig
        .cli
        .network_inspect(&rig.job_network())
        .await
        .expect("the job's network");
    assert!(net.internal, "the job's network is internal");

    let host = HostListener::start();
    let port = host.port;
    let script = format!(
        "{PROBE_FN}{{ probe gateway 8080 sidecar; probe host.docker.internal {port} host; probe 1.1.1.1 443 internet; probe 8.8.8.8 53 resolver; }} > /workspace/net.txt\necho done > /workspace/done\n"
    );
    let agent = start_agent(&rig, handle, &run_key(1), &handle.dummy_token, &script);
    wait_done(&agent);
    let seen = std::fs::read_to_string(agent.workspace.join("net.txt")).expect("the probe output");
    assert!(
        seen.contains("sidecar=open"),
        "the agent reaches the gateway: {seen}"
    );
    assert!(
        seen.contains("host=fail"),
        "the agent reached the host: {seen}"
    );
    assert!(
        seen.contains("internet=fail"),
        "the agent reached the internet: {seen}"
    );
    assert!(
        seen.contains("resolver=fail"),
        "the agent reached a resolver: {seen}"
    );
    assert_eq!(host.accepted(), 0, "the host listener saw the agent");

    // Control: a container on the default bridge reaches the same listener.
    let control_script = format!("{PROBE_FN}probe host.docker.internal {port} host\n");
    // It carries no `candler.job` label, so no sweep of the job meets it, and `--rm` has docker
    // remove it when its script ends.
    let said = docker(&[
        "run",
        "--rm",
        "--label",
        "candler.role=control",
        "--label",
        "candler.test=gateway",
        "--add-host",
        "host.docker.internal:host-gateway",
        "--entrypoint",
        "sh",
        AGENT_IMAGE,
        "-c",
        &control_script,
    ])
    .must("running the control");
    assert!(
        said.contains("host=open"),
        "the control must reach the host listener: {said}"
    );
    assert_eq!(host.accepted(), 1, "only the control reached the listener");
}
