//! The data the isolation modules share: images, containers, networks, labels, plans and
//! sweep results. These are plain values; the traits that act on them are in the sibling modules.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use candler_schemas::container::SweepScope as WireScope;
use candler_schemas::primitives::{ContainerId, JobId, RunKey, Timestamp};
use url::Url;

use super::container::mounts::MountSpec;

/// The labels a container, an image or a network carries, by key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Labels(BTreeMap<String, String>);

impl Labels {
    /// No labels.
    #[must_use]
    pub const fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Sets the label `key` to `value`, replacing an earlier value.
    #[must_use]
    pub fn with(mut self, key: &str, value: &str) -> Self {
        self.0.insert(key.to_owned(), value.to_owned());
        self
    }

    /// The value of the label `key`.
    #[must_use]
    pub fn get<'a>(&'a self, key: &str) -> Option<&'a str> {
        self.0.get(key).map(String::as_str)
    }

    /// The labels in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }
}

/// The name of a docker network of a job: its internal network
/// `candler-<first 12 hex digits of the job id>-net`, or its egress network
/// `candler-<first 12 hex digits of the job id>-egress`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetworkName(String);

impl NetworkName {
    /// The network of `job`.
    #[must_use]
    pub fn for_job(job: &JobId) -> Self {
        Self(format!("candler-{}-net", Self::short(job)))
    }

    /// The egress network of `job`: user-defined and not internal. The gateway reaches its
    /// upstream on it, and the agent's container joins it only when its cell asks for
    /// `isolation.network = bridge`.
    #[must_use]
    pub fn egress_for_job(job: &JobId) -> Self {
        Self(format!("candler-{}-egress", Self::short(job)))
    }

    /// The first 12 hex digits of the job id.
    fn short(job: &JobId) -> String {
        job.as_str()
            .chars()
            .filter(char::is_ascii_hexdigit)
            .take(12)
            .collect()
    }

    /// The name as docker takes it.
    #[must_use]
    pub const fn as_str<'a>(&'a self) -> &'a str {
        // BORROW: explicit .as_str() instead of Deref coercion
        self.0.as_str()
    }
}

/// The id of an image as docker reports it, `sha256:<hex>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageId(pub String);

/// The digest of one layer of an image.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LayerDigest(pub String);

/// What docker says about an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInfo {
    /// The image id.
    pub id: ImageId,
    /// The layers, base first.
    pub layers: Vec<LayerDigest>,
    /// The labels the image carries.
    pub labels: Labels,
}

impl ImageInfo {
    /// What docker says about the image `id`.
    #[must_use]
    pub const fn new(id: ImageId, layers: Vec<LayerDigest>, labels: Labels) -> Self {
        Self { id, layers, labels }
    }
}

/// An image a run may start containers from, and the Claude Code version inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRef {
    /// The image id.
    pub id: ImageId,
    /// The version the image's `claude --version` reports, when it has been read.
    pub cc_version: Option<semver::Version>,
}

/// A container as the guard and the sweeper see it: its id, labels and creation time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerRef {
    /// The container id.
    pub cid: ContainerId,
    /// The labels the container carries.
    pub labels: Labels,
    /// When docker created the container.
    pub created_at: Timestamp,
}

impl ContainerRef {
    /// A container with its labels and creation time.
    #[must_use]
    pub const fn new(cid: ContainerId, labels: Labels, created_at: Timestamp) -> Self {
        Self {
            cid,
            labels,
            created_at,
        }
    }
}

/// A container the guard declined to destroy, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The container.
    pub container: ContainerId,
    /// The rule that refused, as a reason code.
    pub reason: String,
}

/// What a destroy or a sweep did. `unknown` is set when it could not tell that it found every
/// container in scope.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContainerSweep {
    /// The containers destroyed, each as inspected right before the destroy; its labels carry
    /// the `candler.role` a `run.stray` entry reports.
    pub destroyed: Vec<ContainerRef>,
    /// The containers refused.
    pub refused: Vec<Refusal>,
    /// Whether the sweep could not tell that it was complete.
    pub unknown: bool,
}

/// How far a sweep reaches. The wire type of the same name has no payload; [`SweepScope::wire`]
/// maps onto it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SweepScope {
    /// The containers of one run.
    Run(RunKey),
    /// The containers of the job that is running.
    CurrentJob,
    /// The containers of a job whose supervisor was lost.
    Lost(JobId),
}

impl SweepScope {
    /// The scope as the events name it.
    #[must_use]
    pub const fn wire(&self) -> WireScope {
        match self {
            Self::Run(_) => WireScope::Run,
            Self::CurrentJob => WireScope::CurrentJob,
            Self::Lost(_) => WireScope::Lost,
        }
    }
}

/// The state block of an inspected container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectState {
    /// The docker status word.
    pub status: String,
    /// Whether the container is running.
    pub running: bool,
    /// The exit code, once it has exited.
    pub exit_code: i64,
    /// Whether the kernel killed the container for memory.
    pub oom_killed: bool,
    /// When the container started.
    pub started_at: Option<Timestamp>,
    /// When the container finished.
    pub finished_at: Option<Timestamp>,
}

impl InspectState {
    /// A state block.
    #[must_use]
    pub const fn new(
        status: String,
        running: bool,
        exit_code: i64,
        oom_killed: bool,
        started_at: Option<Timestamp>,
        finished_at: Option<Timestamp>,
    ) -> Self {
        Self {
            status,
            running,
            exit_code,
            oom_killed,
            started_at,
            finished_at,
        }
    }
}

/// What `docker inspect` says about a container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspect {
    /// The container id.
    pub id: ContainerId,
    /// The id of the image the container runs.
    pub image: ImageId,
    /// When docker created the container.
    pub created: Timestamp,
    /// The labels of the container.
    pub labels: Labels,
    /// The state block.
    pub state: InspectState,
}

impl Inspect {
    /// An inspect answer.
    #[must_use]
    pub const fn new(
        id: ContainerId,
        image: ImageId,
        created: Timestamp,
        labels: Labels,
        state: InspectState,
    ) -> Self {
        Self {
            id,
            image,
            created,
            labels,
            state,
        }
    }
}

/// What `docker network inspect` says about a network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkInfo {
    /// The network name.
    pub name: NetworkName,
    /// Whether the network is internal.
    pub internal: bool,
    /// The labels of the network.
    pub labels: Labels,
}

/// A label selector for `docker ps` and `docker network ls`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelSel {
    /// The label key.
    pub key: String,
    /// The value it must have.
    pub value: String,
}

/// A request to build an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRequest {
    /// The build context directory.
    pub context: PathBuf,
    /// The tag the built image gets (`docker build -t`).
    pub tag: String,
    /// The build arguments, as `(name, value)` pairs (`--build-arg name=value`), in order.
    pub build_args: Vec<(String, String)>,
    /// The labels the image gets.
    pub labels: Labels,
}

/// The resource limits of a container.
#[derive(Debug, Clone, PartialEq)]
pub struct Limits {
    /// The process limit.
    pub pids_limit: u32,
    /// The memory limit, in docker's own notation.
    pub memory: String,
    /// The cpu limit.
    pub cpus: f64,
}

/// Which user the container runs as.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum UserSpec {
    /// The image's own user.
    Engine,
    /// The host user and group.
    Host {
        /// The user id.
        uid: u32,
        /// The group id.
        gid: u32,
    },
}

/// What a run's container is made of, decided before it starts.
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerPlan {
    /// The image to start from.
    pub image: ImageRef,
    /// The labels the container gets.
    pub labels: Labels,
    /// The mounts.
    pub mounts: Vec<MountSpec>,
    /// The environment, by name.
    pub env: Vec<(String, String)>,
    /// The network: the job's internal network, where the gateway answers.
    pub network: NetworkName,
    /// A second network the container joins, the job's egress network, when the cell asks for
    /// `isolation.network = bridge`. Docker Engine 25 or newer takes both `--network` flags on
    /// one `docker run`, as long as both networks are user-defined.
    pub egress: Option<NetworkName>,
    /// The resource limits.
    pub limits: Limits,
    /// The file docker writes the container id to.
    pub cidfile: PathBuf,
    /// The user.
    pub user: UserSpec,
    /// The working directory inside the container.
    pub workdir: String,
    /// The home directory inside the container.
    pub home: String,
    /// The run's wall-clock cap, which the container's expiry is cut from.
    pub expiry: Duration,
    /// The project root; a mount source may sit under it.
    pub project_root: PathBuf,
    /// The store root; a mount source may sit under it.
    pub store_root: PathBuf,
}

/// What a run's container left behind: its final inspect, and what destroying it did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContainerOutcome {
    /// The inspect taken before the container was destroyed, when it could be taken.
    pub inspect: Option<Inspect>,
    /// What the destroy did.
    pub sweep: ContainerSweep,
}

/// A running gateway: its container, where the agent reaches it and the token the agent sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayHandle {
    /// The gateway's container.
    pub container: ContainerRef,
    /// The base url the agent's container uses.
    pub base_url: Url,
    /// The network both containers share.
    pub network: NetworkName,
    /// The job's egress network, which the gateway reaches its upstream on. The agent's
    /// container joins it only when its cell asks for `isolation.network = bridge`.
    pub egress: NetworkName,
    /// The token the agent holds in place of the real one.
    pub dummy_token: String,
}

impl GatewayHandle {
    /// The base url of one run: `<base_url>/run/<run key>`. The gateway strips the prefix before
    /// the upstream and logs it, so a log line names its run.
    #[must_use]
    pub fn run_base_url(&self, run: &RunKey) -> Url {
        let text = format!("{}/run/{run}", self.base_url.as_str().trim_end_matches('/'));
        Url::parse(&text).unwrap_or_else(|_| self.base_url.clone())
    }
}
