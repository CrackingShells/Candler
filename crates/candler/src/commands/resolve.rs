//! `candler resolve`: resolves an experiment and prints what its settings come to.

use super::{CliError, Exit};

/// The arguments of `candler resolve`.
#[derive(Debug, clap::Args)]
pub struct Args {}

/// Resolves an experiment and prints what its settings come to.
///
/// # Errors
/// [`CliError::Refused`] always, until the verb is written.
pub fn run(_args: &Args) -> Result<Exit, CliError> {
    Err(CliError::refused("m3 scaffold"))
}
