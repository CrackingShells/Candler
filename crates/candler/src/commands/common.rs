//! What every command shares: the one store root (`--store`, else the configuration), the `Resolver`
//! with each milestone's preflight steps registered one per line, and the event sink the
//! supervisor reports to.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use candler_core::analysis::report::choose_evaluator;
use candler_core::evaluation::selection::select::{Selection, select};
use candler_core::resolve::config::{Config, ConfigEnv};
use candler_core::resolve::layers::{Resolution, ResolveInput};
use candler_core::resolve::preflight::Resolver;
use candler_core::store::Store;
use candler_schemas::primitives::{FieldPath, NameVersion};

use super::CliError;
use super::refusal::CliRefusal;
use crate::Cli;

use candler_core::evaluation::registry::{Registry, register_evaluation_steps};
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::RunKey;

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
pub fn evaluation_resolver(
    verb: &'static str,
    config: &Arc<Config>,
    registry: &Registry,
    env_root: &Path,
    supervisor: &Arc<Supervisor>,
) -> Result<Resolver, CliError> {
    let mut resolver = plain_resolver(config);
    resolver = register_evaluation_steps(resolver, registry, env_root, supervisor)
        .map_err(|e| CliError::failed(verb, e))?;
    Ok(resolver)
}

/// Runs the preflight of `resolver`, with `explicit` (`--store` or `CANDLER_STORE`) as the
/// explicit store root when given.
///
/// # Errors
/// Returns [`CliError::Failed`] when the values cannot be layered or a step fails.
pub async fn resolve(
    verb: &'static str,
    explicit: Option<&Path>,
    config: &Config,
    resolver: &Resolver,
) -> Result<Resolution, CliError> {
    let mut input = ResolveInput::default();
    if let Some(store) = explicit {
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
pub fn resolved_root(resolution: &Resolution) -> Result<PathBuf, CliError> {
    resolution
        .settings
        .iter()
        // BORROW: explicit .as_str() to compare the path text
        .find(|entry| entry.path().as_str() == STORE_ROOT)
        .and_then(|entry| entry.value())
        .and_then(|value| value.as_value().as_str())
        .map(PathBuf::from)
        .ok_or_else(|| CliError::refused_by(CliRefusal::NoStore))
}

/// The root of the store every verb uses: `explicit` (`--store` or `CANDLER_STORE`), else the
/// configuration's `store.root`, else the core default, as the resolver layers them.
///
/// # Errors
/// Returns the errors of [`load_config`], [`resolve`] and [`resolved_root`].
pub async fn store_root(verb: &'static str, explicit: Option<&Path>) -> Result<PathBuf, CliError> {
    let config = load_config(verb)?;
    let resolver = plain_resolver(&config);
    let resolution = resolve(verb, explicit, &config, &resolver).await?;
    resolved_root(&resolution)
}

/// Opens the store the command line names, running no preflight step.
///
/// # Errors
/// Returns the errors of [`store_root`] and the store's own.
pub async fn open_store(verb: &'static str, cli: &Cli) -> Result<Store, CliError> {
    let root = store_root(verb, cli.store.as_deref()).await?;
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

/// The evaluator a verb works on, as the flag `flag` names it.
///
/// A `name@version` is that evaluator, as it was written; a bare version is the one evaluator of
/// the selection that holds it; no request is the one evaluator the selection holds. The choice of
/// the last two is [`choose_evaluator`], the one `analyze` makes.
///
/// # Errors
/// [`CliError::Coded`] for a `name@version` that does not parse and for the refusals of
/// [`choose_evaluator`]: no outcomes, several evaluators and no request, a version no
/// evaluator holds, and a version several evaluators hold.
pub fn evaluator_of(
    selection: &Selection,
    flag: &'static str,
    requested: Option<&str>,
) -> Result<NameVersion, CliError> {
    if let Some(text) = requested.filter(|text| text.contains('@')) {
        return named_evaluator(flag, text);
    }
    choose_evaluator(selection, requested)
        .map_err(|e| CliError::refused_by(e).led_by(format!("{flag}: ")))
}

/// Refuses `text` when it holds an `@` and is not a `name@version`, as [`evaluator_of`] would, so
/// that the refusal comes before the store is read; any other text passes.
///
/// # Errors
/// [`CliError::Coded`] for a `name@version` that does not parse.
pub fn check_evaluator_text(flag: &'static str, text: &str) -> Result<(), CliError> {
    if text.contains('@') {
        named_evaluator(flag, text).map(drop)
    } else {
        Ok(())
    }
}

/// The evaluator `text` names, written as `name@version`.
fn named_evaluator(flag: &'static str, text: &str) -> Result<NameVersion, CliError> {
    text.parse::<NameVersion>().map_err(|e| {
        CliError::refused_by(CliRefusal::EvaluatorInvalid {
            flag: flag.to_owned(), // BORROW: the refusal owns the flag
            text: text.to_owned(), // BORROW: the refusal owns the text
            detail: e.to_string(),
        })
    })
}

/// The sink the supervisor reports to: one JSON line per event on stderr.
#[derive(Debug, Default, Clone, Copy)]
pub struct StderrSink;

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
