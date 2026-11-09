//! `candler diff`: the outcomes of two evaluator versions, compared.

use std::path::PathBuf;

use super::{CliError, Exit};
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

/// Not wired in this commit.
///
/// # Errors
/// Returns [`CliError::Unavailable`].
#[allow(
    clippy::unused_async,
    reason = "every verb has the async entry `main` calls"
)]
pub async fn run(_cli: &Cli, _args: &DiffArgs) -> Result<Exit, CliError> {
    Err(CliError::Unavailable { verb: "diff" })
}
