//! The `candler` binary: `run`, `ps`, `kill` and `watch` for jobs; `import`, `evaluate`, `show`
//! and `diff` over a store.
//!
//! One file per verb under [`commands`]; each verb is an `async fn` that takes its parsed
//! arguments and returns an [`Exit`] or a [`CliError`]. Exit codes, as `diff(1)` has them:
//! `Exit::Success` is 0, `Exit::Changed` is 1 and any [`CliError`] is 2.

mod commands;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::commands::{CliError, Exit, kill, ps, run, supervise, watch};

/// The command line.
#[derive(Debug, Parser)]
#[command(
    name = "candler",
    version,
    about = "Run, evaluate and compare agent experiments"
)]
struct Cli {
    /// The store root, for every verb; by default the store the configuration names.
    #[arg(long, env = "CANDLER_STORE", global = true)]
    store: Option<PathBuf>,
    /// The verb to run.
    #[command(subcommand)]
    command: Command,
}

/// The verbs, one variant per line so that later verbs append.
#[derive(Debug, Subcommand)]
enum Command {
    /// Starts a job for an experiment.
    Run(run::Args),
    /// Lists the jobs and their state.
    Ps(ps::Args),
    /// Stops a job.
    Kill(kill::Args),
    /// Follows the events of a job.
    Watch(watch::Args),
    /// Runs a job's supervisor; started by `run`, never by a person.
    #[command(hide = true)]
    Supervise(supervise::Args),
    /// Import a legacy results folder into the store.
    Import(commands::import::ImportArgs),
    /// Grade stored runs with a plug-in's suite.
    Evaluate(commands::evaluate::EvaluateArgs),
    /// Print stored outcomes, or their rates.
    Show(commands::show::ShowArgs),
    /// Compare the outcomes of two evaluator versions.
    Diff(commands::diff::DiffArgs),
}

/// Runs the verb on a multi-thread runtime, one arm per line.
fn dispatch(cli: Cli) -> Result<Exit, CliError> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| CliError::Runtime {
            detail: format!("the runtime could not start: {error}"),
        })?;
    runtime.block_on(async move {
        match cli.command {
            Command::Run(args) => run::run(args, cli.store.as_deref()).await,
            Command::Ps(args) => ps::run(args, cli.store.as_deref()).await,
            Command::Kill(args) => kill::run(args, cli.store.as_deref()).await,
            Command::Watch(args) => watch::run(args, cli.store.as_deref()).await,
            Command::Supervise(args) => supervise::run(args, cli.store.as_deref()).await,
            Command::Import(ref args) => commands::import::run(&cli, args).await,
            Command::Evaluate(ref args) => commands::evaluate::run(&cli, args).await,
            Command::Show(ref args) => commands::show::run(&cli, args).await,
            Command::Diff(ref args) => commands::diff::run(&cli, args).await,
        }
    })
}

/// The process status of a verb's result: the one mapping every verb shares.
fn status(result: &Result<Exit, CliError>) -> u8 {
    match result {
        Ok(Exit::Success) => 0,
        Ok(Exit::Changed { .. }) => 1,
        Err(_) => 2,
    }
}

fn main() -> ExitCode {
    let result = dispatch(Cli::parse());
    if let Err(error) = &result {
        #[allow(
            clippy::print_stderr,
            reason = "the binary reports its failure on stderr"
        )]
        {
            eprintln!("candler: {error}");
        }
    }
    ExitCode::from(status(&result))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards: R2 — the binary has one exit mapping: no change is 0, a change found by `diff` is
    /// 1, and any error is 2. Oracle: the `diff(1)` convention (0 same, 1 different, 2 trouble).
    #[test]
    fn the_exit_mapping_follows_the_diff_convention() {
        let table: [(Result<Exit, CliError>, u8); 3] = [
            (Ok(Exit::Success), 0),
            (Ok(Exit::Changed { count: 3 }), 1),
            (
                Err(CliError::Runtime {
                    detail: "trouble".to_owned(),
                }),
                2,
            ),
        ];
        for (result, expected) in &table {
            assert_eq!(status(result), *expected, "{result:?}");
        }
    }
}
