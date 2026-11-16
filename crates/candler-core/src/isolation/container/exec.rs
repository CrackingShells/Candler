//! The two calls that run a command in the world of a run's agent container: `docker exec` in the
//! running container, and `docker run --rm` of a throwaway container over the run's mounts.
//!
//! The first serves a pre-stop, while the container runs. The second serves a collect after the
//! agent exited: a stopped container takes no `exec`, and what a harness keeps (its sessions) is
//! in the run's mounts, so a control container over the same mounts and environment reads it.
//! Neither carries the gateway's address or token.

use std::time::Duration;

use candler_harness::spec::CommandSpec;
use candler_schemas::primitives::ContainerId;

use super::expiry::{ceil_secs, wrap_expiry};
use super::mounts::{MountRoots, mounts};
use super::run::{RunArgvError, check_name, env_args, env_pairs};
use crate::isolation::control::ControlSpec;
use crate::isolation::hardening::HardLimits;
use crate::isolation::layout::CONTAINER_WORKDIR;
use crate::isolation::model::{ContainerPlan, UserSpec};

/// The working directory of `cmd` inside the container.
fn workdir_of(base: &str, cmd: &CommandSpec) -> String {
    cmd.cwd().map_or_else(
        // BORROW: the base is copied into the argument
        || base.to_owned(),
        |cwd| format!("{}/{cwd}", base.trim_end_matches('/')),
    )
}

/// The arguments of `docker exec` that run `cmd` in the container `cid`, after `docker`. The
/// command inherits the container's environment; only its own literal overlay is added.
///
/// # Errors
/// [`RunArgvError`] when the overlay holds a secret or a name docker would misread.
pub fn exec_args(cid: &ContainerId, cmd: &CommandSpec) -> Result<Vec<String>, RunArgvError> {
    let mut args = vec![String::from("exec")];
    for entry in cmd.env_overlay() {
        // BORROW: explicit .as_str() instead of Deref coercion
        check_name(entry.name.as_str())?;
    }
    args.extend(env_args(cmd.env_overlay(), &[])?);
    args.push(String::from("--workdir"));
    args.push(workdir_of(CONTAINER_WORKDIR, cmd));
    args.push(cid.as_str().to_owned()); // BORROW: explicit .as_str() makes the owned id
    args.extend(cmd.argv().iter().cloned()); // BORROW: the argv owns its arguments
    Ok(args)
}

/// The control container that runs `cmd` in a throwaway container of the plan's image, over the
/// plan's mounts, environment and user. The container has no network, the plan's limits, the label
/// `candler.role=control` and no job label, so no sweep meets it and the guard never destroys it.
/// The command runs under an expiry of `wall`.
///
/// # Errors
/// [`RunArgvError`] when a mount is refused, the overlay holds a secret, or a name is misread.
pub fn collect_spec(
    plan: &ContainerPlan,
    cmd: &CommandSpec,
    wall: Duration,
) -> Result<ControlSpec, RunArgvError> {
    let wrapped = wrap_expiry(cmd, ceil_secs(wall)).map_err(|source| RunArgvError::Command {
        detail: source.to_string(),
    })?;
    let mut env = Vec::with_capacity(plan.env.len());
    for (name, value) in &plan.env {
        // BORROW: explicit .as_str() instead of Deref coercion
        check_name(name.as_str())?;
        env.push(format!("{name}={value}"));
    }
    for entry in wrapped.env_overlay() {
        if plan.env.iter().any(|(name, _)| *name == entry.name) {
            return Err(RunArgvError::DuplicateName {
                name: entry.name.clone(), // BORROW: the error owns the name
            });
        }
        // BORROW: explicit .as_str() instead of Deref coercion
        check_name(entry.name.as_str())?;
    }
    env.extend(env_pairs(wrapped.env_overlay(), &[])?);
    let mut spec = ControlSpec::new(&plan.image.id.0, HardLimits::sized(&plan.limits))
        .workdir(workdir_of(plan.workdir.as_str(), cmd)) // BORROW: explicit .as_str()
        .env(env)
        .command(wrapped.argv().to_vec()); // BORROW: the spec owns its words
    if let UserSpec::Host { uid, gid } = &plan.user {
        spec = spec.user(*uid, *gid);
    }
    for mount in mounts(plan, &MountRoots::from_plan(plan))? {
        spec = spec.mount(mount.to_arg()?);
    }
    Ok(spec)
}
