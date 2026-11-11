//! `candler inspect`: prints what the store holds of one run.

use super::{CliError, Exit};

/// The arguments of `candler inspect`.
#[derive(Debug, clap::Args)]
pub struct Args {}

/// Prints what the store holds of one run.
///
/// # Errors
/// [`CliError::Refused`] always, until the verb is written.
pub fn run(_args: &Args) -> Result<Exit, CliError> {
    Err(CliError::refused("m3 scaffold"))
}
