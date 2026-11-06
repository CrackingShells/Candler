//! `candler watch`: follows the events of a job.

use super::CliError;

/// The arguments of `candler watch`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The job to follow.
    pub job: String,
}

/// Runs the verb.
///
/// # Errors
/// [`CliError`] when the events cannot be read.
#[allow(
    clippy::unused_async,
    reason = "the verb awaits once its leaf fills it in"
)]
pub async fn run(args: Args) -> Result<(), CliError> {
    let _ = args;
    Err(CliError::Unavailable { verb: "watch" })
}
