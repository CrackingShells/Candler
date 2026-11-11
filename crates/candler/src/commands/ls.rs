//! `candler ls`: lists what the store holds.

use super::{CliError, Exit};

/// The arguments of `candler ls`.
#[derive(Debug, clap::Args)]
pub struct Args {}

/// Lists what the store holds.
///
/// # Errors
/// [`CliError::Refused`] always, until the verb is written.
pub fn run(_args: &Args) -> Result<Exit, CliError> {
    Err(CliError::refused("m3 scaffold"))
}
