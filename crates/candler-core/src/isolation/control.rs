//! The one builder of a control container: a throwaway container Candler runs to read or prepare
//! something, such as a version probe, a package probe, a lock compile, a
//! collect of a run's sessions or the fake model.
//!
//! A [`ControlSpec`] is every control container's flags in one place: it removes itself, wears the
//! label `candler.role=control` and no job label (so no sweep meets it and the guard never
//! destroys it), carries [`hardening`] over the limits its kind needs, and has a network chosen by
//! its kind. [`outcome`] is the one reading of how it ended.

use candler_schemas::process::ExitStatus;

use super::docker::DockerError;
use super::hardening::{HardLimits, Role, hardening};
use super::layout::LABEL_ROLE;
use crate::supervise::stop::{Captured, Ended};

/// The exit code of coreutils `timeout` when the command it runs overran.
const TIMEOUT_EXIT: i32 = 124;

/// How many bytes of a container's error output an error carries.
const STDERR_TAIL: usize = 2048;

/// The network of a control container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ControlNetwork {
    /// No network: the default of every kind that has no need of one.
    None,
    /// The default bridge, named: the lock compile reaches the package index through it.
    Bridge,
    /// Docker's default network, named by no flag: a container another network is connected to
    /// afterwards.
    Default,
}

/// A control container: its image, limits, network and command.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlSpec {
    image: String,
    limits: HardLimits,
    network: ControlNetwork,
    detach: bool,
    user: Option<(u32, u32)>,
    workdir: Option<String>,
    entrypoint: Option<String>,
    env: Vec<String>,
    mounts: Vec<String>,
    command: Vec<String>,
}

impl ControlSpec {
    /// A control container of the image `image` under `limits`, with no network and no command.
    #[must_use]
    pub fn new(image: &str, limits: HardLimits) -> Self {
        Self {
            image: image.to_owned(), // BORROW: the spec owns its image id
            limits,
            network: ControlNetwork::None,
            detach: false,
            user: None,
            workdir: None,
            entrypoint: None,
            env: Vec::new(),
            mounts: Vec::new(),
            command: Vec::new(),
        }
    }

    /// The same container on `network`.
    #[must_use]
    pub const fn network(mut self, network: ControlNetwork) -> Self {
        self.network = network;
        self
    }

    /// The same container, started in the background: docker prints its id and returns.
    #[must_use]
    pub const fn detached(mut self) -> Self {
        self.detach = true;
        self
    }

    /// The same container, run as the user and group `uid` and `gid`.
    #[must_use]
    pub const fn user(mut self, uid: u32, gid: u32) -> Self {
        self.user = Some((uid, gid));
        self
    }

    /// The same container, with `dir` as its working directory.
    #[must_use]
    pub fn workdir(mut self, dir: String) -> Self {
        self.workdir = Some(dir);
        self
    }

    /// The same container, with `program` as its entrypoint.
    #[must_use]
    pub fn entrypoint(mut self, program: &str) -> Self {
        self.entrypoint = Some(program.to_owned()); // BORROW: the spec owns its entrypoint
        self
    }

    /// The same container with the environment entries `pairs`, each written `NAME=value`.
    #[must_use]
    pub fn env(mut self, pairs: Vec<String>) -> Self {
        self.env.extend(pairs);
        self
    }

    /// The same container with one more mount, as `--mount` takes it.
    #[must_use]
    pub fn mount(mut self, mount: String) -> Self {
        self.mounts.push(mount);
        self
    }

    /// The same container running `command`, whose first word follows the image.
    #[must_use]
    pub fn command(mut self, command: Vec<String>) -> Self {
        self.command = command;
        self
    }

    /// The arguments of `docker`, from `run` to the end of the command.
    #[must_use]
    pub fn args(&self) -> Vec<String> {
        let mut args = vec![String::from("run"), String::from("--rm")];
        if self.detach {
            args.push(String::from("-d"));
        }
        let mut hard = hardening(&self.limits).into_iter();
        args.extend(hard.next());
        match self.network {
            ControlNetwork::None => args.extend([String::from("--network"), String::from("none")]),
            ControlNetwork::Bridge => {
                args.extend([String::from("--network"), String::from("bridge")]);
            }
            ControlNetwork::Default => {}
        }
        args.extend(hard);
        if let Some((uid, gid)) = self.user {
            args.extend([String::from("--user"), format!("{uid}:{gid}")]);
        }
        if let Some(dir) = &self.workdir {
            args.extend([String::from("--workdir"), dir.clone()]); // BORROW: the argv owns its text
        }
        args.extend([
            String::from("--label"),
            format!("{LABEL_ROLE}={}", Role::Control.label()),
        ]);
        if let Some(program) = &self.entrypoint {
            args.extend([String::from("--entrypoint"), program.clone()]); // BORROW: the argv owns its text
        }
        for pair in &self.env {
            args.extend([String::from("-e"), pair.clone()]); // BORROW: the argv owns its text
        }
        for mount in &self.mounts {
            args.extend([String::from("--mount"), mount.clone()]); // BORROW: the argv owns its text
        }
        args.push(self.image.clone()); // BORROW: the argv owns its text
        args.extend(self.command.iter().cloned()); // BORROW: the argv owns its words
        args
    }
}

/// What a control container printed, from how it ended.
///
/// # Errors
/// [`DockerError::Failed`] when the command's own `timeout` ended it (exit 124), for another exit,
/// a signal, or an end the container did not make on its own.
pub fn outcome(captured: Captured) -> Result<Vec<u8>, DockerError> {
    if !matches!(captured.exit.ended, Ended::Exited) {
        return Err(DockerError::Failed {
            detail: String::from("the control container did not end on its own"),
        });
    }
    match captured.exit.status {
        ExitStatus::Code { code: 0 } => Ok(captured.stdout),
        ExitStatus::Code { code: TIMEOUT_EXIT } => Err(DockerError::Failed {
            detail: String::from("the control container's command overran its time"),
        }),
        ExitStatus::Code { code } => Err(DockerError::Failed {
            detail: format!(
                "the control container exited with status {code}: {}",
                tail_of(&captured.stderr)
            ),
        }),
        ExitStatus::Signal { signal } => Err(DockerError::Failed {
            detail: format!("a signal ({signal}) ended the control container"),
        }),
    }
}

/// The last bytes of `bytes` as text.
fn tail_of(bytes: &[u8]) -> String {
    let start = bytes.len().saturating_sub(STDERR_TAIL);
    String::from_utf8_lossy(bytes.get(start..).unwrap_or_default())
        .trim()
        .to_owned() // BORROW: the error owns its text
}
