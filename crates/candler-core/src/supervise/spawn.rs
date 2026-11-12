//! Spawning a child in a process group of its own, and the request that describes it.
//!
//! A [`SpawnRequest`] names the program, the exact environment, the marker, where each of the
//! child's three streams goes, and how the child is stopped. [`Supervisor::spawn`] starts the
//! child with `process_group(0)` and an empty environment plus the request's, writes its record,
//! reports `process.spawned`, and hands back a [`Child`].

use std::fmt;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use candler_harness::spec::{Framing, StopSignal, StopSpec};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{Marker, RunKey};
use candler_schemas::process::{ChildRecord, ProcessStart};
use candler_schemas::raw::{RawFrame, StreamName};
use rustix::process::Signal;
use tokio::io::AsyncWriteExt;
use tokio::process::{ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use super::hook::StopHook;
use super::output::{FileTail, FramedStream, Pumped, pump};
use super::platform::process_start;
use super::record::write_record;
use super::signal::Guard;
use super::stop::StraySweeper;
use super::{EventSink, SuperviseError, Supervisor, now};

/// The environment variable that carries the marker of a run to its children.
const MARKER_VARIABLE: &str = "CANDLER_RUN";

/// How long a child gets between the stop signal and the group's termination when the request
/// does not say.
pub const DEFAULT_GRACE: Duration = Duration::from_secs(5);

/// What a child reads on its standard input.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StdinSpec {
    /// Nothing: the child reads end of file at once.
    Null,
    /// These bytes, then end of file.
    Bytes(Vec<u8>),
    /// A pipe the caller takes with [`Child::take_stdin`].
    Pipe,
}

impl fmt::Debug for StdinSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("Null"),
            Self::Bytes(bytes) => write!(f, "Bytes({} bytes)", bytes.len()),
            Self::Pipe => f.write_str("Pipe"),
        }
    }
}

/// Sees each frame of a framed stream once the frame is flushed to its file: how a live reader
/// gets its frames.
pub trait FrameObserver: Send {
    /// Takes the frame `frame`, the `index`th of its stream, counting from 0.
    fn frame(&mut self, index: u64, frame: &RawFrame);
}

/// Where a child's standard output or standard error goes.
#[non_exhaustive]
pub enum OutputSpec {
    /// A raw frame file `<dir>/<stream>.jsonl.gz`, written while the child runs.
    #[non_exhaustive]
    Framed {
        /// The directory of the file.
        dir: PathBuf,
        /// The stream the file archives.
        stream: StreamName,
        /// How the stream is cut into frames.
        mode: Framing,
        /// Told about each frame once it is flushed.
        // TRAIT_OBJECT: the observer is supplied by the caller of the supervisor
        observer: Option<Box<dyn FrameObserver>>,
    },
    /// A pipe the caller takes with [`Child::take_stdout`] or [`Child::take_stderr`].
    Pipe,
    /// A file, created if needed and appended to.
    File(PathBuf),
    /// Nowhere: the output is discarded.
    Null,
}

impl OutputSpec {
    /// A raw frame file `<dir>/<stream>.jsonl.gz` with no observer.
    #[must_use]
    pub const fn framed(dir: PathBuf, stream: StreamName, mode: Framing) -> Self {
        Self::Framed {
            dir,
            stream,
            mode,
            observer: None,
        }
    }

    /// The same output with `observer` told about each frame; other kinds of output are
    /// returned unchanged, because they have no frames.
    #[must_use]
    // TRAIT_OBJECT: the observer is supplied by the caller of the supervisor
    pub fn with_observer(self, observer: Box<dyn FrameObserver>) -> Self {
        match self {
            Self::Framed {
                dir, stream, mode, ..
            } => Self::Framed {
                dir,
                stream,
                mode,
                observer: Some(observer),
            },
            Self::Pipe | Self::File(_) | Self::Null => self,
        }
    }

    /// The directory and stream of a framed output, which name its file.
    fn file_key<'a>(&'a self) -> Option<(&'a Path, &'a StreamName)> {
        match self {
            Self::Framed { dir, stream, .. } => Some((dir.as_path(), stream)),
            Self::Pipe | Self::File(_) | Self::Null => None,
        }
    }
}

impl fmt::Debug for OutputSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framed {
                dir,
                stream,
                mode,
                observer,
            } => f
                .debug_struct("Framed")
                .field("dir", dir)
                .field("stream", stream)
                .field("mode", mode)
                .field("observer", &observer.is_some())
                .finish(),
            Self::Pipe => f.write_str("Pipe"),
            Self::File(path) => f.debug_tuple("File").field(path).finish(),
            Self::Null => f.write_str("Null"),
        }
    }
}

/// What to spawn and how to supervise it. Its `Debug` prints environment names, never values.
#[non_exhaustive]
pub struct SpawnRequest {
    argv: Vec<String>,
    env: Vec<(String, String)>,
    marker: Marker,
    run: Option<RunKey>,
    cwd: Option<PathBuf>,
    stdin: StdinSpec,
    stdout: OutputSpec,
    stderr: OutputSpec,
    deadline: Option<Duration>,
    stop: StopSpec,
    // TRAIT_OBJECT: the hook is chosen by the run that attaches it
    hook: Option<Arc<dyn StopHook>>,
    record_dir: PathBuf,
}

impl SpawnRequest {
    /// Starts a request; `argv`, `marker` and `record_dir` must be set before it builds. It takes
    /// no arguments: `build` reports which required field is missing.
    #[must_use]
    pub fn builder() -> SpawnRequestBuilder {
        SpawnRequestBuilder::default()
    }

    /// The same request with both outputs replaced by pipes.
    pub(crate) fn piped(mut self) -> Self {
        self.stdout = OutputSpec::Pipe;
        self.stderr = OutputSpec::Pipe;
        self
    }

    /// The same request that is stopped as `stop` says.
    #[must_use]
    pub fn with_stop(mut self, stop: StopSpec) -> Self {
        self.stop = stop;
        self
    }

    /// How long the child gets between the stop signal and the group's termination.
    pub(crate) const fn grace(&self) -> Duration {
        self.stop.grace
    }
}

impl fmt::Debug for SpawnRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self
            .env
            .iter()
            // BORROW: explicit .as_str() instead of Deref coercion
            .map(|(name, _)| name.as_str())
            .collect();
        f.debug_struct("SpawnRequest")
            .field("program", &self.argv.first())
            .field("args", &self.argv.len().saturating_sub(1))
            .field("env_names", &names)
            .field("marker", &self.marker)
            .field("run", &self.run)
            .field("cwd", &self.cwd)
            .field("stdin", &self.stdin)
            .field("stdout", &self.stdout)
            .field("stderr", &self.stderr)
            .field("deadline", &self.deadline)
            .field("pre_stop", &self.stop.pre_stop.is_some())
            .field("stop_hook", &self.hook.is_some())
            .field("record_dir", &self.record_dir)
            .finish_non_exhaustive()
    }
}

/// Builds a [`SpawnRequest`].
#[non_exhaustive]
pub struct SpawnRequestBuilder {
    argv: Option<Vec<String>>,
    env: Vec<(String, String)>,
    marker: Option<Marker>,
    run: Option<RunKey>,
    cwd: Option<PathBuf>,
    stdin: StdinSpec,
    stdout: OutputSpec,
    stderr: OutputSpec,
    deadline: Option<Duration>,
    stop: Option<StopSpec>,
    // TRAIT_OBJECT: the hook is chosen by the run that attaches it
    hook: Option<Arc<dyn StopHook>>,
    record_dir: Option<PathBuf>,
}

impl Default for SpawnRequestBuilder {
    fn default() -> Self {
        Self {
            argv: None,
            env: Vec::new(),
            marker: None,
            run: None,
            cwd: None,
            stdin: StdinSpec::Null,
            stdout: OutputSpec::Null,
            stderr: OutputSpec::Null,
            deadline: None,
            stop: None,
            hook: None,
            record_dir: None,
        }
    }
}

impl fmt::Debug for SpawnRequestBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpawnRequestBuilder")
            .finish_non_exhaustive()
    }
}

impl SpawnRequestBuilder {
    /// Sets the program and its arguments; the first element is the program.
    #[must_use]
    pub fn argv(mut self, argv: Vec<String>) -> Self {
        self.argv = Some(argv);
        self
    }

    /// Sets the exact environment of the child: nothing else is inherited.
    #[must_use]
    pub fn env(mut self, env: Vec<(String, String)>) -> Self {
        self.env = env;
        self
    }

    /// Sets the marker, which the child receives as `CANDLER_RUN`.
    #[must_use]
    pub fn marker(mut self, marker: Marker) -> Self {
        self.marker = Some(marker);
        self
    }

    /// Sets the run the child's events belong to.
    #[must_use]
    pub fn run(mut self, run: RunKey) -> Self {
        self.run = Some(run);
        self
    }

    /// Sets the working directory of the child.
    #[must_use]
    pub fn cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = Some(cwd);
        self
    }

    /// Sets what the child reads on standard input; the default is [`StdinSpec::Null`].
    #[must_use]
    pub fn stdin(mut self, stdin: StdinSpec) -> Self {
        self.stdin = stdin;
        self
    }

    /// Sets where standard output goes; the default is [`OutputSpec::Null`].
    #[must_use]
    pub fn stdout(mut self, stdout: OutputSpec) -> Self {
        self.stdout = stdout;
        self
    }

    /// Sets where standard error goes; the default is [`OutputSpec::Null`].
    #[must_use]
    pub fn stderr(mut self, stderr: OutputSpec) -> Self {
        self.stderr = stderr;
        self
    }

    /// Sets how long the child may run before it is stopped; the default is no limit.
    #[must_use]
    pub const fn deadline(mut self, deadline: Duration) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Sets how the child is stopped; the default is `SIGTERM` with a grace of
    /// [`DEFAULT_GRACE`] and no pre-stop action.
    #[must_use]
    pub fn stop(mut self, stop: StopSpec) -> Self {
        self.stop = Some(stop);
        self
    }

    /// Attaches a hook that the ladder calls when it ends the child. A hook belongs to one
    /// request: a supervisor holds none, so a control call carries none.
    #[must_use]
    // TRAIT_OBJECT: the hook is chosen by the run that attaches it
    pub fn stop_hook(mut self, hook: Arc<dyn StopHook>) -> Self {
        self.hook = Some(hook);
        self
    }

    /// Sets the directory that receives the child's record.
    #[must_use]
    pub fn record_dir(mut self, record_dir: PathBuf) -> Self {
        self.record_dir = Some(record_dir);
        self
    }

    /// Builds the request.
    ///
    /// # Errors
    /// Returns [`SuperviseError::MissingField`] if `argv`, `marker` or `record_dir` was not set,
    /// and [`SuperviseError::InvalidRequest`] for an empty `argv`, an environment name that is
    /// empty or holds `=` or a NUL byte, a value that holds a NUL byte, a name set twice, a
    /// `CANDLER_RUN` entry, or two framed outputs that name the same file.
    pub fn build(self) -> Result<SpawnRequest, SuperviseError> {
        let argv = self
            .argv
            .ok_or(SuperviseError::MissingField { field: "argv" })?;
        let marker = self
            .marker
            .ok_or(SuperviseError::MissingField { field: "marker" })?;
        let record_dir = self.record_dir.ok_or(SuperviseError::MissingField {
            field: "record_dir",
        })?;
        if argv.is_empty() {
            return Err(invalid("argv is empty"));
        }
        check_env(&self.env)?;
        if let (Some(out), Some(err)) = (self.stdout.file_key(), self.stderr.file_key())
            && out == err
        {
            return Err(invalid("stdout and stderr name the same raw frame file"));
        }
        Ok(SpawnRequest {
            argv,
            env: self.env,
            marker,
            run: self.run,
            cwd: self.cwd,
            stdin: self.stdin,
            stdout: self.stdout,
            stderr: self.stderr,
            deadline: self.deadline,
            stop: self
                .stop
                .unwrap_or_else(|| StopSpec::new(None, StopSignal::Term, DEFAULT_GRACE)),
            hook: self.hook,
            record_dir,
        })
    }
}

fn invalid(detail: &str) -> SuperviseError {
    SuperviseError::InvalidRequest {
        // BORROW: .to_owned() keeps the description in the error
        detail: detail.to_owned(),
    }
}

/// Refuses an environment the operating system would refuse, and one that sets `CANDLER_RUN`,
/// which the marker sets. The errors name variables and never show a value.
fn check_env(env: &[(String, String)]) -> Result<(), SuperviseError> {
    let mut seen: Vec<&str> = Vec::with_capacity(env.len());
    for (name, value) in env {
        // BORROW: explicit .as_str() instead of Deref coercion
        let name = name.as_str();
        if name.is_empty() || name.contains(['=', '\0']) {
            return Err(invalid(
                "an environment name is empty or holds `=` or a NUL byte",
            ));
        }
        if value.contains('\0') {
            return Err(SuperviseError::InvalidRequest {
                detail: format!("the value of environment variable {name} holds a NUL byte"),
            });
        }
        if name == MARKER_VARIABLE {
            return Err(invalid(
                "the environment sets CANDLER_RUN, which the marker sets",
            ));
        }
        if seen.contains(&name) {
            return Err(SuperviseError::InvalidRequest {
                detail: format!("environment variable {name} is set twice"),
            });
        }
        seen.push(name);
    }
    Ok(())
}

/// The pumps that move a child's framed output into files, and the switch that ends them.
pub(crate) struct Pumps {
    pub(crate) cutoff: watch::Sender<bool>,
    pub(crate) stdout: Option<JoinHandle<Pumped>>,
    pub(crate) stderr: Option<JoinHandle<Pumped>>,
}

/// What a child needs to be stopped, besides its process: where to report, how to stop it, and
/// the environment its pre-stop command starts from.
pub(crate) struct Context {
    // TRAIT_OBJECT: the sink is chosen by the job runner and by tests
    pub(crate) sink: Arc<dyn EventSink>,
    // TRAIT_OBJECT: the sweeper is injected by the isolation backend
    pub(crate) sweeper: Option<Arc<dyn StraySweeper>>,
    pub(crate) guard: Arc<Guard>,
    pub(crate) run: Option<RunKey>,
    pub(crate) stop: StopSpec,
    // TRAIT_OBJECT: the hook is chosen by the run that attaches it
    pub(crate) hook: Option<Arc<dyn StopHook>>,
    /// When the child must be ended if it is still running: its deadline counted from the spawn.
    pub(crate) deadline: Option<Instant>,
    pub(crate) env: Vec<(String, String)>,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) stderr_file: Option<FileTail>,
}

/// A running child: its record, its pipes, and the tasks that move its output.
///
/// Dropping a child without waiting for it does not stop the process. The pumps end and the
/// child's pipes close.
#[non_exhaustive]
pub struct Child {
    pub(crate) record: ChildRecord,
    pub(crate) process: tokio::process::Child,
    pub(crate) pumps: Pumps,
    pub(crate) stdin_task: Option<JoinHandle<()>>,
    pub(crate) switch: Arc<watch::Sender<bool>>,
    pub(crate) context: Context,
}

impl Child {
    /// The record written for this child.
    #[must_use]
    pub const fn record<'a>(&'a self) -> &'a ChildRecord {
        &self.record
    }

    /// The pipe to the child's standard input, once, for a [`StdinSpec::Pipe`].
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.process.stdin.take()
    }

    /// The pipe from the child's standard output, once, for an [`OutputSpec::Pipe`].
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.process.stdout.take()
    }

    /// The pipe from the child's standard error, once, for an [`OutputSpec::Pipe`].
    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.process.stderr.take()
    }
}

impl fmt::Debug for Child {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Child")
            .field("pid", &self.record.pid())
            .field("pgid", &self.record.pgid())
            .finish_non_exhaustive()
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        self.pumps.cutoff.send_replace(true);
        if let Some(task) = &self.stdin_task {
            task.abort();
        }
    }
}

/// Opens a file output for appending, creating it if needed.
fn open_append(path: &Path) -> Result<std::fs::File, SuperviseError> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| SuperviseError::Io {
            context: "cannot open the output file",
            source,
        })
}

/// What a child's output becomes: the stdio the process gets, the framed stream that will
/// receive the pipe if the output is framed, and the file region to read back if it is a file.
fn prepare(
    output: OutputSpec,
) -> Result<(Stdio, Option<FramedStream>, Option<FileTail>), SuperviseError> {
    match output {
        OutputSpec::Framed {
            dir,
            stream,
            mode,
            observer,
        } => Ok((
            Stdio::piped(),
            Some(FramedStream::create(&dir, stream, mode, observer)?),
            None,
        )),
        OutputSpec::Pipe => Ok((Stdio::piped(), None, None)),
        OutputSpec::File(path) => {
            let file = open_append(&path)?;
            let tail = FileTail::begin(path, &file)?;
            Ok((Stdio::from(file), None, Some(tail)))
        }
        OutputSpec::Null => Ok((Stdio::null(), None, None)),
    }
}

/// Removes the frame files of a spawn that failed, so that it leaves no `.jsonl.tmp` behind.
fn discard_streams(out: Option<FramedStream>, err: Option<FramedStream>) {
    for stream in [out, err].into_iter().flatten() {
        stream.discard();
    }
}

/// Ends a child that was started and could not be taken into the signal guard's table.
///
/// This is the one kill outside the signal guard. The child is still this
/// process's own, unreaped child, held through its `tokio` handle: its process id cannot name
/// another process, which is the only thing the guard protects against. The handle signals that
/// one process and reaps it in the background; a grandchild started in the instants since the
/// spawn is not signalled.
fn end_unreaped(mut process: tokio::process::Child) {
    // EXPLICIT: handle kill of this process's own unreaped child; no pid or group number is passed
    let _ = process.start_kill();
    tokio::spawn(async move {
        let _ = process.wait().await;
    });
}

impl Supervisor {
    /// Starts the child in a new process group with exactly the environment of the request plus
    /// `CANDLER_RUN`, writes `<record_dir>/<pid>.json` with the file and the directory synced,
    /// emits `process.spawned`, and returns. The record is on disk when this returns.
    ///
    /// Must be called within a Tokio runtime.
    ///
    /// # Errors
    /// Returns [`SuperviseError::Spawn`] if the program cannot start, and
    /// [`SuperviseError::Io`] if an output file or the record cannot be written; in the second
    /// case the child's group is killed first.
    pub fn spawn(&self, request: SpawnRequest) -> Result<Child, SuperviseError> {
        let SpawnRequest {
            argv,
            env,
            marker,
            run,
            cwd,
            stdin,
            stdout,
            stderr,
            deadline,
            stop,
            hook,
            record_dir,
        } = request;
        let program = argv.first().ok_or_else(|| invalid("argv is empty"))?;
        let mut command = Command::new(program);
        command
            .args(argv.iter().skip(1))
            .env_clear()
            .envs(env.iter().map(|(name, value)| (name, value)))
            .env(MARKER_VARIABLE, marker.to_string())
            .process_group(0);
        if let Some(dir) = &cwd {
            command.current_dir(dir);
        }
        command.stdin(match stdin {
            StdinSpec::Null => Stdio::null(),
            StdinSpec::Bytes(_) | StdinSpec::Pipe => Stdio::piped(),
        });
        let (out_stdio, out_stream, _) = prepare(stdout)?;
        let (err_stdio, err_stream, err_file) = match prepare(stderr) {
            Ok(prepared) => prepared,
            Err(error) => {
                discard_streams(out_stream, None);
                return Err(error);
            }
        };
        command.stdout(out_stdio).stderr(err_stdio);

        // Read before the child exists, so that a clock failure leaves no child behind.
        let spawned_at = match now() {
            Ok(at) => at,
            Err(error) => {
                discard_streams(out_stream, err_stream);
                return Err(error);
            }
        };
        let mut process = match command.spawn() {
            Ok(process) => process,
            Err(source) => {
                discard_streams(out_stream, err_stream);
                return Err(SuperviseError::Spawn {
                    // BORROW: .to_owned() keeps the program name in the error
                    program: program.to_owned(),
                    source,
                });
            }
        };
        let Some(pid) = process.id() else {
            discard_streams(out_stream, err_stream);
            return Err(SuperviseError::Io {
                context: "the child has no process id",
                source: std::io::Error::other("the child was reaped before its record was written"),
            });
        };
        // The deadline runs from here: the child exists.
        let deadline = deadline.map(|limit| Instant::now() + limit);
        // The child leads the group it was started in, so its process group id is its own id.
        let start = process_start(pid);
        // The guard signals only a leader whose start it can read again, so a child whose start
        // cannot be read could never be stopped. It is not left running.
        let registered = if matches!(start, ProcessStart::Unavailable) {
            Err(SuperviseError::Io {
                context: "cannot read the start time of the child",
                source: std::io::Error::other("the child could not be identified for signalling"),
            })
        } else {
            // From here the guard lets the supervisor signal this group, and no other.
            // BORROW: .clone() gives the table its own start
            self.guard
                .register(pid, start.clone())
                .map_err(|refusal| SuperviseError::Io {
                    context: "the signal guard refused the child",
                    source: std::io::Error::other(refusal.to_string()),
                })
        };
        if let Err(error) = registered {
            end_unreaped(process);
            discard_streams(out_stream, err_stream);
            return Err(error);
        }
        let started = ChildRecord::builder()
            .pid(pid)
            .pgid(pid)
            .start(start)
            .marker(marker)
            .spawned_at(spawned_at)
            .build()
            .map_err(|e| SuperviseError::Event {
                detail: e.to_string(),
            });
        let written = started.and_then(|record| {
            write_record(&record_dir, &record)
                .map(|_| record)
                .map_err(|source| SuperviseError::Io {
                    context: "cannot write the child record",
                    source,
                })
        });
        let record = match written {
            Ok(record) => record,
            Err(error) => {
                // The group may already be gone, which is what the kill is after; a refused
                // group is not signalled.
                let _ = self.guard.signal_group(pid, Signal::KILL);
                self.guard.forget(pid);
                discard_streams(out_stream, err_stream);
                return Err(error);
            }
        };
        // BORROW: .clone() gives the event its own copy of the record
        self.sink
            .emit(run.as_ref(), EventPayload::process_spawned(record.clone()));

        let (cutoff, _) = watch::channel(false);
        let stdout_pump = match (out_stream, process.stdout.take()) {
            (Some(stream), Some(pipe)) => {
                Some(tokio::spawn(pump(pipe, Some(stream), cutoff.subscribe())))
            }
            // An output left as a pipe is the caller's to take.
            (None, pipe) => {
                process.stdout = pipe;
                None
            }
            (Some(_), None) => None,
        };
        let stderr_pump = match (err_stream, process.stderr.take()) {
            (Some(stream), Some(pipe)) => {
                Some(tokio::spawn(pump(pipe, Some(stream), cutoff.subscribe())))
            }
            // An output left as a pipe is the caller's to take.
            (None, pipe) => {
                process.stderr = pipe;
                None
            }
            (Some(_), None) => None,
        };
        let stdin_task = match (stdin, process.stdin.take()) {
            (StdinSpec::Bytes(bytes), Some(mut pipe)) => Some(tokio::spawn(async move {
                // A child that closes its input early makes the write fail, which is its right.
                let _ = pipe.write_all(&bytes).await;
            })),
            (StdinSpec::Null | StdinSpec::Pipe | StdinSpec::Bytes(_), pipe) => {
                process.stdin = pipe;
                None
            }
        };
        Ok(Child {
            record,
            process,
            pumps: Pumps {
                cutoff,
                stdout: stdout_pump,
                stderr: stderr_pump,
            },
            stdin_task,
            switch: Arc::new(watch::channel(false).0),
            context: Context {
                // TRAIT_OBJECT: the sink is chosen by the job runner and by tests
                sink: Arc::clone(&self.sink),
                // TRAIT_OBJECT: the sweeper is injected by the isolation backend
                sweeper: self.sweeper.as_ref().map(Arc::clone),
                guard: Arc::clone(&self.guard),
                run,
                stop,
                hook,
                deadline,
                env,
                cwd,
                stderr_file: err_file,
            },
        })
    }
}
