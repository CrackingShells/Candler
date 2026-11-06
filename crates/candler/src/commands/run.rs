//! `candler run`: resolves an experiment, plans its runs and starts the job's supervisor.

use std::path::PathBuf;
use std::process::ExitCode;

use super::CliError;

/// The arguments of `candler run`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The experiment file.
    pub experiment: PathBuf,
}

/// Runs the verb.
///
/// # Errors
/// [`CliError`] when the experiment cannot be started.
#[allow(
    clippy::unused_async,
    reason = "the verb awaits once its leaf fills it in"
)]
pub async fn run(args: Args) -> Result<ExitCode, CliError> {
    let _ = args;
    // composition: register_m2b_steps
    Err(CliError::Unavailable { verb: "run" })
}
