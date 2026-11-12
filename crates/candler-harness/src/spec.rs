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

/// The placeholder a command or call of a stop or close action holds where the id of the root
/// session goes; the core fills it from the live reader's [`Observed`].
pub const SESSION_ID_PLACEHOLDER: &str = "{session_id}";

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
    /// A session id that cannot be written into a command: it is empty or holds a character
    /// outside `A-Z`, `a-z`, `0-9`, `.`, `_` and `-`, so it could change what a shell script reads.
    #[error("session id {id:?} cannot be written into a command")]
    UnsafeSessionId {
        /// The id that was refused.
        id: String,
    },
}

/// Whether `id` is plain enough to stand in a command line or a script.
fn plain_session_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// `text` with the session placeholder replaced by `id`.
fn fill_session_id(text: &str, id: &str) -> String {
    text.replace(SESSION_ID_PLACEHOLDER, id)
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

    /// Whether an argument or a literal variable holds [`SESSION_ID_PLACEHOLDER`].
    #[must_use]
    pub fn uses_session_id(&self) -> bool {
        self.argv
            .iter()
            .any(|arg| arg.contains(SESSION_ID_PLACEHOLDER))
            || self.env_overlay.iter().any(|entry| match &entry.value {
                EnvValue::Literal(text) => text.contains(SESSION_ID_PLACEHOLDER),
                EnvValue::Secret(_) => false,
            })
    }

    /// The command with [`SESSION_ID_PLACEHOLDER`] replaced by `id` in every argument and
    /// literal variable.
    ///
    /// # Errors
    /// Returns [`CommandSpecError::UnsafeSessionId`] if `id` is empty or holds a character
    /// outside `A-Z`, `a-z`, `0-9`, `.`, `_` and `-`.
    pub fn with_session_id(&self, id: &SessionId) -> Result<Self, CommandSpecError> {
        // BORROW: explicit .as_str() instead of Deref coercion
        let id = id.as_str();
        if !plain_session_id(id) {
            return Err(CommandSpecError::UnsafeSessionId {
                // BORROW: .to_owned() keeps the refused id in the error
                id: id.to_owned(),
            });
        }
        Ok(Self {
            argv: self
                .argv
                .iter()
                // BORROW: explicit .as_str() instead of Deref coercion
                .map(|arg| fill_session_id(arg.as_str(), id))
                .collect(),
            env_overlay: self
                .env_overlay
                .iter()
                .map(|entry| match &entry.value {
                    EnvValue::Literal(text) => EnvEntry::literal(
                        // BORROW: .clone() copies the name into the new entry
                        entry.name.clone(),
                        // BORROW: explicit .as_str() instead of Deref coercion
                        fill_session_id(text.as_str(), id),
                    ),
                    // BORROW: .clone() copies the entry unchanged
                    EnvValue::Secret(_) => entry.clone(),
                })
                .collect(),
            // BORROW: .clone() copies the working directory
            cwd: self.cwd.clone(),
        })
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

/// An action the core runs before it signals a process: a command on the host, a command inside
/// the agent's isolation, or an HTTP call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    /// A command the core runs on the host.
    Command(CommandSpec),
    /// A command the core runs inside the agent's isolation, with the agent's user, environment
    /// and mounts. It may hold [`SESSION_ID_PLACEHOLDER`]. The command, not the core, finds
    /// whatever the harness hides from the host, such as a loopback port or a password.
    Exec(CommandSpec),
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

    /// A command to run inside the agent's isolation.
    #[must_use]
    pub const fn exec(spec: CommandSpec) -> Self {
        Self::Exec(spec)
    }

    /// Whether the action holds [`SESSION_ID_PLACEHOLDER`].
    #[must_use]
    pub fn uses_session_id(&self) -> bool {
        match self {
            Self::Command(spec) | Self::Exec(spec) => spec.uses_session_id(),
            Self::Http { url, body, .. } => {
                url.contains(SESSION_ID_PLACEHOLDER)
                    || body
                        .as_ref()
                        .is_some_and(|text| text.contains(SESSION_ID_PLACEHOLDER))
            }
        }
    }

    /// The action the core can run now: the action itself when it holds no placeholder, the action
    /// with the placeholder filled when `id` is a plain id, and `None` when it needs an id the core
    /// does not have or cannot write into it. The core then falls back to the signal.
    #[must_use]
    pub fn resolve(&self, id: Option<&SessionId>) -> Option<Self> {
        if !self.uses_session_id() {
            return Some(self.clone()); // BORROW: .clone() copies the action
        }
        self.with_session_id(id?).ok()
    }

    /// The action with [`SESSION_ID_PLACEHOLDER`] replaced by `id`.
    ///
    /// # Errors
    /// Returns [`CommandSpecError::UnsafeSessionId`] if `id` is empty or holds a character
    /// outside `A-Z`, `a-z`, `0-9`, `.`, `_` and `-`.
    pub fn with_session_id(&self, id: &SessionId) -> Result<Self, CommandSpecError> {
        match self {
            Self::Command(spec) => spec.with_session_id(id).map(Self::Command),
            Self::Exec(spec) => spec.with_session_id(id).map(Self::Exec),
            Self::Http { method, url, body } => {
                // BORROW: explicit .as_str() instead of Deref coercion
                let text = id.as_str();
                if !plain_session_id(text) {
                    return Err(CommandSpecError::UnsafeSessionId {
                        // BORROW: .to_owned() keeps the refused id in the error
                        id: text.to_owned(),
                    });
                }
                Ok(Self::Http {
                    method: *method,
                    // BORROW: explicit .as_str() instead of Deref coercion
                    url: fill_session_id(url.as_str(), text),
                    body: body
                        .as_ref()
                        // BORROW: explicit .as_str() instead of Deref coercion
                        .map(|body| fill_session_id(body.as_str(), text)),
                })
            }
        }
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
    /// The standard output of a command the core runs on the host, archived as the stream `name`.
    #[non_exhaustive]
    Command {
        /// The command the core runs.
        spec: CommandSpec,
        /// The name of the stream that archives the output.
        name: StreamName,
    },
    /// The standard output of a command the core runs inside the agent's isolation, archived as
    /// the stream `name`. The command inherits the run's environment: the variables of the
    /// driver's [`StatePath`]s hold the values the run used, which the core supplies. A harness
    /// that keeps its sessions in per-run state reads them through this form.
    ///
    /// The command inherits the whole run environment, including the variable that holds the
    /// gateway token, and its output is archived raw: the command must not print its environment.
    #[non_exhaustive]
    Exec {
        /// The command the core runs inside the isolation.
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

    /// The output of `spec`, run inside the agent's isolation with the run's state environment,
    /// archived as the stream `name`.
    #[must_use]
    pub const fn exec(spec: CommandSpec, name: StreamName) -> Self {
        Self::Exec { spec, name }
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "INDEPENDENT: the capabilities are independent flags, not the states of one value"
)]
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
    /// The harness runs a session: its prompt goes through stdin, stdin stays open and a
    /// background job's notification can start a further turn. A driver that sets it also
    /// returns a [`SessionSpec`] and a [`SessionTracker`].
    pub session: bool,
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

    /// Sets `session`.
    #[must_use]
    pub const fn session(mut self, on: bool) -> Self {
        self.caps.session = on;
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
    /// The tools and MCP servers the agent had, when the stream shows them; `None` while the
    /// reader has seen nothing that names them, which is not an empty inventory.
    pub init: Option<InitReport>,
    /// The last word of the stream on how the run ended, when it has one.
    pub end: Option<ObservedEnd>,
}

impl Observed {
    /// What was observed: the session ids, no inventory and no end.
    #[must_use]
    pub const fn new(session_ids: Vec<SessionId>) -> Self {
        Self {
            session_ids,
            init: None,
            end: None,
        }
    }

    /// The same view with the inventory the stream showed.
    #[must_use]
    pub fn with_init(mut self, init: InitReport) -> Self {
        self.init = Some(init);
        self
    }

    /// The same view with the last word of the stream on how the run ended.
    #[must_use]
    pub fn with_end(mut self, end: ObservedEnd) -> Self {
        self.end = Some(end);
        self
    }

    /// The id of the root session: the first one seen.
    #[must_use]
    pub fn root_session_id<'a>(&'a self) -> Option<&'a SessionId> {
        self.session_ids.first()
    }
}

/// Where an inventory of tools came from. Two inventories from different sources are never the
/// same observation, and neither equals an unrecorded one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CensusSource {
    /// The harness reported it, in an init event of its own stream.
    Harness,
    /// The gateway saw it, in the tool names of the model requests it relayed.
    Gateway,
    /// A command the harness runs inside the isolation listed it, and the core archived the
    /// output.
    Collected,
}

/// Why an inventory could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum InitReportError {
    /// A tool list that is not a list of names.
    #[error("the tool list is not a list of names")]
    ToolsNotNames,
    /// A tool list that holds an empty name.
    #[error("the tool list holds an empty name")]
    EmptyToolName,
    /// A server list that is not a list.
    #[error("the server list is not a list")]
    ServersNotList,
    /// An empty tool list from a source other than the harness's own init event. Such a source
    /// that saw no tool has seen nothing: the census is unrecorded, not empty.
    #[error("a collected or gateway source saw no tool, which is an unrecorded inventory")]
    NoTools,
}

/// The tools and MCP servers an agent had at its start, as the raw JSON the census reads, and the
/// source that observed them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InitReport {
    /// The tool names, a JSON list of strings.
    pub tools: serde_json::Value,
    /// The MCP servers, a JSON list, or `None` when the source cannot see servers: that is
    /// unrecorded, not an empty list.
    pub mcp_servers: Option<serde_json::Value>,
    /// Who observed the inventory.
    pub source: CensusSource,
}

impl InitReport {
    /// An inventory.
    ///
    /// # Errors
    /// Returns [`InitReportError::ToolsNotNames`] if `tools` is not a list of strings.
    /// Returns [`InitReportError::ServersNotList`] if `mcp_servers` is given and is not a list.
    /// Returns [`InitReportError::EmptyToolName`] if a tool name is empty.
    /// Returns [`InitReportError::NoTools`] if `source` is not [`CensusSource::Harness`] and
    /// `tools` is empty.
    pub fn new(
        tools: serde_json::Value,
        mcp_servers: Option<serde_json::Value>,
        source: CensusSource,
    ) -> Result<Self, InitReportError> {
        let names = tools
            .as_array()
            .is_some_and(|list| list.iter().all(serde_json::Value::is_string));
        if !names {
            return Err(InitReportError::ToolsNotNames);
        }
        if tools
            .as_array()
            .is_some_and(|list| list.iter().any(|tool| tool.as_str() == Some("")))
        {
            return Err(InitReportError::EmptyToolName);
        }
        if source != CensusSource::Harness && tools.as_array().is_some_and(Vec::is_empty) {
            return Err(InitReportError::NoTools);
        }
        if mcp_servers
            .as_ref()
            .is_some_and(|servers| !servers.is_array())
        {
            return Err(InitReportError::ServersNotList);
        }
        Ok(Self {
            tools,
            mcp_servers,
            source,
        })
    }
}

/// What the stream itself said about the end of the run, which the exit status alone cannot say:
/// a harness that exits 1 for a failure, an interrupt through its API and an abort alike is told
/// apart by its last error event.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ObservedEnd {
    /// The stream ended with no error.
    Finished,
    /// The last error event, with the harness's own name for its kind.
    Error {
        /// The harness's name for the error.
        kind: String,
    },
    /// The stream says the session was aborted or interrupted from outside.
    Aborted,
}

/// The names of the variables through which the core hands the gateway to the agent. The core
/// sets the values; the driver's configuration refers to the names.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GatewayEnv {
    /// The variable that carries the gateway's address.
    pub base_url: String,
    /// The variable that carries the token the agent presents.
    pub auth_token: String,
    /// Every name the core owns, which a driver's own environment may not set. It holds
    /// `base_url` and `auth_token`.
    pub reserved: Vec<String>,
}

impl GatewayEnv {
    /// A gateway environment.
    #[must_use]
    pub const fn new(base_url: String, auth_token: String, reserved: Vec<String>) -> Self {
        Self {
            base_url,
            auth_token,
            reserved,
        }
    }
}

/// What a state path names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StateKind {
    /// A directory the core creates.
    Dir,
    /// A file the harness creates; the core creates its parent directory.
    File,
}

/// A place a harness keeps state, inside the agent's home: the core creates it, sets `env` to its
/// absolute path inside the container, and keeps it per run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct StatePath {
    /// The variable the core sets to the path.
    pub env: String,
    /// Whether the path is a directory or a file.
    pub kind: StateKind,
    /// The path relative to the agent's home.
    pub subpath: RelPath,
}

impl StatePath {
    /// A state path.
    #[must_use]
    pub const fn new(env: String, kind: StateKind, subpath: RelPath) -> Self {
        Self { env, kind, subpath }
    }
}

/// What can wake an agent that has ended its turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PendingKind {
    /// A background task.
    Task,
    /// A monitor.
    Monitor,
    /// A cron.
    Cron,
    /// A one-shot wake-up.
    WakeUp,
}

/// One thing pending that could wake the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PendingItem {
    /// What it is.
    pub kind: PendingKind,
    /// The harness's id for it.
    pub id: String,
}

impl PendingItem {
    /// A pending item.
    #[must_use]
    pub const fn new(kind: PendingKind, id: String) -> Self {
        Self { kind, id }
    }
}

/// Where a session stands: how many turns have ended, whether the latest has, and what is pending.
///
/// An empty `pending` means nothing is pending only when `known` is true. The core must not close
/// a session while `known` is false. A harness whose stream does not show every kind of pending
/// work (Claude Code's stream lists background tasks, never wake-ups or crons) leaves `known`
/// false until its other source has been read, so a turn that ends with no such record keeps
/// `known` false and the session runs to its cap instead of closing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct PendingState {
    /// The number of turns that have ended.
    pub turns: u64,
    /// Whether the latest turn has ended and no later one has started.
    pub turn_ended: bool,
    /// Whether `pending` was read from the harness: false until a readable source has been seen,
    /// and again after any list or entry that could not be read (`pending` then keeps the last
    /// readable list). Pending work that cannot be read is never "nothing".
    pub known: bool,
    /// What is pending and could wake the agent.
    pub pending: Vec<PendingItem>,
}

impl PendingState {
    /// A state.
    #[must_use]
    pub const fn new(turns: u64, turn_ended: bool, known: bool, pending: Vec<PendingItem>) -> Self {
        Self {
            turns,
            turn_ended,
            known,
            pending,
        }
    }
}

/// One record a session tracker reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SessionRecord<'a> {
    /// One frame of the harness's live output.
    Stream(&'a [u8]),
    /// One line of the hook log.
    Hook(&'a [u8]),
}

/// Follows a session from its stream frames and hook records. It holds no clock; the core decides
/// when to close.
pub trait SessionTracker: Send {
    /// Reads one record.
    ///
    /// # Errors
    /// Returns [`crate::driver::DriverError::Record`] if the record cannot be read.
    fn feed(&mut self, record: SessionRecord<'_>) -> Result<(), crate::driver::DriverError>;

    /// Where the session stands after the records read so far.
    fn state(&self) -> PendingState;
}

/// How the core ends a session.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SessionClose {
    /// The core closes the child's stdin.
    Eof,
    /// The core runs an action, which may hold [`SESSION_ID_PLACEHOLDER`].
    Action(Action),
}

/// Where the observation hooks write and how the driver installs them. The hooks only observe:
/// they never block, decide or print.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HookSpec {
    /// The files that install the hooks, rooted at the agent's home ([`FileRoot::Home`]), outside
    /// the workspace.
    pub files: Vec<PreparedFile>,
    /// The file the hooks append to, relative to the agent's home.
    pub log: RelPath,
    /// The raw stream that archives the log.
    pub stream: StreamName,
}

impl HookSpec {
    /// A hook spec.
    #[must_use]
    pub const fn new(files: Vec<PreparedFile>, log: RelPath, stream: StreamName) -> Self {
        Self { files, log, stream }
    }
}

/// How a harness runs a session: the command, the bytes that open it, how to close it and the
/// hooks that observe it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SessionSpec {
    /// The command that runs the cell as a session; the core keeps its stdin open.
    pub command: CommandSpec,
    /// The bytes the core writes to the child's stdin first; empty when the harness takes its
    /// prompt another way.
    pub input: Vec<u8>,
    /// How the core ends the session.
    pub close: SessionClose,
    /// The hooks that observe the session, if the driver installs any.
    pub hooks: Option<HookSpec>,
}

impl SessionSpec {
    /// A session spec without hooks.
    #[must_use]
    pub const fn new(command: CommandSpec, input: Vec<u8>, close: SessionClose) -> Self {
        Self {
            command,
            input,
            close,
            hooks: None,
        }
    }

    /// The same spec with observation hooks.
    #[must_use]
    pub fn with_hooks(mut self, hooks: HookSpec) -> Self {
        self.hooks = Some(hooks);
        self
    }
}

/// How a reader takes its frames: one frame per line, or the whole stream as one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[expect(
    clippy::exhaustive_enums,
    reason = "EXHAUSTIVE: a raw frame file holds a stream as one frame per line or as one frame, and a third framing is a new frame-file format that changes this enum and the frame writer together"
)]
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

/// What a [`PreparedFile`]'s path is relative to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FileRoot {
    /// The workspace.
    #[default]
    Workspace,
    /// The agent's home directory, outside the workspace: where a harness reads its own
    /// configuration and skills.
    Home,
}

/// A file the driver asks the core to write, into the workspace or the agent's home.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PreparedFile {
    /// The path relative to the file's root.
    pub path: RelPath,
    /// What the path is relative to.
    pub root: FileRoot,
    /// The bytes of the file.
    pub contents: Vec<u8>,
    /// Whether the file is executable.
    pub executable: bool,
}

impl PreparedFile {
    /// A file to write into the workspace.
    #[must_use]
    pub const fn new(path: RelPath, contents: Vec<u8>, executable: bool) -> Self {
        Self {
            path,
            root: FileRoot::Workspace,
            contents,
            executable,
        }
    }

    /// A file to write into the agent's home.
    #[must_use]
    pub const fn in_home(path: RelPath, contents: Vec<u8>, executable: bool) -> Self {
        Self {
            path,
            root: FileRoot::Home,
            contents,
            executable,
        }
    }
}

impl fmt::Debug for PreparedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedFile")
            .field("path", &self.path)
            .field("root", &self.root)
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
