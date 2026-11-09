//! The real-Docker guards of the gateway: B17 and B18 (the token stays out of the agent
//! container, which has no route out) and B37 (the credential goes only to a trusted upstream).
//!
//! Every test builds its own world under a `candler.job=<id>` label, wraps its drivers in the
//! test-side allowlist, and tears down only what carries its own job label. The upstream is a
//! Python-stdlib TLS server (`fixtures/gateway/tls_upstream.py`) with a certificate made by the
//! `openssl` CLI; the only token is a random canary.
#![cfg(test)]

use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use candler_core::isolation::container::guard::ContainerGuard;
use candler_core::isolation::docker::{ClientEnv, DockerCli as DockerClient, DockerDestroyer};
use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
use candler_core::isolation::gateway::{
    ChildRecorder, DockerCli, DockerGateway, Gateway, GatewaySignal, GatewayStart, SecretError,
    SecretSource, SecretValue, UpstreamConfig, assert_clean,
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

// The helpers the supervision gates share.
#[path = "support/gateway.rs"]
mod gateway_support;

use gateway_support::{
    AGENT_IMAGE, CELL_HEX, EXPERIMENT, FAKE_SCRIPT, GATEWAY_CONTEXT, PYTHON_IMAGE, Ran,
    SECRET_NAME, UPSTREAM_HOST, UPSTREAM_PORT, canary, client_env, docker, make_ca,
    make_server_cert, random_job, run,
};

/// `docker exec <cid> <args>`.
fn exec(cid: &str, args: &[&str]) -> Ran {
    let mut all = vec!["exec", cid];
    all.extend_from_slice(args);
    docker(&all)
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
                // BORROW: explicit .as_str() names the network
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

/// The standard base64 of `text`, with padding, from the `base64` program.
fn base64_of(text: &str) -> String {
    run("base64", &[], text.as_bytes())
        .must("base64 of the token")
        .replace('\n', "")
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
        // BORROW: Value::as_str reads the text of a json string
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
        let (job, run, egress, dir, secret) = (
            rig.job.clone(), // BORROW: an owned copy moves to the blocking thread
            rig.run.clone(), // BORROW: an owned copy moves to the blocking thread
            rig.egress_network(),
            certs.dir.clone(), // BORROW: an owned copy moves to the blocking thread
            String::from(token),
        );
        tokio::task::spawn_blocking(move || run_fake(&job, &run, &egress, &dir, &secret))
    };
    let started = gw
        .start(
            &rig.sup,
            // BORROW: explicit .as_ref() instead of Deref coercion
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

/// What distinguishes one agent from another: the key it holds, its other variables, the
/// networks it joins besides the job's own, and what it runs under `sh`.
struct AgentSpec<'a> {
    api_key: &'a str,
    extra_env: &'a [(&'a str, &'a str)],
    extra_networks: &'a [&'a str],
    script: &'a str,
}

/// Starts an agent that runs `spec.script` under `sh`, holding `spec.api_key` as
/// `ANTHROPIC_API_KEY`, the run's base url as `ANTHROPIC_BASE_URL` and the variables of
/// `spec.extra_env`, with a bind-mounted workspace and a tmpfs HOME.
fn start_agent(rig: &Rig, handle: &GatewayHandle, run: &RunKey, spec: &AgentSpec<'_>) -> Agent {
    let workspace = rig.work_dir("workspace");
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o777))
        .expect("a world-writable workspace");
    let base_url = handle.run_base_url(run);
    let mut args: Vec<String> = [
        "create",
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
        &format!("ANTHROPIC_API_KEY={}", spec.api_key),
        "-e",
        &format!("ANTHROPIC_BASE_URL={base_url}"),
        "-e",
        "HOME=/home/agent",
    ]
    .into_iter()
    // BORROW: .to_owned() builds the owned argv
    .map(str::to_owned)
    .collect();
    for (name, value) in spec.extra_env {
        args.push(String::from("-e"));
        args.push(format!("{name}={value}"));
    }
    args.extend(
        [
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
            spec.script,
        ]
        .into_iter()
        // BORROW: .to_owned() builds the owned argv
        .map(str::to_owned),
    );
    // BORROW: explicit .as_str() instead of Deref coercion
    let created =
        // BORROW: explicit .as_str() instead of Deref coercion
        docker(&args.iter().map(String::as_str).collect::<Vec<_>>()).must("creating the agent");
    let cid = String::from(created.trim());
    // The container is created, so a failed connect below leaves it for the teardown to find.
    for &net in spec.extra_networks {
        docker(&["network", "connect", net, &cid]).must("joining the agent to a network");
    }
    docker(&["start", &cid]).must("starting the agent");
    Agent { cid, workspace }
}

/// Waits until the agent's script has written `/workspace/done`.
fn wait_done(agent: &Agent) {
    wait_file(agent, "done");
}

/// Waits until the agent's script has written `/workspace/<name>`.
fn wait_file(agent: &Agent, name: &str) {
    let marker = agent.workspace.join(name);
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
/// process's environ and command line, the workspace, HOME, the response headers the agent
/// received, every labelled container's `docker inspect` and the gateway logs, while the upstream
/// echoes the credential in a body, an error page, `X-Echo-Auth`, `Set-Cookie` and `Location`, and
/// the canary demonstrably reached the upstream. A call with another key is refused with 401.
/// The same scan looks for the hex of the job's tag secret.
/// Bite: put the real token in a second agent variable, `-e AGENT_NOTE`.
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
req GET /echo-headers /workspace/headers.txt
req GET /echo-forms /workspace/forms.txt
req GET /v1/models /workspace/models.txt
( ANTHROPIC_API_KEY=not-the-dummy; req GET /echo /workspace/wrong.txt )
echo profile > \"$HOME/.agent-profile\"
echo agent-ready
echo done > /workspace/done
exec sleep 600
"
    );
    let agent = start_agent(
        &rig,
        handle,
        &run,
        &AgentSpec {
            api_key: &handle.dummy_token,
            extra_env: &[("AGENT_NOTE", "plain")],
            extra_networks: &[],
            script: &script,
        },
    );
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

    // The upstream did echo the credential in headers, and none of those reached the agent.
    assert!(
        seen.iter()
            .any(|r| field(r, "path").ends_with("/echo-headers") && field(r, "x_api_key") == token),
        "the header echo never reached the upstream: {seen:?}"
    );
    let headers = std::fs::read(agent.workspace.join("headers.txt")).expect("headers");
    let headers_text = String::from_utf8_lossy(&headers).to_lowercase();
    assert!(
        headers_text.contains("302"),
        "the redirect came back: {headers_text}"
    );
    for echoed in ["x-echo-auth", "set-cookie", "location"] {
        assert!(
            !headers_text.contains(echoed),
            "the header {echoed} came back: {headers_text}"
        );
    }
    // The JSON-escaped and the base64 form of the token are scrubbed from a body as well.
    let forms = std::fs::read_to_string(agent.workspace.join("forms.txt")).expect("forms");
    assert!(
        forms.contains("200") && forms.contains("<redacted>"),
        "the forms were not scrubbed: {forms}"
    );
    let escaped = token.replace('/', "\\/");
    assert!(
        !forms.contains(&escaped) && !forms.contains(&base64_of(&token)),
        "a form of the token came back: {forms}"
    );
    // A call with a key that is not the job's dummy is refused before the upstream sees it.
    let wrong = std::fs::read_to_string(agent.workspace.join("wrong.txt")).expect("wrong");
    assert!(
        wrong.contains(" 401"),
        "another key was not refused: {wrong}"
    );
    assert!(
        !seen
            .iter()
            .any(|r| field(r, "authorization").contains("not-the-dummy")),
        "a foreign key reached the upstream: {seen:?}"
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

    // A refused key is the gateway's own answer, not a signal of the upstream.
    let signals = world
        .gw
        .run_signals(handle, &run)
        .await
        .expect("the signals");
    assert!(
        !signals.contains(&GatewaySignal::UpstreamStatus(401)),
        "a refused key became an upstream signal: {signals:?}"
    );
    assert!(
        signals.contains(&GatewaySignal::UpstreamStatus(500)),
        "the upstream's own 500 is a signal: {signals:?}"
    );

    let mut haystacks: Vec<(&str, &[u8])> = vec![
        ("agent env", env.as_bytes()),
        ("agent /proc/1/environ", pid1.as_bytes()),
        (
            "agent every process environ and cmdline",
            proc_files.as_bytes(),
        ),
        ("agent response headers", &headers),
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
    // The job's tag secret is nowhere an agent or a log can read it either.
    if let Err(leak) = assert_clean(&handle.secret_hex(), &haystacks) {
        panic!("the tag secret leaked: {leak:?}");
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
/// same host listener and the same internet target, and the listener counts exactly that one
/// connection. A control that cannot reach the internet fails the test: the oracle is vacuous.
/// Bite: also attach the agent to the job's egress network.
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
    // The networks the agent joins besides the job's own: none.
    let extra: Vec<&str> = Vec::new();
    let agent = start_agent(
        &rig,
        handle,
        &run_key(1),
        &AgentSpec {
            api_key: &handle.dummy_token,
            extra_env: &[],
            extra_networks: &extra,
            script: &script,
        },
    );
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
    let control_script =
        format!("{PROBE_FN}probe host.docker.internal {port} host\nprobe 1.1.1.1 443 internet\n");
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
        said.contains("internet=open"),
        "no internet: the oracle is vacuous ({said})"
    );
    assert!(
        said.contains("host=open"),
        "the control must reach the host listener: {said}"
    );
    assert_eq!(host.accepted(), 1, "only the control reached the listener");
}

/// Replaces the certificate the fake serves with the chain in `dir` (`cert.pem`, `key.pem`), and
/// waits until the fake serves it.
fn swap_fake_certificate(fake: &Fake, dir: &Path) {
    let script = "mkdir -p /tmp/swap && cat > /tmp/swap/NAME";
    for name in ["cert.pem", "key.pem"] {
        let bytes = std::fs::read(dir.join(name)).expect("a pem file");
        run(
            "docker",
            &[
                "exec",
                "-i",
                &fake.cid,
                "sh",
                "-c",
                &script.replace("NAME", name),
            ],
            &bytes,
        )
        .must("copying a pem file into the fake");
    }
    exec(&fake.cid, &["touch", "/tmp/swap/ready"]).must("marking the swap ready");
    let wanted = std::fs::read_to_string(dir.join("cert.pem")).expect("the certificate");
    let fetch = "import ssl; print(ssl.get_server_certificate(('127.0.0.1', 8443)))";
    let deadline = Instant::now() + Duration::from_secs(30);
    while exec(&fake.cid, &["python3", "-c", fetch]).out.trim() != wanted.trim() {
        assert!(
            Instant::now() < deadline,
            "the fake never served the new certificate"
        );
        std::thread::sleep(Duration::from_millis(300));
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
/// Guards: B37 — the gateway sends the credential only to an upstream its authority vouches for.
/// Oracle: after a first call proves the credential reaches the trusted upstream, the upstream's
/// certificate is replaced by one that another authority signed; the agent's calls then fail with
/// 502 and the fake's own record holds no new request, so no `Authorization` header.
/// Bite: set `proxy_ssl_verify off` in the sidecar's config.
async fn b37_untrusted_upstream_gets_no_credential() {
    let rig = Rig::new();
    let token = canary();
    let world = up(&rig, &token).await;
    let (fake, handle) = (&world.fake, &world.handle);
    let script = format!(
        "{AGENT_REQUEST}
req GET /v1/models /workspace/good.txt
echo ok > /workspace/first
while [ ! -e /workspace/go ]; do sleep 0.2; done
req GET /v1/models /workspace/bad.txt
req POST /v1/messages /workspace/bad2.txt
echo done > /workspace/done
exec sleep 600
"
    );
    let agent = start_agent(
        &rig,
        handle,
        &run_key(1),
        &AgentSpec {
            api_key: &handle.dummy_token,
            extra_env: &[],
            extra_networks: &[],
            script: &script,
        },
    );
    wait_file(&agent, "first");
    let good = std::fs::read_to_string(agent.workspace.join("good.txt")).expect("good");
    assert!(
        good.contains(" 200"),
        "the trusted upstream answers: {good}"
    );
    let trusted = records(fake);
    assert!(
        trusted
            .iter()
            .any(|r| field(r, "authorization") == format!("Bearer {token}")),
        "the credential never reached the trusted upstream: {trusted:?}"
    );

    // From now on the upstream's certificate is signed by an authority the sidecar does not trust.
    let evil = rig.work_dir("evil");
    make_ca(&evil, "evil");
    make_server_cert(&evil, "evil");
    swap_fake_certificate(fake, &evil);
    let before = records(fake).len();
    std::fs::write(agent.workspace.join("go"), "go\n").expect("the go marker");
    wait_done(&agent);

    for name in ["bad.txt", "bad2.txt"] {
        let text = std::fs::read_to_string(agent.workspace.join(name)).expect("an answer");
        assert!(
            text.contains(" 502"),
            "{name}: the untrusted upstream answered: {text}"
        );
    }
    // The failed calls are signals of their own run, and of no other.
    let own = world
        .gw
        .run_signals(handle, &run_key(1))
        .await
        .expect("the signals");
    assert_eq!(own.len(), 2, "the run's own signals: {own:?}");
    let other = world
        .gw
        .run_signals(handle, &run_key(2))
        .await
        .expect("the signals");
    assert_eq!(other.len(), 0, "another run's signals: {other:?}");
    let after = records(fake);
    let received: Vec<&serde_json::Value> = after.iter().skip(before).collect();
    assert_eq!(
        received.len(),
        0,
        "the untrusted upstream got a request: {received:?}"
    );
    assert_eq!(
        received
            .iter()
            .filter(|r| !field(r, "authorization").is_empty())
            .count(),
        0,
        "the untrusted upstream got an Authorization header"
    );
}
