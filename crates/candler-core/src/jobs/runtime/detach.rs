//! The launcher's half: starting a job's supervisor, handing it its spec and leaving it running.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use candler_schemas::primitives::JobId;

use super::RuntimeError;
use crate::jobs::dir::JobDir;
use crate::jobs::lock::{JobLock, LockState};

/// The file the supervisor's output goes to, in the job's directory.
const LOG_FILE: &str = "supervisor.log";
/// How long the launcher waits for the supervisor to take the job's lock.
const START_DEADLINE: Duration = Duration::from_secs(10);
/// The pause between two looks at the lock.
const POLL: Duration = Duration::from_millis(20);

/// What [`detach`] starts a supervisor from.
#[derive(Debug, Clone)]
pub struct DetachArgs {
    /// The binary that runs the hidden verb `supervise --job <id>`.
    pub exe: PathBuf,
    /// The directory of the job, named by its id.
    pub job_dir: PathBuf,
    /// The bytes of the job's spec, written to the supervisor's stdin.
    pub spec: Vec<u8>,
    /// The environment the supervisor has, and nothing else: the caller's allowlist.
    pub env: Vec<(String, String)>,
}

/// A supervisor that holds its job's lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetachedJob {
    /// The supervisor's process id.
    pub pid: u32,
    /// The job it supervises.
    pub job: JobId,
}

fn fail(detail: &impl ToString) -> RuntimeError {
    RuntimeError::Detach {
        detail: detail.to_string(),
    }
}

/// Starts the supervisor of the job in `job_dir` and returns once it holds the job's lock.
/// The supervisor leads its own session, which it makes itself, so it is never started in a new
/// process group here. The launcher never waits for it and drops its handle, so no client of it
/// stays behind.
///
/// # Errors
/// [`RuntimeError::Detach`] when the supervisor cannot be started or given its spec,
/// [`RuntimeError::DidNotStart`] when it has not taken the lock by the deadline or exits first.
pub fn detach(args: DetachArgs) -> Result<DetachedJob, RuntimeError> {
    let DetachArgs {
        exe,
        job_dir,
        spec,
        env,
    } = args;
    let job: JobId = job_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| fail(&"the job directory has no name"))?
        .parse()
        .map_err(|e| fail(&e))?;
    let log_path = job_dir.join(LOG_FILE);
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| fail(&e))?;
    let log_err = log.try_clone().map_err(|e| fail(&e))?;
    let mut child = Command::new(&exe)
        .arg("supervise")
        .arg("--job")
        // BORROW: .as_str() names the job on the command line
        .arg(job.as_str())
        .env_clear()
        .envs(env.iter().map(|(name, value)| (name, value)))
        .stdin(Stdio::piped())
        .stdout(log)
        .stderr(log_err)
        .spawn()
        .map_err(|e| fail(&e))?;
    let pid = child.id();
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&spec).map_err(|e| fail(&e))?;
    }
    let dir = JobDir::new(job.clone(), job_dir.clone()); // BORROW: the dir owns its names
    let started = Instant::now();
    loop {
        if JobLock::probe(&dir).map_err(|e| fail(&e))? == LockState::Held {
            return Ok(DetachedJob { pid, job });
        }
        let exited = matches!(child.try_wait(), Ok(Some(_)));
        if exited || started.elapsed() >= START_DEADLINE {
            return Err(RuntimeError::DidNotStart {
                job_dir: job_dir.display().to_string(),
                log: log_path.display().to_string(),
            });
        }
        std::thread::sleep(POLL);
    }
}
