//! `candler ps`: lists the jobs and their state.

use super::CliError;

/// The arguments of `candler ps`.
#[derive(Debug, clap::Args)]
pub struct Args {}

/// Runs the verb.
///
/// # Errors
/// [`CliError`] when the jobs cannot be listed.
#[allow(
    clippy::unused_async,
    reason = "the verb awaits once its leaf fills it in"
)]
pub async fn run(args: Args) -> Result<(), CliError> {
    let _ = args;
    Err(CliError::Unavailable { verb: "ps" })
}
