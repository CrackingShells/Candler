//! The data a harness driver hands to the core: commands, stop and collect specs, capabilities,
//! the exit of a process, and the read-only views of a cell and a workspace.
//!
//! None of these types acts on a process. A [`CommandSpec`] names a program, its arguments and the
//! environment it adds; the core spawns it. An [`EnvValue::Secret`] names a secret and never holds
//! its value, and the `Debug` output of a command or an environment entry shows names only.

use std::fmt;
use std::path::Path;
use std::time::Duration;

use candler_schemas::primitives::{FieldPath, Name, RelPath, SecretName};
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::StreamName;
use candler_schemas::trace::SessionId;
use candler_schemas::values::{JsonValue, ResolvedValue};

/// Why a command could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CommandSpecError {
    /// A builder that was asked to build before the argument vector was set.
    #[error("command field is missing (`{field}` was not set)")]
    MissingField {
        /// The name of the field.
        field: &'static str,
    },
    /// An argument vector with no element, so no program to run.
    #[error("command has an empty argv")]
    EmptyArgv,
    /// An environment overlay that sets one variable twice.
    #[error("command sets environment variable {name:?} more than once")]
    DuplicateEnv {
        /// The name of the variable.
        name: String,
    },
}

/// The value of an environment variable: a literal, or the name of a secret the core resolves when it spawns the process.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EnvValue {
    /// A value written in the spec.
    Literal(String),
    /// The name of a secret; the spec never holds the secret's value.
    Secret(SecretName),
}

impl fmt::Debug for EnvValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(_) => f.write_str("Literal(..)"),
            Self::Secret(name) => write!(f, "Secret({name})"),
        }
    }
}

/// One variable of an environment overlay.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct EnvEntry {
    /// The name of the variable.
    pub name: String,
    /// The value of the variable.
    pub value: EnvValue,
}

impl EnvEntry {
    /// An entry with the value `value`.
    #[must_use]
    pub const fn new(name: String, value: EnvValue) -> Self {
        Self { name, value }
    }

    /// An entry with a literal value.
    #[must_use]
    pub const fn literal(name: String, value: String) -> Self {
        Self::new(name, EnvValue::Literal(value))
    }

    /// An entry whose value is the secret `secret`, resolved when the process is spawned.
    #[must_use]
    pub const fn secret(name: String, secret: SecretName) -> Self {
        Self::new(name, EnvValue::Secret(secret))
    }
}

impl fmt::Debug for EnvEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.value {
            EnvValue::Literal(_) => write!(f, "{}=<literal>", self.name),
            EnvValue::Secret(secret) => write!(f, "{}=<secret {secret}>", self.name),
        }
    }
}

/// A command the core runs: a non-empty argument vector, the environment it adds to the core's allowlist, and an optional working directory.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CommandSpec {
    argv: Vec<String>,
    env_overlay: Vec<EnvEntry>,
    cwd: Option<RelPath>,
}

impl CommandSpec {
    /// Starts a builder; `argv` must be set before it builds.
    #[must_use]
    pub fn builder() -> CommandSpecBuilder {
        CommandSpecBuilder::default()
    }

    /// The program and its arguments; never empty.
    #[must_use]
    pub const fn argv<'a>(&'a self) -> &'a [String] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.argv.as_slice()
    }

    /// The variables the command adds to the core's environment, each name once.
    #[must_use]
    pub const fn env_overlay<'a>(&'a self) -> &'a [EnvEntry] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.env_overlay.as_slice()
    }

    /// The working directory relative to the workspace, if the command sets one.
    #[must_use]
    pub const fn cwd<'a>(&'a self) -> Option<&'a RelPath> {
        self.cwd.as_ref()
    }
}

impl fmt::Debug for CommandSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandSpec")
            .field("program", &self.argv.first())
            .field("args", &self.argv.len().saturating_sub(1))
            .field("env_overlay", &self.env_overlay)
            .field("cwd", &self.cwd)
            .finish()
    }
}

/// Builds a [`CommandSpec`].
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct CommandSpecBuilder {
    argv: Option<Vec<String>>,
    env_overlay: Vec<EnvEntry>,
    cwd: Option<RelPath>,
}

impl CommandSpecBuilder {
    /// Sets the program and its arguments.
    #[must_use]
    pub fn argv(mut self, argv: Vec<String>) -> Self {
        self.argv = Some(argv);
        self
    }

    /// Adds one variable to the overlay.
    #[must_use]
    pub fn env(mut self, entry: EnvEntry) -> Self {
        self.env_overlay.push(entry);
        self
    }

    /// Sets the working directory relative to the workspace.
    #[must_use]
    pub fn cwd(mut self, cwd: RelPath) -> Self {
        self.cwd = Some(cwd);
        self
    }

    /// Builds the command.
    ///
    /// # Errors
    /// Returns [`CommandSpecError::MissingField`] if `argv` was not set.
    /// Returns [`CommandSpecError::EmptyArgv`] if `argv` has no element.
    /// Returns [`CommandSpecError::DuplicateEnv`] if two overlay entries have the same name.
    pub fn build(self) -> Result<CommandSpec, CommandSpecError> {
        let argv = self
            .argv
            .ok_or(CommandSpecError::MissingField { field: "argv" })?;
        if argv.is_empty() {
            return Err(CommandSpecError::EmptyArgv);
        }
        let mut seen: Vec<&str> = Vec::with_capacity(self.env_overlay.len());
        for entry in &self.env_overlay {
            // BORROW: explicit .as_str() instead of Deref coercion
            let name = entry.name.as_str();
            if seen.contains(&name) {
                return Err(CommandSpecError::DuplicateEnv {
                    // BORROW: .to_owned() keeps the offending name in the error
                    name: name.to_owned(),
                });
            }
            seen.push(name);
        }
        Ok(CommandSpec {
            argv,
            env_overlay: self.env_overlay,
            cwd: self.cwd,
        })
    }
}

/// The signal that asks a harness to stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StopSignal {
    /// `SIGTERM`.
    Term,
    /// `SIGINT`.
    Int,
    /// `SIGHUP`.
    Hup,
}

/// The method of an HTTP pre-stop call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum HttpMethod {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
    /// `PUT`.
    Put,
    /// `DELETE`.
    Delete,
}

/// An action the core runs before it signals a process: a command, or an HTTP call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    /// A command the core runs.
    Command(CommandSpec),
    /// An HTTP call the core makes; whether the URL is loopback is the supervisor's check.
    #[non_exhaustive]
    Http {
        /// The method of the call.
        method: HttpMethod,
        /// The URL of the call.
        url: String,
        /// The body of the call, if it has one.
        body: Option<String>,
    },
}

impl Action {
    /// An HTTP call.
    #[must_use]
    pub const fn http(method: HttpMethod, url: String, body: Option<String>) -> Self {
        Self::Http { method, url, body }
    }
}

/// How the core stops a harness: an optional pre-stop action, then a signal, then a grace period before it escalates.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct StopSpec {
    /// The action the core runs first, if the harness has one.
    pub pre_stop: Option<Action>,
    /// The signal the core sends to the process group.
    pub signal: StopSignal,
    /// How long the core waits after the signal before it terminates the group.
    pub grace: Duration,
}

impl StopSpec {
    /// A stop spec.
    #[must_use]
    pub const fn new(pre_stop: Option<Action>, signal: StopSignal, grace: Duration) -> Self {
        Self {
            pre_stop,
            signal,
            grace,
        }
    }
}

/// What the core collects after a run: files that match a glob, or the output of a command.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CollectSpec {
    /// The files that match a glob, relative to the workspace.
    #[non_exhaustive]
    File {
        /// The glob.
        glob: String,
    },
    /// The standard output of a command, archived as the stream `name`.
    #[non_exhaustive]
    Command {
        /// The command the core runs.
        spec: CommandSpec,
        /// The name of the stream that archives the output.
        name: StreamName,
    },
}

impl CollectSpec {
    /// The files that match `glob`.
    #[must_use]
    pub const fn file(glob: String) -> Self {
        Self::File { glob }
    }

    /// The output of `spec`, archived as the stream `name`.
    #[must_use]
    pub const fn command(spec: CommandSpec, name: StreamName) -> Self {
        Self::Command { spec, name }
    }
}

/// Which per-run budgets a harness can enforce itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct BudgetCaps {
    /// The harness can cap the number of turns.
    pub turns: bool,
    /// The harness can cap the cost.
    pub cost: bool,
}

impl BudgetCaps {
    /// Budget capabilities.
    #[must_use]
    pub const fn new(turns: bool, cost: bool) -> Self {
        Self { turns, cost }
    }
}

/// What a harness can do; the core refuses an experiment that needs a capability the harness lacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
#[allow(clippy::struct_excessive_bools)] // INDEPENDENT: the capabilities are independent flags, not the states of one value
pub struct Capabilities {
    /// The harness stops on a signal and ends its sessions cleanly.
    pub graceful_stop: bool,
    /// The harness streams events while it runs.
    pub live_events: bool,
    /// The budgets the harness enforces.
    pub budgets: BudgetCaps,
    /// The harness has a guard of its own that can confine tool calls.
    pub in_harness_guard: bool,
    /// The driver can report the tool catalog the agent had.
    pub census: bool,
    /// Execution lives in a server outside the child process.
    pub execution_outside_child: bool,
}

impl Capabilities {
    /// Starts a builder with every capability off.
    #[must_use]
    pub fn builder() -> CapabilitiesBuilder {
        CapabilitiesBuilder::default()
    }
}

/// Builds [`Capabilities`]; every capability is off until set.
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct CapabilitiesBuilder {
    caps: Capabilities,
}

impl CapabilitiesBuilder {
    /// Sets `graceful_stop`.
    #[must_use]
    pub const fn graceful_stop(mut self, on: bool) -> Self {
        self.caps.graceful_stop = on;
        self
    }

    /// Sets `live_events`.
    #[must_use]
    pub const fn live_events(mut self, on: bool) -> Self {
        self.caps.live_events = on;
        self
    }

    /// Sets the budgets the harness enforces.
    #[must_use]
    pub const fn budgets(mut self, budgets: BudgetCaps) -> Self {
        self.caps.budgets = budgets;
        self
    }

    /// Sets `in_harness_guard`.
    #[must_use]
    pub const fn in_harness_guard(mut self, on: bool) -> Self {
        self.caps.in_harness_guard = on;
        self
    }

    /// Sets `census`.
    #[must_use]
    pub const fn census(mut self, on: bool) -> Self {
        self.caps.census = on;
        self
    }

    /// Sets `execution_outside_child`.
    #[must_use]
    pub const fn execution_outside_child(mut self, on: bool) -> Self {
        self.caps.execution_outside_child = on;
        self
    }

    /// The capabilities set so far.
    #[must_use]
    pub const fn build(self) -> Capabilities {
        self.caps
    }
}

/// How a supervised process came to an end, as the supervisor saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProcessEnd {
    /// The process exited on its own.
    Exited,
    /// The process was still running at its deadline and was ended.
    Deadline,
    /// The core stopped the process. A run the experimenter stopped (job budget, `candler kill`)
    /// is classified by the core, whatever `classify_exit` returns.
    Stopped,
}

/// The exit of a process: its status, how it ended, and the tail of its output.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProcessExit {
    /// The exit code or the signal.
    pub status: ExitStatus,
    /// Whether the process exited, hit its deadline or was stopped.
    pub ended: ProcessEnd,
    /// The last bytes of the process's output.
    pub tail: Vec<u8>,
}

impl ProcessExit {
    /// A process exit.
    #[must_use]
    pub const fn new(status: ExitStatus, ended: ProcessEnd, tail: Vec<u8>) -> Self {
        Self {
            status,
            ended,
            tail,
        }
    }
}

/// What a reader has seen so far in a harness's output.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Observed {
    /// The ids of the sessions seen, in the order they appeared.
    pub session_ids: Vec<SessionId>,
}

impl Observed {
    /// What was observed.
    #[must_use]
    pub const fn new(session_ids: Vec<SessionId>) -> Self {
        Self { session_ids }
    }
}

/// How a reader takes its frames: one frame per line, or the whole stream as one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(clippy::exhaustive_enums)] // EXHAUSTIVE: a stream is read by lines or as one document, and in no third way
pub enum Framing {
    /// One frame per line of the stream.
    Lines,
    /// One frame holding the whole stream.
    Document,
}

/// The identity of a harness: its plain name, the value `comparable.harness.name` and the experiment's harness request hold.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub struct HarnessId(Name);

impl HarnessId {
    /// An id from a harness name.
    #[must_use]
    pub const fn new(name: Name) -> Self {
        Self(name)
    }

    /// The harness name.
    #[must_use]
    pub const fn as_name<'a>(&'a self) -> &'a Name {
        &self.0
    }
}

impl fmt::Display for HarnessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// The version of a trace format a reader recognised.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub struct FormatVersion(semver::Version);

impl FormatVersion {
    /// A format version.
    #[must_use]
    pub const fn new(version: semver::Version) -> Self {
        Self(version)
    }

    /// The `SemVer` version.
    #[must_use]
    pub const fn as_version<'a>(&'a self) -> &'a semver::Version {
        &self.0
    }
}

impl fmt::Display for FormatVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// How sure a reader is that a sample is in the format it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Confidence {
    /// The sample carries an explicit marker of the format.
    Certain,
    /// The sample has the shape of the format and no marker.
    Likely,
}

/// A file the driver asks the core to write into the workspace.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PreparedFile {
    /// The path relative to the workspace.
    pub path: RelPath,
    /// The bytes of the file.
    pub contents: Vec<u8>,
    /// Whether the file is executable.
    pub executable: bool,
}

impl PreparedFile {
    /// A file to write.
    #[must_use]
    pub const fn new(path: RelPath, contents: Vec<u8>, executable: bool) -> Self {
        Self {
            path,
            contents,
            executable,
        }
    }
}

impl fmt::Debug for PreparedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedFile")
            .field("path", &self.path)
            .field("bytes", &self.contents.len())
            .field("executable", &self.executable)
            .finish()
    }
}

/// What a driver writes into the workspace and adds to the environment before the launch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Prepared {
    /// The files to write.
    pub files: Vec<PreparedFile>,
    /// The variables to add to the child's environment.
    pub env_overlay: Vec<EnvEntry>,
}

impl Prepared {
    /// What the driver prepared.
    #[must_use]
    pub const fn new(files: Vec<PreparedFile>, env_overlay: Vec<EnvEntry>) -> Self {
        Self { files, env_overlay }
    }
}

/// The resolved values of one cell, read by field path.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Cell<'a> {
    values: &'a [ResolvedValue],
}

impl<'a> Cell<'a> {
    /// A view of the resolved values of a cell.
    #[must_use]
    pub const fn new(values: &'a [ResolvedValue]) -> Self {
        Self { values }
    }

    /// The value at `path`, or `None` if the cell has no such path or the source recorded no value.
    #[must_use]
    pub fn get(&self, path: &FieldPath) -> Option<&'a JsonValue> {
        self.values
            .iter()
            .find(|resolved| resolved.path() == path)
            .and_then(ResolvedValue::value)
    }
}

/// The directory a run works in.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Workspace<'a> {
    root: &'a Path,
}

impl<'a> Workspace<'a> {
    /// A workspace rooted at `root`.
    #[must_use]
    pub const fn new(root: &'a Path) -> Self {
        Self { root }
    }

    /// The workspace root.
    #[must_use]
    pub const fn root(&self) -> &'a Path {
        self.root
    }
}
