//! The session runner: it keeps a harness's input open, follows the turns and the pending work the
//! driver's tracker reports, and closes the input once the agent has yielded with nothing pending.
//!
//! The close rule is [`CloseRule`], a pure state machine over the tracker's [`PendingState`] and
//! the readings of a clock it is handed: it never reads one itself. [`SessionWatch`] holds the
//! tracker and the rule behind one lock, so a frame and a close decision never interleave; the live
//! observer feeds it each stdout frame as the frame is flushed, and [`SessionRun`] reads the hook
//! log into it, archives the log as a raw stream and decides: wait, close the input, or end the run
//! at the turn cap of the cell.

use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use candler_harness::driver::HarnessDriver;
use candler_harness::spec::{
    Cell, FileRoot, Framing, PendingState, SessionClose, SessionRecord, SessionSpec, SessionTracker,
};
use candler_schemas::primitives::FieldPath;
use candler_schemas::raw::{Clock as FrameClock, ClockAnchor, StreamName};
use candler_schemas::trace::{Cap, EndedBy};
use candler_schemas::values::JsonValue;
use serde_json::{Number, Value};
use tokio::io::AsyncWriteExt as _;
use tokio::process::ChildStdin;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

pub use crate::runner::evidence::session::{Idle, SessionEnd, SessionMeasures};

use crate::experiment::paths;
use crate::frames::{FrameWriter, RawFileInfo};
use crate::isolation::IsolationError;
use crate::resolve::steps::session::{DEFAULT_IDLE_S, MODE_SESSION, settle};
use crate::supervise::now;
use crate::supervise::spawn::Child;
use crate::supervise::stop::StopHandle;

/// How long a close waits after the turn's end, whatever `runner.idle_s` says: a hook line is
/// written before the turn's `result` but can be visible on the host a few milliseconds after it,
/// and a close decided before the line is read could miss a wake-up the line lists.
pub const HOOK_SETTLE: Duration = Duration::from_millis(250);

/// How often the runner reads the hook log and decides.
const POLL: Duration = Duration::from_millis(50);

/// A reading of a monotonic clock: the time since the clock's origin.
// TRAIT_OBJECT: the clock is the run's own, or a test's
pub type Clock = Arc<dyn Fn() -> Duration + Send + Sync>;

/// A clock whose origin is now and which reads the monotonic time since.
#[must_use]
pub fn monotonic_clock() -> Clock {
    let origin = Instant::now();
    Arc::new(move || origin.elapsed())
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---------------------------------------------------------------------------------------------
// The cell's session settings.
// ---------------------------------------------------------------------------------------------

/// What a cell asks of a session run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SessionConfig {
    /// How long the session stays open after the agent yields with nothing pending.
    pub idle: Idle,
    /// The turn limit across the session, when the cell has one.
    pub turn_limit: Option<u64>,
}

fn plan_error(detail: String) -> IsolationError {
    IsolationError::Plan { detail }
}

/// The value of the cell at `path`.
fn cell_value<'a>(cell: &'a Cell<'_>, path: &str) -> Result<Option<&'a Value>, IsolationError> {
    let field: FieldPath =
        path.parse()
            .map_err(|error: candler_schemas::primitives::FormatError| {
                plan_error(error.to_string()) // BORROW: .to_string() gives owned text
            })?;
    Ok(cell.get(&field).map(JsonValue::as_value))
}

/// The session settings of a cell, or `None` for a headless cell.
///
/// The rule is [`settle`], which resolve and planning have applied already: a refusal here is a
/// backstop for a cell that reached the run without them.
///
/// # Errors
/// [`IsolationError::Plan`] when the cell fails the session rule or its turn limit is not a whole
/// number.
pub fn session_config(cell: &Cell<'_>) -> Result<Option<SessionConfig>, IsolationError> {
    let mode = cell_value(cell, paths::RUNNER_MODE)?;
    let idle = cell_value(cell, paths::RUNNER_IDLE_S)?;
    settle(mode, idle, None).map_err(|refusal| plan_error(refusal.detail().to_owned()))?; // BORROW: owned text
    if mode.and_then(Value::as_str) != Some(MODE_SESSION) {
        return Ok(None);
    }
    let idle = match idle {
        Some(Value::Number(number)) => Idle::Seconds(number.as_u64().unwrap_or(DEFAULT_IDLE_S)),
        Some(Value::String(_)) => Idle::Never,
        _ => Idle::Seconds(DEFAULT_IDLE_S),
    };
    let turn_limit = match cell_value(cell, paths::BUDGET_PER_RUN_TURNS)? {
        None => None,
        Some(value) => Some(value.as_u64().ok_or_else(|| {
            plan_error("budget.per_run.turns is not a whole number".to_owned()) // BORROW: owned text
        })?),
    };
    Ok(Some(SessionConfig { idle, turn_limit }))
}

// ---------------------------------------------------------------------------------------------
// The close rule.
// ---------------------------------------------------------------------------------------------

/// When to close a session's input: a pure state machine over what the tracker reports and the
/// readings of a clock it is handed.
///
/// The input may close only while the agent is quiet: its latest turn has ended, the tracker has
/// read what is pending and nothing is. The idle time counts from the moment the session became
/// quiet, which is the turn's end in the usual case; an event that does not change the tracker's
/// state does not move it, and a new turn, a new turn end, pending work or a state the tracker
/// could not read takes it away. It never fires with `Idle::Never`.
#[derive(Debug, Clone)]
pub struct CloseRule {
    idle: Idle,
    settle: Duration,
    turns: u64,
    quiet_since: Option<Duration>,
}

impl CloseRule {
    /// A rule that waits `idle` after the session becomes quiet, and at least `settle`.
    #[must_use]
    pub const fn new(idle: Idle, settle: Duration) -> Self {
        Self {
            idle,
            settle,
            turns: 0,
            quiet_since: None,
        }
    }

    /// Takes the tracker's state as read at `at`.
    pub fn observe(&mut self, at: Duration, state: &PendingState) {
        if state.turns != self.turns {
            self.turns = state.turns;
            self.quiet_since = None;
        }
        let quiet = state.turns > 0 && state.turn_ended && state.known && state.pending.is_empty();
        if !quiet {
            self.quiet_since = None;
        } else if self.quiet_since.is_none() {
            self.quiet_since = Some(at);
        }
    }

    /// The reading at which the input closes if nothing changes, or `None` while the session is
    /// not quiet or never closes.
    #[must_use]
    pub fn due_at(&self) -> Option<Duration> {
        let since = self.quiet_since?;
        match self.idle {
            Idle::Never => None,
            Idle::Seconds(seconds) => {
                let wait = Duration::from_secs(seconds).max(self.settle);
                Some(since.saturating_add(wait))
            }
        }
    }

    /// Whether the input closes at the reading `now`.
    #[must_use]
    pub fn is_due(&self, now: Duration) -> bool {
        self.due_at().is_some_and(|due| now >= due)
    }
}

/// What the runner does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Verdict {
    /// Nothing yet.
    Wait,
    /// Close the input.
    Close,
    /// The turns of the session exceed the cell's limit: end the run at the cap.
    Cap,
}

// ---------------------------------------------------------------------------------------------
// The watch.
// ---------------------------------------------------------------------------------------------

struct Inner {
    // TRAIT_OBJECT: the tracker is the harness driver's
    tracker: Box<dyn SessionTracker>,
    rule: CloseRule,
    state: PendingState,
    last_turn_end: Option<Duration>,
    turns_sum: u64,
    usage_sum: Option<Value>,
    turn_limit: Option<u64>,
}

/// What a `result` frame says of its turn.
struct ResultFacts {
    turns: Option<u64>,
    usage: Option<Value>,
}

/// The turns and the usage of `frame` when it is a `result` frame.
fn result_of(frame: &[u8]) -> Option<ResultFacts> {
    let Ok(Value::Object(object)) = serde_json::from_slice::<Value>(frame) else {
        return None;
    };
    if object.get("type").and_then(Value::as_str) != Some("result") {
        return None;
    }
    Some(ResultFacts {
        turns: object.get("num_turns").and_then(Value::as_u64),
        usage: object.get("usage").cloned(), // BORROW: the facts own their copy
    })
}

/// Adds `new`, one result's usage, to the running sum: whole numbers add, objects add field by
/// field, and anything else (a service level, a speed, a list) takes the latest value.
pub fn add_usage(sum: &mut Option<Value>, new: &Value) {
    match sum {
        None => *sum = Some(new.clone()), // BORROW: the sum owns its copy
        Some(total) => merge_usage(total, new),
    }
}

fn merge_usage(total: &mut Value, new: &Value) {
    match (total, new) {
        (Value::Object(total), Value::Object(new)) => {
            for (key, value) in new {
                match total.get_mut(key) {
                    Some(held) => merge_usage(held, value),
                    None => {
                        total.insert(key.clone(), value.clone()); // BORROW: the sum owns its copy
                    }
                }
            }
        }
        (Value::Number(held), Value::Number(added)) if held.is_u64() && added.is_u64() => {
            let sum = held
                .as_u64()
                .unwrap_or(0)
                .saturating_add(added.as_u64().unwrap_or(0));
            *held = Number::from(sum);
        }
        (held, other) => *held = other.clone(), // BORROW: the sum owns its copy
    }
}

/// The tracker and the close rule behind one lock, fed by the live observer and the hook log.
pub struct SessionWatch {
    inner: Mutex<Inner>,
    clock: Clock,
    wake: Notify,
    closed: AtomicBool,
    forced: AtomicBool,
    capped: AtomicBool,
}

impl SessionWatch {
    /// A watch over `tracker` with the rule `rule`, reading `clock`.
    #[must_use]
    pub fn new(
        tracker: Box<dyn SessionTracker>, // TRAIT_OBJECT: the tracker is the harness driver's
        rule: CloseRule,
        turn_limit: Option<u64>,
        clock: Clock,
    ) -> Arc<Self> {
        let state = tracker.state();
        Arc::new(Self {
            inner: Mutex::new(Inner {
                tracker,
                rule,
                state,
                last_turn_end: None,
                turns_sum: 0,
                usage_sum: None,
                turn_limit,
            }),
            clock,
            wake: Notify::new(),
            closed: AtomicBool::new(false),
            forced: AtomicBool::new(false),
            capped: AtomicBool::new(false),
        })
    }

    /// Reads one record into the tracker and the rule, at the clock's reading.
    fn take(&self, record: SessionRecord<'_>, result: Option<ResultFacts>) -> bool {
        let mut inner = locked(&self.inner);
        // The clock is read under the lock, so the readings follow the order the records are taken.
        let at = (self.clock)();
        // EXPLICIT: a record the tracker cannot read leaves its state unknown, which it reports
        let readable = inner.tracker.feed(record).is_ok();
        let state = inner.tracker.state();
        if state.turns > inner.state.turns {
            inner.last_turn_end = Some(at);
            // A result that gives no turn count adds none: a count is never carried over from
            // an earlier result.
            if let Some(ResultFacts { turns, usage }) = result {
                if let Some(counted) = turns {
                    inner.turns_sum = inner.turns_sum.saturating_add(counted);
                }
                if let Some(usage) = usage {
                    add_usage(&mut inner.usage_sum, &usage);
                }
            }
        }
        inner.rule.observe(at, &state);
        inner.state = state;
        drop(inner);
        self.wake.notify_one();
        readable
    }

    /// Feeds one frame of the harness's stdout. A `result` frame also gives the turns and the
    /// token usage its turn counted, read from the frame itself.
    pub fn feed_stream(&self, frame: &[u8]) {
        self.take(SessionRecord::Stream(frame), result_of(frame));
    }

    /// Feeds one line of the hook log, without its newline.
    pub fn feed_hook(&self, line: &[u8]) {
        self.take(SessionRecord::Hook(line), None);
    }

    /// What to do now: end at the cap when the turns across the session exceed the limit, close
    /// when the rule is due, else wait.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        let now = (self.clock)();
        let inner = locked(&self.inner);
        if inner
            .turn_limit
            .is_some_and(|limit| inner.turns_sum > limit)
        {
            Verdict::Cap
        } else if inner.rule.is_due(now) {
            Verdict::Close
        } else {
            Verdict::Wait
        }
    }

    /// The tracker's state, when the last turn ended and the turns summed over the session.
    #[must_use]
    pub fn snapshot(&self) -> (PendingState, Option<Duration>, u64) {
        let inner = locked(&self.inner);
        (inner.state.clone(), inner.last_turn_end, inner.turns_sum) // BORROW: the snapshot owns its state
    }

    /// The token usage summed over the session's results, when any result gave one.
    #[must_use]
    pub fn usage(&self) -> Option<Value> {
        locked(&self.inner).usage_sum.clone() // BORROW: the caller owns the sum
    }
}

// ---------------------------------------------------------------------------------------------
// The hook log.
// ---------------------------------------------------------------------------------------------

/// The hook log of the agent's home, read as it grows and archived as a raw stream.
struct HookLog {
    path: PathBuf,
    offset: u64,
    partial: Vec<u8>,
    writer: Option<FrameWriter>,
    began: Instant,
}

impl HookLog {
    fn new(path: PathBuf, writer: Option<FrameWriter>) -> Self {
        Self {
            path,
            offset: 0,
            partial: Vec::new(),
            writer,
            began: Instant::now(),
        }
    }

    /// The complete lines written since the last read, without their newlines.
    fn drain(&mut self) -> Vec<Vec<u8>> {
        let Ok(mut file) = File::open(&self.path) else {
            return Vec::new();
        };
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return Vec::new();
        }
        let mut chunk = Vec::new();
        if file.read_to_end(&mut chunk).is_err() {
            return Vec::new();
        }
        self.offset = self
            .offset
            .saturating_add(u64::try_from(chunk.len()).unwrap_or(0));
        self.partial.extend(chunk);
        let mut lines = Vec::new();
        while let Some(end) = self.partial.iter().position(|byte| *byte == b'\n') {
            let mut line: Vec<u8> = self.partial.drain(..=end).collect();
            if let Some(writer) = self.writer.as_mut() {
                let at = u64::try_from(self.began.elapsed().as_nanos()).unwrap_or(u64::MAX);
                if writer.write(&line, Some(at)).is_err() {
                    // EXPLICIT: a log that cannot be archived is still read; the raw stream is lost
                    self.writer = None;
                }
            }
            line.pop();
            lines.push(line);
        }
        lines
    }

    /// Finishes the archive of the log.
    fn finish(self) -> Option<RawFileInfo> {
        // EXPLICIT: an archive that cannot be finished is left out of the seal
        self.writer?.finish().ok()
    }
}

// ---------------------------------------------------------------------------------------------
// One session run.
// ---------------------------------------------------------------------------------------------

/// A session that is prepared and not yet started: the hooks are installed, the watch exists.
pub struct SessionRun {
    watch: Arc<SessionWatch>,
    input: Vec<u8>,
    grace: Duration,
    log: Option<HookLog>,
    hook_stream: Option<StreamName>,
    config: SessionConfig,
}

/// How a session run stands once the child has ended.
#[derive(Debug)]
#[non_exhaustive]
pub struct SessionOutcome {
    /// What the runner observed, before the run's end is known.
    pub watch: Arc<SessionWatch>,
    /// The raw stream of the hook log, when there was one and it was archived.
    pub hook_stream: Option<(StreamName, RawFileInfo)>,
    /// Whether the runner closed the input.
    pub closed: bool,
    /// Whether the runner stopped the harness after the input was closed.
    pub forced: bool,
    /// Whether the runner ended the run at the turn limit.
    pub capped: bool,
    config: SessionConfig,
}

impl std::fmt::Debug for SessionWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionWatch").finish_non_exhaustive()
    }
}

impl SessionRun {
    /// Installs the hooks of `spec` under `home` and prepares the watch.
    ///
    /// The tracker is `harness`'s. The hook log is archived as a raw stream under `frames`.
    ///
    /// # Errors
    /// [`IsolationError::Plan`] when the driver declares a session it gives no tracker for, closes
    /// it by an action (which the runner does not run), or when a hook file or the archive cannot
    /// be made.
    pub fn prepare(
        harness: &dyn HarnessDriver, // TRAIT_OBJECT: the driver is the binary's choice, or a fake in tests
        spec: &SessionSpec,
        home: &Path,
        frames: &Path,
        config: SessionConfig,
        clock: Clock,
    ) -> Result<Self, IsolationError> {
        if !matches!(spec.close, SessionClose::Eof) {
            return Err(plan_error(
                "the session is closed by an action, which the runner does not run".to_owned(), // BORROW: owned text
            ));
        }
        let tracker = harness.session_tracker().ok_or_else(|| {
            plan_error(format!(
                "the harness driver {} runs sessions and gives no tracker",
                harness.id()
            ))
        })?;
        let mut log = None;
        if let Some(hooks) = &spec.hooks {
            for file in &hooks.files {
                if file.root != FileRoot::Home {
                    return Err(plan_error(format!(
                        "the hook file {} is not rooted at the agent's home",
                        file.path
                    )));
                }
                super::write_file(home, file)
                    .map_err(|error| plan_error(format!("cannot write a hook file: {error}")))?;
            }
            let anchor = ClockAnchor::new(
                now().map_err(|error| plan_error(error.to_string()))?, // BORROW: .to_string() gives owned text
                0,
            );
            let writer = FrameWriter::create(
                frames,
                hooks.stream.clone(), // BORROW: the writer owns its stream name
                Framing::Lines,
                FrameClock::monotonic(anchor),
                None,
            )
            // EXPLICIT: a log that cannot be archived is still read; the raw stream is lost
            .ok();
            log = Some(HookLog::new(home.join(hooks.log.as_str()), writer)); // BORROW: explicit .as_str()
        }
        let rule = CloseRule::new(config.idle, HOOK_SETTLE);
        Ok(Self {
            watch: SessionWatch::new(tracker, rule, config.turn_limit, clock),
            input: spec.input.clone(), // BORROW: the run owns its opening bytes
            grace: harness.stop().grace,
            log,
            hook_stream: spec.hooks.as_ref().map(|hooks| hooks.stream.clone()), // BORROW: the run owns the stream name
            config,
        })
    }

    /// The watch the live observer feeds.
    #[must_use]
    pub fn watch(&self) -> Arc<SessionWatch> {
        Arc::clone(&self.watch)
    }

    /// Takes the child's stdin, writes the opening input and starts deciding.
    ///
    /// # Errors
    /// [`IsolationError::Plan`] when the child has no stdin pipe.
    pub fn start(self, child: &mut Child) -> Result<SessionHandle, IsolationError> {
        let stdin = child.take_stdin().ok_or_else(|| {
            plan_error("the session's child has no stdin pipe".to_owned()) // BORROW: owned text
        })?;
        let stop = child.stop_handle();
        let log = Arc::new(Mutex::new(self.log));
        let task = tokio::spawn(control(
            Arc::clone(&self.watch),
            stdin,
            self.input,
            stop,
            self.grace,
            Arc::clone(&log),
        ));
        Ok(SessionHandle {
            watch: self.watch,
            task,
            log,
            hook_stream: self.hook_stream,
            config: self.config,
        })
    }
}

/// A started session: the task that decides, and what it needs to report once the child ended.
pub struct SessionHandle {
    watch: Arc<SessionWatch>,
    task: JoinHandle<()>,
    log: Arc<Mutex<Option<HookLog>>>,
    hook_stream: Option<StreamName>,
    config: SessionConfig,
}

/// Reads the hook log into the watch.
fn drain_hooks(watch: &SessionWatch, log: &Mutex<Option<HookLog>>) {
    let lines = locked(log).as_mut().map(HookLog::drain).unwrap_or_default();
    for line in lines {
        watch.feed_hook(&line);
    }
}

/// Writes the opening input, then reads the hook log and decides every [`POLL`] until the task is
/// ended: the input closes when the rule is due, the harness is stopped once the driver's grace
/// has passed since the close, and the run is stopped at the turn limit.
async fn control(
    watch: Arc<SessionWatch>,
    mut stdin: ChildStdin,
    input: Vec<u8>,
    stop: StopHandle,
    grace: Duration,
    log: Arc<Mutex<Option<HookLog>>>,
) {
    if !input.is_empty() {
        // EXPLICIT: a harness that has already exited cannot be written to; its exit ends the run
        let _ = stdin.write_all(&input).await;
        let _ = stdin.flush().await;
    }
    let mut stdin = Some(stdin);
    let mut closed_at: Option<Duration> = None;
    let mut tick = tokio::time::interval(POLL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = tick.tick() => {}
            () = watch.wake.notified() => {}
        }
        drain_hooks(&watch, &log);
        match watch.verdict() {
            Verdict::Cap => {
                watch.capped.store(true, Ordering::SeqCst);
                stop.request();
                return;
            }
            Verdict::Close if stdin.is_some() => {
                watch.closed.store(true, Ordering::SeqCst);
                closed_at = Some((watch.clock)());
                stdin = None;
            }
            Verdict::Close | Verdict::Wait => {}
        }
        if let Some(at) = closed_at
            && (watch.clock)() >= at.saturating_add(grace)
        {
            watch.forced.store(true, Ordering::SeqCst);
            stop.request();
            return;
        }
    }
}

impl SessionHandle {
    /// Ends the task, reads what the hook log still holds, finishes its archive and reports.
    #[must_use]
    pub fn finish(self) -> SessionOutcome {
        self.task.abort();
        drain_hooks(&self.watch, &self.log);
        let archived = locked(&self.log).take().and_then(HookLog::finish);
        SessionOutcome {
            hook_stream: self.hook_stream.zip(archived),
            closed: self.watch.closed.load(Ordering::SeqCst),
            forced: self.watch.forced.load(Ordering::SeqCst),
            capped: self.watch.capped.load(Ordering::SeqCst),
            watch: self.watch,
            config: self.config,
        }
    }
}

/// How the session ended, from whether the runner closed the input and how the run ended.
///
/// The harness's dollar budget is `Budget`; the wall or the turn limit is `Cap`; otherwise the
/// session is `Closed` when the runner closed the input and `HarnessExit` when the harness left
/// first.
#[must_use]
pub fn session_end(closed: bool, ended_by: &EndedBy) -> SessionEnd {
    if let EndedBy::Cap { cap, .. } = ended_by {
        if *cap == Cap::Cost {
            SessionEnd::Budget
        } else {
            SessionEnd::Cap
        }
    } else if closed {
        SessionEnd::Closed
    } else {
        SessionEnd::HarnessExit
    }
}

impl SessionOutcome {
    /// The turn limit the run was ended at, when it was.
    #[must_use]
    pub const fn cap(&self) -> Option<Cap> {
        if self.capped { Some(Cap::Turns) } else { None }
    }

    /// Whether a turn ended whose `Stop` hook line was never read, or the pending state could not
    /// be read at the end: the hook channel failed to say what was pending.
    #[must_use]
    pub fn hooks_incomplete(&self) -> bool {
        let state = self.watch.snapshot().0;
        state.turns > 0 && !state.known
    }

    /// The token usage summed over the session's results.
    #[must_use]
    pub fn usage_counted(&self) -> Option<Value> {
        self.watch.usage()
    }

    /// The turns the session counted, summed over its results.
    #[must_use]
    pub fn turns_counted(&self) -> u64 {
        self.watch.snapshot().2
    }

    /// The measures of the session. `own_end` is whether the run's end is the arm's own: it is not
    /// when the experimenter stopped the run or the run failed, and no end is recorded then.
    #[must_use]
    pub fn measures(
        &self,
        ended_by: &EndedBy,
        own_end: bool,
        hooks_incomplete: bool,
    ) -> SessionMeasures {
        let (state, last, _) = self.watch.snapshot();
        SessionMeasures::new(
            self.config.idle,
            state.turns,
            last.map(|at| u64::try_from(at.as_millis()).unwrap_or(u64::MAX)),
            state.known.then_some(state.pending),
            if hooks_incomplete {
                Some(SessionEnd::HooksIncomplete)
            } else {
                own_end.then(|| session_end(self.closed, ended_by))
            },
            self.forced,
        )
    }
}
