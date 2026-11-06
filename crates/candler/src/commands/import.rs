//! `candler import`: a legacy results folder into the store.

use std::path::PathBuf;
use std::process::ExitCode;

use crate::Cli;

/// The arguments of `candler import`.
#[derive(Debug, clap::Args)]
pub struct ImportArgs {
    /// The legacy results folder.
    pub corpus: PathBuf,
    /// The evaluator that graded it, as `id@version`.
    #[arg(long)]
    pub instrument: String,
}

/// Imports `args.corpus` into the store.
#[allow(
    clippy::unused_async,
    reason = "a stub until the surfaces leaf fills it"
)]
pub async fn run(_cli: &Cli, _args: &ImportArgs) -> ExitCode {
    super::not_built("import")
}
