#![cfg(test)]
//! The allowlist against a real daemon: one rig, eleven rows, each judged by the connection log of
//! the target it concerns and never by the client's error.
//!
//! Nothing here reaches the internet. The targets are plain TCP sinks on the job's egress network
//! that log every connection and every received byte; a resolver container answers the listed
//! names for the proxy, so no target carries a Docker alias equal to a listed name.

use std::io::Write as _;
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use candler_core::isolation::docker::{ClientEnv, DockerCli as DockerClient};
use candler_core::isolation::gateway::{
    ChildRecorder, DockerGateway, Gateway, GatewayStart, SecretError, SecretSource, SecretValue,
    UpstreamConfig, egress_create_args,
};
use candler_core::isolation::model::NetworkName;
use candler_core::resolve::steps::egress::settle;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::container::ContainerRecord;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{Digest, RunKey};
use serde_json::json;

use crate::support::gateway::{
    CELL_HEX, EXPERIMENT, FAKE_SCRIPT, PYTHON_IMAGE, SECRET_NAME, UPSTREAM_HOST, UPSTREAM_PORT,
    canary, docker, make_ca, make_server_cert, random_job,
};
use crate::support::test_image::{SIDECAR_CONTEXT, TESTAGENT_CONTEXT, TestImage};

/// The listed names. Port 8080 belongs to one host, 9443 is shared, the last three resolve to
/// loopback, link-local and, at first, to a decoy; `multi.test` answers two addresses, the first
/// refusing connections and the second a link-local sink; `low.test` listens below 1024.
const LISTED: [&str; 9] = [
    "allowed.test:8080",
    "a.test:9443",
    "b.test:9443",
    "loop.test:7001",
    "ll.test:7002",
    "rebind.test:7003",
    "multi.test:7004",
    "low.test:443",
    "upstream.test:8443",
];

/// A TLS client that verifies the fake's certificate against the rig's authority, sends one
/// request and prints the protocol version and the status line.
const TLS_CLIENT: &str = r"
import socket, ssl
ctx = ssl.create_default_context(cafile='/fake/ca.pem')
token = open('/fake/token').read().strip()
raw = socket.create_connection(('upstream.test', 8443), 10)
with ctx.wrap_socket(raw, server_hostname='upstream.test') as tls:
    tls.sendall(('GET /v1/models HTTP/1.0\r\nHost: upstream.test\r\nAuthorization: Bearer %s\r\nx-api-key: %s\r\nContent-Length: 0\r\n\r\n' % (token, token)).encode())
    data = b''
    while True:
        chunk = tls.recv(65536)
        if not chunk:
            break
        data += chunk
    print(tls.version(), data.split(b'\r\n')[0].decode())
";

const SINK: &str = r"
import socket, sys, threading
def handle(c, p):
    c.settimeout(3)
    try:
        while True:
            d = c.recv(4096)
            if not d:
                break
            print('RECV', p, len(d), flush=True)
    except Exception:
        pass
    c.close()
def serve(addr, p):
    s = socket.socket()
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.bind((addr, p))
    s.listen(16)
    while True:
        c, a = s.accept()
        print('CONN', p, a[0], flush=True)
        threading.Thread(target=handle, args=(c, p), daemon=True).start()
for spec in sys.argv[1:]:
    addr, port = spec.rsplit(':', 1)
    threading.Thread(target=serve, args=(addr, int(port)), daemon=True).start()
threading.Event().wait()
";

const RESOLVER: &str = r"
import socket, struct
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.bind(('0.0.0.0', 53))
def table():
    t = {}
    for line in open('/dns/map'):
        p = line.split()
        if len(p) == 2:
            t[p[0]] = p[1]
    return t
while True:
    q, a = s.recvfrom(512)
    i = 12
    labels = []
    while q[i]:
        n = q[i]
        labels.append(q[i + 1:i + 1 + n].decode())
        i += n + 1
    name = '.'.join(labels).lower()
    qt = struct.unpack('>H', q[i + 1:i + 3])[0]
    end = i + 5
    ip = table().get(name)
    ips = ip.split(',') if ip else []
    if ips and qt == 1:
        rr = b''.join(b'\xc0\x0c\x00\x01\x00\x01\x00\x00\x00\x01\x00\x04' + socket.inet_aton(x) for x in ips)
        r = q[:2] + b'\x81\x80' + q[4:6] + struct.pack('>H', len(ips)) + b'\x00\x00\x00\x00' + q[12:end] + rr
    elif ips:
        r = q[:2] + b'\x81\x80' + q[4:6] + b'\x00\x00\x00\x00\x00\x00' + q[12:end]
    else:
        r = q[:2] + b'\x81\x83' + q[4:6] + b'\x00\x00\x00\x00\x00\x00' + q[12:end]
    s.sendto(r, a)
";

/// The ids this rig made, removed by exact id when it ends, however it ends.
#[derive(Default)]
struct Made {
    containers: Vec<String>,
    networks: Vec<String>,
    /// Every container and network the rig makes carries these labels; the first is the rig's own.
    labels: Vec<String>,
}

/// The whitespace-separated words docker prints for `args`.
fn listed(args: &[&str]) -> Vec<String> {
    Command::new("docker")
        .args(args)
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

impl Made {
    /// Removes by id what was recorded and what carries a rig label, containers first.
    fn sweep(&self) {
        let mut containers = self.containers.clone();
        let mut networks = self.networks.clone();
        for label in &self.labels {
            let filter = format!("label={label}");
            containers.extend(listed(&["ps", "-aq", "--filter", &filter]));
            networks.extend(listed(&["network", "ls", "-q", "--filter", &filter]));
        }
        for id in containers.iter().rev() {
            let _ = Command::new("docker").args(["rm", "-f", id]).output();
        }
        for id in &networks {
            let _ = Command::new("docker").args(["network", "rm", id]).output();
        }
    }

    /// What still carries a rig label.
    fn leftovers(&self) -> Vec<String> {
        let mut left = Vec::new();
        for label in &self.labels {
            let filter = format!("label={label}");
            left.extend(listed(&["ps", "-aq", "--filter", &filter]));
            left.extend(listed(&["network", "ls", "-q", "--filter", &filter]));
        }
        left
    }
}

impl Drop for Made {
    fn drop(&mut self) {
        self.sweep();
    }
}

struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

#[derive(Default)]
struct Recorder;

impl ChildRecorder for Recorder {
    fn record_child(&self, _record: &ContainerRecord) -> std::io::Result<()> {
        Ok(())
    }
}

struct Fixed(String);

impl SecretSource for Fixed {
    fn resolve(&self, _name: &str) -> Result<SecretValue, SecretError> {
        Ok(SecretValue::new(self.0.clone()))
    }
}

fn must(args: &[&str], what: &str) -> String {
    docker(args).must(what).trim().to_owned()
}

fn ip_on(cid: &str, net: &str) -> String {
    must(
        &[
            "inspect",
            "-f",
            &format!("{{{{(index .NetworkSettings.Networks \"{net}\").IPAddress}}}}"),
            cid,
        ],
        "an address",
    )
}

/// A minimal TLS `ClientHello`, with `sni` as the server name when given.
fn hello(sni: Option<&str>) -> Vec<u8> {
    let mut ext = Vec::new();
    if let Some(name) = sni {
        let name = name.as_bytes();
        let len = u16::try_from(name.len()).expect("a short name");
        let mut entry = Vec::new();
        entry.extend((len + 3).to_be_bytes());
        entry.push(0);
        entry.extend(len.to_be_bytes());
        entry.extend(name);
        ext.extend([0, 0]);
        ext.extend(u16::try_from(entry.len()).expect("short").to_be_bytes());
        ext.extend(entry);
    }
    let mut body = vec![3, 3];
    body.extend([0u8; 32]);
    body.push(0);
    body.extend([0, 2, 0x13, 0x01, 1, 0]);
    body.extend(u16::try_from(ext.len()).expect("short").to_be_bytes());
    body.extend(ext);
    let mut hs = vec![1];
    hs.extend(&u32::try_from(body.len()).expect("short").to_be_bytes()[1..]);
    hs.extend(body);
    let mut rec = vec![0x16, 3, 1];
    rec.extend(u16::try_from(hs.len()).expect("short").to_be_bytes());
    rec.extend(hs);
    rec
}

const PLAIN: &[u8] = b"GET / HTTP/1.0\r\nHost: denied.test\r\n\r\n";

/// One client attempt from inside the agent, `bytes` on stdin.
fn attempt(agent: &str, host: &str, port: u16, bytes: &[u8]) {
    let mut child = Command::new("docker")
        .args([
            "exec",
            "-i",
            agent,
            "nc",
            "-w",
            "2",
            host,
            &port.to_string(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("docker exec");
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(bytes);
    }
    let _ = child.wait();
}

/// The bytes a sink has logged as received.
fn rx(cid: &str) -> usize {
    let out = docker(&["logs", cid]).both();
    out.lines()
        .filter_map(|l| l.strip_prefix("RECV "))
        .filter_map(|l| l.split_whitespace().nth(1)?.parse::<usize>().ok())
        .sum()
}

fn write_exec(dir: &Path, name: &str, text: &str) {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("a file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("a mode");
}

/// A sink container on `net`, with `aliases` there, listening on every `spec`.
fn sink(made: &mut Made, dir: &Path, net: &str, aliases: &[&str], specs: &[&str]) -> String {
    let mount = format!("{}:/rig:ro", dir.display());
    let label = made.labels[0].clone();
    let mut args = vec!["create", "--label", &label, "--network", net];
    for alias in aliases {
        args.extend(["--network-alias", alias]);
    }
    args.extend(["-v", &mount, PYTHON_IMAGE]);
    args.extend(["python3", "-u", "/rig/sink.py"]);
    args.extend(specs);
    let cid = must(&args, "creating a sink");
    made.containers.push(cid.clone());
    must(&["start", &cid], "starting a sink");
    cid
}

fn wait_for(what: &str, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !check() {
        assert!(Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// A host listener that counts the bytes any peer sends it.
struct HostListener {
    label: &'static str,
    port: u16,
    bytes: Arc<AtomicUsize>,
}

impl HostListener {
    /// Binds `addr` and counts in a thread; `None` when the host cannot bind it.
    fn bind(addr: &'static str) -> Option<Self> {
        let listener = TcpListener::bind(addr).ok()?;
        let port = listener.local_addr().ok()?.port();
        let bytes = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&bytes);
        std::thread::spawn(move || {
            while let Ok((mut conn, _)) = listener.accept() {
                let seen = Arc::clone(&seen);
                std::thread::spawn(move || {
                    let mut buf = [0u8; 4096];
                    while let Ok(n @ 1..) = std::io::Read::read(&mut conn, &mut buf) {
                        seen.fetch_add(n, Ordering::SeqCst);
                    }
                });
            }
        });
        Some(Self {
            label: addr,
            port,
            bytes,
        })
    }
}

/// The address a bridge gateway has on docker network `net`: the first address of its subnet.
/// A network created without a host address reports no gateway, so the address is derived from
/// the subnet, where a bridge that kept its host address would hold it.
fn bridge_gateway(net: &str) -> String {
    let subnet = must(
        &[
            "network",
            "inspect",
            "-f",
            "{{(index .IPAM.Config 0).Subnet}}",
            net,
        ],
        "the subnet",
    );
    let base = subnet.split('/').next().expect("an address");
    let (head, _) = base.rsplit_once('.').expect("an IPv4 subnet");
    format!("{head}.1")
}

/// The host's link-local IPv6 address on the bridge of docker network `net`, when it has one.
/// Only a Linux host has the bridge as an interface.
fn host_link_local(net: &str) -> Option<String> {
    let id = must(
        &["network", "inspect", "-f", "{{.Id}}", net],
        "a network id",
    );
    let bridge = format!("br-{}", id.get(..12)?);
    let out = Command::new("ip")
        .args(["-6", "-o", "addr", "show", "dev", &bridge, "scope", "link"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let addr = text
        .split_whitespace()
        .skip_while(|w| *w != "inet6")
        .nth(1)?;
    Some(addr.split('/').next()?.to_owned())
}

/// Row 8: from `agent`, tries every host address it could name against every host listener; each
/// listener must receive no byte.
fn probe_host(
    cell: &str,
    agent: &str,
    gateways: &[&str],
    link_local: Option<&str>,
    listeners: &[HostListener],
    log: &std::cell::RefCell<Vec<String>>,
    failures: &std::cell::RefCell<Vec<String>>,
) {
    let mut targets: Vec<String> = gateways.iter().map(|g| (*g).to_owned()).collect();
    targets.push("host.docker.internal".to_owned());
    if let Some(ll) = link_local {
        targets.extend(["eth0", "eth1"].map(|nic| format!("{ll}%{nic}")));
    }
    let before: Vec<usize> = listeners
        .iter()
        .map(|l| l.bytes.load(Ordering::SeqCst))
        .collect();
    for target in &targets {
        for listener in listeners {
            attempt(agent, target, listener.port, PLAIN);
        }
    }
    for (listener, before) in listeners.iter().zip(before) {
        let got = listener.bytes.load(Ordering::SeqCst) - before;
        log.borrow_mut().push(format!(
            "row 8 [{cell}] listener {} received {got} bytes; targets {targets:?}",
            listener.label
        ));
        if got > 0 {
            failures.borrow_mut().push(format!(
                "row 8 [{cell}]: the host listener {} received {got} bytes",
                listener.label
            ));
        }
    }
}

/// Guards: R01 §Agent environment and secrets, the egress allowlist — an agent whose cell lists
/// hosts reaches those host:port pairs and the model gateway, and nothing else: no other port,
/// host, literal address, external name, host gateway, name that resolves to loopback or
/// link-local, or proxying through the gateway. Oracle: the connection log of each target (and
/// of a listener on the host), read with `docker logs`; a row passes on the bytes the target
/// received, never on the client's error.
/// Guards: D2 — an allowlisted TLS host is reachable byte-for-byte through the proxy: a real TLS
/// client finishes the handshake against the rig's TLS fake, verifies its certificate and reads a
/// 200 through the proxy. Oracle: the client's own TLS stack (a corrupted handshake fails its
/// verification) and the status line the fake answers. Bite: `EGRESS_RIG_BITE=string_events` (the
/// filter takes the string events `upload` and `download`, which rewrite invalid UTF-8) turns the
/// handshake row red.
/// Bite: `EGRESS_RIG_BITE=agent_on_egress` (the product's `agent_networks` is asked for a
/// `bridge` cell, so the agent joins the egress network),
/// `pinned` (the proxy caches an upstream address for an hour), `no_filter` (the proxy's
/// loopback filter forwards everything) and `first_entry_retry` (the filter reads only the first
/// address nginx tried, and nginx retries the next one) each turn rows red.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
async fn egress_allowlist_holds_on_a_real_daemon() {
    let started = Instant::now();
    let bite = std::env::var("EGRESS_RIG_BITE").unwrap_or_default();
    let mut made = Made::default();
    let agent_image = TestImage::new(TESTAGENT_CONTEXT);
    let sidecar = TestImage::new(SIDECAR_CONTEXT);
    if !docker(&["image", "inspect", PYTHON_IMAGE]).ok() {
        docker(&["pull", "--quiet", PYTHON_IMAGE]).must("docker pull of the pinned python image");
    }
    let home = PathBuf::from(std::env::var("HOME").expect("HOME"));
    let tmp = tempfile::Builder::new()
        .prefix("egress-rig-")
        .tempdir_in(&home)
        .expect("a scratch directory under HOME");
    let dir = tmp.path().to_path_buf();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).expect("a mode");
    write_exec(&dir, "sink.py", SINK);
    write_exec(&dir, "resolver.py", RESOLVER);
    std::fs::create_dir_all(dir.join("dns")).expect("a dns directory");
    std::fs::set_permissions(dir.join("dns"), std::fs::Permissions::from_mode(0o755)).expect("m");
    std::fs::create_dir_all(dir.join("fake")).expect("a fake directory");
    make_ca(&dir.join("fake"), "ca");
    make_server_cert(&dir.join("fake"), "ca");
    let ca_pem = std::fs::read(dir.join("fake/ca.pem")).expect("the authority");
    std::fs::write(dir.join("fake/tls_upstream.py"), FAKE_SCRIPT).expect("the fake script");
    let token = canary();
    std::fs::write(dir.join("fake/token"), format!("{token}\n")).expect("a token");
    for entry in std::fs::read_dir(dir.join("fake"))
        .expect("a dir")
        .flatten()
    {
        let _ = std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o644));
    }
    std::fs::set_permissions(dir.join("fake"), std::fs::Permissions::from_mode(0o755)).expect("m");

    // The model gateway, through the core, with the TLS fake upstream beside it.
    let job = random_job();
    made.labels = vec![format!("candler.rig={job}"), format!("candler.job={job}")];
    let rig = made.labels[0].clone();
    let experiment: Digest = EXPERIMENT.parse().expect("a digest");
    let run: RunKey = format!("{CELL_HEX}-r1-a1").parse().expect("a run key");
    let sink_events: Arc<dyn EventSink> = Arc::new(NullSink);
    let sup = Arc::new(Supervisor::new(sink_events));
    let cli = DockerClient::new(Arc::clone(&sup), ClientEnv::from_process(), dir.clone())
        .expect("a docker client");
    let upstream = UpstreamConfig::builder()
        .host(UPSTREAM_HOST)
        .port(UPSTREAM_PORT)
        .ca_pem(ca_pem)
        .build()
        .expect("an upstream");
    let gw = DockerGateway::new(
        sidecar.reference(),
        upstream,
        4,
        DockerClient::clone(&cli),
        "gateway".parse().expect("a marker"),
        Arc::clone(&sup),
    );
    let egress = NetworkName::egress_for_job(&job);
    let fake_task = {
        let (egress, dir, job, rig) = (egress.clone(), dir.clone(), job.to_string(), rig.clone());
        tokio::task::spawn_blocking(move || {
            // nginx in the gateway resolves the upstream when it starts, so the fake must join
            // the network the moment it exists.
            let deadline = Instant::now() + Duration::from_secs(60);
            while !docker(&["network", "inspect", egress.as_str()]).ok() {
                assert!(
                    Instant::now() < deadline,
                    "the egress network never appeared"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            let mount = format!("{}:/fake:ro", dir.join("fake").display());
            let port = UPSTREAM_PORT.to_string();
            let cid = must(
                &[
                    "run",
                    "-d",
                    "--label",
                    &format!("candler.job={job}"),
                    "--label",
                    &rig,
                    "--network",
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
                ],
                "the upstream fake",
            );
            let probe = format!(
                "import socket; socket.create_connection(('127.0.0.1', {UPSTREAM_PORT}), 2)"
            );
            wait_for("the upstream fake", || {
                docker(&["exec", &cid, "python3", "-c", &probe]).ok()
            });
            cid
        })
    };
    let started_gw = gw
        .start(
            &sup,
            &cli,
            GatewayStart {
                job: &job,
                experiment: &experiment,
                recorder: &Recorder,
                token: &Fixed(token.clone()),
                secret: SECRET_NAME,
            },
        )
        .await;
    let fake_cid = fake_task.await.expect("the fake's thread");
    made.containers.push(fake_cid.clone());
    let handle = started_gw.expect("a started gateway");
    made.containers.push(handle.container.cid.to_string());
    let job_net = handle.network.as_str().to_owned();
    let egress_net = egress.as_str().to_owned();
    for net in [&job_net, &egress_net] {
        made.networks.push(must(
            &["network", "inspect", "-f", "{{.Id}}", net],
            "a network id",
        ));
    }

    // The link-local target: a rig network on a link-local subnet the proxy joins. The loopback
    // name cannot have a sink: the proxy listens on every listed port on 0.0.0.0, so no sink can
    // bind 127.0.0.1 on that port in its namespace, and nginx would reach only itself.
    let ll_name = format!("rig-ll-{job}");
    let ll_net_id = must(
        &[
            "network",
            "create",
            "--internal",
            "--label",
            &rig,
            "--subnet",
            "169.254.231.0/24",
            &ll_name,
        ],
        "the link-local rig network",
    );
    made.networks.push(ll_net_id.clone());
    let local = sink(
        &mut made,
        &dir,
        &ll_name,
        &[],
        &["0.0.0.0:7002", "0.0.0.0:7003", "0.0.0.0:7004"],
    );
    let ip_ll = ip_on(&local, &ll_name);

    // The targets on the egress network, none aliased to a listed name.
    let allowed = sink(
        &mut made,
        &dir,
        &egress_net,
        &[],
        &["0.0.0.0:8080", "0.0.0.0:8081"],
    );
    let ta = sink(&mut made, &dir, &egress_net, &[], &["0.0.0.0:9443"]);
    let tb = sink(&mut made, &dir, &egress_net, &[], &["0.0.0.0:9443"]);
    let low = sink(&mut made, &dir, &egress_net, &[], &["0.0.0.0:443"]);
    // The denied sink answers to `denied.test` and `external.test` on the egress network, names
    // that are not listed: an agent that reaches the egress network resolves them and connects.
    let denied = sink(
        &mut made,
        &dir,
        &egress_net,
        &["denied.test", "external.test"],
        &[
            "0.0.0.0:80",
            "0.0.0.0:443",
            "0.0.0.0:8080",
            "0.0.0.0:8081",
            "0.0.0.0:9443",
            "0.0.0.0:7003",
        ],
    );
    let ip_fake = ip_on(&fake_cid, &egress_net);
    let (ip_allowed, ip_a, ip_b, ip_denied, ip_low) = (
        ip_on(&allowed, &egress_net),
        ip_on(&ta, &egress_net),
        ip_on(&tb, &egress_net),
        ip_on(&denied, &egress_net),
        ip_on(&low, &egress_net),
    );
    let map = format!(
        "allowed.test {ip_allowed}\na.test {ip_a}\nb.test {ip_b}\nloop.test 127.0.0.1\n\
         ll.test {ip_ll}\nrebind.test {ip_denied}\nexternal.test {ip_denied}\n\
         multi.test {ip_a},{ip_ll}\nlow.test {ip_low}\nupstream.test {ip_fake}\n"
    );
    std::fs::write(dir.join("dns/map"), &map).expect("the resolver's map");
    let mount = format!("{}:/rig:ro", dir.display());
    let dns_mount = format!("{}:/dns:ro", dir.join("dns").display());
    let resolver = must(
        &[
            "create",
            "--label",
            &rig,
            "--network",
            &egress_net,
            "-v",
            &mount,
            "-v",
            &dns_mount,
            PYTHON_IMAGE,
            "python3",
            "-u",
            "/rig/resolver.py",
        ],
        "creating the resolver",
    );
    made.containers.push(resolver.clone());
    must(&["start", &resolver], "starting the resolver");
    let ip_resolver = ip_on(&resolver, &egress_net);

    // One proxy for the allowlist, made from the core's arguments plus the rig's resolver.
    let list = settle(Some(&json!(LISTED)), None)
        .expect("a list")
        .expect("not empty");
    let allow = NetworkName::allow_for(&job, &list);
    let allow_id = must(
        &[
            "network",
            "create",
            "--internal",
            "-o",
            "com.docker.network.bridge.inhibit_ipv4=true",
            "--ipv6=false",
            "--label",
            &format!("candler.job={job}"),
            "--label",
            &rig,
            allow.as_str(),
        ],
        "the allow network",
    );
    made.networks.push(allow_id);
    let mut create = egress_create_args(sidecar.id(), &job, &experiment, &list, &allow);
    let image = create.pop().expect("the image is last");
    create.extend(["--dns".into(), ip_resolver, "--label".into(), rig.clone()]);
    let bite_dir = dir.join("bite");
    std::fs::create_dir_all(&bite_dir).expect("a bite directory");
    std::fs::set_permissions(&bite_dir, std::fs::Permissions::from_mode(0o755)).expect("m");
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/docker/gateway");
    let tpl0 = std::fs::read_to_string(assets.join("egress.conf.template")).expect("tpl");
    let js0 = std::fs::read_to_string(assets.join("egress.js")).expect("egress.js");
    let (mut tpl, mut js) = (tpl0.clone(), js0.clone());
    match bite.as_str() {
        "pinned" => tpl = tpl.replace("valid=1s", "valid=3600s"),
        "no_filter" => {
            js = "function guard(s) {\n  s.on('upload', function (d, f) { s.send(d, f); });\n  s.on('download', function (d, f) { s.send(d, f); });\n}\nexport default { guard };\n".to_owned();
        }
        "first_entry_retry" => {
            js = js.replace(".split(',')", ".split(',').slice(0, 1)");
            tpl = tpl.replace("proxy_next_upstream off;", "");
        }
        "string_events" => {
            js = js
                .replace("'upstream'", "'upload'")
                .replace("'downstream'", "'download'");
        }
        _ => {}
    }
    for (name, text, original) in [
        ("egress.conf.template", &tpl, &tpl0),
        ("egress.js", &js, &js0),
    ] {
        if text != original {
            write_exec(&bite_dir, name, text);
            create.extend([
                "-v".into(),
                format!("{}:/etc/gw/{name}:ro", bite_dir.join(name).display()),
            ]);
        }
    }
    create.push(image);
    let mut full = vec![String::from("create")];
    full.extend(create);
    let proxy = must(
        &full.iter().map(String::as_str).collect::<Vec<_>>(),
        "creating the proxy",
    );
    made.containers.push(proxy.clone());
    must(
        &["network", "connect", &egress_net, &proxy],
        "the proxy joins the egress network",
    );
    must(
        &["network", "connect", &ll_net_id, &proxy],
        "the proxy joins the link-local rig network",
    );
    must(&["start", &proxy], "starting the proxy");
    let proxy_ip = ip_on(&proxy, allow.as_str());

    // Listeners on the host, for row 8: every address family the host could answer on.
    let listeners: Vec<HostListener> = ["0.0.0.0:0", "[::]:0"]
        .into_iter()
        .filter_map(HostListener::bind)
        .collect();

    // The agent: the job's internal network, its allow network, one --add-host per listed name.
    let base_url = handle.run_base_url(&run).to_string();
    let mut names: Vec<&str> = LISTED.iter().filter_map(|e| e.split(':').next()).collect();
    names.sort_unstable();
    let mut args: Vec<String> = [
        "create",
        "--label",
        &rig,
        "--init",
        "--network",
        &job_net,
        "--user",
        "1000:1000",
        "--add-host",
        "host.docker.internal:host-gateway",
        "--tmpfs",
        "/workspace:rw,mode=1777",
        "-e",
        &format!("ANTHROPIC_API_KEY={}", handle.dummy_token),
        "-e",
        &format!("ANTHROPIC_BASE_URL={base_url}"),
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    for name in &names {
        args.extend(["--add-host".into(), format!("{name}:{proxy_ip}")]);
    }
    args.extend([
        "--entrypoint".into(),
        "sleep".into(),
        agent_image.id().to_owned(),
        "3600".into(),
    ]);
    let agent = must(
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
        "creating the agent",
    );
    made.containers.push(agent.clone());
    // The agent's second network comes from the product: its allow network, or, under the bite,
    // the egress network of a `bridge` cell (the agent keeps its allow network to reach the proxy).
    let network = (bite == "agent_on_egress").then_some("bridge");
    let (joined_job, joined) = handle
        .agent_networks(&job, Some(list.as_slice()), network)
        .expect("the agent's networks");
    assert_eq!(joined_job.as_str(), job_net, "the agent's job network");
    let joined = joined.expect("a second network");
    if bite.is_empty() {
        assert_eq!(joined.as_str(), allow.as_str(), "the agent's allow network");
    }
    must(
        &["network", "connect", joined.as_str(), &agent],
        "the agent joins its second network",
    );
    if bite == "agent_on_egress" {
        must(
            &["network", "connect", allow.as_str(), &agent],
            "the bite: the agent keeps its allow network",
        );
    }
    must(&["start", &agent], "starting the agent");
    let job_gw = bridge_gateway(&job_net);
    let host_gw = bridge_gateway(allow.as_str());

    let sinks = [&allowed, &ta, &tb, &denied, &local, &low];
    let snap = || sinks.map(|c| rx(c));
    let failures: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
    let log: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
    // A row: run `act`, then compare each sink's received bytes with what the row expects.
    let row = |label: &str, expect_hit: &[usize], act: &dyn Fn()| {
        let before = snap();
        act();
        let after = snap();
        let delta: Vec<usize> = after.iter().zip(before).map(|(a, b)| a - b).collect();
        log.borrow_mut().push(format!(
            "row {label}: received [allowed, a, b, denied, loopback/link-local, low] = {delta:?}"
        ));
        for (i, got) in delta.iter().enumerate() {
            let want = expect_hit.contains(&i);
            if want && *got == 0 || !want && *got > 0 {
                failures.borrow_mut().push(format!(
                    "row {label}: sink {i} received {got} bytes, expected {}",
                    if want { ">0" } else { "0" }
                ));
            }
        }
    };
    // Wait until the proxy forwards, so the rows judge the filter and not the start.
    let ready = Instant::now() + Duration::from_secs(60);
    while rx(&allowed) == 0 && Instant::now() < ready {
        attempt(&agent, "allowed.test", 8080, PLAIN);
    }
    row("1 plain listed host:port", &[0], &|| {
        attempt(&agent, "allowed.test", 8080, PLAIN);
    });
    row("1 SNI a on shared port", &[1], &|| {
        attempt(&agent, "a.test", 9443, &hello(Some("a.test")));
    });
    row("1 SNI b on shared port", &[2], &|| {
        attempt(&agent, "b.test", 9443, &hello(Some("b.test")));
    });
    row("1 a port below 1024 under cap-drop", &[5], &|| {
        attempt(&agent, "low.test", 443, PLAIN);
    });
    // Row 2: a model call through the gateway reaches the fake.
    let call = "P=${ANTHROPIC_BASE_URL#http://gateway:8080}; { printf 'GET %s/v1/models HTTP/1.0\\r\\nHost: gateway\\r\\nAuthorization: Bearer %s\\r\\nx-api-key: %s\\r\\nContent-Length: 0\\r\\n\\r\\n' \"$P\" \"$ANTHROPIC_API_KEY\" \"$ANTHROPIC_API_KEY\"; sleep 2; } | nc -w 5 gateway 8080";
    let reply = docker(&["exec", &agent, "sh", "-c", call]).both();
    log.borrow_mut().push(format!(
        "row 2 gateway reply head: {}",
        reply.lines().next().unwrap_or("")
    ));
    if !reply.contains(" 200") {
        failures.borrow_mut().push(format!(
            "row 2: the model call was not answered 200: {reply:.200}"
        ));
    }
    // Row 12: a real TLS client, on the allow network, finishes a handshake with the TLS fake
    // through the proxy and reads one response.
    let tls = docker(&[
        "run",
        "--rm",
        "--label",
        &rig,
        "--network",
        allow.as_str(),
        "--add-host",
        &format!("upstream.test:{proxy_ip}"),
        "-v",
        &format!("{}:/fake:ro", dir.join("fake").display()),
        PYTHON_IMAGE,
        "python3",
        "-c",
        TLS_CLIENT,
    ]);
    let said = tls.both();
    log.borrow_mut().push(format!(
        "row 12 TLS client through the proxy: {}",
        said.trim()
    ));
    if !tls.ok() || !said.contains(" 200") {
        failures.borrow_mut().push(format!(
            "row 12: no TLS handshake and 200 through the proxy: {said:.400}"
        ));
    }
    // 2b is every 'must succeed' above: the listed names have no alias on any proxy network.
    row("3 another port of the listed host", &[], &|| {
        attempt(&agent, "allowed.test", 8081, PLAIN);
    });
    row("4 an unlisted host", &[], &|| {
        attempt(&agent, "denied.test", 8080, PLAIN);
        attempt(&agent, "unlisted.test", 9443, &hello(Some("denied.test")));
    });
    row("5 an IPv4 literal", &[], &|| {
        for (ip, port) in [
            (&ip_denied, 8080),
            (&ip_denied, 9443),
            (&ip_allowed, 8080),
            (&ip_a, 9443),
        ] {
            attempt(&agent, ip, port, PLAIN);
        }
    });
    row("6 an IPv6 literal", &[], &|| {
        for lit in ["::1", "fd00::1", "fe80::1"] {
            attempt(&agent, lit, 8080, PLAIN);
        }
    });
    row("7 an external DNS name", &[], &|| {
        for name in ["external.test", "example.com"] {
            attempt(&agent, name, 8080, PLAIN);
        }
    });
    let host_ll = host_link_local(&job_net);
    log.borrow_mut().push(match &host_ll {
        Some(ll) => format!("row 8 host link-local on the job bridge: {ll}"),
        None => {
            "row 8 the host has no link-local IPv6 on the job bridge: target skipped".to_owned()
        }
    });
    probe_host(
        "agent on the job and allow networks",
        &agent,
        &[&job_gw, &host_gw],
        host_ll.as_deref(),
        &listeners,
        &log,
        &failures,
    );
    // The no-egress shape: an agent on the job's internal network only.
    let bare = must(
        &[
            "create",
            "--label",
            &rig,
            "--init",
            "--network",
            &job_net,
            "--user",
            "1000:1000",
            "--add-host",
            "host.docker.internal:host-gateway",
            "--entrypoint",
            "sleep",
            agent_image.id(),
            "3600",
        ],
        "creating the no-egress agent",
    );
    made.containers.push(bare.clone());
    must(&["start", &bare], "starting the no-egress agent");
    probe_host(
        "no-egress agent on the job network only",
        &bare,
        &[&job_gw],
        host_ll.as_deref(),
        &listeners,
        &log,
        &failures,
    );
    row("9 wrong or missing SNI on a shared port", &[], &|| {
        attempt(&agent, "a.test", 9443, &hello(Some("denied.test")));
        attempt(&agent, "b.test", 9443, &hello(None));
        attempt(&agent, "a.test", 9443, PLAIN);
        attempt(&agent, "allowed.test", 8080, &hello(Some("denied.test")));
        // Names that resolve to a sink listening on the shared port, and a name listed elsewhere.
        for sni in ["rebind.test", "external.test", "allowed.test"] {
            attempt(&agent, "a.test", 9443, &hello(Some(sni)));
        }
    });
    // Row 10: a listed name that answers a decoy, then loopback, then link-local.
    row("10 warm: rebind.test answers the decoy", &[3], &|| {
        attempt(&agent, "rebind.test", 7003, PLAIN);
    });
    std::fs::write(
        dir.join("dns/map"),
        map.replace(
            &format!("rebind.test {ip_denied}"),
            &format!("rebind.test {ip_ll}"),
        ),
    )
    .expect("the flipped map");
    std::thread::sleep(Duration::from_secs(3));
    row(
        "10 loopback, link-local and rebound to link-local",
        &[],
        &|| {
            attempt(&agent, "loop.test", 7001, PLAIN);
            attempt(&agent, "ll.test", 7002, PLAIN);
            attempt(&agent, "rebind.test", 7003, PLAIN);
        },
    );
    // Row 10b: nginx tries a refusing address first, then the link-local sink; the retry must not
    // reach it, and the filter must refuse a session whose list holds a refused entry.
    row("10b [refusing address, link-local sink]", &[], &|| {
        for _ in 0..8 {
            attempt(&agent, "multi.test", 7004, PLAIN);
        }
    });
    row(
        "11 CONNECT and absolute URI through the gateway",
        &[],
        &|| {
            attempt(
                &agent,
                "gateway",
                8080,
                b"CONNECT denied.test:8080 HTTP/1.1\r\nHost: denied.test:8080\r\n\r\n",
            );
            attempt(
                &agent,
                "gateway",
                8080,
                b"GET http://denied.test:8080/ HTTP/1.0\r\nHost: denied.test:8080\r\n\r\n",
            );
            attempt(
                &agent,
                "gateway",
                8080,
                b"GET http://allowed.test:8080/ HTTP/1.0\r\nHost: allowed.test\r\n\r\n",
            );
        },
    );
    for line in log.borrow().iter() {
        eprintln!("{line}");
    }
    eprintln!(
        "sink logs: allowed={} ta={} tb={} denied={} local={}",
        rx(&allowed),
        rx(&ta),
        rx(&tb),
        rx(&denied),
        rx(&local)
    );
    eprintln!("local sink log:\n{}", docker(&["logs", &local]).both());
    eprintln!("proxy log:\n{}", docker(&["logs", &proxy]).both());
    eprintln!("rig wall time: {:?} (bite: {bite:?})", started.elapsed());
    made.sweep();
    let left = made.leftovers();
    assert!(left.is_empty(), "the rig left behind: {left:?}");
    let failures = failures.into_inner();
    assert!(
        failures.is_empty(),
        "the allowlist leaked:\n{}",
        failures.join("\n")
    );
}
