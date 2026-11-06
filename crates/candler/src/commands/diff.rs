//! `candler diff`: the outcomes of two evaluator versions, compared.

use std::path::PathBuf;
use std::process::ExitCode;

use crate::Cli;

/// The arguments of `candler diff`.
#[derive(Debug, clap::Args)]
pub struct DiffArgs {
    /// The base evaluator, as `id@version`.
    #[arg(long)]
    pub from: String,
    /// The compared evaluator, as `id@version`.
    #[arg(long)]
    pub to: String,
    /// An equivalence file naming the rows excused from the comparison.
    #[arg(long)]
    pub equivalence: Option<PathBuf>,
    /// The output format.
    #[arg(long, value_enum)]
    pub format: DiffFormat,
}

/// The output formats of `candler diff`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum DiffFormat {
    /// JSON.
    Json,
    /// Text.
    Text,
}

/// Compares the two versions; exits 1 when it finds a change.
#[allow(
    clippy::unused_async,
    reason = "a stub until the surfaces leaf fills it"
)]
pub async fn run(_cli: &Cli, _args: &DiffArgs) -> ExitCode {
    super::not_built("diff")
}
