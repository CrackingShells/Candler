//! `candler show`: stored outcomes, or their rates.

use std::process::ExitCode;

use crate::Cli;

/// The arguments of `candler show`.
#[derive(Debug, clap::Args)]
pub struct ShowArgs {
    /// Print the per-cell rates instead of the outcomes.
    #[arg(long)]
    pub rates: bool,
    /// The output format.
    #[arg(long, value_enum)]
    pub format: ShowFormat,
}

/// The output formats of `candler show`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ShowFormat {
    /// JSON.
    Json,
    /// TOML.
    Toml,
}

/// Prints the outcomes or the rates.
#[allow(
    clippy::unused_async,
    reason = "a stub until the surfaces leaf fills it"
)]
pub async fn run(_cli: &Cli, _args: &ShowArgs) -> ExitCode {
    super::not_built("show")
}
