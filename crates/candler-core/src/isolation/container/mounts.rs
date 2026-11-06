//! The mounts of a run's container.
//!
//! A mount list is built only from canonical paths inside the project root or the store. A source
//! outside both, a symlink that leaves them, the docker socket and anything under `/var/run` or
//! `/run` are refused with the offending path in the error.

use std::path::{Component, Path, PathBuf};

use crate::isolation::model::ContainerPlan;

/// The directories that hold the host side of the host paths the engine may mount.
const RUN_DIRS: [&str; 3] = ["/var/run", "/private/var/run", "/run"];

/// The name of the docker socket, which no mount may name.
const DOCKER_SOCK: &str = "docker.sock";

/// The environment name that moves the config directory of Claude Code.
const CONFIG_DIR_ENV: &str = "CLAUDE_CONFIG_DIR";

/// Whether a mount may be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MountMode {
    /// The container may read only.
    ReadOnly,
    /// The container may read and write.
    ReadWrite,
}

/// One bind mount.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountSpec {
    /// The host path.
    pub source: PathBuf,
    /// The path inside the container.
    pub target: String,
    /// Whether the container may write.
    pub mode: MountMode,
}

/// The two roots a mount source may sit under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountRoots {
    /// The project root.
    pub project: PathBuf,
    /// The store root.
    pub store: PathBuf,
}

impl MountRoots {
    /// The roots the plan names.
    #[must_use]
    pub fn from_plan(plan: &ContainerPlan) -> Self {
        Self {
            // BORROW: the roots are copied out of the plan
            project: plan.project_root.clone(),
            // BORROW: the roots are copied out of the plan
            store: plan.store_root.clone(),
        }
    }
}

/// Why a mount list was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MountError {
    /// The source sits outside the project root and the store.
    #[error("mount source {path} is outside the project root and the store")]
    OutsideRoots {
        /// The source as given.
        path: PathBuf,
    },
    /// The source is inside a root by its name but its target is not.
    #[error("mount source {path} is a symlink that leaves the project root and the store")]
    SymlinkEscape {
        /// The source as given.
        path: PathBuf,
    },
    /// The source names the docker socket or sits under `/var/run` or `/run`.
    #[error("mount source {path} is the docker socket or sits under a run directory")]
    DockerSocket {
        /// The source as given.
        path: PathBuf,
    },
    /// A path holds a comma, a newline or text that is not UTF-8, which would inject an option.
    #[error("mount path {path:?} cannot be written as a --mount option")]
    BadPath {
        /// The path, lossily written.
        path: String,
    },
    /// No mount makes the config directory writable.
    #[error("no writable mount holds the config directory")]
    ConfigDirNotWritable,
    /// The source cannot be resolved on the host.
    #[error("mount source {path} cannot be resolved: {detail}")]
    Unreadable {
        /// The source as given.
        path: PathBuf,
        /// What the operating system said.
        detail: String,
    },
}

impl MountSpec {
    /// The text after `--mount`: `type=bind,source=..,target=..[,readonly]`.
    ///
    /// # Errors
    /// [`MountError::BadPath`] when the source or the target holds a comma or a newline, or the
    /// source is not UTF-8.
    pub fn to_arg(&self) -> Result<String, MountError> {
        let source = self
            .source
            .to_str()
            .ok_or_else(|| bad_path(&self.source.to_string_lossy()))?;
        // BORROW: explicit .as_str() instead of Deref coercion
        let target = self.target.as_str();
        for text in [source, target] {
            if text.contains([',', '\n', '\r', '\0']) {
                return Err(bad_path(text));
            }
        }
        let suffix = match self.mode {
            MountMode::ReadOnly => ",readonly",
            MountMode::ReadWrite => "",
        };
        Ok(format!("type=bind,source={source},target={target}{suffix}"))
    }
}

fn bad_path(text: &str) -> MountError {
    MountError::BadPath {
        // BORROW: .to_owned() keeps the offending path in the error
        path: text.to_owned(),
    }
}

/// Whether a path names the docker socket or sits under a run directory.
fn is_socket_path(path: &Path) -> bool {
    path.components()
        .any(|part| matches!(part, Component::Normal(name) if name == DOCKER_SOCK))
        || RUN_DIRS.iter().any(|dir| path.starts_with(dir))
}

/// The canonical form of `path`, or the error that says why it has none.
fn canonical(path: &Path) -> Result<PathBuf, MountError> {
    std::fs::canonicalize(path).map_err(|source| MountError::Unreadable {
        // BORROW: .to_path_buf() keeps the offending path in the error
        path: path.to_path_buf(),
        detail: source.to_string(),
    })
}

/// Checks every mount of the plan and returns them with canonical sources.
///
/// # Errors
/// A [`MountError`] naming the first source that is refused, or
/// [`MountError::ConfigDirNotWritable`] when no writable mount holds the home directory or the
/// directory `CLAUDE_CONFIG_DIR` names.
pub fn mounts(plan: &ContainerPlan, roots: &MountRoots) -> Result<Vec<MountSpec>, MountError> {
    let project = canonical(&roots.project)?;
    let store = canonical(&roots.store)?;
    let mut checked = Vec::with_capacity(plan.mounts.len());
    for mount in &plan.mounts {
        checked.push(MountSpec {
            source: check_source(
                &mount.source,
                &[&roots.project, &roots.store],
                &[&project, &store],
            )?,
            // BORROW: the target is copied into the checked mount
            target: mount.target.clone(),
            mode: mount.mode,
        });
    }
    let config_dir = plan
        .env
        .iter()
        .find(|(name, _)| name == CONFIG_DIR_ENV)
        // BORROW: explicit .as_str() instead of Deref coercion
        .map(|(_, value)| value.as_str());
    let writable = checked.iter().any(|mount| {
        mount.mode == MountMode::ReadWrite
            // BORROW: explicit .as_str() instead of Deref coercion
            && (mount.target == plan.home || Some(mount.target.as_str()) == config_dir)
    });
    if writable {
        Ok(checked)
    } else {
        Err(MountError::ConfigDirNotWritable)
    }
}

/// Checks one source against the roots and returns its canonical path.
fn check_source(
    source: &Path,
    lexical_roots: &[&Path],
    canonical_roots: &[&Path],
) -> Result<PathBuf, MountError> {
    // BORROW: the owned copy is made where it is needed
    let refuse = |make: fn(PathBuf) -> MountError| make(source.to_path_buf());
    let as_given = |path: PathBuf| MountError::DockerSocket { path };
    if is_socket_path(source) {
        return Err(refuse(as_given));
    }
    let real = canonical(source)?;
    if is_socket_path(&real) {
        return Err(refuse(as_given));
    }
    let inside_real = canonical_roots.iter().any(|root| real.starts_with(root));
    if inside_real {
        return Ok(real);
    }
    let inside_name = lexical_roots
        .iter()
        .chain(canonical_roots.iter())
        .any(|root| source.starts_with(root));
    if inside_name {
        Err(refuse(|path| MountError::SymlinkEscape { path }))
    } else {
        Err(refuse(|path| MountError::OutsideRoots { path }))
    }
}

#[cfg(test)]
mod scaffold {
    use std::os::unix::fs::symlink;
    use std::str::FromStr as _;
    use std::time::Duration;

    use candler_schemas::primitives::JobId;

    use super::*;
    use crate::isolation::model::{
        ContainerPlan, ImageId, ImageRef, Labels, Limits, NetworkName, UserSpec,
    };

    fn plan(project: &Path, store: &Path, list: Vec<MountSpec>) -> ContainerPlan {
        let job = JobId::from_str("01900000-0000-7000-8000-000000000000").expect("a job id");
        ContainerPlan {
            image: ImageRef {
                id: ImageId(String::from("sha256:abc")),
                cc_version: None,
            },
            labels: Labels::new(),
            mounts: list,
            env: Vec::new(),
            network: NetworkName::for_job(&job),
            egress: None,
            limits: Limits {
                pids_limit: 64,
                memory: String::from("1g"),
                cpus: 1.0,
            },
            cidfile: project.join("children/a.cid"),
            user: UserSpec::Engine,
            workdir: String::from("/work"),
            home: String::from("/home/agent"),
            expiry: Duration::from_secs(10),
            project_root: project.to_path_buf(),
            store_root: store.to_path_buf(),
        }
    }

    fn rw(source: &Path, target: &str) -> MountSpec {
        MountSpec {
            source: source.to_path_buf(),
            target: String::from(target),
            mode: MountMode::ReadWrite,
        }
    }

    fn check(
        list: Vec<MountSpec>,
        project: &Path,
        store: &Path,
    ) -> Result<Vec<MountSpec>, MountError> {
        let p = plan(project, store, list);
        mounts(&p, &MountRoots::from_plan(&p))
    }

    struct Dirs {
        _tmp: tempfile::TempDir,
        project: PathBuf,
        store: PathBuf,
        outside: PathBuf,
    }

    fn dirs() -> Dirs {
        let tmp = tempfile::tempdir().expect("tmp");
        let base = tmp.path().canonicalize().expect("canonical");
        let project = base.join("project");
        let store = base.join("store");
        let outside = base.join("outside");
        for d in [&project, &store, &outside] {
            std::fs::create_dir_all(d).expect("mkdir");
        }
        Dirs {
            _tmp: tmp,
            project,
            store,
            outside,
        }
    }

    #[test]
    fn inside_roots_pass() {
        let d = dirs();
        let home = d.project.join("home");
        std::fs::create_dir_all(&home).expect("mkdir");
        let out = check(vec![rw(&home, "/home/agent")], &d.project, &d.store).expect("ok");
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn outside_is_refused() {
        let d = dirs();
        let r = check(vec![rw(&d.outside, "/home/agent")], &d.project, &d.store);
        assert!(matches!(r, Err(MountError::OutsideRoots { .. })), "{r:?}");
    }

    #[test]
    fn symlink_out_is_refused_and_symlink_in_passes() {
        let d = dirs();
        let link = d.project.join("link");
        symlink(&d.outside, &link).expect("symlink");
        let r = check(vec![rw(&link, "/home/agent")], &d.project, &d.store);
        assert!(matches!(r, Err(MountError::SymlinkEscape { .. })), "{r:?}");
        let real = d.project.join("real");
        std::fs::create_dir_all(&real).expect("mkdir");
        let inlink = d.project.join("inlink");
        symlink(&real, &inlink).expect("symlink");
        assert!(check(vec![rw(&inlink, "/home/agent")], &d.project, &d.store).is_ok());
    }

    #[test]
    fn docker_socket_and_run_are_refused() {
        let d = dirs();
        for src in [
            "/var/run/docker.sock",
            "/run",
            "/var/run",
            "/private/var/run",
        ] {
            let r = check(
                vec![rw(Path::new(src), "/home/agent")],
                &d.project,
                &d.store,
            );
            assert!(r.is_err(), "{src}: {r:?}");
        }
        let link = d.project.join("varrun");
        symlink("/var/run", &link).expect("symlink");
        assert!(check(vec![rw(&link, "/home/agent")], &d.project, &d.store).is_err());
    }

    #[test]
    fn comma_in_name_is_refused_in_the_argument() {
        let spec = rw(Path::new("/a,b"), "/x");
        assert!(matches!(spec.to_arg(), Err(MountError::BadPath { .. })));
        let ok = rw(Path::new("/a"), "/x");
        assert_eq!(ok.to_arg().expect("arg"), "type=bind,source=/a,target=/x");
    }

    #[test]
    fn a_writable_home_is_required() {
        let d = dirs();
        let ro = MountSpec {
            source: d.project.clone(),
            target: String::from("/home/agent"),
            mode: MountMode::ReadOnly,
        };
        let r = check(vec![ro], &d.project, &d.store);
        assert!(matches!(r, Err(MountError::ConfigDirNotWritable)), "{r:?}");
    }
}
