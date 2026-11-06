//! `candler supervise`: the detached supervisor of a job. `candler run` starts it by re-executing
//! the binary in a new process group; the supervisor calls `setsid` on itself.

use super::{CliError, Exit};

/// The arguments of `candler supervise`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The job to supervise.
    #[arg(long)]
    pub job: String,
}

/// Runs the verb.
///
/// # Errors
/// [`CliError`] when the job cannot be supervised.
#[allow(
    clippy::unused_async,
    reason = "the verb awaits once its leaf fills it in"
)]
pub async fn run(args: Args) -> Result<Exit, CliError> {
    let _ = args;
    Err(CliError::Unavailable { verb: "supervise" })
}
