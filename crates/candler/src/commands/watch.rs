//! `candler watch`: follows the events of a job, one JSON document per line.

use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use candler_core::jobs::dir::{JobDir, JobError};
use candler_core::jobs::events::{JobEvents, TailPos};
use candler_core::jobs::lost::{self, JobLiveness};
use candler_core::store::layout::StoreLayout;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::JobId;

use super::ps::{recover_job, store_root};
use super::{CliError, Exit};

/// How often the log is read for new events.
const POLL: Duration = Duration::from_millis(200);

/// The arguments of `candler watch`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The job to follow.
    pub id: JobId,
    /// Print each event as a JSON document on its own line; the only form there is.
    #[arg(long)]
    pub json: bool,
    /// The store to read; by default the store the configuration names.
    #[arg(long)]
    pub store: Option<PathBuf>,
}

fn failed(error: impl std::error::Error + Send + Sync + 'static) -> CliError {
    CliError::Failed {
        verb: "watch",
        source: Box::new(error),
    }
}

/// Prints the events after `pos`, moves `pos` past them and says whether one of them ended the
/// job. A closed output ends the printing without an error.
fn print_from(
    events: &JobEvents,
    pos: &mut TailPos,
    out: &mut dyn Write, // TRAIT_OBJECT: the output is stdout, or a buffer in tests
) -> Result<(bool, bool), CliError> {
    let tail = events.tail(*pos).map_err(|error| {
        if let JobError::Corrupt { offset, .. } = &error {
            CliError::Failed {
                verb: "watch",
                source: format!("the event log has a line that is not an event at byte {offset}")
                    .into(),
            }
        } else {
            failed(error)
        }
    })?;
    let mut ended = false;
    for envelope in &tail.events {
        let line = serde_json::to_string(envelope).map_err(failed)?;
        match writeln!(out, "{line}").and_then(|()| out.flush()) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => return Ok((ended, true)),
            Err(error) => return Err(failed(error)),
        }
        ended |= matches!(envelope.event(), EventPayload::JobEnded { .. });
    }
    *pos = tail.next;
    Ok((ended, false))
}

/// Prints the events of the job as they are appended, until it ends, is found lost and is
/// recovered, the output closes, or `stop` is set.
///
/// # Errors
/// [`CliError::Refused`] without `--json`, [`CliError::Failed`] when the log cannot be read or
/// holds a line that is not an event.
// TRAIT_OBJECT: the output is stdout, or a buffer in tests
pub async fn follow(args: &Args, out: &mut dyn Write, stop: &AtomicBool) -> Result<(), CliError> {
    if !args.json {
        return Err(CliError::Refused {
            reason: "watch needs --json".to_owned(), // BORROW: the error owns its reason
        });
    }
    let root = store_root(args.store.as_deref())?;
    let dir = JobDir::open(&StoreLayout::new(&root), &args.id).map_err(failed)?;
    let events = JobEvents::open(&dir).map_err(failed)?;
    let mut pos = TailPos(0);
    loop {
        let (ended, closed) = print_from(&events, &mut pos, out)?;
        if ended || closed || stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        match lost::classify(&dir).map_err(failed)? {
            JobLiveness::Running => {}
            JobLiveness::Lost => {
                if let Err(reason) = recover_job(&dir).await {
                    return Err(CliError::Failed {
                        verb: "watch",
                        source: format!("the job is lost and recover failed: {reason}").into(),
                    });
                }
                print_from(&events, &mut pos, out)?;
                return Ok(());
            }
            JobLiveness::Ended(_) | _ => {
                print_from(&events, &mut pos, out)?;
                return Ok(());
            }
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Runs the verb.
///
/// # Errors
/// [`CliError`] when the events cannot be followed.
pub async fn run(args: Args) -> Result<Exit, CliError> {
    let mut stdout = io::stdout().lock();
    follow(&args, &mut stdout, &AtomicBool::new(false)).await?;
    Ok(Exit::Success)
}
