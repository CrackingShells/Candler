//! Helpers for the guards of the gateway: builders and fixtures, never expected values.
use std::fmt::Write as _;
use std::io::{Read as _, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};

use candler_schemas::primitives::JobId;

/// The directory of the sidecar image's build context.
pub const GATEWAY_CONTEXT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/gateway");

/// The upstream fake, written into the fake's own directory at test time.
pub const FAKE_SCRIPT: &str = include_str!("../../fixtures/gateway/tls_upstream.py");

/// The pinned Python image the fake runs in (3.12, slim-bookworm), by digest.
pub const PYTHON_IMAGE: &str =
    "python@sha256:935c3c5b12357a7e10caa9c1e6ca5692344d97cf229832ef584be2cec9fc03df";

/// The test agent image: busybox with `nc`, `wget` and `sh`.
pub const AGENT_IMAGE: &str = "candler-testagent:test";

/// The host name the fake's certificate is valid for and the sidecar forwards to.
pub const UPSTREAM_HOST: &str = "upstream.test";

/// The port the fake serves TLS on.
pub const UPSTREAM_PORT: u16 = 8443;

/// The name the gateway's token is resolved under.
pub const SECRET_NAME: &str = "model.token_secret";

/// The cell digest of the run keys these tests use.
pub const CELL_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// The experiment digest the sidecar is labelled with.
pub const EXPERIMENT: &str =
    "sha256:cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";

// ---------------------------------------------------------------------------------------------
// The docker command line, as a test uses it.
// ---------------------------------------------------------------------------------------------

/// What a command printed and how it ended.
pub struct Ran {
    /// The exit code, or none when a signal ended the command.
    pub code: Option<i32>,
    /// Standard output, as text.
    pub out: String,
    /// Standard error, as text.
    pub err: String,
}

impl Ran {
    /// Whether the command exited with code 0.
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }

    /// Standard output of a command that must have succeeded.
    pub fn must(self, what: &str) -> String {
        assert!(
            self.ok(),
            "{what} failed (exit {:?}): {}",
            self.code,
            self.err.trim()
        );
        self.out
    }

    /// Both streams as one text.
    pub fn both(&self) -> String {
        format!("{}\n{}", self.out, self.err)
    }
}

/// The environment of the docker client: what it needs to find its daemon, and nothing else.
pub fn client_env() -> Vec<(String, String)> {
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
pub fn run(program: &str, args: &[&str], stdin: &[u8]) -> Ran {
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

/// Runs `docker` with `args` and returns what it printed.
pub fn docker(args: &[&str]) -> Ran {
    run("docker", args, &[])
}

/// 128 random bits as lowercase hex, from the operating system.
pub fn random_hex() -> String {
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
pub fn random_job() -> JobId {
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

/// A random canary token: it stands in for the real one.
pub fn canary() -> String {
    // The slash makes the JSON-escaped form differ from the plain one.
    format!("cnd-canary-{}/{}", random_hex(), random_hex())
}

/// `path` as text; the path must be UTF-8.
pub fn path_text(path: &Path) -> String {
    String::from(path.to_str().expect("a utf-8 path"))
}

/// Makes a certificate authority `<name>.pem` with its key `<name>.key` in `dir`.
pub fn make_ca(dir: &Path, name: &str) {
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
pub fn make_server_cert(dir: &Path, ca: &str) {
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
