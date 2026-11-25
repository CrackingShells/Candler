//! The `docker run` command line of a run's container, and the spawn request that carries it.
//!
//! The environment is built from nothing: the plan's entries, the gateway's base url and
//! dummy token under the variable names the harness driver gives, and the harness's plain
//! overlay. A secret in the overlay, and the names the driver reserves for the model endpoint and
//! credentials, are refused rather than overridden.

use std::sync::Arc;

use candler_harness::spec::{CommandSpec, EnvEntry, EnvValue};
use candler_schemas::primitives::{ContainerId, Marker, RunKey};
use url::Url;

use super::expiry::{ceil_secs, wrap_expiry};
use super::mounts::{MountError, MountRoots, mounts};
use crate::isolation::RunOutputs;
use crate::isolation::docker::{AttachedRun, PROGRAM};
use crate::isolation::driver::DockerDriver;
use crate::isolation::hardening::{HardLimits, Role, hardening};
use crate::isolation::layout::{EXPIRY_MARGIN, LABEL_JOB, LABEL_ROLE, LABEL_RUN};
use crate::isolation::model::{ContainerPlan, UserSpec};
use crate::supervise::hook::StopHook;
use crate::supervise::spawn::SpawnRequest;

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
    #[error("environment variable {name:?} is declared twice")]
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

/// The `NAME=value` entries of the harness's overlay. `reserved` lists the names the wrapper
/// owns, which the overlay may not set.
///
/// # Errors
/// [`RunArgvError::SecretRefused`] for a secret, [`RunArgvError::ReservedName`] for a name the
/// wrapper owns.
pub fn env_pairs(overlay: &[EnvEntry], reserved: &[String]) -> Result<Vec<String>, RunArgvError> {
    let mut pairs = Vec::with_capacity(overlay.len());
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
            pairs.push(format!("{name}={value}"));
            continue;
        }
        // EXPLICIT: a secret and any value kind not read above both refuse, until a new kind is read
        return Err(RunArgvError::SecretRefused {
            // BORROW: .to_owned() keeps the name in the error
            name: name.to_owned(),
        });
    }
    Ok(pairs)
}

/// The `-e NAME=value` pairs of the harness's overlay, as arguments. See [`env_pairs`].
///
/// # Errors
/// [`RunArgvError::SecretRefused`] for a secret, [`RunArgvError::ReservedName`] for a name the
/// wrapper owns.
pub fn env_args(overlay: &[EnvEntry], reserved: &[String]) -> Result<Vec<String>, RunArgvError> {
    Ok(env_pairs(overlay, reserved)?
        .into_iter()
        .flat_map(|pair| [String::from("-e"), pair])
        .collect())
}

/// Checks an environment name docker could take.
pub(super) fn check_name(name: &str) -> Result<(), RunArgvError> {
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

/// The `docker create` argv of an agent container, from `docker` to the end of the command. The
/// command is run as given: [`agent_argv`] wraps it in the expiry first.
///
/// The order is fixed: `-i` when the plan is interactive, hardening, labels, environment, mounts,
/// the image id, the command. There is no `--rm`, so the supervisor can still read the exit and
/// the guard destroys the container.
///
/// # Errors
/// [`RunArgvError`] when the plan lacks the job, run or role label, when the overlay is refused
/// (see [`env_args`]) or repeats a name the plan sets, or when a mount cannot be written.
pub fn create_argv(plan: &ContainerPlan, cmd: &CommandSpec) -> Result<Vec<String>, RunArgvError> {
    label(plan, LABEL_JOB)?;
    label(plan, LABEL_RUN)?;
    label(plan, LABEL_ROLE)?;
    let mut argv = vec![String::from(PROGRAM), String::from("create")];
    if plan.interactive {
        argv.push(String::from("-i"));
    }
    argv.extend(hardening(&HardLimits::agent(&plan.limits)));
    // BORROW: the owned copy is made where it is needed
    argv.extend([String::from("--network"), plan.network.as_str().to_owned()]);
    if let Some(egress) = &plan.egress {
        // A second user-defined network: docker takes two on one `docker run` (Engine 25 and
        // later) and refuses the default `bridge` beside a user-defined one (exit 125).
        // BORROW: the owned copy is made where it is needed
        argv.extend([String::from("--network"), egress.as_str().to_owned()]);
    }
    for (name, ip) in &plan.add_hosts {
        argv.extend([String::from("--add-host"), format!("{name}:{ip}")]);
    }
    if let UserSpec::Host { uid, gid } = &plan.user {
        argv.extend([String::from("--user"), format!("{uid}:{gid}")]);
    }
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

/// The names the plan reserves for the wrapper: the driver's gateway variables, none when the
/// plan names no gateway.
fn reserved_names<'a>(plan: &'a ContainerPlan) -> &'a [String] {
    plan.gateway
        .as_ref()
        .map_or(&[], |gateway| gateway.reserved.as_slice())
}

/// The `docker create` argv of the agent container that runs `cmd` for the run `key`: the mounts
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
        .with(LABEL_ROLE, Role::Agent.label());
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
    create_argv(&planned, &wrapped)
}

/// The `docker start` argv that attaches to the created container `cid`: `-a`, and `-i` when the
/// plan is interactive, which is when the container was created with `-i` as well.
#[must_use]
pub fn start_argv(plan: &ContainerPlan, cid: &ContainerId) -> Vec<String> {
    let mut argv = vec![
        String::from(PROGRAM),
        String::from("start"),
        String::from("-a"),
    ];
    if plan.interactive {
        argv.push(String::from("-i"));
    }
    argv.push(cid.as_str().to_owned()); // BORROW: the argv owns the id
    argv
}

/// The supervised request that starts the created container `cid` and stays attached to it until
/// it exits, over the docker binary `docker` runs: `docker start -a`, with `-i` when the plan is
/// interactive, the client's own environment, and the supervisor's deadline at the wall. The
/// request carries `hook` as its stop hook.
///
/// # Errors
/// [`RunArgvError::Request`] when the request is refused.
pub fn build_start_request(
    docker: &dyn DockerDriver, // TRAIT_OBJECT: the driver is docker, or a fake binary in tests
    plan: &ContainerPlan,
    cid: &ContainerId,
    marker: Marker,
    key: &RunKey,
    hook: Option<Arc<dyn StopHook>>,
    outputs: RunOutputs,
) -> Result<SpawnRequest, RunArgvError> {
    let argv = start_argv(plan, cid);
    docker
        .attach(AttachedRun {
            argv,
            marker,
            key: key.clone(), // BORROW: .clone() gives the request its own run key
            stdout: outputs.stdout,
            stderr: outputs.stderr,
            deadline: plan.expiry,
            record_dir: plan.record_dir.clone(), // BORROW: the request owns its directory
            interactive: plan.interactive,
            hook,
        })
        .map_err(|source| RunArgvError::Request {
            detail: source.to_string(),
        })
}
