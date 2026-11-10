//! Helpers for the guards of the job runtime: builders and fixtures, never expected values.
#![allow(dead_code, reason = "each guard binary uses a subset of the helpers")]

use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Write as IoWrite;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use candler_core::jobs::model::{JobCause, JobControl, JobOutcome, JobSpec};
use candler_core::jobs::runtime::JobServices;
use candler_core::runner::Runtime;
use candler_core::runner::evidence::provenance::EvalInfra;
use candler_core::runner::exec::SkillTree;
use candler_core::supervise::platform::process_start;
use candler_core::supervise::signal::Signaller;
use candler_core::supervise::spawn::SpawnRequest;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::driver::DriverError;
use candler_harness::spec::{CommandSpec, PreparedFile, StopSignal, StopSpec};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::{EventPayload, JobEnd, TreatmentRef};
use candler_schemas::primitives::{Marker, RunKey};
use candler_schemas::process::ProcessStart;
use futures::future::BoxFuture;
use rustix::io::Errno;
use rustix::process::{Pid, Signal, kill_process, kill_process_group, test_kill_process};

use super::run_plan::{Facts, experiment, resolution};

/// The names of the variables the re-executed supervisor reads its work from.
pub const ENV_JOB: &str = "CANDLER_TEST_SUPERVISOR_JOB";
/// The store the supervisor child opens.
pub const ENV_STORE: &str = "CANDLER_TEST_STORE";
/// The directory the supervisor child's body writes its files in.
pub const ENV_WORK: &str = "CANDLER_TEST_WORK";

/// The marker of the children the fake body starts.
pub const BODY_MARKER: &str = "b32-body";

/// Keeps every event a supervisor emits, in order.
#[derive(Default)]
pub struct Recorder {
    events: Mutex<Vec<(Option<RunKey>, EventPayload)>>,
}

impl Recorder {
    /// The events so far.
    pub fn events(&self) -> Vec<(Option<RunKey>, EventPayload)> {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone() // BORROW: .clone() copies the events out of the lock
    }
}

impl EventSink for Recorder {
    fn emit(&self, run: Option<&RunKey>, payload: EventPayload) {
        // BORROW: .cloned() keeps the run key for the test to read
        let run = run.cloned();
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((run, payload));
    }
}

/// A skill tree the guards never ask for anything.
pub struct NoSkills;

impl SkillTree for NoSkills {
    fn archive_command(
        &self,
        _treatment: &TreatmentRef,
        _source: &Path,
    ) -> Result<CommandSpec, DriverError> {
        Err(DriverError::VersionOutput {
            // BORROW: the error owns its detail
            detail: "the guards read no skill tree".to_owned(),
        })
    }

    fn files(
        &self,
        _treatment: &TreatmentRef,
        _archive: &[u8],
    ) -> Result<Vec<PreparedFile>, DriverError> {
        Ok(Vec::new())
    }
}

/// What the harness gives every run, as stubs the guards never call.
pub fn services() -> JobServices {
    JobServices {
        // TRAIT_OBJECT: the driver is the harness's real one, which a guard never calls
        driver: Arc::new(ClaudeCodeDriver::new().expect("the driver builds from constants")),
        // TRAIT_OBJECT: the tree is a stub that holds no file
        skills: Arc::new(NoSkills),
        eval_infra: EvalInfra::default(),
    }
}

/// A spec of no runs, from the fixtures of the planner.
pub fn job_spec() -> JobSpec {
    let file = experiment(&["m"], 1);
    JobSpec {
        budget: file.budget().clone(), // BORROW: the spec owns its budget
        experiment: file,
        resolution: resolution(&Facts::base()),
        plans: Vec::new(),
        force: false,
        inputs: Vec::new(),
    }
}

/// Writes an executable `docker` that does nothing into `dir`, so a client that looks for one on
/// `PATH` finds one, and returns `dir`.
pub fn stub_docker(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).expect("a stub directory");
    let path = dir.join("docker");
    fs::write(&path, "#!/bin/sh\nexit 0\n").expect("a stub");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("a mode");
    dir.to_path_buf()
}

/// A signaller that writes each `(kind, target, signal)` to a file, one line, before it delivers
/// the signal. It is a test-side wrapper: the guard has already approved the target when it is
/// called.
pub struct LoggingSignaller {
    path: PathBuf,
}

impl LoggingSignaller {
    /// A signaller that logs to `path`.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Appends one `kind target signal` line to the signal log.
    fn log(&self, kind: &str, target: i32, signal: i32) {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .expect("the signal log");
        writeln!(file, "{kind} {target} {signal}").expect("a log line");
    }
}

impl Signaller for LoggingSignaller {
    fn signal_group(&self, group: Pid, signal: Signal) -> Result<(), Errno> {
        self.log("group", group.as_raw_nonzero().get(), signal.as_raw());
        kill_process_group(group, signal)
    }

    fn signal_process(&self, pid: Pid, signal: Signal) -> Result<(), Errno> {
        self.log("process", pid.as_raw_nonzero().get(), signal.as_raw());
        kill_process(pid, signal)
    }
}

/// The `(kind, target, signal)` lines of a signal log; none when the file is not there.
pub fn read_log(path: &Path) -> Vec<(String, i32, i32)> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut parts = line.split(' ');
            // BORROW: the entry owns its kind
            let kind = parts.next()?.to_owned();
            let target = parts.next()?.parse().ok()?;
            let signal = parts.next()?.parse().ok()?;
            Some((kind, target, signal))
        })
        .collect()
}

/// Sends SIGTERM to the process `child` is, and to nothing else: the pid is read from the handle
/// the test holds, and a pid at or below 1 or this test's own is refused.
pub fn term_child(child: &Child) {
    let pid = child.id();
    assert!(pid > 1 && pid != std::process::id(), "refused pid {pid}");
    let target = i32::try_from(pid)
        .ok()
        .and_then(Pid::from_raw)
        .expect("a process id");
    // EXPLICIT: single-pid TERM to the supervisor this test spawned and still holds a handle of
    kill_process(target, Signal::TERM).expect("the supervisor takes a signal");
}

/// A supervisor process the test spawned and holds. When the test ends with it still running,
/// even by a failed assertion, the drop ends it with the one signal a `Child` sends itself
/// (SIGKILL to its own pid) and reaps it, so that no supervisor outlives its test.
pub struct HeldChild(Child);

impl HeldChild {
    /// Takes over `child`.
    #[must_use]
    pub const fn new(child: Child) -> Self {
        Self(child)
    }
}

impl std::ops::Deref for HeldChild {
    type Target = Child;

    fn deref(&self) -> &Child {
        &self.0
    }
}

impl std::ops::DerefMut for HeldChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

impl Drop for HeldChild {
    fn drop(&mut self) {
        // EXPLICIT: Child::kill signals only the process this handle spawned; one already
        // reaped is a no-op
        drop(self.0.kill());
        drop(self.0.wait());
    }
}

/// Waits up to `limit` for `path` to exist and returns its text.
pub fn wait_for(path: &Path, limit: Duration) -> Option<String> {
    let until = Instant::now() + limit;
    loop {
        if let Ok(text) = fs::read_to_string(path) {
            return Some(text);
        }
        if Instant::now() >= until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Whether the process `pid` is gone, as the system answers a signal-0 probe, or is another
/// process than the one that started at `before`.
pub fn gone(pid: u32, before: &ProcessStart) -> bool {
    let Some(target) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return true;
    };
    match test_kill_process(target) {
        Ok(()) => process_start(pid) != *before,
        Err(error) => error == Errno::SRCH,
    }
}

/// Waits up to `limit` for every process of `starts` to be gone.
pub fn all_gone(starts: &[(u32, ProcessStart)], limit: Duration) -> bool {
    let until = Instant::now() + limit;
    loop {
        if starts.iter().all(|(pid, before)| gone(*pid, before)) {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The body of the supervisor child of B32: it starts two `sleep` children through a supervisor
/// that signals through the logging signaller, writes their pids, waits for `Killed`, writes
/// `killed-seen`, stops the children and returns `JobEnd::Completed`: the runtime decides the end.
pub fn killed_body(
    rt: Runtime,
    _spec: JobSpec,
    control: JobControl,
) -> BoxFuture<'static, JobOutcome> {
    Box::pin(async move {
        let work = PathBuf::from(std::env::var_os(ENV_WORK).expect("a work directory"));
        let logging = Arc::new(LoggingSignaller::new(work.join("signals.log")));
        let records = work.join("records");
        fs::create_dir_all(&records).expect("a record directory");
        let supervisor = Supervisor::new(Arc::clone(&rt.sink)).with_signaller(logging);
        let marker: Marker = BODY_MARKER.parse().expect("a marker");
        let mut children = Vec::new();
        let mut lines = String::new();
        for _ in 0..2 {
            let request = SpawnRequest::builder()
                // BORROW: .to_owned() makes the owned arguments
                .argv(vec!["sleep".to_owned(), "20".to_owned()])
                // BORROW: .to_owned() makes the owned name and value
                .env(vec![("PATH".to_owned(), "/usr/bin:/bin".to_owned())])
                .marker(marker.clone()) // BORROW: each request owns its marker
                .record_dir(records.clone()) // BORROW: each request owns its directory
                .stop(StopSpec::new(
                    None,
                    StopSignal::Term,
                    Duration::from_millis(500),
                ))
                .build()
                .expect("a request");
            let child = supervisor.spawn(request).expect("a child");
            writeln!(lines, "child {}", child.record().pid()).expect("a string");
            children.push(child);
        }
        // The test reads the pid file as soon as it exists: it appears whole, by a rename.
        let part = work.join("children.pids.part");
        fs::write(&part, lines).expect("the pid file");
        fs::rename(&part, work.join("children.pids")).expect("the pid file in place");
        let mut cancel = control.cancel.subscribe();
        loop {
            if matches!(*cancel.borrow_and_update(), Some(JobCause::Killed)) {
                break;
            }
            if cancel.changed().await.is_err() {
                break;
            }
        }
        fs::write(work.join("killed-seen"), "yes").expect("the marker file");
        for child in children {
            let _ = child.stop().await;
        }
        JobOutcome {
            end: JobEnd::Completed,
            runs: Vec::new(),
        }
    })
}

/// The path of an absolute `perl`, from `PATH` or the usual places.
pub fn perl() -> PathBuf {
    let from_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("perl"))
            .find(|path| path.is_absolute() && path.is_file())
    });
    from_path.unwrap_or_else(|| PathBuf::from("/usr/bin/perl"))
}

/// M1's `tree.sh`, rewritten so that its detached grandchild is started through
/// `env -i CANDLER_RUN=<marker> <perl>` and every process of it sleeps 20 s at most. Written to
/// `dir`; returns the path.
pub fn marked_tree(dir: &Path, marker: &str) -> PathBuf {
    let original = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("supervise")
        .join("tree.sh");
    let text = fs::read_to_string(&original).expect("tree.sh");
    let started = format!("env -i CANDLER_RUN={marker} {} -MPOSIX", perl().display());
    let rewritten = text
        .replace("perl -MPOSIX", &started)
        .replace("sleep 60", "sleep 20")
        .replace("q(sleep), q(60)", "q(/bin/sleep), q(20)");
    assert!(rewritten.contains("env -i CANDLER_RUN="), "tree.sh changed");
    assert!(rewritten.contains("q(20)"), "tree.sh changed");
    let path = dir.join("tree.sh");
    fs::write(&path, rewritten).expect("a wrapper");
    path
}
