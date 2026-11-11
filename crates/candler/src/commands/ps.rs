//! `candler ps`: lists the jobs and their state.
//!
//! A job is `running`, `ended` or `lost` from its lock and its event log alone; the `state`
//! field of `job.json` is a hint nothing reads. A lost job is recovered once, then shown as
//! `ended`. A job whose supervisor has not logged `job.started` is `starting`, never `lost`:
//! the launcher is still waiting for it, or it never began and nothing of it needs recovering.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::Path;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::{JobEvents, TailPos};
use candler_core::jobs::lost::{self, JobLiveness, RecoverOutcome, RecoverReport};
use candler_core::jobs::model::JobSpecFile;
use candler_core::jobs::runtime::detach::START_DEADLINE;
use candler_core::jobs::runtime::session::{SPEC_FILE, recover_job_dir};
use candler_core::runner::plan::experiment_of;
use candler_core::store::layout::StoreLayout;
use candler_schemas::execution::{EventPayload, JobEnd};
use candler_schemas::primitives::{JobId, RunKey};

use super::{CliError, Exit, common};

/// The arguments of `candler ps`.
#[derive(Debug, clap::Args)]
pub struct Args {}

/// One line of the listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The job id, or the directory name when the directory cannot be read.
    pub id: String,
    /// The digest of the job's spec, which is what the job record holds of the experiment.
    pub spec: String,
    /// `running`, `ended (<reason>)`, `unreadable <path>` or a `lost: ...` line.
    pub state: String,
    /// `<runs ended>/<runs started>`.
    pub progress: String,
    /// What the job runs, the last field with no space in it: `experiment:<12 hex of the
    /// experiment digest>`, `suite:<file name>`, or `-` when the job has no readable spec file.
    pub what: String,
    /// Whether the row reports a failure, which makes the command exit non-zero.
    pub failed: bool,
}

/// How many hex digits of the experiment digest a row shows.
const SHORT: usize = 12;

/// What the job of `dir` runs, read from the spec file the supervisor keeps in the job directory.
/// A job with no file, or one that does not read as a spec, shows `-`.
fn what_of(dir: &JobDir) -> String {
    let none = || "-".to_owned(); // BORROW: the row owns its text
    let Ok(bytes) = std::fs::read(dir.path().join(SPEC_FILE)) else {
        return none();
    };
    let Ok(file) = serde_json::from_slice::<JobSpecFile>(&bytes) else {
        return none();
    };
    match file {
        JobSpecFile::Evaluate(spec) => spec
            .suite
            .file_name()
            .map_or_else(none, |name| format!("suite:{}", name.to_string_lossy())),
        JobSpecFile::Run(spec) => experiment_of(&spec.experiment, &spec.resolution).map_or_else(
            |_| none(),
            |digest| {
                format!(
                    "experiment:{}",
                    digest.hex().chars().take(SHORT).collect::<String>()
                )
            },
        ),
    }
}

/// What a recovery left, as the text a row or a message shows, and whether it is a failure.
#[must_use]
pub fn describe_recovery(report: &RecoverReport) -> (String, bool) {
    match report.outcome {
        RecoverOutcome::Incomplete => (
            format!(
                "lost: recovery incomplete: {} refused, sweep complete: {}",
                report.refused.len(),
                !report.unknown
            ),
            true,
        ),
        RecoverOutcome::Starting => ("starting".to_owned(), false), // BORROW: the row owns its text
        RecoverOutcome::Recovered | RecoverOutcome::AlreadyEnded | RecoverOutcome::NotLost => {
            ("ended (failed)".to_owned(), false) // BORROW: the row owns its text
        }
        _ => ("lost: recovery outcome unknown".to_owned(), true), // BORROW: the row owns its text
    }
}

/// How many runs the log shows ended and started.
fn progress_of(dir: &JobDir) -> String {
    let Ok(events) = JobEvents::open(dir) else {
        return "?".to_owned(); // BORROW: the row owns its text
    };
    let Ok(tail) = events.tail(TailPos(0)) else {
        return "?".to_owned(); // BORROW: the row owns its text
    };
    let mut started: BTreeSet<RunKey> = BTreeSet::new();
    let mut ended: BTreeSet<RunKey> = BTreeSet::new();
    for envelope in &tail.events {
        let Some(run) = envelope.run() else {
            continue;
        };
        if matches!(envelope.event(), EventPayload::RunStarted { .. }) {
            started.insert(run.clone()); // BORROW: the set owns its keys
        } else if matches!(
            envelope.event(),
            EventPayload::RunEnded { .. } | EventPayload::RunFailed { .. }
        ) {
            ended.insert(run.clone()); // BORROW: the set owns its keys
        }
    }
    format!("{}/{}", ended.len(), started.len())
}

/// The state column of a job that ended with `end`.
#[must_use]
pub fn end_text(end: JobEnd) -> String {
    let word = match end {
        JobEnd::Completed => "completed",
        JobEnd::BudgetStop => "budget_stop",
        JobEnd::Killed => "killed",
        JobEnd::Failed => "failed",
        _ => "unknown",
    };
    format!("ended ({word})")
}

/// Whether the job of `dir` is younger than the launcher's wait for its supervisor and has not
/// logged `job.started`: the launcher may still be waiting, so nothing touches the job.
fn is_starting(dir: &JobDir) -> bool {
    let young = std::fs::metadata(dir.path().join("job.json"))
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|made| made.elapsed().ok())
        .is_some_and(|age| age < START_DEADLINE);
    young && matches!(lost::has_started(dir), Ok(false))
}

/// The row of the job in `path`, recovering it once when it is lost.
async fn row_of(store: &StoreLayout, name: &str, path: &Path) -> Row {
    let unreadable = |detail: String| Row {
        id: name.to_owned(),  // BORROW: the row owns its id
        spec: "-".to_owned(), // BORROW: the row owns its text
        state: format!("unreadable {}: {detail}", path.display()),
        progress: "-".to_owned(), // BORROW: the row owns its text
        what: "-".to_owned(),     // BORROW: the row owns its text
        failed: true,
    };
    let Ok(id) = name.parse::<JobId>() else {
        return unreadable("not a job id".to_owned()); // BORROW: the row owns its text
    };
    let dir = match JobDir::open(store, &id) {
        Ok(dir) => dir,
        Err(error) => return unreadable(error.to_string()),
    };
    let record = match dir.record() {
        Ok(record) => record,
        Err(error) => return unreadable(error.to_string()),
    };
    let mut failed = false;
    let liveness = match lost::classify(&dir) {
        Ok(liveness) => liveness,
        Err(error) => return unreadable(error.to_string()),
    };
    let state = match liveness {
        JobLiveness::Running => "running".to_owned(), // BORROW: the row owns its text
        JobLiveness::Ended(end) => end_text(end),
        JobLiveness::Lost if is_starting(&dir) => "starting".to_owned(), // BORROW: the row owns its text
        JobLiveness::Lost => match recover_job_dir(&dir).await {
            Ok(report) => {
                let (text, bad) = describe_recovery(&report);
                failed = bad;
                text
            }
            Err(reason) => {
                failed = true;
                format!("lost: recover failed: {reason}")
            }
        },
        _ => "unknown".to_owned(), // BORROW: the row owns its text
    };
    Row {
        id: id.to_string(),
        spec: record.spec_digest().to_string(),
        state,
        progress: progress_of(&dir),
        what: what_of(&dir),
        failed,
    }
}

/// Lists the jobs of the store, recovering each lost one, and says whether any row failed.
///
/// # Errors
/// [`CliError::Failed`] when the jobs directory cannot be read or `out` cannot be written.
// TRAIT_OBJECT: the output is stdout, or a buffer in tests
pub async fn render(store: Option<&Path>, out: &mut dyn Write) -> Result<bool, CliError> {
    let root = common::store_root("ps", store).await?;
    let store = StoreLayout::new(&root);
    let jobs = root.join("jobs");
    let fail = |error: io::Error| CliError::Failed {
        verb: "ps",
        source: Box::new(error),
    };
    let mut entries = match std::fs::read_dir(&jobs) {
        Ok(read) => read
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(fail(error)),
    };
    entries.sort();
    let mut rows = Vec::new();
    for path in entries {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        rows.push(row_of(&store, &name, &path).await);
    }
    render_rows(&rows, out).map_err(fail)?;
    Ok(rows.iter().any(|row| row.failed))
}

/// Writes one line per row: id, spec digest, state, progress and what the job runs.
///
/// # Errors
/// The write error of `out`.
// TRAIT_OBJECT: the output is stdout, or a buffer in tests
pub fn render_rows(rows: &[Row], out: &mut dyn Write) -> io::Result<()> {
    for row in rows {
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}",
            row.id,
            row.spec,
            row.state,
            row.progress,
            row.what.split_whitespace().collect::<Vec<_>>().join("_")
        )?;
    }
    Ok(())
}

/// Runs the verb.
///
/// # Errors
/// [`CliError`] when the jobs cannot be listed or a job's recovery failed.
pub async fn run(_args: Args, store: Option<&Path>) -> Result<Exit, CliError> {
    let mut stdout = io::stdout().lock();
    if render(store, &mut stdout).await? {
        return Err(CliError::Failed {
            verb: "ps",
            source: "a job could not be listed or recovered; see its row".into(),
        });
    }
    Ok(Exit::Success)
}
