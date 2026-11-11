//! The supervisor's entry: its session, its role as subreaper, its spec, its lock and the order
//! in which it starts and ends a job.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use candler_schemas::execution::{EventPayload, JobEnd, JobRecord, JobState};
use candler_schemas::primitives::{Digest, JobId};
use futures::FutureExt as _;
use rustix::process::{getpid, setsid};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::signals::SignalWatch;
use super::sweep::{ProcMarkerSweeper, reap_confirmed, sweep_orphans};
use super::{JobEntry, RuntimeError};
use crate::isolation::container::guard::ContainerGuard;
use crate::isolation::container::sweeper::LabelSweeper;
use crate::isolation::docker::{ClientEnv, DockerCli};
use crate::isolation::driver::DockerDriver;
use crate::jobs::dir::{JobDir, JobError};
use crate::jobs::events::JobEvents;
use crate::jobs::lock::JobLock;
use crate::jobs::lost::{self, JobLiveness, RecoverReport};
use crate::jobs::model::{JobBody, JobCause, JobControl, JobOutcome, JobSpec};
use crate::jobs::slots::Slots;
use crate::jobs::view::JobView;
use crate::resolve::config::{ConfigEnv, UserConfig};
use crate::runner::Runtime;
use crate::supervise::{EventSink, Supervisor};

/// How many slots a store has when the configuration does not say.
const DEFAULT_SLOTS: u32 = 4;
/// How long the recovery of one lost job may spend listing and destroying its containers.
const SWEEP_LIMIT: Duration = Duration::from_mins(2);
/// The longest pause between two looks for children of the supervisor that have exited.
const ZOMBIE_TICK: Duration = Duration::from_millis(250);
/// The record file of a job.
const RECORD_FILE: &str = "job.json";

fn session_error(detail: &impl ToString) -> RuntimeError {
    RuntimeError::Session {
        detail: detail.to_string(),
    }
}

fn job_error(error: &JobError) -> RuntimeError {
    RuntimeError::Job {
        detail: error.to_string(),
    }
}

fn setup_error(detail: &impl ToString) -> RuntimeError {
    RuntimeError::Setup {
        detail: detail.to_string(),
    }
}

/// Makes this process the leader of a new session, and so of a new process group. It fails when
/// the process already leads a group, so the launcher never starts the supervisor with its own
/// group.
///
/// # Errors
/// [`RuntimeError::Session`] when the system refuses.
pub fn detach_session() -> Result<(), RuntimeError> {
    setsid().map(|_| ()).map_err(|e| session_error(&e))
}

/// Makes this process the subreaper of its descendants on Linux, so an orphan is adopted by it
/// and not by init. Elsewhere there is no such role and this does nothing.
///
/// # Errors
/// [`RuntimeError::Session`] when the system refuses.
pub fn become_subreaper() -> Result<(), RuntimeError> {
    #[cfg(target_os = "linux")]
    {
        rustix::process::set_child_subreaper(Some(getpid())).map_err(|e| session_error(&e))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = getpid();
        Ok(())
    }
}

/// The digest of `bytes`: `sha256:` and the hex of their SHA-256.
pub(crate) fn digest_of(bytes: &[u8]) -> Result<Digest, RuntimeError> {
    use sha2::Digest as _;
    let mut hex = String::with_capacity(64);
    for byte in sha2::Sha256::digest(bytes) {
        hex.extend(char::from_digit(u32::from(byte >> 4), 16));
        hex.extend(char::from_digit(u32::from(byte & 0xf), 16));
    }
    Digest::from_hex(&hex).map_err(|e| RuntimeError::SpecDigest {
        detail: e.to_string(),
    })
}

/// Reads the job's spec from `stdin` to its end, checks the SHA-256 of those bytes against the
/// digest in `<job_dir>/job.json`, and only then reads the bytes as a spec.
///
/// # Errors
/// [`RuntimeError::SpecDigest`] when `stdin` or `job.json` cannot be read, the digest differs,
/// or the bytes are not a spec.
pub fn read_spec(job_dir: &Path, mut stdin: impl Read) -> Result<JobSpec, RuntimeError> {
    let refused = |detail: String| RuntimeError::SpecDigest { detail };
    let mut bytes = Vec::new();
    stdin
        .read_to_end(&mut bytes)
        .map_err(|e| refused(e.to_string()))?;
    let record_path = job_dir.join(RECORD_FILE);
    let record_bytes = std::fs::read(&record_path).map_err(|e| refused(e.to_string()))?;
    let record: JobRecord =
        serde_json::from_slice(&record_bytes).map_err(|e| refused(e.to_string()))?;
    let digest = digest_of(&bytes)?;
    if &digest != record.spec_digest() {
        return Err(refused(format!(
            "the spec's digest is {digest}, the job records {}",
            record.spec_digest()
        )));
    }
    serde_json::from_slice(&bytes).map_err(|e| refused(e.to_string()))
}

/// Recovers the lost job of `dir`, reporting to that job's own log and through a supervisor of
/// its own, over the real container guard. This is the composition of `candler ps`, `kill`,
/// `watch` and the supervisor's own sweep of lost jobs.
///
/// # Errors
/// The reason the recovery could not run, as text.
pub async fn recover_job_dir(dir: &JobDir) -> Result<RecoverReport, String> {
    recover_job_dir_with(dir, ContainerGuard::over_docker).await
}

/// [`recover_job_dir`] with the guard built by `guard` from the docker client and the job's view,
/// for a caller that fits the guard with a destroy half of its own. The guard it returns reports
/// to the job's own log.
///
/// # Errors
/// The reason the recovery could not run, as text.
pub async fn recover_job_dir_with<F>(dir: &JobDir, guard: F) -> Result<RecoverReport, String>
where
    // TRAIT_OBJECT: the guard reads the job through its view
    F: FnOnce(&DockerCli, Arc<dyn JobView>) -> ContainerGuard,
{
    let events = Arc::new(JobEvents::open(dir).map_err(|e| e.to_string())?);
    // TRAIT_OBJECT: the supervisor and the guard take their sink as a trait object
    let sink: Arc<dyn EventSink> = events;
    let sup = Arc::new(Supervisor::new(Arc::clone(&sink)));
    let cli = DockerCli::new(
        Arc::clone(&sup),
        ClientEnv::from_process(),
        dir.control_dir(),
    )
    .map_err(|e| e.to_string())?;
    // TRAIT_OBJECT: the guard and the sweeper read the job through its directory
    let view: Arc<dyn JobView> = Arc::new(dir.clone()); // BORROW: the view owns its directory
    let guard = Arc::new(guard(&cli, Arc::clone(&view)).with_events(Arc::clone(&sink)));
    // TRAIT_OBJECT: the sweeper lists through the docker client
    let docker: Arc<dyn DockerDriver> = Arc::new(cli);
    let sweeper = LabelSweeper::new(docker, guard, view, SWEEP_LIMIT).with_events(sink);
    lost::recover(&sup, &sweeper, dir)
        .await
        .map_err(|e| e.to_string())
}

/// The task that waits, on a tick, for the children of the supervisor that have exited and that
/// no runtime of it owns; it ends with the value. A process is taken only when two ticks in a
/// row saw it exited, so that a child still being spawned is left to the runtime that spawns it.
#[derive(Debug)]
pub struct ZombieTick(JoinHandle<()>);

impl ZombieTick {
    /// Starts the tick over the groups `sup` holds.
    #[must_use]
    pub fn start(sup: &Supervisor) -> Self {
        let guard = sup.guard();
        Self(tokio::spawn(async move {
            let mut tick = tokio::time::interval(ZOMBIE_TICK);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut seen = Vec::new();
            loop {
                tick.tick().await;
                let held = Arc::clone(&guard);
                let before = std::mem::take(&mut seen);
                let looked = tokio::task::spawn_blocking(move || {
                    let mut now = before;
                    let _reaped = reap_confirmed(&held, &mut now);
                    now
                })
                .await;
                seen = looked.unwrap_or_default();
            }
        }))
    }
}

impl Drop for ZombieTick {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Whether the job has been told to stop.
fn cancelled(control: &JobControl) -> bool {
    control.cancel.borrow().is_some()
}

/// Recovers every job of the store, but `own`, whose supervisor was lost, and only through
/// [`lost::recover`]. A job that is told to stop recovers no more: the cancel is looked at before
/// each recovery. A directory that cannot be read is skipped and named in the result with
/// its reason, never read as clean; a recovery that stays incomplete is returned as it is and
/// not repeated here.
pub async fn reap_lost_jobs(
    rt: &Runtime,
    own: &JobId,
    control: &JobControl,
) -> Vec<(JobId, Result<RecoverReport, String>)> {
    let layout = rt.store.layout();
    let jobs = layout.root().join("jobs");
    let Ok(listing) = std::fs::read_dir(&jobs) else {
        return Vec::new();
    };
    let mut ids: Vec<JobId> = listing
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
        .filter(|id| id != own)
        .collect();
    ids.sort();
    let mut reports = Vec::new();
    for id in ids {
        if cancelled(control) {
            break;
        }
        let dir = match JobDir::open(layout, &id) {
            Ok(dir) => dir,
            Err(error) => {
                reports.push((id, Err(error.to_string())));
                continue;
            }
        };
        match lost::classify(&dir) {
            Ok(JobLiveness::Lost) => {
                let report = recover_job_dir(&dir).await;
                reports.push((id, report));
            }
            // EXPLICIT: `Running` has a live supervisor and `Ended(_)` has already ended; neither
            // needs recovery
            Ok(_) => {}
            Err(error) => reports.push((id, Err(error.to_string()))),
        }
    }
    reports
}

/// Writes what the supervisor could not put in the job's log to its own output, which `detach`
/// points at `supervisor.log`.
pub(crate) fn note(line: &str) {
    use std::io::Write as _;
    let _ = writeln!(std::io::stderr(), "{line}");
}

/// Everything between `job.started` and `job.ended`: set up what the runs need, reap the lost
/// jobs, run the body and sweep the orphans it left.
async fn serve(
    entry: JobEntry,
    dir: &JobDir,
    // TRAIT_OBJECT: the sink is the job's event log, or a fake in tests
    sink: &Arc<dyn EventSink>,
    control: JobControl,
    body: JobBody,
) -> Result<JobOutcome, RuntimeError> {
    let JobEntry {
        store,
        job,
        spec,
        services,
    } = entry;
    let env = ConfigEnv::from_process();
    let user = UserConfig::load(&env).map_err(|e| setup_error(&e))?;
    let state = env
        .state_home()
        .ok_or_else(|| setup_error(&"no state directory for the slots"))?;
    let count = user
        .slots_count
        .unwrap_or(DEFAULT_SLOTS)
        .min(spec.budget.max_parallel);
    if count == 0 {
        return Err(setup_error(&"no slot to run in"));
    }
    let slots =
        Slots::open(&state.join("candler").join("slots"), count).map_err(|e| setup_error(&e))?;
    let first = Supervisor::new(Arc::clone(sink));
    let sweeper = ProcMarkerSweeper::new(first.guard()).with_events(Arc::clone(sink));
    let sup = Arc::new(first.with_sweeper(Arc::new(sweeper)));
    let _tick = ZombieTick::start(&sup);
    let cli = DockerCli::new(
        Arc::clone(&sup),
        ClientEnv::from_process(),
        dir.control_dir(),
    )
    .map_err(|e| setup_error(&e))?;
    // TRAIT_OBJECT: the runtime reads docker through the client
    let docker: Arc<dyn DockerDriver> = Arc::new(cli);
    let runtime = Runtime {
        supervisor: Arc::clone(&sup),
        store,
        sink: Arc::clone(sink),
        slots,
        docker,
        isolation: None,
        gateway: None,
        job: job.clone(), // BORROW: the runtime owns its job id
        driver: services.driver,
        skills: services.skills,
        cells: BTreeMap::new(),
        eval_infra: services.eval_infra,
    };
    for (lost, result) in reap_lost_jobs(&runtime, &job, &control).await {
        match result {
            Ok(report) => note(&format!("lost job {lost}: {:?}", report.outcome)),
            Err(reason) => note(&format!("lost job {lost} not recovered: {reason}")),
        }
    }
    let ran = tokio::spawn(body(runtime, spec, control)).await;
    let swept = {
        let (sup, sink) = (Arc::clone(&sup), Arc::clone(sink));
        tokio::task::spawn_blocking(move || sweep_orphans(&sup, sink.as_ref())).await
    };
    if let Err(error) = swept {
        note(&format!("the orphan sweep failed: {error}"));
    }
    ran.map_err(|error| {
        if error.is_panic() {
            RuntimeError::Body {
                detail: error.to_string(),
            }
        } else {
            session_error(&error)
        }
    })
}

/// Runs `body` as the supervisor of the job `entry` names, in the order that keeps a signal from
/// being lost and a job from ending without its event: the handlers, the session, the subreaper,
/// the lock, `job.started`, the work, the sweep, `job.ended`, the state, and the lock last. A
/// job that was told to stop ends `killed`, whatever its body returns. A body that panicked ends
/// without `job.ended` and the ended state, so that the job reads as lost.
///
/// # Errors
/// [`RuntimeError::Session`] when the handlers, the session or the subreaper role cannot be
/// had, [`RuntimeError::Job`] when the job's directory, log or state fails,
/// [`RuntimeError::AlreadySupervised`] when another supervisor holds the lock, and whatever
/// the setup and the body return.
pub(crate) async fn run(entry: JobEntry, body: JobBody) -> Result<JobOutcome, RuntimeError> {
    let (cancel, _kept) = watch::channel(None);
    let control = JobControl {
        cancel: Arc::new(cancel),
    };
    let _watch = SignalWatch::install(control.clone()).map_err(|e| session_error(&e))?;
    detach_session()?;
    become_subreaper()?;
    let dir = JobDir::open(entry.store.layout(), &entry.job).map_err(|e| job_error(&e))?;
    let lock = match JobLock::acquire(&dir) {
        Ok(lock) => lock,
        Err(JobError::Held) => return Err(RuntimeError::AlreadySupervised),
        Err(error) => return Err(job_error(&error)),
    };
    let events = Arc::new(JobEvents::open(&dir).map_err(|e| job_error(&e))?);
    // TRAIT_OBJECT: the supervisor and the runs take their sink as a trait object
    let sink: Arc<dyn EventSink> = events;
    sink.emit(None, EventPayload::job_started());
    let seen = control.clone();
    let started = dir.set_state(JobState::Running).map_err(|e| job_error(&e));
    let result = match started {
        Ok(()) => std::panic::AssertUnwindSafe(serve(entry, &dir, &sink, control, body))
            .catch_unwind()
            .await
            .unwrap_or_else(|_| {
                Err(RuntimeError::Body {
                    // BORROW: the error owns its detail
                    detail: "the supervisor panicked".to_owned(),
                })
            }),
        Err(error) => Err(error),
    };
    if let Err(error @ RuntimeError::Body { .. }) = &result {
        // A body that panicked may have skipped its teardown: the job ends without `job.ended`
        // and without the ended state, so that it reads as lost and is recovered by label.
        note(&error.to_string());
        drop(lock);
        return result;
    }
    // The cause the handler set decides how a stopped job ends, whatever the body returns. A
    // SIGTERM during the setup ends the job `killed` too: the setup error stays in
    // `supervisor.log`, which `note` writes.
    let told_to_die = matches!(*seen.cancel.borrow(), Some(JobCause::Killed));
    if let Err(error) = &result {
        note(&error.to_string());
    }
    let result = result.map(|outcome| {
        if told_to_die {
            JobOutcome {
                end: JobEnd::Killed,
                ..outcome
            }
        } else {
            outcome
        }
    });
    let end = match &result {
        _ if told_to_die => JobEnd::Killed,
        Ok(outcome) => outcome.end,
        Err(_) => JobEnd::Failed,
    };
    sink.emit(None, EventPayload::job_ended(end));
    let sealed = dir.set_state(JobState::Ended).map_err(|e| job_error(&e));
    drop(lock);
    match (result, sealed) {
        (Ok(outcome), Ok(())) => Ok(outcome),
        (Ok(_), Err(error)) | (Err(error), _) => Err(error),
    }
}
