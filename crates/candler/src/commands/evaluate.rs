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

use candler_core::evaluation::engine::RunSelection;
use candler_core::evaluation::registry::{Registry, default_env_root, discover_manifests};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::{JobEvents, TailPos};
use candler_core::jobs::lost::{self, JobLiveness};
use candler_core::jobs::model::{EvaluateSpec, JobSpecFile};
use candler_core::jobs::runtime::TEARDOWN_BOUND;
use candler_core::jobs::runtime::detach::{DetachArgs, detach};
use candler_core::jobs::runtime::evaluate::REPORT_FILE;
use candler_core::jobs::runtime::launch::{mint_job_id, own_identity, spec_digest};
use candler_core::jobs::runtime::session::recover_job_dir;
use candler_core::store::Store;
use candler_core::store::layout::StoreLayout;
use candler_core::supervise::Supervisor;
use candler_schemas::execution::{EventPayload, JobEnd, JobRecord, JobState};
use candler_schemas::primitives::{JobId, RunKey};
use serde_json::Value;
use tokio::signal::unix::{Signal, SignalKind, signal};

use super::run::STORE_ENV;
use super::{CliError, Exit, common, kill};
use crate::Cli;

/// How often the job's log is looked at while the command waits.
const POLL: Duration = Duration::from_millis(200);

/// How many of the last lines of the supervisor's log a failed job's error carries.
const LOG_LINES: usize = 5;

/// The variables of the caller the supervisor is given, besides [`STORE_ENV`]. The user
/// configuration is read from `XDG_CONFIG_HOME`, because `slots.evaluators` lives there.
const ENV_ALLOWLIST: [&str; 4] = ["PATH", "HOME", "XDG_STATE_HOME", "XDG_CONFIG_HOME"];

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

/// How the job the command waited for ended, and whether Ctrl-C reached the command meanwhile.
struct Followed {
    /// How the job ended.
    end: JobEnd,
    /// Whether Ctrl-C reached the command while it waited.
    interrupted: bool,
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

/// The variables the supervisor is started with: the allowlist as the caller has it and the
/// store root.
fn supervisor_env(root: &Path) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = ENV_ALLOWLIST
        .iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| ((*name).to_owned(), value)) // BORROW: the pair owns its name
        })
        .collect();
    env.push((STORE_ENV.to_owned(), root.display().to_string())); // BORROW: the pair owns its text
    env
}

/// The last lines of the supervisor's log of the job in `dir`, on one line.
fn log_tail(dir: &JobDir) -> String {
    let text = std::fs::read_to_string(dir.path().join("supervisor.log")).unwrap_or_default();
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let from = lines.len().saturating_sub(LOG_LINES);
    let kept = lines.get(from..).unwrap_or_default();
    if kept.is_empty() {
        "its supervisor.log is empty".to_owned() // BORROW: the text is a literal
    } else {
        kept.join("; ")
    }
}

/// Waits for the job to end: looks at its log and lock every [`POLL`], recovers a job found lost,
/// and, when Ctrl-C comes, stops the job through `kill`'s path and waits for its end for at most
/// [`TEARDOWN_BOUND`].
async fn follow(
    layout: &StoreLayout,
    id: &JobId,
    sigint: &mut Signal,
) -> Result<Followed, CliError> {
    let dir = JobDir::open(layout, id).map_err(failed)?;
    let mut interrupted = false;
    let mut stopping: Option<(Instant, Option<String>)> = None;
    loop {
        match lost::classify(&dir).map_err(failed)? {
            JobLiveness::Ended(end) => return Ok(Followed { end, interrupted }),
            JobLiveness::Lost => {
                recover_job_dir(&dir).await.map_err(|reason| {
                    failed_with(format!("the job is lost and recover failed: {reason}"))
                })?;
                if let JobLiveness::Ended(end) = lost::classify(&dir).map_err(failed)? {
                    return Ok(Followed { end, interrupted });
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
                let reason = tokio::select! {
                    stopped = kill::stop_job(layout, id) => stopped.err(),
                    _ = sigint.recv() => return Err(interrupted_again(id)),
                };
                stopping = Some((Instant::now() + TEARDOWN_BOUND, reason));
            }
            () = tokio::time::sleep(POLL) => {}
        }
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
        return Err(CliError::Refused {
            reason: "no plug-in directory: list one under `plugins.dirs` in a `candler.toml` at or above the working directory".to_owned(), // BORROW: the error owns its text
        });
    }
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

    let cwd = std::env::current_dir().map_err(|e| CliError::Runtime {
        detail: format!("cannot read the working directory: {e}"),
    })?;
    let suite = cwd.join(&args.suite);
    let suite_bytes = std::fs::read(&suite).map_err(|e| CliError::Refused {
        reason: format!("cannot read {}: {e}", suite.display()),
    })?;
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
    tokio::task::spawn_blocking(move || detach(launch))
        .await
        .map_err(|e| CliError::Runtime {
            detail: e.to_string(),
        })?
        .map_err(failed)?;
    eprintln!("{id}");

    let followed = follow(store.layout(), &id, &mut sigint).await?;
    match followed.end {
        JobEnd::Completed => print_report(&dir),
        JobEnd::Killed if followed.interrupted => Err(CliError::Runtime {
            detail: "`evaluate` was cancelled".to_owned(), // BORROW: the error owns its text
        }),
        JobEnd::Killed => Err(CliError::Runtime {
            detail: "`evaluate` was killed".to_owned(), // BORROW: the error owns its text
        }),
        JobEnd::Failed | JobEnd::BudgetStop | _ => Err(failed_with(format!(
            "job {id} ended without a report: {}",
            log_tail(&dir)
        ))),
    }
}
