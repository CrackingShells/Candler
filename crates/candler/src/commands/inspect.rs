//! `candler inspect`: shows one run, or one job, with its outcomes, their evidence, the provenance
//! the run recorded and whether it counts.
//!
//! The target says what to look at and the rest is derived from it. A run directory
//! (`<store>/runs/<key>`) names its own store two levels up, so no `--store` is read for it; a job
//! id and a prefix of a run key are looked up in the store the other verbs use. The store is
//! opened read-only, and a lost job is shown as lost, never recovered.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::str::FromStr as _;

use candler_core::analysis::refusal::ReportError;
use candler_core::analysis::spend::spend_of;
use candler_core::evaluation::selection::select::select;
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::lost::{JobLiveness, classify};
use candler_core::store::Store;
use candler_core::trace::{TranscriptForm, render_transcript};
use candler_schemas::inspect::{
    InspectJob, InspectJobRun, InspectJobTag, InspectOutcome, InspectRun, InspectRunTag, Spent,
};
use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::{JobId, RunKey};
use serde_json::Value;

use super::refusal::CliRefusal;
use super::{CliError, Exit, OutputArgs, common, ps, write_record};
use crate::Cli;

/// How many candidates an ambiguous key prefix lists.
const CANDIDATES: usize = 5;

/// The fewest characters a key prefix has.
const MIN_PREFIX: usize = 6;

/// The three forms a target takes, as the refusals list them.
pub(crate) const FORMS: &str = "a run directory (`<store>/runs/<key>`), a prefix of a run key (at least 6 characters), or a job id";

/// The arguments of `candler inspect`.
#[derive(Debug, clap::Args)]
pub struct InspectArgs {
    /// A run directory, a prefix of a run key, or a job id. cannot be derived: it names what to look at.
    pub target: String,
    /// Print the run's stored trace, one event per line, instead of its record: each event as
    /// stored with JSON, one line of text per event with `--format text`. cannot be derived: it
    /// names what to show of the run, which no format or target says.
    #[arg(long)]
    pub transcript: bool,
    #[command(flatten)]
    pub output: OutputArgs,
}

/// What a target names.
#[derive(Debug)]
#[non_exhaustive]
pub enum Target {
    /// A run of a store.
    Run {
        /// The store that holds the run.
        store: Store,
        /// The run.
        key: RunKey,
    },
    /// A job of a store.
    Job {
        /// The store that holds the job.
        store: Store,
        /// The job.
        id: JobId,
    },
}

/// Whether `text` can be the start of a run key: at least [`MIN_PREFIX`] characters of the digits of
/// a cell digest and of the `-r<replicate>-a<attempt>` that follows it.
fn looks_like_key_prefix(text: &str) -> bool {
    text.len() >= MIN_PREFIX
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f' | 'r' | '-'))
}

/// The run a directory holds, with the store two levels up.
fn run_directory(path: &Path) -> Result<Target, CliError> {
    let fail = |detail: String| {
        CliError::refused_by(CliRefusal::RunDirectoryInvalid {
            path: path.display().to_string(),
            detail,
        })
    };
    let dir = fs::canonicalize(path).map_err(|e| CliError::failed("inspect", e))?;
    let key = dir
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(|name| RunKey::from_str(name).ok())
        .ok_or_else(|| fail(String::from("the directory is not named by a run key")))?;
    let runs = dir
        .parent()
        .filter(|parent| parent.file_name().and_then(std::ffi::OsStr::to_str) == Some("runs"))
        .ok_or_else(|| {
            fail(String::from(
                "the directory is not under a `runs` directory",
            ))
        })?;
    let root = runs
        .parent()
        .ok_or_else(|| fail(String::from("the `runs` directory has no parent")))?;
    let store = Store::open(root).map_err(|e| fail(format!("no store above it: {e}")))?;
    Ok(Target::Run { store, key })
}

/// What `target` names: an existing run directory, else a job of the store, else the one run of
/// the store whose key starts with it.
///
/// # Errors
/// Returns [`CliError::Coded`] for a target that matches none of the three forms, for a prefix
/// that names no run or more than one, and for a run directory that no store holds, and
/// [`CliError::Failed`] when the store cannot be opened or read.
pub async fn resolve_target(cli: &Cli, target: &str) -> Result<Target, CliError> {
    let path = Path::new(target);
    if path.join("manifest.json").is_file() {
        return run_directory(path);
    }
    let job = JobId::from_str(target).ok();
    let prefix = looks_like_key_prefix(target);
    if job.is_none() && !prefix {
        return Err(CliError::refused_by(CliRefusal::InspectTargetInvalid {
            target: target.to_owned(), // BORROW: the refusal owns the target
        }));
    }
    let store = common::open_store("inspect", cli).await?;
    if let Some(id) = job
        && store.layout().job_dir(&id).is_dir()
    {
        return Ok(Target::Job { store, id });
    }
    if !prefix {
        return Err(CliError::refused(format!(
            "no job `{target}` in the store; a target is {FORMS}"
        )));
    }
    let matches: Vec<RunKey> = store
        .runs()
        .map_err(|e| CliError::failed("inspect", e))?
        .into_iter()
        .map(|entry| entry.key)
        .filter(|key| key.to_string().starts_with(target))
        .collect();
    match matches.as_slice() {
        [] => Err(CliError::refused_by(CliRefusal::NoSuchRun {
            target: target.to_owned(), // BORROW: the refusal owns the target
        })),
        [only] => Ok(Target::Run {
            store,
            key: only.clone(), // BORROW: the target owns the key
        }),
        many => {
            let listed: Vec<String> = many
                .iter()
                .take(CANDIDATES)
                .map(ToString::to_string)
                .collect();
            let more = many.len().saturating_sub(CANDIDATES);
            let tail = if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            };
            Err(CliError::refused_by(CliRefusal::AmbiguousRun {
                target: target.to_owned(), // BORROW: the refusal owns the target
                count: many.len(),
                listed: listed.join(", "),
                tail,
            }))
        }
    }
}

/// The outcome records of the run `key`, every request of every evaluator.
fn records_of(store: &Store, key: &RunKey) -> Result<Vec<OutcomeRecord>, CliError> {
    let mut records = Vec::new();
    for file in store
        .outcome_files(key)
        .map_err(|e| CliError::failed("inspect", e))?
    {
        records.extend(
            store
                .read_outcomes(&file)
                .map_err(|e| CliError::failed("inspect", e))?,
        );
    }
    Ok(records)
}

/// The outcome record as `inspect` shows it.
fn outcome_of(record: &OutcomeRecord) -> Result<InspectOutcome, CliError> {
    let whole = serde_json::to_value(record).map_err(|e| CliError::failed("inspect", e))?;
    let field = |name: &str| whole.get(name).cloned(); // BORROW: the record owns its copy
    Ok(InspectOutcome {
        evaluator: field("evaluator"),
        expectation: field("expectation_id"),
        role: field("role"),
        status: field("status"),
        value: field("value"),
        weight: field("weight"),
        cause: field("cause"),
        evidence: field("evidence"),
        request: field("request_id"),
    })
}

/// The record of the run `key`: whether it is sealed and counts, what it was read from, the
/// provenance and the other values it measured, and its outcomes with their evidence.
///
/// # Errors
/// Returns [`CliError::Failed`] when the manifest or an outcome file cannot be read.
pub fn render_run(store: &Store, key: &RunKey) -> Result<InspectRun, CliError> {
    let manifest = store
        .read_manifest(key)
        .map_err(|e| CliError::failed("inspect", e))?;
    let whole = serde_json::to_value(&manifest).map_err(|e| CliError::failed("inspect", e))?;
    let field = |name: &str| whole.get(name).cloned(); // BORROW: the record owns its copy
    let mut provenance = BTreeMap::new();
    let mut measured = BTreeMap::new();
    for entry in whole
        .get("measured")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (Some(path), Some(value)) = (
            entry.get("path").and_then(Value::as_str),
            entry.get("value"),
        ) else {
            continue;
        };
        let into = if path.starts_with("provenance.") {
            &mut provenance
        } else {
            &mut measured
        };
        into.insert(path.to_owned(), value.clone()); // BORROW: the record owns its copy
    }
    let mut records = records_of(store, key)?;
    records.sort_by(|a, b| {
        a.evaluator()
            .to_string()
            .cmp(&b.evaluator().to_string())
            .then_with(|| {
                a.expectation_id()
                    .to_string()
                    .cmp(&b.expectation_id().to_string())
            })
            .then_with(|| b.request_id().cmp(a.request_id()))
    });
    let outcomes = records
        .iter()
        .map(outcome_of)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InspectRun {
        schema: InspectRunTag,
        run: key.to_string(),
        sealed: store.layout().done(key).exists(),
        integrity: field("integrity"),
        experiment: field("experiment"),
        job: field("job"),
        started_at: field("started_at"),
        ended_at: field("ended_at"),
        host: field("host"),
        import: field("import"),
        provenance,
        measured,
        outcomes,
    })
}

/// How many current outcomes of `key` have each status.
fn status_counts(store: &Store, key: &RunKey) -> Result<BTreeMap<String, u64>, CliError> {
    let mut counts: BTreeMap<String, u64> =
        ["pass", "fail", "blocked", "not_measured", "not_applicable"]
            .iter()
            .map(|status| ((*status).to_owned(), 0))
            .collect();
    let selection = select(records_of(store, key)?);
    for record in selection.current() {
        let status =
            serde_json::to_value(record.status()).map_err(|e| CliError::failed("inspect", e))?;
        if let Some(count) = status.as_str().and_then(|text| counts.get_mut(text)) {
            *count = count.saturating_add(1);
        }
    }
    Ok(counts)
}

/// The state of the job of `dir`, as `ps` words it, without recovering a lost one. A job whose
/// supervisor has not created its lock is `starting`: probing would create the lock, and a
/// reading verb writes nothing.
fn job_state(dir: &JobDir) -> Result<String, CliError> {
    if !dir.path().join("lock").exists() {
        return Ok(String::from("starting"));
    }
    let liveness = classify(dir).map_err(|e| CliError::failed("inspect", e))?;
    Ok(match liveness {
        JobLiveness::Running => String::from("running"),
        JobLiveness::Ended(end) => ps::end_text(end),
        JobLiveness::Lost => String::from("lost"),
        _ => String::from("unknown"),
    })
}

/// The record of the job `id`: its state and the runs whose manifest names it.
///
/// # Errors
/// Returns [`CliError::Failed`] when the job record, the runs or their outcomes cannot be read.
pub fn render_job(store: &Store, id: &JobId) -> Result<InspectJob, CliError> {
    let dir = JobDir::open(store.layout(), id).map_err(|e| CliError::failed("inspect", e))?;
    let record = dir.record().map_err(|e| CliError::failed("inspect", e))?;
    let state = job_state(&dir)?;
    let mut runs = Vec::new();
    let mut sealed = Vec::new();
    for entry in store.runs().map_err(|e| CliError::failed("inspect", e))? {
        // EXPLICIT: a run whose manifest cannot be read names no job
        let Ok(manifest) = store.read_manifest(&entry.key) else {
            continue;
        };
        if manifest.job() != Some(id) {
            continue;
        }
        let integrity = serde_json::to_value(manifest.integrity())
            .map_err(|e| CliError::failed("inspect", e))?;
        runs.push(InspectJobRun {
            key: entry.key.to_string(),
            integrity,
            outcomes: status_counts(store, &entry.key)?,
        });
        if entry.done {
            sealed.push(manifest);
        }
    }
    let spend = spend_of(sealed.iter());
    Ok(InspectJob {
        schema: InspectJobTag,
        id: record.id().to_string(),
        state,
        runs,
        spent: Spent {
            cost_usd: spend.cost_usd,
            runs_counted: spend.runs_counted,
            runs_without_total: spend.runs_without_total,
        },
    })
}

/// The run record as lines: the run, its seal, what it measured and each outcome.
fn text_of_run(run: &InspectRun) -> String {
    let mut lines = vec![format!("run {}", run.run), format!("sealed {}", run.sealed)];
    for (path, value) in run.provenance.iter().chain(run.measured.iter()) {
        lines.push(format!("{path}\t{value}"));
    }
    for outcome in &run.outcomes {
        let word =
            |value: &Option<Value>| value.as_ref().map_or_else(String::new, ToString::to_string);
        lines.push(format!(
            "outcome {} {} {}",
            word(&outcome.evaluator),
            word(&outcome.expectation),
            word(&outcome.status)
        ));
    }
    lines.join("\n")
}

/// The job record as lines: the job, its state, each run's outcome counts and what it spent.
fn text_of_job(job: &InspectJob) -> String {
    let mut lines = vec![format!("job {} {}", job.id, job.state)];
    for run in &job.runs {
        let counts: Vec<String> = run
            .outcomes
            .iter()
            .map(|(status, count)| format!("{status}={count}"))
            .collect();
        lines.push(format!("run {} {}", run.key, counts.join(" ")));
    }
    lines.push(format!(
        "spent {} usd over {} run(s), {} without a total",
        job.spent.cost_usd, job.spent.runs_counted, job.spent.runs_without_total
    ));
    lines.join("\n")
}

/// The lines of the transcript of the run `key`: its stored trace, one line per event.
///
/// # Errors
/// Returns [`ReportError::NoStoredTrace`] when the run has no stored trace and
/// [`CliError::Failed`] when the stored file cannot be read as a trace.
fn transcript_lines(
    store: &Store,
    key: &RunKey,
    target: &str,
    form: TranscriptForm,
) -> Result<Vec<String>, CliError> {
    if !store.layout().trace(key).is_file() {
        return Err(CliError::refused_by(ReportError::NoStoredTrace {
            target: target.to_owned(), // BORROW: the refusal owns the text
        }));
    }
    let trace = store
        .read_trace(key)
        .map_err(|e| CliError::failed("inspect", e))?;
    render_transcript(&trace, form).map_err(|e| CliError::failed("inspect", e))
}

/// Prints what the store holds of the run, or the job, that the target names.
///
/// # Errors
/// Returns the errors of [`resolve_target`], [`render_run`] and [`render_job`].
pub async fn run(cli: &Cli, args: &InspectArgs) -> Result<Exit, CliError> {
    let mut stdout = std::io::stdout().lock();
    match resolve_target(cli, args.target.as_str()).await? {
        // BORROW: explicit .as_str()
        Target::Run { store, key } if args.transcript => {
            let form = if args.output.text() {
                TranscriptForm::Text
            } else {
                TranscriptForm::Json
            };
            let lines = transcript_lines(&store, &key, args.target.as_str(), form)?;
            for line in lines {
                writeln!(stdout, "{line}").map_err(|e| CliError::failed("inspect", e))?;
            }
        }
        Target::Job { .. } if args.transcript => {
            return Err(CliError::refused_by(ReportError::NoStoredTrace {
                target: args.target.clone(), // BORROW: the refusal owns the text
            }));
        }
        Target::Run { store, key } => {
            let record = render_run(&store, &key)?;
            if args.output.text() {
                println!("{}", text_of_run(&record));
            } else {
                write_record("inspect", &mut stdout, &record)?;
            }
        }
        Target::Job { store, id } => {
            let record = render_job(&store, &id)?;
            if args.output.text() {
                println!("{}", text_of_job(&record));
            } else {
                write_record("inspect", &mut stdout, &record)?;
            }
        }
    }
    Ok(Exit::Success)
}
