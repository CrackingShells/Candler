//! The `candler` binary: `run`, `ps`, `kill` and `watch` for jobs; `import`, `evaluate`, `show`
//! and `diff` over a store.
//!
//! One file per verb under [`commands`]; each verb is an `async fn` that takes its parsed
//! arguments and returns an [`Exit`] or a [`CliError`]. `main` maps `Exit::Success` to 0,
//! `Exit::Changed` to 1 and any [`CliError`] to 2.

mod commands;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::commands::{CliError, Exit, kill, ps, run, supervise, watch};

/// The command line.
#[derive(Debug, Parser)]
#[command(name = "candler", version, about = "Run and measure agent experiments")]
struct Cli {
    /// The store root.
    #[arg(long, env = "CANDLER_STORE", global = true)]
    store: Option<PathBuf>,
    /// The verb to run.
    #[command(subcommand)]
    command: Command,
}

/// The verbs.
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

/// Runs the verb on a multi-thread runtime.
fn dispatch(cli: Cli) -> Result<Exit, CliError> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| CliError::Runtime {
            detail: error.to_string(),
        })?;
    runtime.block_on(async move {
        match cli.command {
            Command::Run(args) => run::run(args).await,
            Command::Ps(args) => ps::run(args).await,
            Command::Kill(args) => kill::run(args).await,
            Command::Watch(args) => watch::run(args).await,
            Command::Supervise(args) => supervise::run(args).await,
            Command::Import(ref args) => {
                let _stub = commands::import::run(&cli, args).await;
                Err(CliError::Unavailable { verb: "import" })
            }
            Command::Evaluate(ref args) => {
                let _stub = commands::evaluate::run(&cli, args).await;
                Err(CliError::Unavailable { verb: "evaluate" })
            }
            Command::Show(ref args) => {
                let _stub = commands::show::run(&cli, args).await;
                Err(CliError::Unavailable { verb: "show" })
            }
            Command::Diff(ref args) => {
                let _stub = commands::diff::run(&cli, args).await;
                Err(CliError::Unavailable { verb: "diff" })
            }
        }
    })
}

fn main() -> ExitCode {
    match dispatch(Cli::parse()) {
        Ok(Exit::Success) => ExitCode::SUCCESS,
        Ok(Exit::Changed { .. }) => ExitCode::from(1),
        Err(error) => {
            #[allow(
                clippy::print_stderr,
                reason = "the binary reports its failure on stderr"
            )]
            {
                eprintln!("candler: {error}");
            }
            ExitCode::from(2)
        }
    }
}
