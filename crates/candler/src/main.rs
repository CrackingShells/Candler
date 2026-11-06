//! The `candler` binary.
//!
//! One file per verb under [`commands`]; each verb is an `async fn` that takes its parsed
//! arguments and returns the process exit code.

mod commands;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::commands::{CliError, kill, ps, run, supervise, watch};

/// The command line.
#[derive(Debug, Parser)]
#[command(name = "candler", version, about = "Run and measure agent experiments")]
struct Cli {
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
}

/// Runs the verb on a multi-thread runtime.
fn dispatch(cli: Cli) -> Result<ExitCode, CliError> {
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
        }
    })
}

fn main() -> ExitCode {
    match dispatch(Cli::parse()) {
        Ok(code) => code,
        Err(error) => {
            #[allow(
                clippy::print_stderr,
                reason = "the binary reports its failure on stderr"
            )]
            {
                eprintln!("candler: {error}");
            }
            ExitCode::FAILURE
        }
    }
}
