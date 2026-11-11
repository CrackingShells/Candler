//! `candler analyze`: reports the statistics of stored outcomes.

use super::{CliError, Exit};

/// The arguments of `candler analyze`.
#[derive(Debug, clap::Args)]
pub struct Args {}

/// Reports the statistics of stored outcomes.
///
/// # Errors
/// [`CliError::Refused`] always, until the verb is written.
pub fn run(_args: &Args) -> Result<Exit, CliError> {
    Err(CliError::refused("m3 scaffold"))
}
