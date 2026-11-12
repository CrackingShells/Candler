//! The `docker run` command line of a run's container, and the spawn request that carries it.
//!
//! The environment is built from nothing: the plan's entries, the gateway's base url and
//! dummy token under the variable names the harness driver gives, and the harness's plain
//! overlay. A secret in the overlay, and the names the driver reserves for the model endpoint and
//! credentials, are refused rather than overridden.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use candler_harness::spec::{CommandSpec, EnvEntry, EnvValue};
use candler_schemas::primitives::{Marker, RunKey};
use url::Url;

use super::expiry::{ceil_secs, wrap_expiry};
use super::mounts::{MountError, MountRoots, mounts};
use crate::isolation::RunOutputs;
use crate::isolation::layout::{CONTAINER_ENV, EXPIRY_MARGIN, LABEL_JOB, LABEL_ROLE, LABEL_RUN};
use crate::isolation::model::{ContainerPlan, ImageId, Limits, UserSpec};
use crate::supervise::hook::StopHook;
use crate::supervise::spawn::SpawnRequest;

/// The size of the tmpfs mounted at `/tmp`, in mebibytes.
const TMPFS_MIB: u32 = 256;

/// The docker client's own variables, the only ones it inherits.
const CLIENT_ENV: [&str; 3] = ["DOCKER_HOST", "DOCKER_CONTEXT", "HOME"];

/// The role label of a control container.
const CONTROL_ROLE: &str = "control";

/// Why an argv or a request could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RunArgvError {
    /// The overlay carries a secret, which never enters the agent container.
    #[error("the environment overlay carries a secret for {name}")]
    SecretRefused {
        /// The variable the secret was for.
        name: String,
    },
    /// The overlay sets a name the wrapper owns.
    #[error("the environment overlay sets {name}, which the wrapper owns")]
    ReservedName {
        /// The variable.
        name: String,
    },
    /// The overlay sets a name the plan already sets.
    #[error("the environment sets {name} twice")]
    DuplicateName {
        /// The variable.
        name: String,
    },
    /// An environment name docker would misread.
    #[error("environment name {name:?} is empty or holds `=` or a NUL byte")]
    BadEnvName {
        /// The variable.
        name: String,
    },
    /// The plan lacks a label the guard decides by.
    #[error("the plan lacks the label {key}")]
    MissingLabel {
        /// The label key.
        key: &'static str,
    },
    /// The plan's cidfile has no directory.
    #[error("the cidfile has no parent directory")]
    BadCidfile,
    /// A mount was refused.
    #[error(transparent)]
    Mount(#[from] MountError),
    /// The wrapped command could not be built.
    #[error("the command cannot be wrapped: {detail}")]
    Command {
        /// What is wrong.
        detail: String,
    },
    /// The spawn request could not be built.
    #[error("the spawn request cannot be built: {detail}")]
    Request {
        /// What is wrong; it never holds an environment value.
        detail: String,
    },
}

/// The hardening every container carries, then its limits.
fn hardening(limits: &Limits, argv: &mut Vec<String>) {
    argv.extend(["--init", "--read-only"].map(str::to_owned));
    argv.push(String::from("--tmpfs"));
    argv.push(format!("/tmp:rw,noexec,nosuid,size={TMPFS_MIB}m"));
    // The one mount without a host source: an anonymous volume docker fills from the image, so the
    // agent's installs land in a per-run copy while `/` stays read-only.
    argv.push(String::from("--mount"));
    argv.push(format!("type=volume,dst={CONTAINER_ENV}"));
    argv.extend(["--cap-drop", "ALL", "--security-opt", "no-new-privileges"].map(str::to_owned));
    argv.extend([
        String::from("--pids-limit"),
        limits.pids_limit.to_string(),
        String::from("--memory"),
        // BORROW: the limit is copied into the argv
        limits.memory.clone(),
        String::from("--cpus"),
        limits.cpus.to_string(),
    ]);
}

/// The `-e NAME=value` pairs of the harness's overlay. `reserved` lists the names the wrapper
/// owns, which the overlay may not set.
///
/// # Errors
/// [`RunArgvError::SecretRefused`] for a secret, [`RunArgvError::ReservedName`] for a name the
/// wrapper owns, [`RunArgvError::BadEnvName`] for a name docker would misread.
pub fn env_args(overlay: &[EnvEntry], reserved: &[String]) -> Result<Vec<String>, RunArgvError> {
    let mut argv = Vec::with_capacity(overlay.len() * 2);
    for entry in overlay {
        // BORROW: explicit .as_str() instead of Deref coercion
        let name = entry.name.as_str();
        if let EnvValue::Literal(value) = &entry.value {
            if reserved.iter().any(|owned| owned == name) {
                return Err(RunArgvError::ReservedName {
                    // BORROW: .to_owned() keeps the name in the error
                    name: name.to_owned(),
                });
            }
            argv.push(String::from("-e"));
            argv.push(format!("{name}={value}"));
            continue;
        }
        // EXPLICIT: a secret and any value kind not read above both refuse, until a new kind is read
        return Err(RunArgvError::SecretRefused {
            // BORROW: .to_owned() keeps the name in the error
            name: name.to_owned(),
        });
    }
    Ok(argv)
}

/// Checks an environment name docker could take.
fn check_name(name: &str) -> Result<(), RunArgvError> {
    if name.is_empty() || name.contains(['=', '\0']) {
        Err(RunArgvError::BadEnvName {
            // BORROW: .to_owned() keeps the name in the error
            name: name.to_owned(),
        })
    } else {
        Ok(())
    }
}

/// The label `key` of the plan, or the error that names it.
fn label<'a>(plan: &'a ContainerPlan, key: &'static str) -> Result<&'a str, RunArgvError> {
    plan.labels
        .get(key)
        .ok_or(RunArgvError::MissingLabel { key })
}

/// The `docker run` argv of an agent container, from `docker` to the end of the command. The
/// command is run as given: [`build_run_request`] wraps it in the expiry first.
///
/// The order is fixed: hardening, labels, environment, mounts, the image id, the command. There
/// is no `--rm`, so the supervisor can still read the exit and the guard destroys the container.
///
/// # Errors
/// [`RunArgvError`] when the plan lacks the job, run or role label, when the overlay is refused
/// (see [`env_args`]) or repeats a name the plan sets, or when a mount cannot be written.
pub fn run_argv(plan: &ContainerPlan, cmd: &CommandSpec) -> Result<Vec<String>, RunArgvError> {
    label(plan, LABEL_JOB)?;
    label(plan, LABEL_RUN)?;
    label(plan, LABEL_ROLE)?;
    let mut argv = vec![String::from("docker"), String::from("run")];
    hardening(&plan.limits, &mut argv);
    // BORROW: the owned copy is made where it is needed
    argv.extend([String::from("--network"), plan.network.as_str().to_owned()]);
    if let Some(egress) = &plan.egress {
        // A second user-defined network: docker takes two on one `docker run` (Engine 25 and
        // later) and refuses the default `bridge` beside a user-defined one (exit 125).
        // BORROW: the owned copy is made where it is needed
        argv.extend([String::from("--network"), egress.as_str().to_owned()]);
    }
    if let UserSpec::Host { uid, gid } = &plan.user {
        argv.extend([String::from("--user"), format!("{uid}:{gid}")]);
    }
    argv.push(String::from("--cidfile"));
    argv.push(plan.cidfile.to_string_lossy().into_owned());
    let workdir = cmd.cwd().map_or_else(
        // BORROW: the plan's workdir is copied into the argv
        || plan.workdir.clone(),
        |cwd| format!("{}/{cwd}", plan.workdir.trim_end_matches('/')),
    );
    argv.extend([String::from("--workdir"), workdir]);
    for (key, value) in plan.labels.iter() {
        argv.push(String::from("--label"));
        argv.push(format!("{key}={value}"));
    }
    for (name, value) in &plan.env {
        // BORROW: explicit .as_str() instead of Deref coercion
        check_name(name.as_str())?;
        argv.push(String::from("-e"));
        argv.push(format!("{name}={value}"));
    }
    for entry in cmd.env_overlay() {
        if plan.env.iter().any(|(name, _)| *name == entry.name) {
            return Err(RunArgvError::DuplicateName {
                // BORROW: .to_owned() keeps the name in the error
                name: entry.name.clone(),
            });
        }
        // BORROW: explicit .as_str() instead of Deref coercion
        check_name(entry.name.as_str())?;
    }
    argv.extend(env_args(cmd.env_overlay(), reserved_names(plan))?);
    for mount in &plan.mounts {
        argv.push(String::from("--mount"));
        argv.push(mount.to_arg()?);
    }
    // BORROW: the owned copy is made where it is needed
    argv.push(plan.image.id.0.clone());
    argv.extend(cmd.argv().iter().cloned());
    Ok(argv)
}

/// The `docker run` argv of a control container, such as a version readout or a catalog capture:
/// `--rm`, the label `candler.role=control` and no `candler.job` (so no sweep meets it), no
/// network, the same hardening, and the command under its own expiry of `wall`. The guard never
/// destroys it.
///
/// # Errors
/// [`RunArgvError`] when the overlay is refused or the command cannot be wrapped.
pub fn control_argv(
    image: &ImageId,
    limits: &Limits,
    wall: Duration,
    cmd: &CommandSpec,
) -> Result<Vec<String>, RunArgvError> {
    let wrapped = wrap_expiry(cmd, ceil_secs(wall)).map_err(|source| RunArgvError::Command {
        detail: source.to_string(),
    })?;
    let mut argv = vec![
        String::from("docker"),
        String::from("run"),
        String::from("--rm"),
    ];
    hardening(limits, &mut argv);
    argv.extend([String::from("--network"), String::from("none")]);
    argv.push(String::from("--label"));
    argv.push(format!("{LABEL_ROLE}={CONTROL_ROLE}"));
    // A control container has no gateway: it reserves no name.
    argv.extend(env_args(wrapped.env_overlay(), &[])?);
    // BORROW: the owned copy is made where it is needed
    argv.push(image.0.clone());
    argv.extend(wrapped.argv().iter().cloned());
    Ok(argv)
}

/// The docker client binary: the first `docker` on the host's `PATH`, else the bare name.
fn docker_binary() -> String {
    let found = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join("docker"))
            .find(|candidate| candidate.is_file())
    });
    found.map_or_else(
        || String::from("docker"),
        |path| path.to_string_lossy().into_owned(),
    )
}

/// The docker client's own environment: only its allowlist, from the host's.
fn client_env() -> Vec<(String, String)> {
    CLIENT_ENV
        .iter()
        .filter_map(|name| {
            let value: OsString = std::env::var_os(name)?;
            // BORROW: the owned copy is made where it is needed
            Some(((*name).to_owned(), value.to_string_lossy().into_owned()))
        })
        .collect()
}

/// The names the plan reserves for the wrapper: the driver's gateway variables, none when the
/// plan names no gateway.
fn reserved_names<'a>(plan: &'a ContainerPlan) -> &'a [String] {
    plan.gateway
        .as_ref()
        .map_or(&[], |gateway| gateway.reserved.as_slice())
}

/// The `docker run` argv of the agent container that runs `cmd` for the run `key`: the mounts
/// checked against the plan's roots, the labels of the run, the gateway's address and dummy token
/// set by the wrapper under the variable names the plan's gateway gives, and the command under an
/// expiry of the wall plus the margin. The program is `docker`.
///
/// # Errors
/// [`RunArgvError`] when a mount or the overlay is refused, when the plan names no gateway
/// variables ([`RunArgvError::Request`]), or when a gateway variable is one the plan already sets.
pub fn agent_argv(
    plan: &ContainerPlan,
    cmd: &CommandSpec,
    key: &RunKey,
    base_url: &Url,
    dummy_token: &str,
) -> Result<Vec<String>, RunArgvError> {
    let gateway = plan.gateway.as_ref().ok_or_else(|| RunArgvError::Request {
        detail: String::from("the plan names no gateway variables"),
    })?;
    for name in [&gateway.base_url, &gateway.auth_token] {
        if plan.env.iter().any(|(own, _)| own == name) {
            return Err(RunArgvError::DuplicateName {
                // BORROW: .clone() keeps the name in the error
                name: name.clone(),
            });
        }
    }
    // BORROW: the plan is copied so the checked mounts and the run's labels can be set
    let mut planned = plan.clone();
    planned.mounts = mounts(plan, &MountRoots::from_plan(plan))?;
    planned.labels = planned
        .labels
        .with(LABEL_RUN, &key.to_string())
        .with(LABEL_ROLE, "agent");
    planned
        .env
        // BORROW: the owned copy is made where it is needed
        .push((gateway.base_url.clone(), base_url.as_str().to_owned()));
    planned
        .env
        // BORROW: the owned copy is made where it is needed
        .push((gateway.auth_token.clone(), dummy_token.to_owned()));
    let secs = ceil_secs(plan.expiry + EXPIRY_MARGIN);
    let wrapped = wrap_expiry(cmd, secs).map_err(|source| RunArgvError::Command {
        detail: source.to_string(),
    })?;
    run_argv(&planned, &wrapped)
}

/// The request that runs `cmd` in the plan's container: the argv of [`agent_argv`], the docker
/// client's own environment, and the supervisor's deadline at the wall. The request carries no
/// stop hook; the isolation attaches one per request.
///
/// # Errors
/// [`RunArgvError`] when a mount, the overlay, the gateway variables or the request is refused.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is one fact of the request and none belongs with another"
)]
pub fn build_run_request(
    plan: &ContainerPlan,
    cmd: &CommandSpec,
    marker: Marker,
    key: &RunKey,
    base_url: &Url,
    dummy_token: &str,
    hook: Option<Arc<dyn StopHook>>,
    outputs: RunOutputs,
) -> Result<SpawnRequest, RunArgvError> {
    let mut argv = agent_argv(plan, cmd, key, base_url, dummy_token)?;
    if let Some(first) = argv.first_mut() {
        *first = docker_binary();
    }
    let record_dir: PathBuf = plan
        .cidfile
        .parent()
        .map(PathBuf::from)
        .ok_or(RunArgvError::BadCidfile)?;
    let mut builder = SpawnRequest::builder()
        .argv(argv)
        .env(client_env())
        .marker(marker)
        // BORROW: .clone() gives the request its own run key
        .run(key.clone())
        .stdout(outputs.stdout)
        .stderr(outputs.stderr)
        .deadline(plan.expiry)
        .record_dir(record_dir);
    if let Some(attached) = hook {
        builder = builder.stop_hook(attached);
    }
    builder.build().map_err(|source| RunArgvError::Request {
        detail: source.to_string(),
    })
}
