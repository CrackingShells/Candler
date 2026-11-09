//! What every command shares: the store root (`--store`, else the configuration), the `Resolver`
//! with each milestone's preflight steps registered one per line, and the event sink the
//! supervisor reports to.

use std::path::PathBuf;
use std::sync::Arc;

use candler_core::evaluation::selection::select::{Selection, select};
use candler_core::resolve::config::{Config, ConfigEnv};
use candler_core::resolve::layers::{Resolution, ResolveInput};
use candler_core::resolve::preflight::Resolver;
use candler_core::store::Store;
use candler_schemas::primitives::FieldPath;

use super::CliError;
use crate::Cli;

#[cfg(unix)]
use candler_core::evaluation::registry::{Registry, register_evaluation_steps};
#[cfg(unix)]
use candler_core::supervise::{EventSink, Supervisor};
#[cfg(unix)]
use candler_schemas::execution::EventPayload;
#[cfg(unix)]
use candler_schemas::primitives::RunKey;
#[cfg(unix)]
use std::path::Path;

/// The path of the store root among the resolved settings.
const STORE_ROOT: &str = "store.root";

/// The configuration found from the working directory and the process environment.
///
/// # Errors
/// Returns [`CliError::Runtime`] when the working directory is unknown and [`CliError::Failed`]
/// when a configuration file is unreadable.
pub fn load_config(verb: &'static str) -> Result<Arc<Config>, CliError> {
    let cwd = std::env::current_dir().map_err(|e| CliError::Runtime {
        detail: format!("cannot read the working directory: {e}"),
    })?;
    Config::load(cwd.as_path(), &ConfigEnv::from_process())
        .map(Arc::new)
        .map_err(|e| CliError::failed(verb, e))
}

/// A resolver with no preflight step.
#[must_use]
pub fn plain_resolver(config: &Arc<Config>) -> Resolver {
    Resolver::new(Arc::clone(config))
}

/// The resolver `evaluate` runs: each milestone's `register_*_steps` call sits on its own line.
///
/// # Errors
/// Returns [`CliError::Failed`] when a step cannot be registered.
#[cfg(unix)]
pub fn evaluation_resolver(
    verb: &'static str,
    config: &Arc<Config>,
    registry: &Registry,
    env_root: &Path,
    supervisor: &Arc<Supervisor>,
) -> Result<Resolver, CliError> {
    let mut resolver = plain_resolver(config);
    resolver = register_evaluation_steps(resolver, registry, env_root, Arc::clone(supervisor))
        .map_err(|e| CliError::failed(verb, e))?;
    Ok(resolver)
}

/// Runs the preflight of `resolver`, with `--store` as the explicit store root when given.
///
/// # Errors
/// Returns [`CliError::Failed`] when the values cannot be layered or a step fails.
pub async fn resolve(
    verb: &'static str,
    cli: &Cli,
    config: &Config,
    resolver: &Resolver,
) -> Result<Resolution, CliError> {
    let mut input = ResolveInput::default();
    if let Some(store) = cli.store.as_deref() {
        let path: FieldPath = STORE_ROOT.parse().map_err(|e| CliError::failed(verb, e))?;
        input = input
            .explicit_path(config, path, store)
            .map_err(|e| CliError::failed(verb, e))?;
    }
    resolver
        .resolve(input)
        .await
        .map_err(|e| CliError::failed(verb, e))
}

/// The store root a resolution settled on.
///
/// # Errors
/// Returns [`CliError::Refused`] when neither `--store`, `CANDLER_STORE` nor the configuration
/// names one.
pub fn store_root(resolution: &Resolution) -> Result<PathBuf, CliError> {
    resolution
        .settings
        .iter()
        // BORROW: explicit .as_str() to compare the path text
        .find(|entry| entry.path().as_str() == STORE_ROOT)
        .and_then(|entry| entry.value())
        .and_then(|value| value.as_value().as_str())
        .map(PathBuf::from)
        .ok_or_else(|| {
            CliError::refused("no store: pass --store, set CANDLER_STORE or configure store.root")
        })
}

/// Opens the store the command line names, running no preflight step.
///
/// # Errors
/// Returns the errors of [`load_config`], [`resolve`], [`store_root`] and the store's own.
pub async fn open_store(verb: &'static str, cli: &Cli) -> Result<Store, CliError> {
    let config = load_config(verb)?;
    let resolver = plain_resolver(&config);
    let resolution = resolve(verb, cli, &config, &resolver).await?;
    let root = store_root(&resolution)?;
    // BORROW: explicit .as_path()
    Store::open(root.as_path()).map_err(|e| CliError::failed(verb, e))
}

/// The selection over every outcome record the store holds.
///
/// # Errors
/// Returns [`CliError::Failed`] when the store cannot be read.
pub fn read_selection(verb: &'static str, store: &Store) -> Result<Selection, CliError> {
    let mut records = Vec::new();
    for entry in store.runs().map_err(|e| CliError::failed(verb, e))? {
        for file in store
            .outcome_files(&entry.key)
            .map_err(|e| CliError::failed(verb, e))?
        {
            records.extend(
                store
                    .read_outcomes(&file)
                    .map_err(|e| CliError::failed(verb, e))?,
            );
        }
    }
    Ok(select(records))
}

/// The sink the supervisor reports to: one JSON line per event on stderr.
#[cfg(unix)]
#[derive(Debug, Default, Clone, Copy)]
pub struct StderrSink;

#[cfg(unix)]
impl EventSink for StderrSink {
    fn emit(&self, run: Option<&RunKey>, payload: EventPayload) {
        if let Ok(line) = serde_json::to_string(&payload) {
            match run {
                Some(run) => eprintln!("candler: event {run} {line}"),
                None => eprintln!("candler: event {line}"),
            }
        }
    }
}
