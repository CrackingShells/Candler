//! The docker command line behind the driver traits.
//!
//! Every call runs through `Supervisor::run_captured`, so a docker client is a supervised child
//! with a record, a deadline and an exact environment. The binary is an absolute path found once
//! when the client is built; the child sees only the variables [`ClientEnv`] holds.
//!
//! [`DockerCli`] reads and runs. [`DockerDestroyer`] stops and removes; only the container guard
//! holds one. A destroy verb takes the full 64-digit id of a container, never a name or a prefix.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use candler_schemas::primitives::{ContainerId, Timestamp};
use candler_schemas::process::ExitStatus;
use futures::future::BoxFuture;
use serde::Deserialize;

use super::driver::{DestroyDriver, DockerDriver};
use super::model::{
    BuildRequest, ContainerRef, ImageId, ImageInfo, Inspect, InspectState, LabelSel, Labels,
    LayerDigest, NetworkInfo, NetworkName,
};
use crate::supervise::Supervisor;
use crate::supervise::signal::control_marker;
use crate::supervise::spawn::SpawnRequest;
use crate::supervise::stop::Ended;

/// The variables a docker client child may see: where the daemon is, which context names it, and
/// the home directory that holds the client's own configuration.
const CLIENT_VARIABLES: [&str; 3] = ["DOCKER_HOST", "DOCKER_CONTEXT", "HOME"];

/// How long a read, a stop or a remove may take before the supervisor ends the client.
pub(crate) const CALL_DEADLINE: Duration = Duration::from_mins(1);

/// How long a container that is asked to stop may take to exit before docker kills it, unless
/// the client is built with another grace.
const DEFAULT_STOP_GRACE: Duration = Duration::from_secs(5);

/// How long a build may take.
const BUILD_DEADLINE: Duration = Duration::from_mins(15);

/// How many bytes of a client's output are kept.
const OUTPUT_LIMIT: usize = 4 * 1024 * 1024;

/// How many characters of a client's error are shown in an error.
const DETAIL_LIMIT: usize = 400;

/// Why a docker call failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DockerError {
    /// The call is not implemented by this driver.
    #[error("docker call is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
    /// Docker exited with an error.
    #[error("docker exited with an error: {detail}")]
    Failed {
        /// What docker said; it never holds a secret.
        detail: String,
    },
    /// Docker's answer could not be read.
    #[error("docker's answer could not be read: {detail}")]
    Unreadable {
        /// What is wrong with it.
        detail: String,
    },
    /// Docker has no such container, network or image.
    #[error("docker has no such resource: {detail}")]
    NotFound {
        /// What docker said.
        detail: String,
    },
    /// A container call named something other than a full container id.
    #[error("not a full container id: {detail}")]
    NotAFullId {
        /// What was given.
        detail: String,
    },
    /// A network call named something other than a job's network.
    #[error("not a job network: {detail}")]
    NotAJobNetwork {
        /// What was given.
        detail: String,
    },
}

/// The environment a docker client child gets: only `DOCKER_HOST`, `DOCKER_CONTEXT` and `HOME`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientEnv {
    pairs: Vec<(String, String)>,
}

impl ClientEnv {
    /// The allowed variables of this process, as they are set.
    #[must_use]
    pub fn from_process() -> Self {
        let pairs = CLIENT_VARIABLES
            .iter()
            .filter_map(|name| {
                std::env::var(name)
                    .ok()
                    // BORROW: .to_owned() gives the pair its own name
                    .map(|value| ((*name).to_owned(), value))
            })
            .collect();
        Self { pairs }
    }

    /// The pairs of `given` whose name is allowed; every other pair is dropped.
    #[must_use]
    pub fn from_pairs(given: &[(String, String)]) -> Self {
        let pairs = given
            .iter()
            // BORROW: .as_str() compares the name with the allowlist
            .filter(|(name, _)| CLIENT_VARIABLES.contains(&name.as_str()))
            .cloned() // BORROW: .cloned() gives the env its own pairs
            .collect();
        Self { pairs }
    }

    /// The pairs, in the order they were given.
    #[must_use]
    pub fn pairs<'a>(&'a self) -> &'a [(String, String)] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.pairs.as_slice()
    }
}

/// Finds `docker` in the directories of `path`, as an absolute path.
fn find_docker(path: &OsString) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join("docker"))
        .find(|candidate| candidate.is_absolute() && candidate.is_file())
}

/// Runs docker clients one call at a time through the supervisor.
#[derive(Clone)]
struct Runner {
    supervisor: Arc<Supervisor>,
    binary: PathBuf,
    env: ClientEnv,
    record_dir: PathBuf,
}

impl Runner {
    /// Runs `docker <args>` and returns what it printed on standard output.
    async fn run(&self, rest: Vec<String>, deadline: Duration) -> Result<String, DockerError> {
        let marker = control_marker();
        let mut argv = Vec::with_capacity(rest.len() + 1);
        // BORROW: .to_string_lossy().into_owned() gives the request the binary path as text
        argv.push(self.binary.to_string_lossy().into_owned());
        argv.extend(rest);
        let request = SpawnRequest::builder()
            .argv(argv)
            .env(self.env.pairs().to_vec()) // BORROW: .to_vec() gives the request its own pairs
            .marker(marker)
            .record_dir(self.record_dir.clone()) // BORROW: .clone() gives the request its own path
            .deadline(deadline)
            .build()
            .map_err(|error| DockerError::Failed {
                detail: error.to_string(),
            })?;
        let captured = self
            .supervisor
            .run_captured(request, OUTPUT_LIMIT)
            .await
            .map_err(|error| DockerError::Failed {
                detail: format!("the docker client could not run: {error}"),
            })?;
        let stderr = detail_of(&captured.stderr);
        match captured.exit.ended {
            Ended::Exited => match captured.exit.status {
                ExitStatus::Code { code: 0 } => {
                    Ok(String::from_utf8_lossy(&captured.stdout).into_owned())
                }
                ExitStatus::Code { .. } | ExitStatus::Signal { .. } => {
                    if is_not_found(&stderr) {
                        Err(DockerError::NotFound { detail: stderr })
                    } else {
                        Err(DockerError::Failed { detail: stderr })
                    }
                }
            },
            Ended::Deadline | Ended::Stopped => Err(DockerError::Failed {
                // BORROW: the error owns its text
                detail: "the supervisor ended the docker client before it finished".to_owned(),
            }),
        }
    }
}

/// The first characters of a client's error output, as one line of text.
fn detail_of(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let line: String = text.trim().chars().take(DETAIL_LIMIT).collect();
    line.replace('\n', " ")
}

/// Whether docker's error says the resource does not exist.
fn is_not_found(stderr: &str) -> bool {
    stderr.contains("No such ") || (stderr.contains("network ") && stderr.contains(" not found"))
}

/// Whether `text` is 64 lowercase hex digits.
fn is_full_id(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Whether `text` is the name of a job's network: `candler-<12 hex digits>-net` or
/// `candler-<12 hex digits>-egress`.
fn is_job_network(text: &str) -> bool {
    text.strip_prefix("candler-")
        .and_then(|rest| {
            rest.strip_suffix("-net")
                .or_else(|| rest.strip_suffix("-egress"))
        })
        .is_some_and(|short| {
            short.len() == 12
                && short
                    .bytes()
                    .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
}

/// The reads and runs of docker, through the supervisor.
#[derive(Clone)]
pub struct DockerCli {
    runner: Runner,
    stop_grace: Duration,
}

impl DockerCli {
    /// A client that finds `docker` on this process's `PATH`; every call writes its child record
    /// into `record_dir`.
    ///
    /// # Errors
    /// [`DockerError::Unavailable`] when `PATH` holds no absolute `docker`.
    pub fn new(
        supervisor: Arc<Supervisor>,
        env: ClientEnv,
        record_dir: PathBuf,
    ) -> Result<Self, DockerError> {
        let binary = std::env::var_os("PATH")
            .as_ref()
            .and_then(find_docker)
            .ok_or_else(|| DockerError::Unavailable {
                // BORROW: the error owns its text
                detail: "no absolute `docker` on PATH".to_owned(),
            })?;
        Ok(Self::with_binary(supervisor, binary, env, record_dir))
    }

    /// A client that runs the docker binary at `binary`.
    #[must_use]
    pub const fn with_binary(
        supervisor: Arc<Supervisor>,
        binary: PathBuf,
        env: ClientEnv,
        record_dir: PathBuf,
    ) -> Self {
        Self {
            runner: Runner {
                supervisor,
                binary,
                env,
                record_dir,
            },
            stop_grace: DEFAULT_STOP_GRACE,
        }
    }

    /// The same client, giving a container asked to stop `grace` (whole seconds) to exit before
    /// docker kills it. The default is five seconds.
    #[must_use]
    pub const fn with_stop_grace(mut self, grace: Duration) -> Self {
        self.stop_grace = grace;
        self
    }

    /// The docker binary the client runs.
    #[must_use]
    pub fn binary<'a>(&'a self) -> &'a Path {
        // BORROW: explicit .as_path() instead of Deref coercion
        self.runner.binary.as_path()
    }

    /// The destroy half over the same client; only the guard is given it.
    #[must_use]
    pub(super) fn destroyer(&self) -> DockerDestroyer {
        DockerDestroyer {
            runner: self.runner.clone(), // BORROW: .clone() gives the destroyer its own runner
            stop_grace: self.stop_grace,
        }
    }

    /// The destroy half for a real-daemon test, which wraps it in an allowlist before the guard
    /// is given it. Not part of a production build.
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn destroyer_for_tests(&self) -> DockerDestroyer {
        self.destroyer()
    }
}

/// The arguments of `docker container inspect` for one full id.
fn inspect_args(id: &str) -> Vec<String> {
    to_args(&["container", "inspect", "--format", "{{json .}}", id])
}

/// The arguments that list every container, running or not, that carries `key=value`.
fn ps_args(sel: &LabelSel) -> Vec<String> {
    let mut args = to_args(&["container", "ls", "--all", "--no-trunc", "--quiet"]);
    args.push("--filter".to_owned()); // BORROW: .to_owned() gives the argument its own text
    args.push(format!("label={}={}", sel.key, sel.value));
    args
}

/// The arguments of `docker network inspect` for one network.
fn network_inspect_args(name: &NetworkName) -> Vec<String> {
    to_args(&[
        "network",
        "inspect",
        "--format",
        "{{json .}}",
        // BORROW: `.as_str()` borrows the text of the name
        name.as_str(),
    ])
}

/// The arguments of `docker image inspect` for one reference.
fn image_inspect_args(reference: &str) -> Vec<String> {
    to_args(&["image", "inspect", "--format", "{{json .}}", reference])
}

/// The arguments of `docker network create`.
fn network_create_args(name: &NetworkName, internal: bool, labels: &Labels) -> Vec<String> {
    let mut args = to_args(&["network", "create"]);
    if internal {
        args.push("--internal".to_owned()); // BORROW: .to_owned() gives the argument its own text
    }
    for (key, value) in labels.iter() {
        args.push("--label".to_owned()); // BORROW: .to_owned() gives the argument its own text
        args.push(format!("{key}={value}"));
    }
    args.push(name.as_str().to_owned()); // BORROW: .to_owned() gives the argument its own text
    args
}

/// The arguments of `docker build`, which prints the id of the image it built.
fn build_args(request: &BuildRequest) -> Vec<String> {
    let mut args = to_args(&["build", "--quiet", "-t"]);
    args.push(request.tag.clone()); // BORROW: .clone() gives the argument its own text
    for (name, value) in &request.build_args {
        args.push("--build-arg".to_owned()); // BORROW: .to_owned() gives the argument its own text
        args.push(format!("{name}={value}"));
    }
    for (key, value) in request.labels.iter() {
        args.push("--label".to_owned()); // BORROW: .to_owned() gives the argument its own text
        args.push(format!("{key}={value}"));
    }
    // BORROW: .to_string_lossy().into_owned() gives the argument the context path as text
    args.push(request.context.to_string_lossy().into_owned());
    args
}

/// Owns a list of literal arguments.
fn to_args(literals: &[&str]) -> Vec<String> {
    // BORROW: .to_owned() gives each argument its own text
    literals.iter().map(|text| (*text).to_owned()).collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawContainer {
    id: String,
    created: String,
    image: String,
    config: RawConfig,
    state: RawState,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawConfig {
    labels: Option<BTreeMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawState {
    status: String,
    running: bool,
    exit_code: i64,
    #[serde(rename = "OOMKilled")]
    oom_killed: bool,
    started_at: Option<String>,
    finished_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawNetwork {
    name: String,
    internal: bool,
    labels: Option<BTreeMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawImage {
    id: String,
    #[serde(rename = "RootFS")]
    root_fs: RawRootFs,
    config: RawConfig,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawRootFs {
    layers: Option<Vec<String>>,
}

/// An unreadable answer of docker.
fn unreadable(what: &str, detail: impl std::fmt::Display) -> DockerError {
    DockerError::Unreadable {
        detail: format!("{what}: {detail}"),
    }
}

/// The labels of a map that docker reports as null when there are none.
fn labels_of(map: Option<BTreeMap<String, String>>) -> Labels {
    map.unwrap_or_default()
        .into_iter()
        .fold(Labels::new(), |labels, (key, value)| {
            // BORROW: .as_str() hands the label text to the builder
            labels.with(key.as_str(), value.as_str())
        })
}

/// A time docker reports; docker's zero time means the event has not happened.
fn time_of(text: Option<&str>) -> Result<Option<Timestamp>, DockerError> {
    match text {
        None => Ok(None),
        Some(zero) if zero.starts_with("0001-01-01") => Ok(None),
        Some(text) => Timestamp::from_str(text)
            .map(Some)
            .map_err(|error| unreadable("a time", error)),
    }
}

/// Reads the output of `docker container inspect --format '{{json .}}'`.
fn parse_inspect(text: &str) -> Result<Inspect, DockerError> {
    let raw: RawContainer =
        serde_json::from_str(text).map_err(|error| unreadable("container inspect", error))?;
    let id = ContainerId::from_str(&raw.id).map_err(|error| unreadable("container id", error))?;
    let created =
        Timestamp::from_str(&raw.created).map_err(|error| unreadable("created time", error))?;
    // BORROW: .as_deref() reads the optional times as text
    let started_at = time_of(raw.state.started_at.as_deref())?;
    // BORROW: .as_deref() reads the optional times as text
    let finished_at = time_of(raw.state.finished_at.as_deref())?;
    Ok(Inspect::new(
        id,
        ImageId(raw.image),
        created,
        labels_of(raw.config.labels),
        InspectState::new(
            raw.state.status,
            raw.state.running,
            raw.state.exit_code,
            raw.state.oom_killed,
            started_at,
            finished_at,
        ),
    ))
}

/// Reads the output of `docker network inspect --format '{{json .}}'` for `asked`.
fn parse_network(text: &str, asked: &NetworkName) -> Result<NetworkInfo, DockerError> {
    let raw: RawNetwork =
        serde_json::from_str(text).map_err(|error| unreadable("network inspect", error))?;
    // BORROW: `.as_str()` borrows the text of the name
    if raw.name != asked.as_str() {
        return Err(unreadable(
            "network inspect",
            "the network answered is not the network asked for",
        ));
    }
    Ok(NetworkInfo {
        name: asked.clone(), // BORROW: .clone() gives the answer its own name
        internal: raw.internal,
        labels: labels_of(raw.labels),
    })
}

/// Reads the output of `docker image inspect --format '{{json .}}'`.
fn parse_image(text: &str) -> Result<ImageInfo, DockerError> {
    let raw: RawImage =
        serde_json::from_str(text).map_err(|error| unreadable("image inspect", error))?;
    let layers = raw
        .root_fs
        .layers
        .unwrap_or_default()
        .into_iter()
        .map(LayerDigest)
        .collect();
    Ok(ImageInfo::new(
        ImageId(raw.id),
        layers,
        labels_of(raw.config.labels),
    ))
}

/// Reads the output of `docker container ls --quiet --no-trunc`: one full id per line.
fn parse_ids(text: &str) -> Result<Vec<String>, DockerError> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            if is_full_id(line) {
                Ok(line.to_owned()) // BORROW: .to_owned() gives the list its own id
            } else {
                Err(unreadable("container list", "a line is not a full id"))
            }
        })
        .collect()
}

impl DockerDriver for DockerCli {
    fn inspect<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Inspect, DockerError>> {
        Box::pin(async move {
            if !is_full_id(id) {
                return Err(DockerError::NotAFullId {
                    detail: id.chars().take(DETAIL_LIMIT).collect(),
                });
            }
            let text = self.runner.run(inspect_args(id), CALL_DEADLINE).await?;
            let inspected = parse_inspect(&text)?;
            // BORROW: `.as_str()` borrows the text of the id
            if inspected.id.as_str() != id {
                return Err(unreadable(
                    "container inspect",
                    "the container answered is not the container asked for",
                ));
            }
            Ok(inspected)
        })
    }

    fn ps_by_label<'a>(
        &'a self,
        sel: &'a LabelSel,
    ) -> BoxFuture<'a, Result<Vec<ContainerRef>, DockerError>> {
        Box::pin(async move {
            if !sel.key.starts_with("candler.") || sel.value.is_empty() {
                return Err(DockerError::Failed {
                    // BORROW: the error owns its text
                    detail: "a label selector names a candler label and a value".to_owned(),
                });
            }
            let listed = self.runner.run(ps_args(sel), CALL_DEADLINE).await?;
            let mut found = Vec::new();
            for id in parse_ids(&listed)? {
                // BORROW: .as_str() hands the id to inspect
                match self.inspect(id.as_str()).await {
                    Ok(inspected) => found.push(ContainerRef::new(
                        inspected.id,
                        inspected.labels,
                        inspected.created,
                    )),
                    Err(DockerError::NotFound { .. }) => {}
                    Err(error) => return Err(error),
                }
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
            self.runner
                .run(network_create_args(name, internal, labels), CALL_DEADLINE)
                .await
                .map(|_| ())
        })
    }

    fn network_inspect<'a>(
        &'a self,
        name: &'a NetworkName,
    ) -> BoxFuture<'a, Result<NetworkInfo, DockerError>> {
        Box::pin(async move {
            let text = self
                .runner
                .run(network_inspect_args(name), CALL_DEADLINE)
                .await?;
            parse_network(&text, name)
        })
    }

    fn image_inspect<'a>(
        &'a self,
        reference: &'a str,
    ) -> BoxFuture<'a, Result<Option<ImageInfo>, DockerError>> {
        Box::pin(async move {
            match self
                .runner
                .run(image_inspect_args(reference), CALL_DEADLINE)
                .await
            {
                Ok(text) => parse_image(&text).map(Some),
                Err(DockerError::NotFound { .. }) => Ok(None),
                Err(error) => Err(error),
            }
        })
    }

    fn build<'a>(
        &'a self,
        request: &'a BuildRequest,
    ) -> BoxFuture<'a, Result<ImageInfo, DockerError>> {
        Box::pin(async move {
            let printed = self.runner.run(build_args(request), BUILD_DEADLINE).await?;
            let id = printed.trim();
            self.image_inspect(id)
                .await?
                .ok_or_else(|| unreadable("build", "the image it printed is not there"))
        })
    }
}

/// The destroy half of docker, through the supervisor. Only the container guard holds one.
#[derive(Clone)]
pub struct DockerDestroyer {
    runner: Runner,
    stop_grace: Duration,
}

impl DestroyDriver for DockerDestroyer {
    fn stop<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        Box::pin(async move {
            if !is_full_id(id) {
                return Err(DockerError::NotAFullId {
                    detail: id.chars().take(DETAIL_LIMIT).collect(),
                });
            }
            let grace = self.stop_grace.as_secs().to_string();
            let args = to_args(&["container", "stop", "-t", &grace, id]);
            self.runner.run(args, CALL_DEADLINE).await.map(|_| ())
        })
    }

    fn rm<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        Box::pin(async move {
            if !is_full_id(id) {
                return Err(DockerError::NotAFullId {
                    detail: id.chars().take(DETAIL_LIMIT).collect(),
                });
            }
            let args = to_args(&["container", "rm", "-v", id]);
            self.runner.run(args, CALL_DEADLINE).await.map(|_| ())
        })
    }

    fn network_rm<'a>(&'a self, name: &'a NetworkName) -> BoxFuture<'a, Result<(), DockerError>> {
        Box::pin(async move {
            // BORROW: `.as_str()` borrows the text of the name
            if !is_job_network(name.as_str()) {
                return Err(DockerError::NotAJobNetwork {
                    // BORROW: `.as_str()` borrows the text of the name
                    detail: name.as_str().chars().take(DETAIL_LIMIT).collect(),
                });
            }
            // BORROW: `.as_str()` borrows the text of the name
            let args = to_args(&["network", "rm", name.as_str()]);
            self.runner.run(args, CALL_DEADLINE).await.map(|_| ())
        })
    }
}
