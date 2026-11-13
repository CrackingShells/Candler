//! The `candler` binary: `run`, `ps`, `kill` and `watch` for jobs; `import`, `evaluate`, `show`
//! and `diff` over a store.
//!
//! One file per verb under [`commands`]; each verb is an `async fn` that takes its parsed
//! arguments and returns an [`Exit`] or a [`CliError`]. Exit codes, as `diff(1)` has them:
//! `Exit::Success` is 0, `Exit::Changed` is 1 and any [`CliError`] is 2.

#![deny(clippy::allow_attributes)]

#[cfg(not(unix))]
compile_error!(
    "candler builds for Linux and macOS only; on Windows run it inside WSL2, where it is Linux"
);

mod commands;
mod drivers;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

use crate::commands::{
    CliError, Exit, analyze, inspect, kill, ls, ps, resolve, run, supervise, watch,
};

/// The command line.
#[derive(Debug, Parser)]
#[command(
    name = "candler",
    version,
    about = "Run, evaluate and compare agent experiments"
)]
struct Cli {
    /// The store root, for every verb; by default the store the configuration names. not an
    /// experiment setting: it chooses where the results of every experiment are kept.
    #[arg(long, env = "CANDLER_STORE", global = true)]
    store: Option<PathBuf>,
    /// How refusals are written on stderr. cannot be derived: what parses the caller's stderr is not
    /// visible from the terminal; cannot live in the experiment file: a refusal can come before any
    /// file is read.
    #[arg(long, value_enum, default_value_t, global = true)]
    error_format: ErrorFormat,
    /// The verb to run.
    #[command(subcommand)]
    command: Command,
}

/// How a refusal is written on stderr: `candler: <message>` on one line, or one JSON object.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
enum ErrorFormat {
    #[default]
    Text,
    Json,
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
    /// Ingest raw trace directories, or legacy results folders, into the store.
    #[command(alias = "import")]
    Ingest(commands::ingest::IngestArgs),
    /// Grade stored runs with a plug-in's suite.
    Evaluate(commands::evaluate::EvaluateArgs),
    /// Print stored outcomes, or their rates.
    Show(commands::show::ShowArgs),
    /// Compare the outcomes of two evaluator versions.
    Diff(commands::diff::DiffArgs),
    /// Resolves an experiment and prints what its settings come to.
    Resolve(resolve::Args),
    /// Lists what the store holds.
    Ls(ls::Args),
    /// Reports the statistics of stored outcomes.
    Analyze(analyze::Args),
    /// Shows one run, or one job, with its outcomes, evidence, provenance and integrity.
    Inspect(inspect::InspectArgs),
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
            Command::Ingest(ref args) => commands::ingest::run(&cli, args).await,
            Command::Evaluate(ref args) => commands::evaluate::run(&cli, args).await,
            Command::Show(ref args) => commands::show::run(&cli, args).await,
            Command::Diff(ref args) => commands::diff::run(&cli, args).await,
            Command::Resolve(args) => resolve::run(args, cli.store.as_deref()).await,
            Command::Ls(args) => ls::run(args, cli.store.as_deref()).await,
            Command::Analyze(ref args) => analyze::run(&cli, args).await,
            Command::Inspect(ref args) => inspect::run(&cli, args).await,
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

/// Whether the arguments ask for the JSON form of refusals: `--error-format json`, or
/// `--error-format=json`. The parser has not accepted them when this is asked, so the value is read
/// from the words themselves.
fn json_requested(args: &[std::ffi::OsString]) -> bool {
    let words: Vec<&str> = args.iter().filter_map(|arg| arg.to_str()).collect();
    words
        .windows(2)
        .any(|pair| pair == ["--error-format", "json"])
        || words.contains(&"--error-format=json")
}

/// Ends the process for a command line the parser refused: help and version are printed as they
/// were, and a usage error is one `refusal/1` record when the arguments ask for JSON, or the
/// parser's own text otherwise. The status is the parser's.
fn usage_failure(error: &clap::Error) -> ExitCode {
    let asked = json_requested(&std::env::args_os().collect::<Vec<_>>());
    if error.use_stderr() && asked {
        eprintln!(
            "{}",
            commands::refusal::usage_record(error.render().to_string().trim_end())
        );
        return ExitCode::from(2);
    }
    error.exit()
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return usage_failure(&error),
    };
    let format = cli.error_format;
    let result = dispatch(cli);
    if let Err(error) = &result {
        eprintln!("{}", commands::refusal::render(error, format));
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
