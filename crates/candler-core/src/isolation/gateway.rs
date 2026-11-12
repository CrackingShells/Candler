//! The gateway container the agent reaches its provider through. The agent holds a dummy token;
//! the gateway holds the real one and forwards the calls.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use candler_schemas::container::{ContainerLabels, ContainerRecord, ContainerRole};
use candler_schemas::primitives::{ContainerId, Digest, JobId, Marker, RunKey};
use candler_schemas::process::ExitStatus;
use futures::future::BoxFuture;
use url::Url;

use super::driver::DockerDriver;
use super::layout::{LABEL_EXPERIMENT, LABEL_JOB, LABEL_ROLE, LABEL_RUN};
use super::model::{ContainerRef, GatewayHandle, ImageRef, Labels, NetworkName};
use crate::supervise::Supervisor;
use crate::supervise::spawn::{SpawnRequest, StdinSpec};

/// Why a secret could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SecretError {
    /// The secret is not set.
    #[error("the secret `{name}` is not set")]
    Missing {
        /// The name of the secret; never its value.
        name: String,
    },
}

/// A secret's value. Its `Debug` never shows the value.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretValue(String);

impl SecretValue {
    /// Wraps a value.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// The value, for the one place that needs it.
    #[must_use]
    pub const fn expose<'a>(&'a self) -> &'a str {
        // BORROW: explicit .as_str() instead of Deref coercion
        self.0.as_str()
    }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretValue(<redacted>)")
    }
}

/// Where the real token comes from.
pub trait SecretSource: Send + Sync {
    /// The value of the secret `name`.
    ///
    /// # Errors
    /// [`SecretError::Missing`] when it is not set.
    fn resolve(&self, name: &str) -> Result<SecretValue, SecretError>;
}

/// Why the gateway could not start or be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GatewayError {
    /// The gateway is not implemented.
    #[error("gateway is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
    /// The gateway container did not start.
    #[error("gateway did not start: {detail}")]
    Start {
        /// What went wrong; it never holds a secret.
        detail: String,
    },
    /// The secret could not be read.
    #[error(transparent)]
    Secret(#[from] SecretError),
    /// The secret has a character the gateway's configuration cannot carry. The error names the
    /// secret and never shows its value.
    #[error("the secret `{name}` holds a character the gateway cannot carry")]
    SecretFormat {
        /// The name of the secret.
        name: String,
    },
    /// The configuration of the gateway is unusable.
    #[error("the gateway is configured wrongly: {detail}")]
    Config {
        /// What is wrong.
        detail: String,
    },
    /// The gateway started but does not serve.
    #[error("the gateway is not ready: {cause}")]
    NotReady {
        /// Why not.
        cause: NotReadyCause,
    },
}

/// Why a started gateway does not serve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum NotReadyCause {
    /// Nothing answered within the retries.
    #[error("no answer")]
    NoAnswer,
    /// The upstream refused the credential.
    #[error("the upstream answered 401")]
    Unauthorized,
    /// The sidecar's concurrency cap answered 429.
    #[error("the concurrency cap answered 429")]
    Capped,
    /// The upstream answered with another error status.
    #[error("the upstream answered {0}")]
    Upstream(u16),
}

/// Whether the gateway can serve.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GatewayStatus {
    /// The gateway answers.
    Ready,
    /// The gateway cannot serve.
    Unavailable {
        /// Why, as a reason code.
        reason: String,
    },
}

/// What the gateway saw of one run's provider calls.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GatewaySignal {
    /// The upstream answered with this status.
    UpstreamStatus(u16),
    /// The upstream could not be reached.
    Unreachable,
    /// The gateway's own probe failed.
    ProbeFailed,
}

/// Writes a job's container record, `children/container-<cid12>.json`. The job directory
/// implements it; the gateway calls it after `docker create` and before the sidecar holds the
/// token, so a teardown of the current job finds the sidecar recorded.
pub trait ChildRecorder: Send + Sync {
    /// Writes `record`.
    ///
    /// # Errors
    /// The I/O error that kept the record from being written.
    fn record_child(&self, record: &ContainerRecord) -> std::io::Result<()>;
}

/// What starting a job's gateway needs.
#[derive(Clone, Copy)]
pub struct GatewayStart<'a> {
    /// The job. Its internal network is `NetworkName::for_job`, its egress network
    /// `NetworkName::egress_for_job`; `start` creates both.
    pub job: &'a JobId,
    /// The experiment digest, the value of the sidecar's `candler.experiment` label. Its
    /// `candler.run` label is the literal `gateway`, which no run's path form equals.
    pub experiment: &'a Digest,
    /// Where the sidecar's container record is written.
    // TRAIT_OBJECT: the job directory, or a recording fake in tests
    pub recorder: &'a dyn ChildRecorder,
    /// Resolves the secret.
    // TRAIT_OBJECT: the secret store, or a fixed fake in tests
    pub token: &'a dyn SecretSource,
    /// The name of the secret, the one `model.token_secret` gives (D9).
    pub secret: &'a str,
}

/// Starts the gateway of a job.
pub trait Gateway: Send + Sync {
    /// Starts the gateway of `request.job`: creates the job's internal and egress networks,
    /// creates the sidecar, records it, then hands it the secret.
    ///
    /// # Errors
    /// [`GatewayError::Start`] when the container does not start.
    fn start<'a>(
        &'a self,
        sup: &'a Supervisor,
        docker: &'a dyn DockerDriver, // TRAIT_OBJECT: the Docker driver is the real one or a fake in tests
        request: GatewayStart<'a>,
    ) -> BoxFuture<'a, Result<GatewayHandle, GatewayError>>;

    /// Asks the gateway whether it can serve.
    fn probe<'a>(&'a self, handle: &'a GatewayHandle) -> BoxFuture<'a, GatewayStatus>;

    /// What the gateway saw of the run `run`'s calls.
    ///
    /// # Errors
    /// [`GatewayError`] when the gateway cannot be asked.
    fn run_signals<'a>(
        &'a self,
        handle: &'a GatewayHandle,
        run: &'a RunKey,
    ) -> BoxFuture<'a, Result<Vec<GatewaySignal>, GatewayError>>;

    /// The model requests the gateway relayed for the run `run`, one JSON line each:
    /// `{"seq":<n>,"tools":["<name>",...]}`, the tool names of the request and nothing else of
    /// it, in the order the requests began. A gateway that records none answers no bytes.
    ///
    /// # Errors
    /// [`GatewayError`] when the gateway cannot be asked.
    fn run_census<'a>(
        &'a self,
        handle: &'a GatewayHandle,
        run: &'a RunKey,
    ) -> BoxFuture<'a, Result<Vec<u8>, GatewayError>> {
        let _ = (handle, run);
        Box::pin(async { Ok(Vec::new()) })
    }
}

/// The name the agent's container resolves the gateway by on the job's network.
const GATEWAY_ALIAS: &str = "gateway";

/// The port nginx listens on inside the sidecar.
const GATEWAY_PORT: u16 = 8080;

/// The value of `candler.role` and of `candler.run` on the sidecar.
const GATEWAY_ROLE: &str = "gateway";

/// How much of a docker command's output is kept.
const CAPTURE_LIMIT: usize = 4 * 1024 * 1024;

/// How long one docker command may run.
const COMMAND_DEADLINE: Duration = Duration::from_secs(90);

/// How many readiness probes are tried before the sidecar is called not ready.
const READY_ATTEMPTS: u32 = 40;

/// The pause between two readiness probes.
const READY_PAUSE: Duration = Duration::from_millis(500);

/// The readiness probe: it proves nginx, the token and the upstream path, where `/healthz`
/// proves nginx only.
const READY_URL: &str = "http://127.0.0.1:8080/v1/models";

/// The credential file name nginx includes.
const AUTH_FILE: &str = "auth.conf";

/// The file that holds the certificate authority the upstream's certificate must chain to.
const CA_FILE: &str = "ca.pem";

/// How a docker command is run: through the supervisor, with the docker client's own
/// environment, and never with the secret.
#[derive(Debug, Clone)]
pub struct DockerCli {
    program: String,
    env: Vec<(String, String)>,
    marker: Marker,
    record_dir: PathBuf,
    cid_dir: PathBuf,
}

impl DockerCli {
    /// A runner of the `docker` program with `env` as the docker client's whole environment,
    /// `marker` on every child, process records in `record_dir` and container id files in
    /// `cid_dir`.
    #[must_use]
    pub fn new(
        env: Vec<(String, String)>,
        marker: Marker,
        record_dir: PathBuf,
        cid_dir: PathBuf,
    ) -> Self {
        Self {
            // BORROW: .to_owned() keeps the program name
            program: String::from("docker"),
            env,
            marker,
            record_dir,
            cid_dir,
        }
    }

    /// The same runner with another docker program.
    #[must_use]
    pub fn program(mut self, program: &str) -> Self {
        // BORROW: .to_owned() keeps the program name
        program.clone_into(&mut self.program);
        self
    }

    fn request(&self, args: Vec<String>, stdin: StdinSpec) -> Result<SpawnRequest, GatewayError> {
        let mut command = Vec::with_capacity(args.len().saturating_add(1));
        // BORROW: .clone() keeps the runner's program for later commands
        command.push(self.program.clone());
        command.extend(args);
        SpawnRequest::builder()
            .argv(command)
            // BORROW: .clone() keeps the runner's environment for later commands
            .env(self.env.clone())
            // BORROW: .clone() keeps the runner's marker for later commands
            .marker(self.marker.clone())
            // BORROW: .clone() keeps the runner's record directory for later commands
            .record_dir(self.record_dir.clone())
            .stdin(stdin)
            .deadline(COMMAND_DEADLINE)
            .build()
            .map_err(|error| GatewayError::Start {
                detail: error.to_string(),
            })
    }

    /// Runs `docker <args>` with `stdin` and returns its exit code, standard output and error.
    async fn run(
        &self,
        sup: &Supervisor,
        args: Vec<String>,
        stdin: StdinSpec,
    ) -> Result<Output, GatewayError> {
        let request = self.request(args, stdin)?;
        let captured = sup
            .run_captured(request, CAPTURE_LIMIT)
            .await
            .map_err(|error| GatewayError::Start {
                detail: error.to_string(),
            })?;
        let code = match captured.exit.status {
            ExitStatus::Code { code } => Some(code),
            ExitStatus::Signal { .. } => None,
        };
        Ok(Output {
            code,
            stdout: String::from_utf8_lossy(&captured.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&captured.stderr).into_owned(),
        })
    }
}

/// What a docker command printed and how it ended.
struct Output {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl Output {
    const fn succeeded(&self) -> bool {
        matches!(self.code, Some(0))
    }

    /// A short, secret-free account of a failure, for an error.
    fn failure(&self, what: &str) -> GatewayError {
        let text: String = self.stderr.trim().chars().take(300).collect();
        GatewayError::Start {
            detail: format!("{what} failed (exit {:?}): {text}", self.code),
        }
    }
}

/// Where the gateway forwards to, and how it trusts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamConfig {
    host: String,
    port: u16,
    ca_pem: Option<Vec<u8>>,
}

impl UpstreamConfig {
    /// Starts a configuration; `host` must be set before it builds.
    #[must_use]
    pub fn builder() -> UpstreamConfigBuilder {
        UpstreamConfigBuilder::default()
    }

    fn authority(&self) -> String {
        if self.port == 443 {
            // BORROW: .clone() keeps the host in the configuration
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

/// Builds an [`UpstreamConfig`].
#[derive(Debug, Clone, Default)]
pub struct UpstreamConfigBuilder {
    host: Option<String>,
    port: Option<u16>,
    ca_pem: Option<Vec<u8>>,
}

impl UpstreamConfigBuilder {
    /// Sets the upstream's host name; the certificate must be valid for it.
    #[must_use]
    pub fn host(mut self, host: &str) -> Self {
        // BORROW: .to_owned() keeps the host
        self.host = Some(host.to_owned());
        self
    }

    /// Sets the upstream's port; the default is 443.
    #[must_use]
    pub const fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Sets the certificate authority, in PEM, that the upstream's certificate must chain to.
    /// Without it the sidecar uses the image's system bundle.
    #[must_use]
    pub fn ca_pem(mut self, pem: Vec<u8>) -> Self {
        self.ca_pem = Some(pem);
        self
    }

    /// Builds the configuration.
    ///
    /// # Errors
    /// [`GatewayError::Config`] when `host` is not set, or holds a character outside letters,
    /// digits, `.`, `_` and `-`.
    pub fn build(self) -> Result<UpstreamConfig, GatewayError> {
        let host = self.host.ok_or_else(|| GatewayError::Config {
            // BORROW: .to_owned() keeps the message
            detail: String::from("the upstream host is not set"),
        })?;
        let plain = !host.is_empty()
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        if !plain {
            return Err(GatewayError::Config {
                // BORROW: .to_owned() keeps the message
                detail: String::from("the upstream host is not a plain name"),
            });
        }
        Ok(UpstreamConfig {
            host,
            port: self.port.unwrap_or(443),
            ca_pem: self.ca_pem,
        })
    }
}

/// The nginx sidecar gateway: design B, an `--internal` network per job and one sidecar that
/// holds the real token in a tmpfs file written through `docker exec -i`.
pub struct DockerGateway {
    image: ImageRef,
    upstream: UpstreamConfig,
    max_parallel: u32,
    cli: DockerCli,
    sup: Arc<Supervisor>,
}

impl DockerGateway {
    /// A gateway from the sidecar image `image`, forwarding to `upstream`, with at most
    /// `max_parallel` calls at once. `sup` runs every docker command after `start`.
    #[must_use]
    pub const fn new(
        image: ImageRef,
        upstream: UpstreamConfig,
        max_parallel: u32,
        cli: DockerCli,
        sup: Arc<Supervisor>,
    ) -> Self {
        Self {
            image,
            upstream,
            max_parallel,
            cli,
            sup,
        }
    }

    /// The reader of the sidecar's logs.
    #[must_use]
    pub fn logs(&self) -> GatewayLogs {
        GatewayLogs {
            // BORROW: .clone() shares the runner
            cli: self.cli.clone(),
            // BORROW: .clone() shares the supervisor
            sup: Arc::clone(&self.sup),
        }
    }

    /// Writes the credential into the sidecar's tmpfs again, after `docker restart` emptied it,
    /// and waits until the sidecar serves.
    ///
    /// # Errors
    /// [`GatewayError::Secret`] when the secret cannot be read, [`GatewayError::Start`] when the
    /// write fails and [`GatewayError::NotReady`] when the sidecar does not serve afterwards.
    pub async fn reinject<'a>(
        &'a self,
        handle: &'a GatewayHandle,
        token: &'a dyn SecretSource, // TRAIT_OBJECT: the secret store, or a fixed fake in tests
        secret: &'a str,
    ) -> Result<(), GatewayError> {
        let auth = auth_conf(token, secret, &handle.dummy_token)?;
        let cid = handle.container.cid.to_string();
        self.inject(&self.sup, &cid, auth).await?;
        self.wait_ready(&self.sup, &cid, &handle.dummy_token).await
    }

    /// Writes the certificate authority, when there is one, then the credential.
    async fn inject(&self, sup: &Supervisor, cid: &str, auth: Vec<u8>) -> Result<(), GatewayError> {
        if let Some(pem) = &self.upstream.ca_pem {
            // BORROW: .clone() hands the pem to the request
            self.write_file(sup, cid, CA_FILE, pem.clone()).await?;
        }
        self.write_file(sup, cid, AUTH_FILE, auth).await
    }

    /// Writes `bytes` to `/run/gw/<name>` through `auth.tmp`-style `mv`, from stdin.
    async fn write_file(
        &self,
        sup: &Supervisor,
        cid: &str,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<(), GatewayError> {
        let script =
            format!("umask 077; cat > /run/gw/{name}.tmp && mv /run/gw/{name}.tmp /run/gw/{name}");
        // BORROW: explicit .as_str() instead of Deref coercion
        let args = ["exec", "-i", cid, "sh", "-c", script.as_str()]
            .into_iter()
            // BORROW: .to_owned() builds the owned argv
            .map(str::to_owned)
            .collect();
        let out = self.cli.run(sup, args, StdinSpec::Bytes(bytes)).await?;
        if out.succeeded() {
            Ok(())
        } else {
            Err(GatewayError::Start {
                detail: format!("writing /run/gw/{name} failed (exit {:?})", out.code),
            })
        }
    }

    /// One readiness probe, made with the job's dummy token: `Ok(())`, or why not.
    async fn ready_once(
        &self,
        sup: &Supervisor,
        cid: &str,
        dummy: &str,
    ) -> Result<(), NotReadyCause> {
        let header = format!("x-api-key: {dummy}");
        // BORROW: explicit .as_str() instead of Deref coercion
        let args = [
            "exec",
            cid,
            "wget",
            "-q",
            "-T",
            "2",
            "-O-",
            "--header",
            // BORROW: explicit .as_str() instead of Deref coercion
            header.as_str(),
            READY_URL,
        ]
        .into_iter()
        // BORROW: .to_owned() builds the owned argv
        .map(str::to_owned)
        .collect();
        let Ok(out) = self.cli.run(sup, args, StdinSpec::Null).await else {
            return Err(NotReadyCause::NoAnswer);
        };
        if out.succeeded() {
            return Ok(());
        }
        Err(match http_status(&out.stderr) {
            Some(401) => NotReadyCause::Unauthorized,
            Some(429) => NotReadyCause::Capped,
            Some(status) => NotReadyCause::Upstream(status),
            None => NotReadyCause::NoAnswer,
        })
    }

    /// Probes until the sidecar serves, or the retries run out or the answer is final.
    async fn wait_ready(
        &self,
        sup: &Supervisor,
        cid: &str,
        dummy: &str,
    ) -> Result<(), GatewayError> {
        let mut cause = NotReadyCause::NoAnswer;
        for _ in 0..READY_ATTEMPTS {
            match self.ready_once(sup, cid, dummy).await {
                Ok(()) => return Ok(()),
                Err(found @ (NotReadyCause::Unauthorized | NotReadyCause::Capped)) => {
                    return Err(GatewayError::NotReady { cause: found });
                }
                Err(found @ (NotReadyCause::NoAnswer | NotReadyCause::Upstream(_))) => {
                    cause = found;
                }
            }
            tokio::time::sleep(READY_PAUSE).await;
        }
        Err(GatewayError::NotReady { cause })
    }

    /// The arguments of `docker create` for the sidecar.
    fn create_args(
        &self,
        request: &GatewayStart<'_>,
        net: &NetworkName,
        cidfile: &str,
    ) -> Vec<String> {
        let labels = sidecar_labels(request);
        let mut args: Vec<String> = ["create", "--init", "--read-only"]
            .into_iter()
            // BORROW: .to_owned() builds the owned argv
            .map(str::to_owned)
            .collect();
        for (key, value) in labels.iter() {
            args.push(String::from("--label"));
            args.push(format!("{key}={value}"));
        }
        let tmpfs = |path: &str, mode: &str, size: &str| {
            format!("{path}:rw,mode={mode},uid=101,gid=101,size={size},noexec,nosuid")
        };
        let rest = [
            String::from("--network"),
            // BORROW: explicit .as_str() instead of Deref coercion
            net.as_str().to_owned(),
            String::from("--network-alias"),
            String::from(GATEWAY_ALIAS),
            String::from("--tmpfs"),
            tmpfs("/run/gw", "0700", "64k"),
            String::from("--tmpfs"),
            tmpfs("/tmp", "1777", "8m"),
            String::from("--tmpfs"),
            tmpfs("/var/cache/nginx", "0700", "64m"),
            String::from("--cap-drop"),
            String::from("ALL"),
            String::from("--security-opt"),
            String::from("no-new-privileges"),
            String::from("--pids-limit"),
            String::from("128"),
            String::from("--memory"),
            String::from("256m"),
            String::from("--cidfile"),
            // BORROW: explicit .to_owned() instead of Deref coercion
            cidfile.to_owned(),
            String::from("-e"),
            format!("GW_UPSTREAM_HOST={}", self.upstream.host),
            String::from("-e"),
            format!("GW_UPSTREAM_AUTHORITY={}", self.upstream.authority()),
            String::from("-e"),
            format!("GW_MAX_PARALLEL={}", self.max_parallel),
            // BORROW: .clone() keeps the image id in the gateway
            self.image.id.0.clone(),
        ];
        args.extend(rest);
        args
    }

    /// The `container-record/1` of the created sidecar.
    async fn record_of(
        &self,
        request: &GatewayStart<'_>,
        cid: &ContainerId,
        cid_text: &str,
        docker: &dyn DockerDriver, // TRAIT_OBJECT: the Docker driver is the real one or a fake in tests
    ) -> Result<ContainerRecord, GatewayError> {
        let failed = |detail: String| GatewayError::Start { detail };
        let inspect = docker
            .inspect(cid_text)
            .await
            .map_err(|error| failed(error.to_string()))?;
        // BORROW: explicit .as_str() instead of Deref coercion
        let image_id: Digest = inspect
            .image
            .0
            // BORROW: explicit .as_str() instead of Deref coercion
            .as_str()
            .parse()
            .map_err(|_| failed(String::from("the sidecar's image id is not a digest")))?;
        let run: Marker = GATEWAY_ROLE
            .parse()
            .map_err(|_| failed(String::from("the gateway marker is not valid")))?;
        // BORROW: .clone() keeps the job and the experiment in the request
        let labels = ContainerLabels::new(request.job.clone(), run, request.experiment.clone());
        let spawned_at = crate::supervise::now().map_err(|error| failed(error.to_string()))?;
        ContainerRecord::builder()
            // BORROW: .clone() keeps the id for the handle
            .cid(cid.clone())
            .labels(labels)
            .created_at(inspect.created)
            .image_id(image_id)
            .role(ContainerRole::Gateway)
            .spawned_at(spawned_at)
            .build()
            .map_err(|error| failed(error.to_string()))
    }

    async fn start_sidecar<'a>(
        &'a self,
        sup: &'a Supervisor,
        docker: &'a dyn DockerDriver, // TRAIT_OBJECT: the Docker driver is the real one or a fake in tests
        request: GatewayStart<'a>,
    ) -> Result<GatewayHandle, GatewayError> {
        let job = request.job;
        let net = NetworkName::for_job(job);
        let egress = NetworkName::egress_for_job(job);
        // The secret is read before anything is created, so a missing one leaves nothing behind.
        let dummy = format!("cnd-dummy-{}", random_hex()?);
        let auth = auth_conf(request.token, request.secret, &dummy)?;
        let docker_err = |error: super::docker::DockerError| GatewayError::Start {
            detail: error.to_string(),
        };

        // BORROW: explicit .as_str() instead of Deref coercion
        let labels = Labels::new().with(LABEL_JOB, job.as_str());
        docker
            .network_create(&net, true, &labels)
            .await
            .map_err(docker_err)?;
        docker
            .network_create(&egress, false, &labels)
            .await
            .map_err(docker_err)?;
        let info = docker.network_inspect(&net).await.map_err(docker_err)?;
        if !info.internal {
            return Err(GatewayError::Start {
                detail: String::from("the job's network is not internal"),
            });
        }

        // BORROW: explicit .as_str() instead of Deref coercion
        let cidfile = self.cli.cid_dir.join(format!("{}.cid", net.as_str()));
        let cidfile_text = cidfile.to_string_lossy().into_owned();
        // BORROW: explicit .as_str() instead of Deref coercion
        let args = self.create_args(&request, &net, cidfile_text.as_str());
        let created = self.cli.run(sup, args, StdinSpec::Null).await?;
        if !created.succeeded() {
            return Err(created.failure("docker create"));
        }
        // BORROW: explicit .to_owned() instead of Deref coercion
        let cid_text = created.stdout.trim().to_owned();
        let cid: ContainerId = cid_text.parse().map_err(|_| GatewayError::Start {
            detail: String::from("docker create printed no container id"),
        })?;

        // The sidecar is recorded before it joins a network, starts or holds the token, so a
        // teardown of the job finds it whatever happens next.
        let record = self.record_of(&request, &cid, &cid_text, docker).await?;
        request
            .recorder
            .record_child(&record)
            .map_err(|error| GatewayError::Start {
                detail: format!("the sidecar's container record was not written: {error}"),
            })?;

        // The sidecar needs a way out to its upstream; the agent never joins this network.
        for step in [
            vec![
                String::from("network"),
                String::from("connect"),
                // BORROW: explicit .to_owned() instead of Deref coercion
                egress.as_str().to_owned(),
                // BORROW: .clone() keeps the id for the next steps
                cid_text.clone(),
            ],
            // BORROW: .clone() keeps the id for the next steps
            vec![String::from("start"), cid_text.clone()],
        ] {
            let out = self.cli.run(sup, step, StdinSpec::Null).await?;
            if !out.succeeded() {
                return Err(out.failure("docker network connect or start"));
            }
        }

        // BORROW: explicit .as_str() instead of Deref coercion
        self.inject(sup, cid_text.as_str(), auth).await?;
        // BORROW: explicit .as_str() instead of Deref coercion
        self.wait_ready(sup, cid_text.as_str(), &dummy).await?;

        let inspect = docker
            // BORROW: explicit .as_str() instead of Deref coercion
            .inspect(cid_text.as_str())
            .await
            .map_err(docker_err)?;
        let base_url =
            Url::parse(&format!("http://{GATEWAY_ALIAS}:{GATEWAY_PORT}")).map_err(|error| {
                GatewayError::Config {
                    detail: error.to_string(),
                }
            })?;
        GatewayHandle::new(
            ContainerRef::new(cid, inspect.labels, inspect.created),
            base_url,
            net,
            egress,
            dummy,
        )
        .map_err(|error| GatewayError::Start {
            detail: format!("no random bytes: {error}"),
        })
    }
}

impl Gateway for DockerGateway {
    fn start<'a>(
        &'a self,
        sup: &'a Supervisor,
        docker: &'a dyn DockerDriver, // TRAIT_OBJECT: the Docker driver is the real one or a fake in tests
        request: GatewayStart<'a>,
    ) -> BoxFuture<'a, Result<GatewayHandle, GatewayError>> {
        Box::pin(self.start_sidecar(sup, docker, request))
    }

    fn probe<'a>(&'a self, handle: &'a GatewayHandle) -> BoxFuture<'a, GatewayStatus> {
        Box::pin(async move {
            let cid = handle.container.cid.to_string();
            // BORROW: explicit .as_str() instead of Deref coercion
            match self
                // BORROW: explicit .as_str() instead of Deref coercion
                .ready_once(&self.sup, cid.as_str(), &handle.dummy_token)
                .await
            {
                Ok(()) => GatewayStatus::Ready,
                Err(cause) => GatewayStatus::Unavailable {
                    reason: match cause {
                        NotReadyCause::NoAnswer => String::from("no_answer"),
                        NotReadyCause::Unauthorized => String::from("unauthorized"),
                        NotReadyCause::Capped => String::from("capped"),
                        NotReadyCause::Upstream(status) => format!("upstream_{status}"),
                    },
                },
            }
        })
    }

    fn run_signals<'a>(
        &'a self,
        handle: &'a GatewayHandle,
        run: &'a RunKey,
    ) -> BoxFuture<'a, Result<Vec<GatewaySignal>, GatewayError>> {
        Box::pin(async move {
            let log = self.logs().read(handle).await?;
            // BORROW: explicit .as_str() instead of Deref coercion
            Ok(signals_of(log.as_str(), run, handle.run_tag(run).as_str()))
        })
    }

    fn run_census<'a>(
        &'a self,
        handle: &'a GatewayHandle,
        run: &'a RunKey,
    ) -> BoxFuture<'a, Result<Vec<u8>, GatewayError>> {
        Box::pin(async move {
            let log = self.logs().read(handle).await?;
            // BORROW: explicit .as_str() instead of Deref coercion
            Ok(census_of(log.as_str(), run, handle.run_tag(run).as_str()))
        })
    }
}

/// Reads the sidecar's logs: the access log (method, path, status, model, run; never the
/// `Authorization` header or a prompt) and nginx's error log.
#[derive(Debug, Clone)]
pub struct GatewayLogs {
    cli: DockerCli,
    sup: Arc<Supervisor>,
}

impl GatewayLogs {
    /// The sidecar's standard output, then its standard error, as text.
    ///
    /// # Errors
    /// [`GatewayError::Start`] when `docker logs` fails.
    pub fn read<'a>(
        &'a self,
        handle: &'a GatewayHandle,
    ) -> BoxFuture<'a, Result<String, GatewayError>> {
        Box::pin(async move {
            let args = vec![String::from("logs"), handle.container.cid.to_string()];
            let out = self.cli.run(&self.sup, args, StdinSpec::Null).await?;
            if !out.succeeded() {
                return Err(out.failure("docker logs"));
            }
            Ok(format!("{}\n{}", out.stdout, out.stderr))
        })
    }
}

/// The labels of the sidecar: the job, the role, a run value that is no run's, and the
/// experiment.
fn sidecar_labels(request: &GatewayStart<'_>) -> Labels {
    Labels::new()
        // BORROW: explicit .as_str() instead of Deref coercion
        .with(LABEL_JOB, request.job.as_str())
        .with(LABEL_ROLE, GATEWAY_ROLE)
        .with(LABEL_RUN, GATEWAY_ROLE)
        // BORROW: explicit .as_str() instead of Deref coercion
        .with(LABEL_EXPERIMENT, request.experiment.to_string().as_str())
}

/// The credential lines nginx includes, from the secret `secret`, and the job's dummy token.
/// This is the one function that holds the value; the lines go to the sidecar on stdin and
/// nowhere else.
// TRAIT_OBJECT: the secret store, or a fixed fake in tests
fn auth_conf(token: &dyn SecretSource, secret: &str, dummy: &str) -> Result<Vec<u8>, GatewayError> {
    use std::fmt::Write as _;

    let value = token.resolve(secret)?;
    let plain = {
        let text = value.expose();
        !text.is_empty()
            && text.chars().all(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '~' | '+' | '/' | '=' | '-')
            })
    };
    if !plain {
        return Err(GatewayError::SecretFormat {
            // BORROW: .to_owned() names the secret in the error
            name: secret.to_owned(),
        });
    }
    let text = value.expose();
    // The body scrub names the token as written, JSON-escaped (`/` as `\/`) and in base64.
    let mut forms = vec![
        String::from(text),
        text.replace('/', "\\\\/"),
        base64(text.as_bytes()),
    ];
    forms.dedup();
    let mut scrub = String::new();
    for form in &forms {
        let _ = writeln!(scrub, "sub_filter \"{form}\" \"<redacted>\";");
    }
    Ok(format!(
        "set $gw_token \"{text}\";\n\
         set $gw_dummy \"{dummy}\";\n\
         proxy_set_header Authorization \"Bearer {text}\";\n\
         proxy_set_header x-api-key \"{text}\";\n\
         {scrub}"
    )
    .into_bytes())
}

/// The first HTTP status that a `wget` error message carries, such as `HTTP/1.1 401 Unauthorized`.
fn http_status(text: &str) -> Option<u16> {
    let after = text.split("HTTP/").nth(1)?;
    after.split_whitespace().nth(1)?.parse().ok()
}

/// What the access log says of the run `run` under its tag `tag`: one signal per failed call
/// of the upstream.
/// A line counts only when it names this run and this tag once each, so an agent's path or a
/// call to another run's address cannot move a signal onto a run.
fn signals_of(log: &str, run: &RunKey, tag: &str) -> Vec<GatewaySignal> {
    let wanted_run = run.to_string();
    let mut signals = Vec::new();
    for line in log.lines() {
        // The quoted path is the last field and the only one an agent writes; it is cut off.
        let head = line.split(" path=\"").next().unwrap_or_default();
        let mut fields: Vec<(&str, &str)> = Vec::new();
        for part in head.split_whitespace() {
            if let Some(pair) = part.split_once('=') {
                fields.push(pair);
            }
        }
        let value = |key: &str| {
            let mut found = fields.iter().filter(|(name, _)| *name == key);
            let first = found.next().map(|(_, text)| *text);
            // A key given twice makes the line unreadable.
            if found.next().is_some() { None } else { first }
        };
        // BORROW: explicit .as_str() instead of Deref coercion
        if value("run") != Some(wanted_run.as_str()) || value("tag") != Some(tag) {
            continue;
        }
        // The upstream's own status; a line without one is the gateway's answer (a refused
        // key, the cap) and says nothing of the upstream.
        let Some(status) = value("up_status").and_then(|text| text.parse::<u16>().ok()) else {
            continue;
        };
        match status {
            502..=504 => signals.push(GatewaySignal::Unreachable),
            400..=501 | 505..=599 => signals.push(GatewaySignal::UpstreamStatus(status)),
            0..=399 | 600..=u16::MAX => {}
        }
    }
    signals
}

/// The most tool names one request's census line may carry.
const CENSUS_TOOL_LIMIT: usize = 1024;

/// The census of the run `run` under its tag `tag`, from the sidecar's log: one line
/// `{"seq":<n>,"tools":[...]}` for each census line of the gateway whose `run=` and `tag=` fields
/// equal these. A census line is `census run=<key> tag=<tag> seq=<n> tools=<JSON array>`; the
/// array is the last field and must be a list of strings, so what an agent writes in a tool name
/// cannot become a field. Only the sequence number and the names are kept: whatever else a line
/// carries is dropped, and a line that is not exactly that shape is dropped whole.
#[must_use]
pub fn census_of(log: &str, run: &RunKey, tag: &str) -> Vec<u8> {
    let wanted_run = format!("run={run}");
    let wanted_tag = format!("tag={tag}");
    let mut out = Vec::new();
    for line in log.lines() {
        let mut parts = line.splitn(5, ' ');
        let (Some("census"), Some(found_run), Some(found_tag), Some(found_seq), Some(rest)) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            continue;
        };
        if found_run != wanted_run || found_tag != wanted_tag {
            continue;
        }
        let Some(seq) = found_seq
            .strip_prefix("seq=")
            .and_then(|text| text.parse::<u64>().ok())
        else {
            continue;
        };
        let Some(tools) = rest
            .strip_prefix("tools=")
            .and_then(|text| serde_json::from_str::<Vec<String>>(text).ok())
            .filter(|names| names.len() <= CENSUS_TOOL_LIMIT)
        else {
            continue;
        };
        if let Ok(mut record) = serde_json::to_vec(&serde_json::json!({"seq": seq, "tools": tools}))
        {
            record.push(b'\n');
            out.extend(record);
        }
    }
    out
}

/// 128 random bits as hex, from the operating system.
fn random_hex() -> Result<String, GatewayError> {
    use std::fmt::Write as _;
    use std::io::Read as _;

    let mut bytes = [0_u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| GatewayError::Start {
            detail: format!("no random bytes: {error}"),
        })?;
    let mut text = String::with_capacity(32);
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    Ok(text)
}

/// A source of a secret found by a scan: which named source held which form of the canary.
#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leak {
    /// The name of the source that held it.
    pub source: String,
    /// Which form was found.
    pub kind: LeakKind,
}

/// Which form of the canary a scan found, or that it scanned nothing.
#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LeakKind {
    /// The canary itself.
    Canary,
    /// The canary in base64.
    Base64,
    /// Its first 16 characters.
    Prefix,
    /// The source held no bytes at all, so the scan proves nothing.
    Empty,
}

/// Scans every named source for the canary, its base64 and its first 16 characters.
///
/// # Errors
/// A [`Leak`] naming the first source that holds one of them, or that is empty.
#[cfg(any(test, feature = "test-support"))]
pub fn assert_clean(canary: &str, haystacks: &[(&str, &[u8])]) -> Result<(), Leak> {
    let encoded = base64(canary.as_bytes());
    let prefix: String = canary.chars().take(16).collect();
    for (source, bytes) in haystacks {
        let leak = |kind| Leak {
            // BORROW: .to_owned() names the source in the leak
            source: (*source).to_owned(),
            kind,
        };
        if bytes.is_empty() {
            return Err(leak(LeakKind::Empty));
        }
        let holds = |needle: &str| {
            let needle = needle.as_bytes();
            !needle.is_empty() && bytes.windows(needle.len()).any(|window| window == needle)
        };
        if holds(canary) {
            return Err(leak(LeakKind::Canary));
        }
        // BORROW: explicit .as_str() instead of Deref coercion
        if holds(encoded.as_str()) {
            return Err(leak(LeakKind::Base64));
        }
        // BORROW: explicit .as_str() instead of Deref coercion
        if holds(prefix.as_str()) {
            return Err(leak(LeakKind::Prefix));
        }
    }
    Ok(())
}

/// Standard base64 with padding.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let mut group = [0_u8; 3];
        for (slot, byte) in group.iter_mut().zip(chunk) {
            *slot = *byte;
        }
        let [first, second, third] = group;
        let word = (u32::from(first) << 16) | (u32::from(second) << 8) | u32::from(third);
        for position in 0..4_usize {
            if position > chunk.len() {
                text.push('=');
            } else {
                let shift = 18_u32.saturating_sub(6 * u32::try_from(position).unwrap_or(0));
                let index = usize::try_from((word >> shift) & 0x3f).unwrap_or(0);
                text.push(char::from(ALPHABET.get(index).copied().unwrap_or(b'A')));
            }
        }
    }
    text
}
