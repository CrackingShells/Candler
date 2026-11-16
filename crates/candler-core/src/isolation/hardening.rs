//! The one hardening every container of Candler carries, and the role label each one wears.
//!
//! [`hardening`] writes the flags of a `docker run` or `docker create` that no container may be
//! without: the init process, a read-only root, no capabilities and no new privileges. The limits
//! and the writable mounts are the caller's, given in [`HardLimits`], so no kind of container is
//! widened by sharing the list.

use super::layout::CONTAINER_ENV;
use super::model::Limits;

/// The role a container plays, which is the value of its `candler.role` label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Role {
    /// The container an agent runs in.
    Agent,
    /// The sidecar the agent reaches its model through.
    Gateway,
    /// The sidecar that carries the agent's allowed outbound traffic.
    Egress,
    /// A throwaway container Candler runs to read or prepare something.
    Control,
}

impl Role {
    /// The value of the `candler.role` label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Gateway => "gateway",
            Self::Egress => "egress",
            Self::Control => "control",
        }
    }
}

/// The limits and the writable mounts of one container.
#[derive(Debug, Clone, PartialEq)]
pub struct HardLimits {
    /// The process limit.
    pub pids_limit: u32,
    /// The memory limit, in docker's own notation.
    pub memory: String,
    /// The cpu limit; none leaves the container uncapped.
    pub cpus: Option<f64>,
    /// The `--tmpfs` arguments, one per writable in-memory mount.
    pub tmpfs: Vec<String>,
    /// Whether the env is an anonymous volume docker fills from the image, so that the agent's
    /// installs land in a per-run copy while `/` stays read-only.
    pub env_volume: bool,
}

impl HardLimits {
    /// The limits of an agent container: the plan's limits, a 256 MiB `/tmp` and the env volume.
    #[must_use]
    pub fn agent(limits: &Limits) -> Self {
        Self {
            env_volume: true,
            ..Self::sized(limits)
        }
    }

    /// The limits of a container that runs over the plan's limits and reads the env from the
    /// image as it is: a 256 MiB `/tmp` and no env volume.
    #[must_use]
    pub fn sized(limits: &Limits) -> Self {
        Self {
            pids_limit: limits.pids_limit,
            memory: limits.memory.clone(), // BORROW: the limits own their text
            cpus: Some(limits.cpus),
            tmpfs: vec![tmp_tmpfs(TMP_MIB)],
            env_volume: false,
        }
    }
}

/// The size of the tmpfs mounted at `/tmp` of an agent, in mebibytes.
const TMP_MIB: u32 = 256;

/// The `--tmpfs` argument of a `/tmp` of `mib` mebibytes: writable, not executable.
#[must_use]
pub fn tmp_tmpfs(mib: u32) -> String {
    format!("/tmp:rw,noexec,nosuid,size={mib}m")
}

/// The hardening flags, then the limits.
///
/// The first word is `--init`, so that a caller that sets the network can place it right after.
#[must_use]
pub fn hardening(limits: &HardLimits) -> Vec<String> {
    let mut argv: Vec<String> = ["--init", "--read-only"].map(str::to_owned).to_vec();
    for tmpfs in &limits.tmpfs {
        argv.push(String::from("--tmpfs"));
        argv.push(tmpfs.clone()); // BORROW: the argv owns its text
    }
    if limits.env_volume {
        // The one mount without a host source.
        argv.push(String::from("--mount"));
        argv.push(format!("type=volume,dst={CONTAINER_ENV}"));
    }
    argv.extend(["--cap-drop", "ALL", "--security-opt", "no-new-privileges"].map(str::to_owned));
    argv.extend([
        String::from("--pids-limit"),
        limits.pids_limit.to_string(),
        String::from("--memory"),
        limits.memory.clone(), // BORROW: the argv owns its text
    ]);
    if let Some(cpus) = limits.cpus {
        argv.extend([String::from("--cpus"), cpus.to_string()]);
    }
    argv
}
