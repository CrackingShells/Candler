//! The fixed names of the container's world: where the workspace is mounted, the home directory,
//! the user, the label keys and the expiry margin. Production code reads them from here; a test
//! states its expected values as literals and never imports this module.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// The workspace directory inside the container.
pub const CONTAINER_WORKDIR: &str = "/work";

/// The home directory inside the container.
pub const CONTAINER_HOME: &str = "/home/agent";

/// The user name inside the container.
pub const CONTAINER_USER: &str = "agent";

/// The label that names the job.
pub const LABEL_JOB: &str = "candler.job";

/// The label that names the run, in its path form.
pub const LABEL_RUN: &str = "candler.run";

/// The label that names the experiment digest.
pub const LABEL_EXPERIMENT: &str = "candler.experiment";

/// The label that names the container's role.
pub const LABEL_ROLE: &str = "candler.role";

/// How long past the run's deadline a container may live before it expires on its own.
pub const EXPIRY_MARGIN: Duration = Duration::from_secs(60);

/// Where the agent's Python environment lives in the container. The env image builds it there,
/// and each agent container mounts a fresh volume at this path, which docker fills from the image,
/// so the agent can install into its own environment while the root filesystem stays read-only.
pub const CONTAINER_ENV: &str = "/opt/env";

/// The version of the sandbox contract: how the agent's container is isolated and what it is
/// given. A manifest records it as `provenance.sandbox_contract`.
pub const SANDBOX_CONTRACT: &str = "v6";

/// The directory under which a control call (a short-lived `docker run` outside any job) keeps its
/// record dir while it runs: `<store>/control`. Never a job's `children/`.
#[must_use]
pub fn control_dir(store_root: &Path) -> PathBuf {
    store_root.join("control")
}

/// The lock file that serialises the build of the image `tag`: `<store>/images/<tag>.lock`.
#[must_use]
pub fn image_lock_path(store_root: &Path, tag: &str) -> PathBuf {
    store_root.join("images").join(format!("{tag}.lock"))
}
