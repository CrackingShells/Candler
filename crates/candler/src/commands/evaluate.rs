//! `candler evaluate`: grade stored runs with a plug-in's suite.

use super::{CliError, Exit};
use crate::Cli;
use std::num::NonZeroUsize;
use std::path::PathBuf;

/// The arguments of `candler evaluate`.
#[derive(Debug, clap::Args)]
pub struct EvaluateArgs {
    /// A plug-in manifest; repeat for several plug-ins.
    #[arg(long = "plugin", required = true)]
    pub plugins: Vec<PathBuf>,
    /// The suite file.
    #[arg(long)]
    pub suite: PathBuf,
    /// A glob over run keys; every run when absent.
    #[arg(long)]
    pub runs: Option<String>,
    /// How many requests run at once.
    #[arg(long)]
    pub concurrency: Option<NonZeroUsize>,
}

/// Grades the selected runs and prints the engine's report.
///
/// # Errors
/// Returns [`CliError::Failed`] when the plug-ins, the preflight or the engine fail,
/// [`CliError::Runtime`] when a request failed or Ctrl-C cancelled the call, and
/// [`CliError::Refused`] when no store is named.
#[cfg(unix)]
pub async fn run(cli: &Cli, args: &EvaluateArgs) -> Result<Exit, CliError> {
    use std::sync::Arc;

    use candler_core::evaluation::engine::{EvaluateRequest, evaluate};
    use candler_core::evaluation::registry::{Registry, default_env_root};
    use candler_core::supervise::Supervisor;
    use serde_json::json;
    use tokio::sync::watch;

    use super::common;

    let config = common::load_config("evaluate")?;
    let registry = Registry::load(args.plugins.as_slice()) // BORROW: explicit .as_slice()
        .map_err(|e| CliError::failed("evaluate", e))?;
    let env_root = default_env_root(&config).map_err(|e| CliError::failed("evaluate", e))?;
    // TRAIT_OBJECT: the supervisor takes any sink
    let supervisor = Arc::new(Supervisor::new(Arc::new(common::StderrSink)));
    let resolver = common::evaluation_resolver(
        "evaluate",
        &config,
        &registry,
        env_root.as_path(), // BORROW: explicit .as_path()
        &supervisor,
    )?;
    let resolution = common::resolve("evaluate", cli.store.as_deref(), &config, &resolver).await?;
    let store = common::resolved_root(&resolution)?;

    // The handler only flips the cancel watch: every stop goes out through the supervisor.
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let cancelled = cancel_rx.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            // EXPLICIT: no receiver left means the call already ended
            let _ = cancel_tx.send(true);
        }
    });

    let request = EvaluateRequest {
        store,
        plugins: args.plugins.clone(), // BORROW: the request owns the manifests
        suite: args.suite.clone(),     // BORROW: the request owns the suite path
        runs: args.runs.clone(),       // BORROW: the request owns the glob
        concurrency: args.concurrency.unwrap_or(NonZeroUsize::MIN),
        env_root,
        request_deadline: None,
    };
    let report = evaluate(request, supervisor, cancel_rx)
        .await
        .map_err(|e| CliError::failed("evaluate", e))?;

    let document = json!({
        "request_ids": report.request_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "counts": report.counts,
        "outcome_files": report
            .outcome_files
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>(),
        "failed_requests": report
            .failed_requests
            .iter()
            .map(|(id, why)| json!({"request": id.to_string(), "reason": why}))
            .collect::<Vec<_>>(),
    });
    println!("{document}");
    if *cancelled.borrow() {
        return Err(CliError::Runtime {
            detail: "`evaluate` was cancelled".to_owned(),
        });
    }
    if report.failed_requests.is_empty() {
        Ok(Exit::Success)
    } else {
        Err(CliError::Runtime {
            detail: format!(
                "`evaluate`: {} request(s) failed",
                report.failed_requests.len()
            ),
        })
    }
}

/// Grades the selected runs.
///
/// # Errors
/// Returns [`CliError::Refused`]: evaluation spawns processes through the supervisor, which this
/// platform lacks.
#[cfg(not(unix))]
#[allow(
    clippy::unused_async,
    reason = "every verb has the async entry `main` calls"
)]
pub async fn run(_cli: &Cli, _args: &EvaluateArgs) -> Result<Exit, CliError> {
    Err(CliError::refused("unsupported on this platform"))
}
