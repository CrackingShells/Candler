//! `candler kill`: stops a job.

use std::process::ExitCode;

use super::CliError;

/// The arguments of `candler kill`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The job to stop.
    pub job: String,
}

/// Runs the verb.
///
/// # Errors
/// [`CliError`] when the job cannot be stopped.
#[allow(
    clippy::unused_async,
    reason = "the verb awaits once its leaf fills it in"
)]
pub async fn run(args: Args) -> Result<ExitCode, CliError> {
    let _ = args;
    Err(CliError::Unavailable { verb: "kill" })
}
