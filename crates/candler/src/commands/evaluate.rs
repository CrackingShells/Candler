//! `candler evaluate`: grade stored runs with a plug-in's suite.

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::Cli;

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

/// Grades the selected runs.
#[allow(
    clippy::unused_async,
    reason = "a stub until the surfaces leaf fills it"
)]
pub async fn run(_cli: &Cli, _args: &EvaluateArgs) -> ExitCode {
    super::not_built("evaluate")
}
