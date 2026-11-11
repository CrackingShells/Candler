//! `candler inspect`: shows one run, or one job, with its outcomes, their evidence, the provenance
//! the run recorded and whether it counts.
//!
//! The target says what to look at and the rest is derived from it. A run directory
//! (`<store>/runs/<key>`) names its own store two levels up, so no `--store` is read for it; a job
//! id and a prefix of a run key are looked up in the store the other verbs use. The store is
//! opened read-only, and a lost job is shown as lost, never recovered.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::str::FromStr as _;

use candler_core::evaluation::selection::select::select;
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::lost::{JobLiveness, classify};
use candler_core::store::Store;
use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::{JobId, RunKey};
use serde_json::{Map, Value, json};

use super::{CliError, Exit, common, ps};
use crate::Cli;

/// How many candidates an ambiguous key prefix lists.
const CANDIDATES: usize = 5;

/// The fewest characters a key prefix has.
const MIN_PREFIX: usize = 6;

/// The three forms a target takes, as the refusals list them.
const FORMS: &str = "a run directory (`<store>/runs/<key>`), a prefix of a run key (at least 6 characters), or a job id";

/// The arguments of `candler inspect`.
#[derive(Debug, clap::Args)]
pub struct InspectArgs {
    /// A run directory, a prefix of a run key, or a job id. cannot be derived: it names what to look at.
    pub target: String,
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
    let fail = |detail: String| CliError::refused(format!("`{}`: {detail}", path.display()));
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
/// Returns [`CliError::Refused`] for a target that matches none of the three forms, for a prefix
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
        return Err(CliError::refused(format!("`{target}` is none of: {FORMS}")));
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
        [] => Err(CliError::refused(format!(
            "no run of the store starts with `{target}`"
        ))),
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
            Err(CliError::refused(format!(
                "`{target}` starts {} runs: {}{tail}; give more of the key",
                many.len(),
                listed.join(", ")
            )))
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

/// The JSON of one outcome record as `inspect` shows it.
fn outcome_json(record: &OutcomeRecord) -> Result<Value, CliError> {
    let whole = serde_json::to_value(record).map_err(|e| CliError::failed("inspect", e))?;
    let mut shown = Map::new();
    for (from, to) in [
        ("evaluator", "evaluator"),
        ("expectation_id", "expectation"),
        ("role", "role"),
        ("status", "status"),
        ("value", "value"),
        ("weight", "weight"),
        ("cause", "cause"),
        ("evidence", "evidence"),
        ("request_id", "request"),
    ] {
        if let Some(found) = whole.get(from) {
            shown.insert(to.to_owned(), found.clone()); // BORROW: the document owns its copy
        }
    }
    Ok(Value::Object(shown))
}

/// The document of the run `key`: whether it is sealed and counts, what it was read from, the
/// provenance and the other values it measured, and its outcomes with their evidence.
///
/// # Errors
/// Returns [`CliError::Failed`] when the manifest or an outcome file cannot be read.
pub fn render_run(store: &Store, key: &RunKey) -> Result<Value, CliError> {
    let manifest = store
        .read_manifest(key)
        .map_err(|e| CliError::failed("inspect", e))?;
    let whole = serde_json::to_value(&manifest).map_err(|e| CliError::failed("inspect", e))?;
    let mut doc = Map::new();
    doc.insert("run".to_owned(), json!(key.to_string())); // BORROW: the document owns its key text
    doc.insert(
        "sealed".to_owned(),
        json!(store.layout().done(key).exists()),
    );
    for name in [
        "integrity",
        "experiment",
        "job",
        "started_at",
        "ended_at",
        "host",
        "import",
    ] {
        if let Some(found) = whole.get(name) {
            doc.insert(name.to_owned(), found.clone()); // BORROW: the document owns its copy
        }
    }
    let mut provenance = Map::new();
    let mut measured = Map::new();
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
        into.insert(path.to_owned(), value.clone()); // BORROW: the document owns its copy
    }
    doc.insert("provenance".to_owned(), Value::Object(provenance));
    doc.insert("measured".to_owned(), Value::Object(measured));
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
        .map(outcome_json)
        .collect::<Result<Vec<_>, _>>()?;
    doc.insert("outcomes".to_owned(), Value::Array(outcomes));
    Ok(Value::Object(doc))
}

/// How many current outcomes of `key` have each status.
fn status_counts(store: &Store, key: &RunKey) -> Result<Value, CliError> {
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
    Ok(json!(counts))
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

/// The document of the job `id`: its state and the runs whose manifest names it.
///
/// # Errors
/// Returns [`CliError::Failed`] when the job record, the runs or their outcomes cannot be read.
pub fn render_job(store: &Store, id: &JobId) -> Result<Value, CliError> {
    let dir = JobDir::open(store.layout(), id).map_err(|e| CliError::failed("inspect", e))?;
    let record = dir.record().map_err(|e| CliError::failed("inspect", e))?;
    let state = job_state(&dir)?;
    let mut runs = Vec::new();
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
        runs.push(json!({
            "key": entry.key.to_string(),
            "integrity": integrity,
            "outcomes": status_counts(store, &entry.key)?,
        }));
    }
    Ok(json!({"id": record.id().to_string(), "state": state, "runs": runs}))
}

/// Prints what the store holds of the run, or the job, that the target names.
///
/// # Errors
/// Returns the errors of [`resolve_target`], [`render_run`] and [`render_job`].
pub async fn run(cli: &Cli, args: &InspectArgs) -> Result<Exit, CliError> {
    let doc = match resolve_target(cli, args.target.as_str()).await? {
        // BORROW: explicit .as_str()
        Target::Run { store, key } => render_run(&store, &key)?,
        Target::Job { store, id } => render_job(&store, &id)?,
    };
    println!("{doc}");
    Ok(Exit::Success)
}
