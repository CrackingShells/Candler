//! Ending a child: by exit, by deadline or by a stop request, through the escalation ladder.
//!
//! The ladder has these steps, each reported as a `process.stop_step` event with an outcome:
//!
//! 1. `pre_stop`: the harness's own stop action, a command or a loopback HTTP call, bounded by
//!    the grace, then the spawn request's [`StopHook`] when one is attached. A failed or
//!    timed-out pre-stop continues the escalation.
//! 2. `signal`: the stop signal to the whole process group, then up to the grace for the leader
//!    to exit.
//! 3. `terminate_group`: `SIGTERM` to the group, then up to the grace (at most one second) for the
//!    group to empty.
//! 4. `kill_group`: `SIGKILL` to the group, then up to two seconds for the group to empty.
//! 5. `sweep`: the injected [`StraySweeper`] finds and kills what left the group; the survivors
//!    are reported as `run.stray`.
//! 6. `destroy_container`: the hook's `destroy`, only when the ladder ended the child (not when
//!    it exited by itself), reported with what was destroyed and refused.
//!
//! A child that exits by itself skips the first four steps. Its group is still ended, with
//! `terminate_group` and `kill_group`, if members remain, and the sweep always runs. A step that
//! cannot be reported (a clock or schema failure) never stops the ladder: the first such failure
//! is returned once the ladder is done.
//!
//! The process group id of a child is reserved while any member lives, so a group that exists is
//! the child's. When no member lives the ladder does not signal the number.

use std::fmt;
use std::os::unix::process::ExitStatusExt as _;
use std::path::Path;
use std::process::ExitStatus as StdExitStatus;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use candler_harness::spec::{
    Action, CommandSpec, EnvValue, HttpMethod, ProcessEnd, ProcessExit, StopSignal, StopSpec,
};
use candler_schemas::container::SweepScope as WireScope;
use candler_schemas::execution::{EventPayload, StopStep, StrayProcess};
use candler_schemas::primitives::{Marker, ReasonCode, RunKey, Timestamp};
use candler_schemas::process::{ChildRecord, ExitStatus, ProcessStart};
use rustix::process::Signal;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep, sleep_until, timeout, timeout_at};

use super::hook::{StopHook, StopHookCtx};
use super::output::Pumped;
use super::platform::{alive, process_start};
use super::signal::{Guard, SignalError, group_exists};
use super::spawn::{Child, Pumps, SpawnRequest};
use super::{EventSink, SuperviseError, Supervisor, http, now};
use crate::frames::RawFileInfo;

/// The longest `terminate_group` waits for the group to empty.
const TERMINATE_WAIT_CAP: Duration = Duration::from_secs(1);

/// How long `kill_group` waits for the group to empty, and how long a killed leader is waited for.
const KILL_WAIT: Duration = Duration::from_secs(2);

/// How often the ladder looks at the leader and the group while it waits.
const POLL: Duration = Duration::from_millis(10);

/// Finds and kills the processes of a run that left their process group, which a group signal
/// cannot reach (a harness starts descendants in new sessions). The execution milestone provides
/// the `/proc` and same-user scans; the supervisor only calls it.
pub trait StraySweeper: Send + Sync {
    /// Finds every process whose environment carries `marker` in `CANDLER_RUN`, kills it, and
    /// returns each by process id and the start time read before the kill.
    fn sweep(&self, marker: &Marker) -> Vec<StrayProcess>;
}

/// A request, from any task, that a child be stopped. Cloning it gives another handle to the same
/// child.
#[derive(Clone)]
pub struct StopHandle {
    switch: Arc<watch::Sender<bool>>,
}

impl StopHandle {
    /// Asks for the stop. The first request starts the escalation; later ones change nothing.
    pub fn request(&self) {
        self.switch.send_replace(true);
    }
}

impl fmt::Debug for StopHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StopHandle").finish_non_exhaustive()
    }
}

/// How a child ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Ended {
    /// The child exited on its own.
    Exited,
    /// The child was still running at its deadline and was ended.
    Deadline,
    /// A stop was requested and the child was ended.
    Stopped,
}

impl From<Ended> for ProcessEnd {
    fn from(ended: Ended) -> Self {
        match ended {
            Ended::Exited => Self::Exited,
            Ended::Deadline => Self::Deadline,
            Ended::Stopped => Self::Stopped,
        }
    }
}

/// One step of the ladder that was taken: which, when it began, and what came of it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct StepRecord {
    /// The step.
    pub step: StopStep,
    /// When the step began.
    pub at: Timestamp,
    /// What came of it, in a short phrase such as `no sweeper`.
    pub outcome: String,
}

impl StepRecord {
    /// A step record.
    #[must_use]
    pub const fn new(step: StopStep, at: Timestamp, outcome: String) -> Self {
        Self { step, at, outcome }
    }
}

/// What a child left behind when it ended.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Exit {
    /// The exit code or the signal.
    pub status: ExitStatus,
    /// Whether the child exited, hit its deadline or was stopped.
    pub ended: Ended,
    /// The steps of the ladder that were taken, in order.
    pub steps: Vec<StepRecord>,
    /// Whether the process group was empty afterwards. It covers the group only: a descendant in
    /// another session is the sweeper's. A zombie leader whose parent does not reap it keeps the
    /// group non-empty.
    pub group_empty: bool,
    /// The raw frame files of the framed streams that finished, standard output first. This and
    /// `stderr_tail` go beyond the status, the steps and `group_empty` that the exit of a stop
    /// strictly needs: the caller that waits is the one that holds the streams, so they are
    /// returned here and not through another call.
    pub raw: Vec<RawFileInfo>,
    /// The last 64 KiB of a framed or file standard error; empty for a pipe or no output.
    pub stderr_tail: Vec<u8>,
}

impl Exit {
    /// An exit.
    #[must_use]
    pub const fn new(
        status: ExitStatus,
        ended: Ended,
        steps: Vec<StepRecord>,
        group_empty: bool,
        raw: Vec<RawFileInfo>,
        stderr_tail: Vec<u8>,
    ) -> Self {
        Self {
            status,
            ended,
            steps,
            group_empty,
            raw,
            stderr_tail,
        }
    }
}

impl fmt::Debug for Exit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Exit")
            .field("status", &self.status)
            .field("ended", &self.ended)
            .field("steps", &self.steps)
            .field("group_empty", &self.group_empty)
            .field("raw", &self.raw)
            .field("stderr_tail", &self.stderr_tail.len())
            .finish()
    }
}

impl From<Exit> for ProcessExit {
    fn from(exit: Exit) -> Self {
        Self::new(exit.status, exit.ended.into(), exit.stderr_tail)
    }
}

/// A probe that ran to its end: its exit and what it printed.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Captured {
    /// How the probe ended.
    pub exit: Exit,
    /// The first bytes of standard output, up to the limit.
    pub stdout: Vec<u8>,
    /// The first bytes of standard error, up to the limit.
    pub stderr: Vec<u8>,
}

impl Captured {
    /// A captured probe.
    #[must_use]
    pub const fn new(exit: Exit, stdout: Vec<u8>, stderr: Vec<u8>) -> Self {
        Self {
            exit,
            stdout,
            stderr,
        }
    }
}

impl fmt::Debug for Captured {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Captured")
            .field("exit", &self.exit)
            .field("stdout", &self.stdout.len())
            .field("stderr", &self.stderr.len())
            .finish()
    }
}

/// How much of the ladder to climb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Every step: the child may still be running.
    Full,
    /// The leader has exited: end the group if members remain, then sweep.
    GroupOnly,
    /// Only the sweep: the leader named by a record is not the process that was spawned.
    SweepOnly,
}

/// The leader of the group being ended.
pub(crate) enum Leader<'a> {
    /// A child of this process, reaped through its handle.
    Owned {
        /// The process.
        process: &'a mut tokio::process::Child,
        /// Its status, once it was reaped.
        status: Option<StdExitStatus>,
    },
    /// A process known by its record alone, which this process does not parent.
    Record {
        /// The process id.
        pid: u32,
        /// The start time the record holds.
        start: &'a ProcessStart,
    },
}

impl Leader<'_> {
    /// Whether the leader has exited, reaping it if it is a child of this process.
    fn exited(&mut self) -> bool {
        match self {
            Self::Owned { process, status } => {
                if status.is_some() {
                    return true;
                }
                if let Ok(Some(done)) = process.try_wait() {
                    *status = Some(done);
                    return true;
                }
                false
            }
            // A zombie reads as running until its parent reaps it.
            Self::Record { pid, start } => !alive(*pid, start),
        }
    }
}

/// What a climb of the ladder did.
#[derive(Debug, Default)]
pub(crate) struct Climb {
    /// The steps taken, in order.
    pub(crate) steps: Vec<StepRecord>,
    /// What the sweep found and killed.
    pub(crate) strays: Vec<StrayProcess>,
    /// The first failure to report a step; the ladder went on regardless.
    pub(crate) error: Option<SuperviseError>,
}

impl Climb {
    fn fail(&mut self, error: SuperviseError) {
        if self.error.is_none() {
            self.error = Some(error);
        }
    }
}

/// The ladder for one child, with what it needs to report and to run a pre-stop command.
pub(crate) struct Ladder<'a> {
    /// Where the steps are reported.
    // TRAIT_OBJECT: the sink is chosen by the job runner and by tests
    pub(crate) sink: &'a dyn EventSink,
    /// The sweep, if one is injected.
    // TRAIT_OBJECT: the sweeper is injected by the execution milestone
    pub(crate) sweeper: Option<&'a Arc<dyn StraySweeper>>,
    /// The only path to a signal.
    pub(crate) guard: &'a Guard,
    /// The record of the child.
    pub(crate) record: &'a ChildRecord,
    /// The run the events belong to.
    pub(crate) run: Option<&'a RunKey>,
    /// How the child is stopped.
    pub(crate) stop: &'a StopSpec,
    /// The environment a pre-stop command starts from; empty when the child is known by its
    /// record alone.
    pub(crate) env: &'a [(String, String)],
    /// The working directory a pre-stop command's relative directory is joined to.
    pub(crate) cwd: Option<&'a Path>,
    /// What the caller adds to the stop, if it attached a hook to the request.
    // TRAIT_OBJECT: the hook is chosen by the run that attaches it
    pub(crate) hook: Option<&'a Arc<dyn StopHook>>,
    /// How the child came to be ended. The hook's `destroy` runs only when it is not
    /// [`Ended::Exited`]: after a normal exit the caller destroys, in order.
    pub(crate) ended: Ended,
}

/// The signal a stop request names.
#[allow(
    clippy::wildcard_enum_match_arm,
    clippy::match_same_arms,
    reason = "StopSignal is non-exhaustive; a signal this core does not know falls back to TERM, which Term names too"
)]
fn signal_of(signal: StopSignal) -> Signal {
    match signal {
        StopSignal::Term => Signal::TERM,
        StopSignal::Int => Signal::INT,
        StopSignal::Hup => Signal::HUP,
        // EXPLICIT: an unknown stop signal is ended the way the default is, with TERM
        _ => Signal::TERM,
    }
}

fn method_name(method: HttpMethod) -> Option<&'static str> {
    [
        (HttpMethod::Get, "GET"),
        (HttpMethod::Post, "POST"),
        (HttpMethod::Put, "PUT"),
        (HttpMethod::Delete, "DELETE"),
    ]
    .into_iter()
    .find_map(|(known, name)| (known == method).then_some(name))
}

/// Says how a process ended, for a step outcome.
fn describe(status: StdExitStatus) -> String {
    if let Some(code) = status.code() {
        format!("command exited with code {code}")
    } else if let Some(signal) = status.signal() {
        format!("command killed by signal {signal}")
    } else {
        "command ended".to_owned() // BORROW: .to_owned() makes the owned outcome from a literal
    }
}

/// The status in the form the schemas carry.
fn wire_status(status: StdExitStatus) -> ExitStatus {
    if let Some(code) = status.code() {
        ExitStatus::Code { code }
    } else if let Some(signal) = status.signal() {
        ExitStatus::Signal { signal }
    } else {
        ExitStatus::Code { code: -1 }
    }
}

impl Ladder<'_> {
    /// Climbs the ladder as far as `mode` says, reporting each step.
    pub(crate) async fn climb(&self, leader: &mut Leader<'_>, mode: Mode) -> Climb {
        let mut climb = Climb::default();
        let group = self.record.pgid();
        match mode {
            Mode::Full => {
                if let Some(action) = &self.stop.pre_stop {
                    self.pre_stop(action, &mut climb).await;
                }
                if let Some(hook) = self.hook {
                    self.hook_pre_stop(hook, &mut climb).await;
                }
                self.signal(leader, &mut climb).await;
                self.terminate(leader, &mut climb).await;
                self.kill(leader, &mut climb).await;
            }
            Mode::GroupOnly => {
                if group_exists(group) {
                    self.terminate(leader, &mut climb).await;
                    self.kill(leader, &mut climb).await;
                }
            }
            Mode::SweepOnly => {}
        }
        self.sweep(&mut climb).await;
        if self.ended != Ended::Exited
            && let Some(hook) = self.hook
        {
            self.hook_destroy(hook, &mut climb).await;
        }
        climb
    }

    /// What a hook is told about the child being ended.
    fn hook_ctx(&self) -> StopHookCtx {
        StopHookCtx::new(
            self.run.cloned(),            // BORROW: .cloned() gives the context its own run key
            self.record.marker().clone(), // BORROW: .clone() gives the context its own marker
            self.ended,
        )
    }

    /// Runs the hook's `pre_stop`; a failure is the step's outcome and the ladder goes on.
    async fn hook_pre_stop(&self, hook: &Arc<dyn StopHook>, climb: &mut Climb) {
        let started = now();
        let ctx = self.hook_ctx();
        let outcome = match hook.pre_stop(&ctx).await {
            Ok(()) => "hook pre_stop done".to_owned(), // BORROW: .to_owned() makes the owned outcome from a literal
            Err(error) => format!("hook pre_stop failed: {error}"),
        };
        self.log(climb, StopStep::PreStop, started, outcome);
    }

    /// Runs the hook's `destroy` after the sweep. A failure is the step's outcome and the ladder
    /// is done anyway. A sweep that cannot say it found everything is also an event of its own.
    async fn hook_destroy(&self, hook: &Arc<dyn StopHook>, climb: &mut Climb) {
        let started = now();
        let ctx = self.hook_ctx();
        let outcome = match hook.destroy(&ctx).await {
            Ok(swept) => {
                let mut text = format!(
                    "destroyed {} container(s), refused {}",
                    swept.destroyed.len(),
                    swept.refused.len()
                );
                if swept.unknown {
                    text.push_str("; the sweep is incomplete");
                    self.sweep_incomplete(climb);
                }
                text
            }
            Err(error) => format!("hook destroy failed: {error}"),
        };
        self.log(climb, StopStep::DestroyContainer, started, outcome);
    }

    /// Reports that the container sweep of this child's run could not say it found everything.
    fn sweep_incomplete(&self, climb: &mut Climb) {
        match "sweep_unknown".parse::<ReasonCode>() {
            Ok(reason) => self.sink.emit(
                self.run,
                EventPayload::container_sweep_incomplete(
                    self.run.cloned(), // BORROW: .cloned() gives the event its own run key
                    WireScope::Run,
                    reason,
                ),
            ),
            Err(error) => climb.fail(SuperviseError::Event {
                detail: error.to_string(),
            }),
        }
    }

    /// Reports a step that began at `started` and ended with `outcome`.
    fn log(
        &self,
        climb: &mut Climb,
        step: StopStep,
        started: Result<Timestamp, SuperviseError>,
        outcome: String,
    ) {
        let at = match started {
            Ok(at) => at,
            Err(error) => {
                climb.fail(error);
                return;
            }
        };
        let payload = EventPayload::process_stop_step(
            self.record.pid(),
            self.record.start().clone(), // BORROW: .clone() gives the event its own start
            step,
            at.clone(),            // BORROW: .clone() gives the event its own time
            Some(outcome.clone()), // BORROW: .clone() gives the event its own outcome
        );
        match payload {
            Ok(payload) => self.sink.emit(self.run, payload),
            Err(error) => climb.fail(SuperviseError::Event {
                detail: error.to_string(),
            }),
        }
        climb.steps.push(StepRecord::new(step, at, outcome));
    }

    async fn pre_stop(&self, action: &Action, climb: &mut Climb) {
        let started = now();
        let grace = self.stop.grace;
        let outcome = if let Action::Command(spec) = action {
            self.run_command(spec, grace).await
        } else if let Action::Http {
            method, url, body, ..
        } = action
        {
            match method_name(*method) {
                Some(name) => http::call(name, url, body.as_deref(), grace).await,
                None => "refused: unknown method".to_owned(), // BORROW: .to_owned() makes the owned outcome from a literal
            }
        } else {
            "refused: unknown action".to_owned() // BORROW: .to_owned() makes the owned outcome from a literal
        };
        self.log(climb, StopStep::PreStop, started, outcome);
    }

    /// Runs a pre-stop command in a group of its own, killing the group if the grace passes.
    async fn run_command(&self, spec: &CommandSpec, grace: Duration) -> String {
        let Some(program) = spec.argv().first() else {
            return "refused: empty argv".to_owned(); // BORROW: .to_owned() makes the owned outcome from a literal
        };
        let mut command = Command::new(program);
        command
            .args(spec.argv().iter().skip(1))
            .env_clear()
            .envs(self.env.iter().map(|(name, value)| (name, value)));
        // The command starts from the child's own environment, which may hold secrets the
        // request carried, and from the marker. Of the command's own overlay, only literal values
        // are set: an overlay entry that names a secret is skipped.
        for entry in spec.env_overlay() {
            if let EnvValue::Literal(value) = &entry.value {
                command.env(&entry.name, value);
            }
        }
        command.env("CANDLER_RUN", self.record.marker().to_string());
        match (self.cwd, spec.cwd()) {
            // BORROW: explicit .as_str() instead of Deref coercion
            (Some(base), Some(relative)) => command.current_dir(base.join(relative.as_str())),
            (Some(base), None) => command.current_dir(base),
            // BORROW: explicit .as_str() instead of Deref coercion
            (None, Some(relative)) => command.current_dir(relative.as_str()),
            (None, None) => &mut command,
        };
        command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .process_group(0);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => return format!("command failed to start: {:?}", error.kind()),
        };
        let group = child.id();
        if let Some(pid) = group {
            // The command leads a group of its own, which the guard may signal while it runs.
            let start = process_start(pid);
            let taken = !matches!(start, ProcessStart::Unavailable)
                && self.guard.register(pid, start).is_ok();
            if !taken {
                // The guard cannot signal what it cannot identify. The command is still this
                // process's own unreaped child, so the handle ends it, as in `Supervisor::spawn`.
                // EXPLICIT: handle kill of this process's own unreaped child; no pid or group number is passed
                let _ = child.start_kill();
                let _ = timeout(KILL_WAIT, child.wait()).await;
                return "command refused: its identity could not be recorded".to_owned(); // BORROW: .to_owned() makes the owned outcome from a literal
            }
        }
        let outcome = match timeout(grace, child.wait()).await {
            Ok(Ok(status)) => describe(status),
            Ok(Err(_)) => "command wait failed".to_owned(), // BORROW: .to_owned() makes the owned outcome from a literal
            Err(_) => {
                if let Some(group) = group {
                    // The group may have emptied meanwhile, which is what the kill is after.
                    let _ = self.guard.signal_group(group, Signal::KILL);
                }
                let _ = timeout(KILL_WAIT, child.wait()).await;
                "command timed out and was killed".to_owned() // BORROW: .to_owned() makes the owned outcome from a literal
            }
        };
        if let Some(pid) = group {
            // The command's leader has been reaped. What it left in its group is ended, through the
            // guard, which signals a reaped leader's group only while it has members; a group that
            // is already empty reads as gone.
            let _ = self.guard.signal_group(pid, Signal::KILL);
            self.guard.forget(pid);
        }
        outcome
    }

    async fn signal(&self, leader: &mut Leader<'_>, climb: &mut Climb) {
        let started = now();
        let outcome = match self
            .guard
            .signal_group(self.record.pgid(), signal_of(self.stop.signal))
        {
            Ok(()) => {
                if wait_leader(leader, self.stop.grace).await {
                    "signal sent; leader exited within grace"
                } else {
                    "signal sent; leader still running after grace"
                }
                .to_owned() // BORROW: .to_owned() makes the owned outcome from a literal
            }
            Err(SignalError::Gone) => "group already gone".to_owned(), // BORROW: .to_owned() makes the owned outcome from a literal
            Err(error) => format!("signal not sent, {error}"),
        };
        self.log(climb, StopStep::Signal, started, outcome);
    }

    async fn terminate(&self, leader: &mut Leader<'_>, climb: &mut Climb) {
        let started = now();
        let group = self.record.pgid();
        let wait = self.stop.grace.min(TERMINATE_WAIT_CAP);
        let outcome = end_group(self.guard, leader, group, Signal::TERM, wait, "SIGTERM").await;
        self.log(climb, StopStep::TerminateGroup, started, outcome);
    }

    async fn kill(&self, leader: &mut Leader<'_>, climb: &mut Climb) {
        let started = now();
        let group = self.record.pgid();
        let outcome = end_group(
            self.guard,
            leader,
            group,
            Signal::KILL,
            KILL_WAIT,
            "SIGKILL",
        )
        .await;
        self.log(climb, StopStep::KillGroup, started, outcome);
    }

    async fn sweep(&self, climb: &mut Climb) {
        let started = now();
        let (outcome, strays) = if let Some(sweeper) = self.sweeper {
            let sweeper = Arc::clone(sweeper); // TRAIT_OBJECT: the sweeper runs on a blocking thread
            let marker = self.record.marker().clone(); // BORROW: .clone() gives the blocking thread its own marker
            // The sweeper runs on a blocking thread that cannot be cancelled, so the wait for it is
            // bounded by the grace; a sweeper that overruns is left to finish and its finds are
            // unknown.
            let sweeping = tokio::task::spawn_blocking(move || sweeper.sweep(&marker));
            match timeout(self.stop.grace, sweeping).await {
                Ok(Ok(found)) => (format!("swept {} process(es)", found.len()), found),
                Ok(Err(_)) => ("sweeper failed".to_owned(), Vec::new()), // BORROW: .to_owned() makes the owned outcome from a literal
                Err(_) => (
                    "sweeper timed out; what it found is unknown".to_owned(), // BORROW: .to_owned() makes the owned outcome from a literal
                    Vec::new(),
                ),
            }
        } else {
            ("no sweeper".to_owned(), Vec::new()) // BORROW: .to_owned() makes the owned outcome from a literal
        };
        self.log(climb, StopStep::Sweep, started, outcome);
        if !strays.is_empty() {
            // BORROW: .clone() gives the event its own list
            self.sink
                .emit(self.run, EventPayload::run_stray(strays.clone()));
        }
        climb.strays = strays;
    }
}

/// Waits up to `limit` for the leader to exit.
async fn wait_leader(leader: &mut Leader<'_>, limit: Duration) -> bool {
    let until = Instant::now() + limit;
    loop {
        if leader.exited() {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        sleep(POLL).await;
    }
}

/// Waits up to `limit` for the group to have no member, reaping the leader as it goes.
async fn wait_empty(leader: &mut Leader<'_>, group: u32, limit: Duration) -> bool {
    let until = Instant::now() + limit;
    loop {
        // A zombie leader keeps its group alive until it is reaped.
        leader.exited();
        if !group_exists(group) {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        sleep(POLL).await;
    }
}

/// Sends `signal` to the group if it has a member, then waits for it to empty.
async fn end_group(
    guard: &Guard,
    leader: &mut Leader<'_>,
    group: u32,
    signal: Signal,
    wait: Duration,
    name: &str,
) -> String {
    if !group_exists(group) {
        return "group already empty".to_owned(); // BORROW: .to_owned() makes the owned outcome from a literal
    }
    match guard.signal_group(group, signal) {
        Ok(()) | Err(SignalError::Gone) => {}
        Err(error) => return format!("signal not sent, {error}"),
    }
    if wait_empty(leader, group, wait).await {
        format!("group empty after {name}")
    } else {
        format!("group still has members after {name}")
    }
}

impl Child {
    /// A handle that asks for this child to be stopped, from any task.
    #[must_use]
    pub fn stop_handle(&self) -> StopHandle {
        StopHandle {
            switch: Arc::clone(&self.switch),
        }
    }

    /// Waits until the child exits, reaches its deadline or is asked to stop, then ends it and
    /// what it left in its group, and returns how it ended. `process.exited` is the last event.
    ///
    /// Output is drained for at most the grace after the process ends, so a descendant that holds
    /// a pipe open cannot hold the wait.
    ///
    /// # Errors
    /// Returns [`SuperviseError::Survived`] if the child is still running after `SIGKILL`,
    /// [`SuperviseError::Io`] if the child cannot be waited for or a raw frame file was lost, and
    /// the first [`SuperviseError::Event`] or [`SuperviseError::Clock`] met while reporting a
    /// step. In each case the child has been ended as far as the system allowed and
    /// `process.exited` was reported where the status is known.
    pub async fn wait(mut self) -> Result<Exit, SuperviseError> {
        let mut requested = self.switch.subscribe();
        let deadline = self.context.deadline;
        let sleeper = async {
            if let Some(at) = deadline {
                sleep_until(at).await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        let (natural, ended) = tokio::select! {
            biased;
            result = self.process.wait() => (Some(result), Ended::Exited),
            _ = requested.wait_for(|asked| *asked) => (None, Ended::Stopped),
            () = sleeper => (None, Ended::Deadline),
        };
        let status = match natural {
            Some(Ok(status)) => Some(status),
            Some(Err(source)) => {
                let _ = self
                    .context
                    .guard
                    .signal_group(self.record.pgid(), Signal::KILL);
                self.context.guard.forget(self.record.pid());
                return Err(SuperviseError::Io {
                    context: "cannot wait for the child",
                    source,
                });
            }
            None => None,
        };
        let mode = if ended == Ended::Exited {
            Mode::GroupOnly
        } else {
            Mode::Full
        };
        let ladder = Ladder {
            sink: &*self.context.sink,
            sweeper: self.context.sweeper.as_ref(),
            guard: &self.context.guard,
            record: &self.record,
            run: self.context.run.as_ref(),
            stop: &self.context.stop,
            env: self.context.env.as_slice(), // BORROW: explicit .as_slice() instead of Deref coercion
            cwd: self.context.cwd.as_deref(),
            hook: self.context.hook.as_ref(),
            ended,
        };
        let mut leader = Leader::Owned {
            process: &mut self.process,
            status,
        };
        let climb = ladder.climb(&mut leader, mode).await;
        let reaped = match leader {
            Leader::Owned {
                status: Some(done), ..
            } => Some(done),
            Leader::Owned {
                process,
                status: None,
            } => timeout(KILL_WAIT, process.wait())
                .await
                .ok()
                .and_then(Result::ok),
            // `wait` never climbs for a record; a record has no status to report.
            Leader::Record { .. } => None,
        };
        let Some(done) = reaped else {
            return Err(SuperviseError::Survived {
                pid: self.record.pid(),
            });
        };
        let status = wire_status(done);

        let grace = self.context.stop.grace;
        let drained = drain(&mut self.pumps, grace).await;
        let stderr_tail = drained.stderr_tail.unwrap_or_else(|| {
            self.context
                .stderr_file
                .as_ref()
                .map(super::output::FileTail::read)
                .unwrap_or_default()
        });
        let group_empty = !group_exists(self.record.pgid());
        if group_empty {
            // The leader is reaped and no member is left: nothing in this group may be signalled
            // again. A group that still has members stays, so that `stop` and `kill_all` can end
            // it.
            self.context.guard.forget(self.record.pid());
        }
        match EventPayload::process_exited(
            self.record.pid(),
            self.record.start().clone(), // BORROW: .clone() gives the event its own start
            status,
            group_empty,
        ) {
            Ok(payload) => self.context.sink.emit(self.context.run.as_ref(), payload),
            Err(error) => {
                return Err(SuperviseError::Event {
                    detail: error.to_string(),
                });
            }
        }
        if let Some(error) = climb.error {
            return Err(error);
        }
        if let Some(error) = drained.error {
            return Err(error);
        }
        Ok(Exit::new(
            status,
            ended,
            climb.steps,
            group_empty,
            drained.raw,
            stderr_tail,
        ))
    }

    /// Asks for the stop and waits for it: the child is ended by the ladder, and the exit says
    /// how.
    ///
    /// # Errors
    /// The errors of [`Child::wait`].
    pub async fn stop(self) -> Result<Exit, SuperviseError> {
        self.stop_handle().request();
        self.wait().await
    }
}

/// What draining the pumps found.
struct Drained {
    raw: Vec<RawFileInfo>,
    stderr_tail: Option<Vec<u8>>,
    error: Option<SuperviseError>,
}

/// Waits for `handle` until `limit`, then raises the cut-off and waits for it to finish.
async fn join_pump(
    mut handle: JoinHandle<Pumped>,
    limit: Instant,
    cutoff: &watch::Sender<bool>,
) -> Option<Pumped> {
    if let Ok(joined) = timeout_at(limit, &mut handle).await {
        joined.ok()
    } else {
        cutoff.send_replace(true);
        handle.await.ok()
    }
}

/// Joins the pumps of a child within `grace`, collecting the finished files.
async fn drain(pumps: &mut Pumps, grace: Duration) -> Drained {
    let limit = Instant::now() + grace;
    let mut drained = Drained {
        raw: Vec::new(),
        stderr_tail: None,
        error: None,
    };
    let stdout = pumps.stdout.take();
    let stderr = pumps.stderr.take();
    for (handle, is_stderr) in [(stdout, false), (stderr, true)] {
        let Some(handle) = handle else { continue };
        let Some(pumped) = join_pump(handle, limit, &pumps.cutoff).await else {
            drained.error.get_or_insert(SuperviseError::Io {
                context: "an output pump failed",
                source: std::io::Error::other("the pump task did not finish"),
            });
            continue;
        };
        if is_stderr {
            drained.stderr_tail = Some(pumped.tail);
        }
        match pumped.file {
            Some(Ok(info)) => drained.raw.push(info),
            Some(Err(source)) => {
                drained.error.get_or_insert(SuperviseError::Io {
                    context: "cannot write the raw frame file",
                    source,
                });
            }
            None => {}
        }
    }
    drained
}

/// Reads `reader` until end of file, keeping the first `limit` bytes in `kept`.
async fn collect<R: AsyncRead + Unpin>(mut reader: R, limit: usize, kept: Arc<Mutex<Vec<u8>>>) {
    let mut chunk = vec![0_u8; 8 * 1024];
    while let Ok(count) = reader.read(&mut chunk).await {
        let Some(bytes) = chunk.get(..count).filter(|bytes| !bytes.is_empty()) else {
            break;
        };
        let mut held = kept.lock().unwrap_or_else(PoisonError::into_inner);
        let room = limit.saturating_sub(held.len());
        held.extend_from_slice(bytes.get(..room.min(bytes.len())).unwrap_or_default());
    }
}

/// Takes what a collector kept.
fn take_kept(kept: &Mutex<Vec<u8>>) -> Vec<u8> {
    std::mem::take(&mut *kept.lock().unwrap_or_else(PoisonError::into_inner))
}

impl Supervisor {
    /// Runs a probe with both outputs piped and returns its exit and the first `limit` bytes of
    /// each output. The rest of an output is read and dropped. The collectors are given at most
    /// the grace after the child ends.
    ///
    /// Must be called within a Tokio runtime.
    ///
    /// # Errors
    /// The errors of [`Supervisor::spawn`] and of [`Child::wait`].
    pub async fn run_captured(
        &self,
        request: SpawnRequest,
        limit: usize,
    ) -> Result<Captured, SuperviseError> {
        let grace = request.grace();
        let mut child = self.spawn(request.piped())?;
        let out = Arc::new(Mutex::new(Vec::new()));
        let err = Arc::new(Mutex::new(Vec::new()));
        let readers: Vec<JoinHandle<()>> = [
            child
                .take_stdout()
                .map(|pipe| tokio::spawn(collect(pipe, limit, Arc::clone(&out)))),
            child
                .take_stderr()
                .map(|pipe| tokio::spawn(collect(pipe, limit, Arc::clone(&err)))),
        ]
        .into_iter()
        .flatten()
        .collect();
        let exit = child.wait().await;
        for mut reader in readers {
            if timeout(grace, &mut reader).await.is_err() {
                reader.abort();
            }
        }
        Ok(Captured::new(exit?, take_kept(&out), take_kept(&err)))
    }
}
