//! `candler kill`: stops a job, or every job, after a confirmation.
//!
//! A running job is stopped by reaping its supervisor from the identity `job.json` records: the
//! record is read against the operating system again and every signal passes the process guard.
//! A job whose supervisor is gone, or still has not ended the job after the grace, is recovered.

use std::io::{self, BufRead, Write};
use std::path::Path;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::JobEvents;
use candler_core::jobs::lost::{self, JobLiveness, RecoverOutcome};
use candler_core::jobs::runtime::TEARDOWN_BOUND;
use candler_core::jobs::runtime::session::recover_job_dir;
use candler_core::store::layout::StoreLayout;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::spec::{StopSignal, StopSpec};
use candler_schemas::primitives::{JobId, Marker, Timestamp};
use candler_schemas::process::ChildRecord;
use std::sync::Arc;

use super::ps::{describe_recovery, end_text};
use super::{CliError, Exit, common};

/// The arguments of `candler kill`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The job to stop.
    #[arg(required_unless_present = "all", conflicts_with = "all")]
    pub id: Option<JobId>,
    /// Stop every running or lost job.
    #[arg(long)]
    pub all: bool,
    /// Do not ask for confirmation.
    #[arg(short = 'y', long = "yes", default_value_t = false)]
    pub yes: bool,
}

/// Whether the answer on `input` is `y` or `yes`; an empty answer, any other word and a closed
/// input are no.
#[must_use]
// TRAIT_OBJECT: the input is stdin, or a buffer in tests
pub fn confirm(input: &mut dyn BufRead) -> bool {
    let mut line = String::new();
    match input.read_line(&mut line) {
        Ok(0) | Err(_) => false,
        // BORROW: explicit .as_str() instead of Deref coercion
        Ok(_) => matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes"),
    }
}

/// The record of a supervisor that leads its own group, from the identity in `job.json`.
fn supervisor_record(dir: &JobDir, id: &JobId) -> Result<ChildRecord, String> {
    let record = dir.record().map_err(|e| e.to_string())?;
    let identity = record.supervisor();
    let marker = id
        .as_str() // BORROW: explicit .as_str() instead of Deref coercion
        .parse::<Marker>()
        .map_err(|e| e.to_string())?;
    // The record does not say when the supervisor was spawned; the reap never reads it.
    let spawned = "1970-01-01T00:00:00Z"
        .parse::<Timestamp>()
        .map_err(|e| e.to_string())?;
    ChildRecord::builder()
        .pid(identity.pid())
        .pgid(identity.pid())
        .start(identity.start().clone()) // BORROW: the record owns its start
        .marker(marker)
        .spawned_at(spawned)
        .build()
        .map_err(|e| e.to_string())
}

/// Stops the job `id` and says what became of it; the error text is the reason it was not
/// stopped.
///
/// # Errors
/// The reason, as text, when the job could not be stopped or recovered.
pub async fn stop_job(store: &StoreLayout, id: &JobId) -> Result<String, String> {
    let dir = JobDir::open(store, id).map_err(|e| e.to_string())?;
    let first = lost::classify(&dir).map_err(|e| e.to_string())?;
    if let JobLiveness::Ended(end) = first {
        return Ok(format!("{id}: {} (left alone)", end_text(end)));
    }
    if first == JobLiveness::Running {
        let events = Arc::new(JobEvents::open(&dir).map_err(|e| e.to_string())?);
        // TRAIT_OBJECT: the supervisor takes its sink as a trait object
        let sink: Arc<dyn EventSink> = events;
        let sup = Supervisor::new(sink);
        let record = supervisor_record(&dir, id)?;
        let stop = StopSpec::new(None, StopSignal::Term, TEARDOWN_BOUND);
        sup.reap(None, &record, &stop)
            .await
            .map_err(|e| e.to_string())?;
        if let JobLiveness::Ended(end) = lost::classify(&dir).map_err(|e| e.to_string())? {
            return Ok(format!("{id}: {}", end_text(end)));
        }
    }
    let report = recover_job_dir(&dir)
        .await
        .map_err(|e| format!("recover failed: {e}"))?;
    match report.outcome {
        RecoverOutcome::NotLost => Err("its supervisor still holds the job".to_owned()), // BORROW: the error owns its text
        RecoverOutcome::Recovered | RecoverOutcome::AlreadyEnded => Ok(format!(
            "{id}: recovered ({} containers destroyed)",
            report.containers
        )),
        RecoverOutcome::Incomplete | _ => {
            let (text, _) = describe_recovery(&report);
            Err(text)
        }
    }
}

/// The ids to stop: the named job, or the name of every directory under `jobs/`.
fn targets(args: &Args, jobs: &std::path::Path) -> Result<Vec<String>, CliError> {
    if let Some(id) = &args.id {
        return Ok(vec![id.to_string()]);
    }
    let mut names = match std::fs::read_dir(jobs) {
        Ok(read) => read
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| !name.starts_with('.'))
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return Err(CliError::Failed {
                verb: "kill",
                source: Box::new(error),
            });
        }
    };
    names.sort();
    Ok(names)
}

/// Stops the jobs `args` names after a confirmation, writing what it did to `out`.
///
/// # Errors
/// [`CliError::Refused`] when the confirmation is not given, [`CliError::Failed`] when a job
/// could not be stopped.
pub async fn kill(
    args: &Args,
    store: Option<&Path>,
    // TRAIT_OBJECT: the input is stdin, or a buffer in tests
    input: &mut dyn BufRead,
    // TRAIT_OBJECT: the output is stdout, or a buffer in tests
    out: &mut dyn Write,
) -> Result<(), CliError> {
    let fail = |error: io::Error| CliError::Failed {
        verb: "kill",
        source: Box::new(error),
    };
    let root = common::store_root("kill", store).await?;
    let names = targets(args, &root.join("jobs"))?;
    if !args.yes {
        writeln!(out, "would stop: {}", names.join(" ")).map_err(fail)?;
        write!(out, "stop them? [y/N] ").map_err(fail)?;
        out.flush().map_err(fail)?;
        if !confirm(input) {
            return Err(CliError::Refused {
                reason: "pass -y to confirm".to_owned(), // BORROW: the error owns its text
            });
        }
    }
    let store = StoreLayout::new(&root);
    let mut failures = Vec::new();
    for name in names {
        let stopped = match name.parse::<JobId>() {
            Ok(id) => stop_job(&store, &id).await,
            Err(error) => Err(error.to_string()),
        };
        match stopped {
            Ok(text) => writeln!(out, "{text}").map_err(fail)?,
            Err(reason) => {
                writeln!(out, "{name}: not stopped: {reason}").map_err(fail)?;
                failures.push(name);
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(CliError::Failed {
            verb: "kill",
            source: format!(
                "{} job(s) not stopped: {}",
                failures.len(),
                failures.join(" ")
            )
            .into(),
        })
    }
}

/// Runs the verb.
///
/// # Errors
/// [`CliError`] when the confirmation is refused or a job could not be stopped.
pub async fn run(args: Args, store: Option<&Path>) -> Result<Exit, CliError> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut stdout = io::stdout().lock();
    kill(&args, store, &mut input, &mut stdout).await?;
    Ok(Exit::Success)
}
