//! `candler watch`: follows the events of a job, one JSON document per line by default, or the
//! progress of the job with `--live`.

use std::collections::BTreeSet;
use std::io::{self, IsTerminal as _, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use candler_core::jobs::dir::{JobDir, JobError};
use candler_core::jobs::events::{JobEvents, TailPos};
use candler_core::jobs::lost::{self, JobLiveness};
use candler_core::jobs::runtime::session::recover_job_dir;
use candler_core::store::layout::StoreLayout;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{JobId, RunKey};

use super::term::Paint;
use super::{CliError, Exit, common};

/// How often the log is read for new events.
const POLL: Duration = Duration::from_millis(200);

/// The arguments of `candler watch`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The job to follow.
    pub id: JobId,
    /// Print each event as a JSON document on its own line; this is the default. not an
    /// experiment setting: it is how the output is read.
    #[arg(long)]
    pub json: bool,
    /// Show the progress of the job instead of its events: one line rewritten on a terminal, one
    /// line per change on a pipe. not an experiment setting: it is how the output is drawn.
    #[arg(long, conflicts_with = "json")]
    pub live: bool,
}

/// What the events seen so far say of the job's progress.
#[derive(Debug, Default)]
struct Progress {
    started: BTreeSet<RunKey>,
    ended: BTreeSet<RunKey>,
    outcomes: u32,
    planned: Option<u32>,
    shown: Option<String>,
}

impl Progress {
    /// Counts one event.
    fn see(&mut self, run: Option<&RunKey>, event: &EventPayload) {
        if let EventPayload::EvaluationPlanned { outcomes, .. } = event {
            self.planned = Some(*outcomes);
        } else if matches!(event, EventPayload::EvaluationOutcome { .. }) {
            self.outcomes = self.outcomes.saturating_add(1);
        } else if let Some(run) = run {
            if matches!(event, EventPayload::RunStarted { .. }) {
                self.started.insert(run.clone()); // BORROW: the set owns its keys
            } else if matches!(
                event,
                EventPayload::RunEnded { .. } | EventPayload::RunFailed { .. }
            ) {
                self.ended.insert(run.clone()); // BORROW: the set owns its keys
            }
        }
    }

    /// The progress line: `progress runs <ended>/<started> outcomes <n>`, with `/<planned>` after
    /// the count when the job planned its outcomes.
    fn line(&self) -> String {
        let outcomes = self.planned.map_or_else(
            || self.outcomes.to_string(),
            |planned| format!("{}/{planned}", self.outcomes),
        );
        format!(
            "progress runs {}/{} outcomes {outcomes}",
            self.ended.len(),
            self.started.len()
        )
    }
}

/// How the events are shown.
enum Mode {
    /// One JSON document per line.
    Json,
    /// The progress line, redrawn on a terminal.
    Live { progress: Progress, terminal: bool },
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
    mode: &mut Mode,
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
        if let Mode::Json = mode {
            let line = serde_json::to_string(envelope).map_err(failed)?;
            match writeln!(out, "{line}").and_then(|()| out.flush()) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
                    return Ok((ended, true));
                }
                Err(error) => return Err(failed(error)),
            }
        } else if let Mode::Live { progress, .. } = mode {
            progress.see(envelope.run(), envelope.event());
        }
        ended |= matches!(envelope.event(), EventPayload::JobEnded { .. });
    }
    *pos = tail.next;
    if let Mode::Live { progress, terminal } = mode
        && !tail.events.is_empty()
    {
        let line = progress.line();
        if progress.shown.as_deref() != Some(line.as_str()) {
            let written = if *terminal {
                // A terminal gets one line, rewritten in place and ended once the job has.
                let paint = Paint::detect();
                write!(out, "{}", paint.rewrite(&line))
                    .and_then(|()| if ended { writeln!(out) } else { Ok(()) })
            } else {
                writeln!(out, "{line}")
            };
            match written.and_then(|()| out.flush()) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
                    return Ok((ended, true));
                }
                Err(error) => return Err(failed(error)),
            }
            progress.shown = Some(line);
        }
    }
    Ok((ended, false))
}

/// Prints the events of the job as they are appended, until it ends, is found lost and is
/// recovered, the output closes, or `stop` is set.
///
/// # Errors
/// [`CliError::Failed`] when the log cannot be read or holds a line that is not an event.
// TRAIT_OBJECT: the output is stdout, or a buffer in tests
pub async fn follow(
    args: &Args,
    store: Option<&Path>,
    out: &mut dyn Write,
    stop: &AtomicBool,
    terminal: bool,
) -> Result<(), CliError> {
    let mut mode = if args.live {
        Mode::Live {
            progress: Progress::default(),
            terminal,
        }
    } else {
        Mode::Json
    };
    let root = common::store_root("watch", store).await?;
    let dir = JobDir::open(&StoreLayout::new(&root), &args.id).map_err(failed)?;
    let events = JobEvents::open(&dir).map_err(failed)?;
    let mut pos = TailPos(0);
    loop {
        let (ended, closed) = print_from(&events, &mut pos, out, &mut mode)?;
        if ended || closed || stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        match lost::classify(&dir).map_err(failed)? {
            JobLiveness::Running => {}
            JobLiveness::Lost => {
                if let Err(reason) = recover_job_dir(&dir).await {
                    return Err(CliError::Failed {
                        verb: "watch",
                        source: format!("the job is lost and recover failed: {reason}").into(),
                    });
                }
                print_from(&events, &mut pos, out, &mut mode)?;
                return Ok(());
            }
            JobLiveness::Ended(_) | _ => {
                print_from(&events, &mut pos, out, &mut mode)?;
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
pub async fn run(args: Args, store: Option<&Path>) -> Result<Exit, CliError> {
    let mut stdout = io::stdout().lock();
    let terminal = io::stdout().is_terminal();
    follow(&args, store, &mut stdout, &AtomicBool::new(false), terminal).await?;
    Ok(Exit::Success)
}
