//! `candler evaluate`: starts an evaluate job over stored runs, follows it and prints its report.
//!
//! The verb derives what it can. The plug-ins are the ones the project's `candler.toml` lists
//! under `plugins.dirs`, and the runs are the ones with no complete outcome set for the suite's
//! evaluator versions. It works in the foreground only up to the preflight that builds the
//! plug-in environments; it then writes the job's directory, starts the detached supervisor,
//! prints the job's id on stderr and waits for the job to end, so that the report goes to stdout
//! as the one document of the call. Ctrl-C stops the job through the guarded `kill` path.

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use candler_core::evaluation::engine::{EvaluateError, RunSelection};
use candler_core::evaluation::registry::{Registry, default_env_root, discover_manifests};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::{JobEvents, TailPos};
use candler_core::jobs::lost::{self, JobLiveness};
use candler_core::jobs::model::{EvaluateSpec, JobSpecFile};
use candler_core::jobs::runtime::TEARDOWN_BOUND;
use candler_core::jobs::runtime::detach::{DetachArgs, detach};
use candler_core::jobs::runtime::evaluate::{REFUSAL_FILE, REPORT_FILE};
use candler_core::jobs::runtime::launch::{mint_job_id, own_identity, spec_digest};
use candler_core::jobs::runtime::reap::reap_one;
use candler_core::jobs::runtime::session::recover_job_dir;
use candler_core::store::Store;
use candler_core::store::layout::StoreLayout;
use candler_core::supervise::Supervisor;
use candler_schemas::execution::{EventPayload, JobEnd, JobRecord, JobState};
use candler_schemas::primitives::{JobId, RunKey};
use candler_schemas::refusal::RefusalRecord;
use serde_json::Value;
use tokio::signal::unix::{Signal, SignalKind, signal};

use super::refusal::CliRefusal;
use super::run::STORE_ENV;
use super::{CliError, Exit, common, kill};
use crate::Cli;

/// How often the job's log is looked at while the command waits.
const POLL: Duration = Duration::from_millis(200);

/// How often the supervisor is looked at for a zombie while a stop is under way.
const REAP_POLL: Duration = Duration::from_millis(50);

/// How many of the last lines of the supervisor's log a failed job's error carries.
const LOG_LINES: usize = 5;

/// The arguments of `candler evaluate`.
#[derive(Debug, clap::Args)]
pub struct EvaluateArgs {
    /// The suite file.
    pub suite: PathBuf,
    /// Grade the runs that this job started, in place of the runs with no complete outcome set.
    #[arg(conflicts_with = "runs")]
    pub job: Option<JobId>,
    /// A glob over run keys to grade, in place of the runs with no complete outcome set.
    /// cannot be derived: which runs to grade again is the caller's choice.
    #[arg(long)]
    pub runs: Option<String>,
}

/// The refusal a failed evaluate job recorded in its directory, kept whole so that the command
/// prints it with the job's own code, side and path. It prints as the text it was made with.
#[derive(Debug)]
pub struct JobRefusal {
    /// The record the supervisor wrote.
    record: RefusalRecord,
    /// The text of the failure, as the command has always worded it.
    text: String,
}

impl std::fmt::Display for JobRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.text.as_str()) // BORROW: explicit .as_str()
    }
}

impl std::error::Error for JobRefusal {}

impl JobRefusal {
    /// The record the supervisor wrote.
    pub(super) const fn record<'a>(&'a self) -> &'a RefusalRecord {
        &self.record
    }
}

/// The error of a job that ended without a report: the job's recorded refusal when it left one,
/// else the text alone.
fn job_failure(dir: &JobDir, id: &JobId, recovered: Option<u32>) -> CliError {
    let text = recovered.map_or_else(
        || format!("job {id} ended without a report: {}", log_tail(dir)),
        |processes| {
            let lost = format!(
                "job {id} ended without a report: it was lost, its supervisor died; recovery stopped {processes} process(es) and ended it failed"
            );
            log_lines(dir).map_or(lost.clone(), |lines| format!("{lost}; {lines}")) // BORROW: both arms own the text
        },
    );
    let record = std::fs::read(dir.path().join(REFUSAL_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<RefusalRecord>(bytes.as_slice()).ok()); // BORROW: explicit .as_slice()
    match record {
        Some(record) => CliError::Failed {
            verb: "evaluate",
            source: Box::new(JobRefusal { record, text }),
        },
        None => failed_with(text),
    }
}

/// How the job the command waited for ended, and whether Ctrl-C reached the command meanwhile.
struct Followed {
    /// How the job ended.
    end: JobEnd,
    /// Whether Ctrl-C reached the command while it waited.
    interrupted: bool,
    /// How many processes the recovery of a lost job stopped, when the job was found lost.
    recovered: Option<u32>,
}

fn failed(error: impl std::error::Error + Send + Sync + 'static) -> CliError {
    CliError::failed("evaluate", error)
}

/// A failure of the verb that has no error type of its own, described by `detail`.
fn failed_with(detail: String) -> CliError {
    CliError::Failed {
        verb: "evaluate",
        source: detail.into(),
    }
}

/// The runs the job `id` started, from its event log, as the selection of keys.
fn runs_of_job(layout: &StoreLayout, id: &JobId) -> Result<RunSelection, CliError> {
    let dir = JobDir::open(layout, id).map_err(failed)?;
    let tail = JobEvents::open(&dir)
        .and_then(|events| events.tail(TailPos(0)))
        .map_err(failed)?;
    let keys: BTreeSet<RunKey> = tail
        .events
        .iter()
        .filter(|envelope| matches!(envelope.event(), EventPayload::RunStarted { .. }))
        .filter_map(|envelope| envelope.run().cloned()) // BORROW: the set owns its keys
        .collect();
    if keys.is_empty() {
        return Err(CliError::Refused {
            reason: format!("job {id} started no run, so there is nothing to grade"),
        });
    }
    Ok(RunSelection::Keys(keys.into_iter().collect()))
}

/// The variables the supervisor is started with: the caller's whole environment, so that the
/// evaluators inherit it, and the store root. They travel as the process environment of the
/// supervisor and are written nowhere: the job's spec, records and log hold none of them.
fn supervisor_env(root: &Path) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .filter(|(name, _)| name != STORE_ENV)
        .collect();
    env.push((STORE_ENV.to_owned(), root.display().to_string())); // BORROW: the pair owns its text
    env
}

/// The last lines of the supervisor's log of the job in `dir`, on one line, or the words that say
/// it holds none.
fn log_tail(dir: &JobDir) -> String {
    log_lines(dir).unwrap_or_else(|| "its supervisor.log is empty".to_owned()) // BORROW: the text is a literal
}

/// The last lines of the supervisor's log of the job in `dir`, on one line, if it holds any.
fn log_lines(dir: &JobDir) -> Option<String> {
    let text = std::fs::read_to_string(dir.path().join("supervisor.log")).unwrap_or_default();
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let from = lines.len().saturating_sub(LOG_LINES);
    let kept = lines.get(from..).unwrap_or_default();
    (!kept.is_empty()).then(|| kept.join("; "))
}

/// Waits for the job to end: looks at its log and lock every [`POLL`], recovers a job found lost,
/// and, when Ctrl-C comes, stops the job through `kill`'s path and waits for its end for at most
/// [`TEARDOWN_BOUND`].
async fn follow(
    layout: &StoreLayout,
    id: &JobId,
    supervisor: Option<u32>,
    sigint: &mut Signal,
) -> Result<Followed, CliError> {
    let dir = JobDir::open(layout, id).map_err(failed)?;
    let mut interrupted = false;
    let mut recovered = None;
    let mut stopping: Option<(Instant, Option<String>)> = None;
    loop {
        reap_supervisor(supervisor);
        match lost::classify(&dir).map_err(failed)? {
            JobLiveness::Ended(end) => {
                return Ok(Followed {
                    end,
                    interrupted,
                    recovered,
                });
            }
            JobLiveness::Lost => {
                let report = recover_job_dir(&dir).await.map_err(|reason| {
                    failed_with(format!("the job is lost and recover failed: {reason}"))
                })?;
                recovered = Some(report.processes);
                if let JobLiveness::Ended(end) = lost::classify(&dir).map_err(failed)? {
                    return Ok(Followed {
                        end,
                        interrupted,
                        recovered,
                    });
                }
            }
            JobLiveness::Running | _ => {}
        }
        if let Some((limit, reason)) = &stopping
            && Instant::now() >= *limit
        {
            let why = reason.as_deref().unwrap_or("the job did not end in time");
            return Err(failed_with(format!(
                "job {id} was told to stop and has not ended: {why}"
            )));
        }
        tokio::select! {
            got = sigint.recv() => {
                // EXPLICIT: a closed stream only ends the wait for Ctrl-C
                let _ = got;
                if interrupted {
                    return Err(interrupted_again(id));
                }
                interrupted = true;
                // A second Ctrl-C ends the wait at once; the job stays stopped by the first.
                let stop = kill::stop_job(layout, id);
                tokio::pin!(stop);
                let reason = loop {
                    tokio::select! {
                        stopped = &mut stop => break stopped.err().map(|error| error.to_string()),
                        _ = sigint.recv() => return Err(interrupted_again(id)),
                        () = tokio::time::sleep(REAP_POLL) => reap_supervisor(supervisor),
                    }
                };
                stopping = Some((Instant::now() + TEARDOWN_BOUND, reason));
            }
            () = tokio::time::sleep(POLL) => {}
        }
    }
}

/// Waits, without blocking, for the supervisor if it has exited. It is this command's child, and a
/// child nobody waits for stays a zombie, which the guard's liveness check reads as alive: the
/// stop of `kill` would wait out its whole grace for a supervisor that is already gone.
fn reap_supervisor(pid: Option<u32>) {
    if let Some(pid) = pid {
        // EXPLICIT: whether it was reaped does not matter; the next look tries again
        let _ = reap_one(pid);
    }
}

/// The error of a command that was interrupted twice: the first Ctrl-C has already told the job
/// to stop.
fn interrupted_again(id: &JobId) -> CliError {
    CliError::Runtime {
        detail: format!("`evaluate` was interrupted again; job {id} is being stopped"),
    }
}

/// Prints the report of the finished job and decides the exit by its failed requests.
fn print_report(dir: &JobDir) -> Result<Exit, CliError> {
    let text = std::fs::read_to_string(dir.path().join(REPORT_FILE)).map_err(|error| {
        failed_with(format!(
            "the job completed without a readable report: {error}"
        ))
    })?;
    let document: Value = serde_json::from_str(text.as_str()) // BORROW: explicit .as_str()
        .map_err(failed)?;
    writeln!(std::io::stdout(), "{document}").map_err(failed)?;
    let count = document
        .get("failed_requests")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if count == 0 {
        Ok(Exit::Success)
    } else {
        Err(CliError::Runtime {
            detail: format!("`evaluate`: {count} request(s) failed"),
        })
    }
}

/// Starts an evaluate job over the selected runs, waits for it and prints its report.
///
/// # Errors
/// [`CliError::Refused`] when no plug-in directory is configured, no store is named, or the job
/// named by `<job-id>` started no run; [`CliError::Failed`] when the plug-ins, the preflight or
/// the job fail, with the end of the job's log; [`CliError::Runtime`] when the job was stopped
/// (by Ctrl-C or by `candler kill`) or a request of the evaluation failed.
pub async fn run(cli: &Cli, args: &EvaluateArgs) -> Result<Exit, CliError> {
    let config = common::load_config("evaluate")?;
    let plugin_dirs = config
        .project
        .as_ref()
        .map(|project| project.plugin_dirs.clone()) // BORROW: the spec owns its directories
        .unwrap_or_default();
    if plugin_dirs.is_empty() {
        return Err(CliError::refused_by(CliRefusal::NoPluginDirs));
    }
    if config.user.slots_evaluators == Some(0) {
        return Err(CliError::refused_by(CliRefusal::NoEvaluatorSlots));
    }
    let cwd = std::env::current_dir().map_err(|e| CliError::Runtime {
        detail: format!("cannot read the working directory: {e}"),
    })?;
    let suite = cwd.join(&args.suite);
    let suite_bytes = std::fs::read(&suite).map_err(|source| {
        failed(EvaluateError::Io {
            op: "read the suite",
            path: suite.clone(), // BORROW: the error owns the path
            source,
        })
    })?;
    let manifests = discover_manifests(plugin_dirs.as_slice()) // BORROW: explicit .as_slice()
        .map_err(failed)?;
    let registry = Registry::load(manifests.as_slice()) // BORROW: explicit .as_slice()
        .map_err(failed)?;
    let env_root = default_env_root(&config).map_err(failed)?;
    // TRAIT_OBJECT: the supervisor takes any sink
    let supervisor = Arc::new(Supervisor::new(Arc::new(common::StderrSink)));
    let resolver = common::evaluation_resolver(
        "evaluate",
        &config,
        &registry,
        env_root.as_path(), // BORROW: explicit .as_path()
        &supervisor,
    )?;
    let resolution = common::resolve("evaluate", cli.store.as_deref(), &config, &resolver).await?;
    let root = common::resolved_root(&resolution)?;
    let store = Store::open(root.as_path()).map_err(failed)?; // BORROW: explicit .as_path()

    let selection = match (&args.job, &args.runs) {
        (Some(job), _) => runs_of_job(store.layout(), job)?,
        (None, Some(glob)) => RunSelection::Glob(glob.clone()), // BORROW: the spec owns its glob
        (None, None) => RunSelection::Pending,
    };
    let spec = EvaluateSpec {
        suite_digest: spec_digest(suite_bytes.as_slice()).map_err(failed)?, // BORROW: explicit .as_slice()
        suite,
        plugin_dirs,
        env_root,
        selection,
        request_deadline_secs: None,
    };
    let bytes = serde_json::to_vec(&JobSpecFile::Evaluate(spec)).map_err(failed)?;
    let digest = spec_digest(bytes.as_slice()).map_err(failed)?; // BORROW: explicit .as_slice()
    let id = mint_job_id().map_err(failed)?;
    let record = JobRecord::builder()
        .id(id.clone()) // BORROW: the record owns its id
        .supervisor(own_identity().map_err(failed)?)
        .spec_digest(digest)
        .state(JobState::Running)
        .build()
        .map_err(failed)?;

    // Ctrl-C is caught from here on: before the job exists the default action stops the call, and
    // after it only the guarded stop does.
    let mut sigint = signal(SignalKind::interrupt()).map_err(failed)?;
    let dir = JobDir::create(store.layout(), id.clone(), &record).map_err(failed)?; // BORROW: the dir owns its id
    let launch = DetachArgs {
        exe: std::env::current_exe().map_err(failed)?,
        job_dir: dir.path().to_path_buf(), // BORROW: the args own their path
        spec: bytes,
        env: supervisor_env(root.as_path()), // BORROW: explicit .as_path()
    };
    let launched = tokio::task::spawn_blocking(move || detach(launch))
        .await
        .map_err(|e| CliError::Runtime {
            detail: e.to_string(),
        })?;
    let supervisor = launched.map_err(failed)?.pid;
    eprintln!("{id}");

    let followed = follow(store.layout(), &id, Some(supervisor), &mut sigint).await?;
    match followed.end {
        JobEnd::Completed => print_report(&dir),
        JobEnd::Killed if followed.interrupted => Err(CliError::Runtime {
            detail: "`evaluate` was cancelled".to_owned(), // BORROW: the error owns its text
        }),
        JobEnd::Killed => Err(CliError::Runtime {
            detail: "`evaluate` was killed".to_owned(), // BORROW: the error owns its text
        }),
        JobEnd::Failed | JobEnd::BudgetStop | _ => Err(job_failure(&dir, &id, followed.recovered)),
    }
}
