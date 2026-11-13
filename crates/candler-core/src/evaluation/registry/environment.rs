//! Building plug-in environments once per lock digest.
//!
//! Each plug-in with a `uv` environment gets a preflight step that runs `uv sync --frozen` into
//! `<env root>/<digest>/.venv`, where the digest covers the lock file and `pyproject.toml`. A
//! `ready` marker, written only after a successful sync, makes every later preflight under the
//! same digest spawn nothing. The environment lies outside the store, so a plug-in never writes
//! into a store blob through it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use candler_schemas::evaluation::PluginEnvironment;
use candler_schemas::primitives::{Digest, Marker, NameToken};
use candler_schemas::process::ExitStatus;
use futures::future::BoxFuture;
use sha2::{Digest as _, Sha256};

use super::manifest::Plugin;
use super::{Registry, RegistryError};
use crate::resolve::config::Config;
use crate::resolve::preflight::{
    CheckResult, PreflightError, PreflightStep, ResolveContext, ResolveError, Resolver, StepReport,
};
use crate::supervise::Supervisor;
use crate::supervise::spawn::SpawnRequest;

/// The marker file that says the environment under a digest is complete.
const READY: &str = "ready";

/// How much of `uv`'s output a failed check keeps.
const OUTPUT_LIMIT: usize = 8 * 1024;

/// How long `uv sync` may run before the supervisor stops it.
const SYNC_DEADLINE: Duration = Duration::from_mins(30);

/// The version token of the steps this module registers.
const STEP_VERSION: &str = "1";

/// Names an environment by the digest of the files it is built from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct EnvironmentKey {
    hex: String,
}

impl EnvironmentKey {
    /// The key of the lock file `lock` and the project file `pyproject`: the `sha256` of the lock
    /// file's bytes followed by the project file's bytes, as 64 lowercase hex digits.
    ///
    /// # Errors
    /// Returns [`RegistryError::EnvironmentFile`] if either file cannot be read.
    pub async fn of_files(lock: &Path, pyproject: &Path) -> Result<Self, RegistryError> {
        let mut hasher = Sha256::new();
        for file in [lock, pyproject] {
            let bytes =
                tokio::fs::read(file)
                    .await
                    .map_err(|source| RegistryError::EnvironmentFile {
                        // BORROW: .to_owned() keeps the path in the error
                        path: file.to_owned(),
                        source,
                    })?;
            hasher.update(bytes);
        }
        Ok(Self {
            hex: Digest::from(hasher).hex().to_owned(), // BORROW: the key owns its hex
        })
    }

    /// The 64 hex digits.
    #[must_use]
    pub const fn digest<'a>(&'a self) -> &'a str {
        // BORROW: explicit .as_str() instead of Deref coercion
        self.hex.as_str()
    }

    /// The directory of this environment under `env_root`.
    #[must_use]
    pub fn dir(&self, env_root: &Path) -> PathBuf {
        env_root.join(self.digest())
    }
}

/// The directory that holds the environments: `<state home>/candler/environments`.
///
/// # Errors
/// Returns [`RegistryError::NoStateHome`] if the configuration has no state home, which is the
/// case when neither `XDG_STATE_HOME` nor `HOME` was set.
pub fn default_env_root(config: &Config) -> Result<PathBuf, RegistryError> {
    config
        .state_home
        .as_ref()
        .map(|home| home.join("candler").join("environments"))
        .ok_or(RegistryError::NoStateHome)
}

/// The preflight step that builds one plug-in's environment.
#[derive(Debug)]
#[non_exhaustive]
pub struct EnvironmentStep {
    name: NameToken,
    check: NameToken,
    project_dir: PathBuf,
    lock: PathBuf,
    pyproject: PathBuf,
    env_root: PathBuf,
    forwarded: Vec<(String, String)>,
    supervisor: Arc<Supervisor>,
}

impl EnvironmentStep {
    /// The step for `plugin`, or `None` if its manifest declares no environment. The files are
    /// resolved against the plug-in directory.
    ///
    /// # Errors
    /// Returns [`ResolveError::Format`] if the plug-in's name cannot form a step name.
    pub fn for_plugin(
        plugin: &Plugin,
        env_root: &Path,
        supervisor: Arc<Supervisor>,
    ) -> Result<Option<Self>, ResolveError> {
        let PluginEnvironment::Uv { project, lock, .. } = plugin.manifest.environment() else {
            // EXPLICIT: a plug-in with no `uv` environment has nothing to build, so no step
            return Ok(None);
        };
        let project_dir = plugin.dir.join(project.as_str());
        let name = plugin.manifest.name();
        Ok(Some(Self {
            name: format!("environment-{name}@{STEP_VERSION}").parse()?,
            check: format!("environment-ready-{name}@{STEP_VERSION}").parse()?,
            lock: plugin.dir.join(lock.as_str()),
            pyproject: project_dir.join("pyproject.toml"),
            project_dir,
            env_root: env_root.to_owned(),
            // The supervisor clears the environment, so the variables `uv` needs go in by name.
            forwarded: ["PATH", "HOME"]
                .iter()
                .filter_map(|name| {
                    std::env::var_os(name)
                        .and_then(|value: OsString| value.into_string().ok())
                        .map(|value| ((*name).to_owned(), value))
                })
                .collect(),
            supervisor,
        }))
    }

    /// The same step with `path` as the `PATH` that `uv` is found by and runs with, instead of the
    /// process's own.
    #[must_use]
    pub fn with_path(mut self, path: &str) -> Self {
        self.forwarded.retain(|(name, _)| name != "PATH");
        // BORROW: .to_owned() makes the owned value the request carries
        self.forwarded.push(("PATH".to_owned(), path.to_owned()));
        self
    }
}

impl PreflightStep for EnvironmentStep {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        _ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        let job = Job {
            // BORROW: .clone() gives the future its own copy
            name: self.name.clone(),
            check: self.check.clone(),
            project_dir: self.project_dir.clone(),
            lock: self.lock.clone(),
            pyproject: self.pyproject.clone(),
            env_root: self.env_root.clone(),
            forwarded: self.forwarded.clone(),
            supervisor: Arc::clone(&self.supervisor),
        };
        Box::pin(job.run())
    }
}

/// What one run of the step owns.
struct Job {
    name: NameToken,
    check: NameToken,
    project_dir: PathBuf,
    lock: PathBuf,
    pyproject: PathBuf,
    env_root: PathBuf,
    forwarded: Vec<(String, String)>,
    supervisor: Arc<Supervisor>,
}

fn failed(detail: impl std::fmt::Display) -> PreflightError {
    PreflightError::Failed {
        detail: detail.to_string(),
    }
}

impl Job {
    async fn run(self) -> Result<StepReport, PreflightError> {
        let key = EnvironmentKey::of_files(&self.lock, &self.pyproject)
            .await
            .map_err(failed)?;
        let dir = key.dir(&self.env_root);
        let report = StepReport::new(self.name.clone());
        let ready = dir.join(READY);
        if tokio::fs::try_exists(&ready).await.map_err(failed)? {
            return Ok(report.check(CheckResult::new(
                self.check,
                true,
                format!("environment {} is ready, nothing was spawned", key.digest()),
            )));
        }
        let records = dir.join("records");
        tokio::fs::create_dir_all(&records).await.map_err(failed)?;
        let request = self.request(&key, &dir, records)?;
        let captured = self
            .supervisor
            .run_captured(request, OUTPUT_LIMIT)
            .await
            .map_err(failed)?;
        if !matches!(captured.exit.status, ExitStatus::Code { code: 0 }) {
            let output = String::from_utf8_lossy(&captured.stderr);
            return Ok(report.check(CheckResult::new(
                self.check,
                false,
                format!(
                    "`uv sync --frozen` for environment {} did not succeed ({:?}): {}",
                    key.digest(),
                    captured.exit.status,
                    output.trim_end()
                ),
            )));
        }
        // The marker appears whole or not at all, and only after the sync succeeded.
        let pending = dir.join("ready.tmp");
        tokio::fs::write(&pending, key.digest())
            .await
            .map_err(failed)?;
        tokio::fs::rename(&pending, &ready).await.map_err(failed)?;
        Ok(report.check(CheckResult::new(
            self.check,
            true,
            format!("environment {} was built", key.digest()),
        )))
    }

    /// The `uv sync` request for the environment `key` in `dir`.
    fn request(
        &self,
        key: &EnvironmentKey,
        dir: &Path,
        records: PathBuf,
    ) -> Result<SpawnRequest, PreflightError> {
        // BORROW: .clone() builds the request's own environment
        let mut env = self.forwarded.clone();
        env.push((
            "UV_PROJECT_ENVIRONMENT".to_owned(),
            dir.join(".venv").display().to_string(),
        ));
        let marker: Marker = format!("environment-{}", key.digest())
            .parse()
            .map_err(failed)?;
        SpawnRequest::builder()
            .argv(vec![
                "uv".to_owned(),
                "sync".to_owned(),
                "--frozen".to_owned(),
                "--project".to_owned(),
                self.project_dir.display().to_string(),
            ])
            .env(env)
            .marker(marker)
            .deadline(SYNC_DEADLINE)
            .record_dir(records)
            .build()
            .map_err(failed)
    }
}

/// Registers one [`EnvironmentStep`] per plug-in that declares a `uv` environment, after the
/// steps `resolver` already holds. Every step spawns through `supervisor`, the shared one, so that
/// stopping the supervisor stops `uv` too. The core never calls this itself; the `candler` crate
/// composes it.
///
/// # Errors
/// Returns the errors of [`EnvironmentStep::for_plugin`] and of [`Resolver::register`].
pub fn register_evaluation_steps(
    mut resolver: Resolver,
    registry: &Registry,
    env_root: &Path,
    supervisor: &Arc<Supervisor>,
) -> Result<Resolver, ResolveError> {
    for plugin in registry.plugins().values() {
        if let Some(step) = EnvironmentStep::for_plugin(plugin, env_root, Arc::clone(supervisor))? {
            // TRAIT_OBJECT: steps of different types are registered side by side
            resolver = resolver.register(Box::new(step))?;
        }
    }
    Ok(resolver)
}
