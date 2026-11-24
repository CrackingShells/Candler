//! The data a harness driver hands to the core: commands, stop and collect specs, capabilities,
//! the exit of a process, and the read-only views of a cell and a workspace.
//!
//! None of these types acts on a process. A [`CommandSpec`] names a program, its arguments and the
//! environment it adds; the core spawns it. An [`EnvValue::Secret`] names a secret and never holds
//! its value, and the `Debug` output of a command or an environment entry shows names only.

use std::fmt;
use std::path::Path;
use std::time::Duration;

use candler_schemas::distinct::first_repeat;
use candler_schemas::execution::paths;
use candler_schemas::field_path;
use candler_schemas::primitives::{
    FieldPath, Name, RelPath, SecretName, is_env_var_name, plain_segment,
};
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::StreamName;
use candler_schemas::servers::McpServerKind;
use candler_schemas::trace::SessionId;
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::{Map, Value};

use crate::driver::DriverError;

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
    #[error("environment variable {name:?} is declared twice")]
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
    /// The command that runs `program` with the arguments `rest`, adding no variable and setting no directory.
    /// It cannot fail: the argument vector holds at least the program.
    #[must_use]
    pub fn new(program: String, rest: Vec<String>) -> Self {
        let mut words = vec![program];
        words.extend(rest);
        Self {
            argv: words,
            env_overlay: Vec::new(),
            cwd: None,
        }
    }

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
        if !plain_segment(id) {
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
        // BORROW: explicit .as_str() instead of Deref coercion
        if let Some(name) = first_repeat(self.env_overlay.iter().map(|entry| entry.name.as_str())) {
            return Err(CommandSpecError::DuplicateEnv {
                // BORROW: .to_owned() keeps the offending name in the error
                name: name.to_owned(),
            });
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
                if !plain_segment(text) {
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
    /// The command runs in a fresh container with no network, sees the agent's state and the state
    /// variables of `state_paths()` but none of the gateway's variables, and its output is
    /// archived raw.
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

/// How a driver meets one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_enums,
    reason = "EXHAUSTIVE: the drivers live in other crates and match on every answer; a variant added here must break each of them."
)]
pub enum Answer {
    /// The harness has a construct with the key's meaning, and the driver writes it.
    Mapped {
        /// The harness construct, in a few words.
        construct: &'static str,
    },
    /// Candler does it itself, as the core's enforcement of a budget does.
    Candler {
        /// What Candler does, in a few words.
        how: &'static str,
    },
    /// Nobody does it: the harness has no construct for it and Candler does not stand in.
    Refused {
        /// Why, in a few words.
        reason: &'static str,
    },
}

impl Answer {
    /// Whether the answer is a refusal.
    #[must_use]
    pub const fn is_refused(self) -> bool {
        matches!(self, Self::Refused { .. })
    }

    /// The words of the answer: the construct, the Candler action or the reason.
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::Mapped { construct } => construct,
            Self::Candler { how } => how,
            Self::Refused { reason } => reason,
        }
    }
}

/// The effort levels of `model.effort`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_enums,
    reason = "EXHAUSTIVE: the drivers live in other crates and answer each level; a new level must break each of them."
)]
pub enum EffortLevel {
    /// `low`.
    Low,
    /// `medium`.
    Medium,
    /// `high`.
    High,
    /// `xhigh`.
    Xhigh,
    /// `max`.
    Max,
    /// `ultracode`: `xhigh` with Claude Code's ultracode mode on.
    Ultracode,
}

impl EffortLevel {
    /// Every level.
    pub const ALL: [Self; 6] = [
        Self::Low,
        Self::Medium,
        Self::High,
        Self::Xhigh,
        Self::Max,
        Self::Ultracode,
    ];

    /// The level a cell's text names, or `None` for any other text.
    #[must_use]
    pub fn from_text(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.as_str() == text)
    }

    /// The text a cell writes for the level.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
            Self::Ultracode => "ultracode",
        }
    }
}

/// The permission modes of `permission_mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_enums,
    reason = "EXHAUSTIVE: the drivers live in other crates and answer each mode; a new mode must break each of them."
)]
pub enum PermissionMode {
    /// `default`.
    Default,
    /// `acceptEdits`.
    AcceptEdits,
    /// `plan`.
    Plan,
    /// `bypassPermissions`.
    BypassPermissions,
}

impl PermissionMode {
    /// Every mode.
    pub const ALL: [Self; 4] = [
        Self::Default,
        Self::AcceptEdits,
        Self::Plan,
        Self::BypassPermissions,
    ];

    /// The mode a cell's text names, or `None` for any other text.
    #[must_use]
    pub fn from_text(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.as_str() == text)
    }

    /// The text a cell writes for the mode.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::Plan => "plan",
            Self::BypassPermissions => "bypassPermissions",
        }
    }
}

/// What an experiment sets that reaches the agent. A key whose values form a closed set carries
/// the value, so a new value is a compile error in every driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_enums,
    reason = "EXHAUSTIVE: the drivers live in other crates and answer each key with one exhaustive match; a wildcard arm would let a new key go unanswered."
)]
pub enum AgentSetting {
    /// `model.id`.
    ModelId,
    /// `model.endpoint`.
    ModelEndpoint,
    /// `model.effort`, at one level.
    Effort(EffortLevel),
    /// `permission_mode`, at one mode.
    PermissionMode(PermissionMode),
    /// `tools.allowed`.
    ToolsAllowed,
    /// `tools.disallowed`.
    ToolsDisallowed,
    /// `env`.
    Env,
    /// `inputs`.
    Inputs,
    /// The skills of the treatment.
    Skills,
    /// The transport of an MCP server.
    McpTransport(McpServerKind),
    /// The `codemode` of an MCP server, at one value.
    Codemode(bool),
    /// The `name` of an MCP server.
    McpName,
    /// `budget.per_run.wall_s`.
    WallBudget,
    /// `budget.per_run.turns`.
    TurnBudget,
    /// `budget.per_run.max_cost_usd`.
    CostBudget,
    /// `runner.mode`.
    SessionMode,
}

impl AgentSetting {
    /// Every key, each closed value once.
    pub const ALL: [Self; 29] = [
        Self::ModelId,
        Self::ModelEndpoint,
        Self::Effort(EffortLevel::Low),
        Self::Effort(EffortLevel::Medium),
        Self::Effort(EffortLevel::High),
        Self::Effort(EffortLevel::Xhigh),
        Self::Effort(EffortLevel::Max),
        Self::Effort(EffortLevel::Ultracode),
        Self::PermissionMode(PermissionMode::Default),
        Self::PermissionMode(PermissionMode::AcceptEdits),
        Self::PermissionMode(PermissionMode::Plan),
        Self::PermissionMode(PermissionMode::BypassPermissions),
        Self::ToolsAllowed,
        Self::ToolsDisallowed,
        Self::Env,
        Self::Inputs,
        Self::Skills,
        Self::McpTransport(McpServerKind::Stdio),
        Self::McpTransport(McpServerKind::Local),
        Self::McpTransport(McpServerKind::Http),
        Self::McpTransport(McpServerKind::Sse),
        Self::McpTransport(McpServerKind::Remote),
        Self::Codemode(false),
        Self::Codemode(true),
        Self::McpName,
        Self::WallBudget,
        Self::TurnBudget,
        Self::CostBudget,
        Self::SessionMode,
    ];

    /// The cell path the key is set at; the skills come from the treatment.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::ModelId => paths::MODEL_ID,
            Self::ModelEndpoint => paths::MODEL_ENDPOINT,
            Self::Effort(_) => paths::MODEL_EFFORT,
            Self::PermissionMode(_) => paths::PERMISSION_MODE,
            Self::ToolsAllowed => paths::TOOLS_ALLOWED,
            Self::ToolsDisallowed => paths::TOOLS_DISALLOWED,
            Self::Env => paths::ENV,
            Self::Inputs => paths::INPUTS,
            Self::Skills => paths::TREATMENT,
            Self::McpTransport(_) | Self::Codemode(_) | Self::McpName => paths::MCP,
            Self::WallBudget => paths::BUDGET_PER_RUN_WALL_S,
            Self::TurnBudget => paths::BUDGET_PER_RUN_TURNS,
            Self::CostBudget => paths::BUDGET_PER_RUN_MAX_COST_USD,
            Self::SessionMode => paths::RUNNER_MODE,
        }
    }

    /// The keys `cell` sets, in the order of [`AgentSetting::ALL`]'s families, each once. The
    /// skills and the session mode are not read here: the treatment holds the first, and the core
    /// refuses the second by the plan's session.
    ///
    /// # Errors
    /// [`DriverError::InvalidValue`] when the cell's MCP servers are not a table of objects, and
    /// the error of [`Cell::table_at`].
    pub fn set_by(cell: &Cell<'_>) -> Result<Vec<Self>, DriverError> {
        let mut set: Vec<Self> = Vec::new();
        let mut add = |setting: Self| {
            if !set.contains(&setting) {
                set.push(setting);
            }
        };
        let text_at = |path: &str| -> Result<Option<&str>, DriverError> {
            cell.value_str(path)
                .map(|value| value.and_then(Value::as_str))
                .map_err(|error| DriverError::VersionOutput {
                    detail: error.to_string(), // BORROW: .to_string() gives owned text
                })
        };
        let is_set = |path: &str| -> Result<bool, DriverError> {
            cell.value_str(path)
                .map(|value| value.is_some())
                .map_err(|error| DriverError::VersionOutput {
                    detail: error.to_string(), // BORROW: .to_string() gives owned text
                })
        };
        for (path, setting) in [
            (paths::MODEL_ID, Self::ModelId),
            (paths::MODEL_ENDPOINT, Self::ModelEndpoint),
        ] {
            if is_set(path)? {
                add(setting);
            }
        }
        if let Some(level) = text_at(paths::MODEL_EFFORT)?.and_then(EffortLevel::from_text) {
            add(Self::Effort(level));
        }
        if let Some(mode) = text_at(paths::PERMISSION_MODE)?.and_then(PermissionMode::from_text) {
            add(Self::PermissionMode(mode));
        }
        for (path, setting) in [
            (paths::TOOLS_ALLOWED, Self::ToolsAllowed),
            (paths::TOOLS_DISALLOWED, Self::ToolsDisallowed),
            (paths::ENV, Self::Env),
            (paths::INPUTS, Self::Inputs),
        ] {
            if is_set(path)? {
                add(setting);
            }
        }
        if let Some(table) = cell.table_at(&field_path!("mcp.servers"))? {
            for entry in table.values() {
                let Value::Object(server) = entry else {
                    continue;
                };
                let kind = server
                    .get("type")
                    .and_then(|kind| serde_json::from_value::<McpServerKind>(kind.clone()).ok()); // BORROW: .clone() copies the value out of the table
                let kind = match (
                    kind,
                    server.contains_key("command"),
                    server.contains_key("url"),
                ) {
                    (Some(kind), _, _) => Some(kind),
                    (None, true, _) => Some(McpServerKind::Stdio),
                    (None, false, true) => Some(McpServerKind::Remote),
                    (None, false, false) => None,
                };
                if let Some(kind) = kind {
                    add(Self::McpTransport(kind));
                }
                if let Some(on) = server.get("codemode").and_then(Value::as_bool) {
                    add(Self::Codemode(on));
                }
                if server.contains_key("name") {
                    add(Self::McpName);
                }
            }
        }
        for (path, setting) in [
            (paths::BUDGET_PER_RUN_WALL_S, Self::WallBudget),
            (paths::BUDGET_PER_RUN_TURNS, Self::TurnBudget),
            (paths::BUDGET_PER_RUN_MAX_COST_USD, Self::CostBudget),
        ] {
            if is_set(path)? {
                add(setting);
            }
        }
        Ok(set)
    }
}

/// What a driver does on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_enums,
    reason = "EXHAUSTIVE: the drivers live in other crates and answer each fact with one exhaustive match; a wildcard arm would let a new fact go unanswered."
)]
pub enum DriverFact {
    /// The harness stops on a signal and ends its sessions cleanly.
    GracefulStop,
    /// The harness streams events while it runs.
    LiveEvents,
    /// The harness has a guard of its own that can confine tool calls.
    InHarnessGuard,
    /// Execution lives in a server outside the child process.
    ExecutionOutsideChild,
    /// The driver reports the tool inventory the agent had.
    Census,
}

impl DriverFact {
    /// Every fact.
    pub const ALL: [Self; 5] = [
        Self::GracefulStop,
        Self::LiveEvents,
        Self::InHarnessGuard,
        Self::ExecutionOutsideChild,
        Self::Census,
    ];
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

/// What the harness's own closing report says the run spent, as the harness wrote each value. A
/// value the harness did not report is `None`: unrecorded, never zero.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct RunTotals {
    /// The token usage of the run.
    pub usage: Option<serde_json::Value>,
    /// The cost of the run in US dollars.
    pub cost_usd: Option<serde_json::Value>,
    /// The number of turns of the run.
    pub num_turns: Option<serde_json::Value>,
}

impl RunTotals {
    /// Totals that hold what a harness reported; each is `None` when it was not reported.
    #[must_use]
    pub const fn new(
        usage: Option<serde_json::Value>,
        cost_usd: Option<serde_json::Value>,
        num_turns: Option<serde_json::Value>,
    ) -> Self {
        Self {
            usage,
            cost_usd,
            num_turns,
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
    /// What the stream's closing report says the run spent; the default reports none.
    pub totals: RunTotals,
}

impl Observed {
    /// What was observed: the session ids, no inventory and no end.
    #[must_use]
    pub const fn new(session_ids: Vec<SessionId>) -> Self {
        Self {
            session_ids,
            init: None,
            end: None,
            totals: RunTotals::new(None, None, None),
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

    /// The same view with the totals the stream's closing report gave.
    #[must_use]
    pub fn with_totals(mut self, totals: RunTotals) -> Self {
        self.totals = totals;
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

/// How the core builds a harness's base image.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BaseImage {
    /// The directory under `assets/docker` that holds the build context.
    pub context: &'static str,
    /// The build arguments, each `(name, value)`: the harness version the image installs. The
    /// Dockerfile gives them no default, so a build without them fails.
    pub build_args: Vec<(String, String)>,
}

impl BaseImage {
    /// A base image built from `assets/docker/<context>` with `build_args`.
    #[must_use]
    pub const fn new(context: &'static str, build_args: Vec<(String, String)>) -> Self {
        Self {
            context,
            build_args,
        }
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

    /// The JSON value at `path`, if the cell has one.
    #[must_use]
    pub fn value(&self, path: &FieldPath) -> Option<&'a Value> {
        self.get(path).map(JsonValue::as_value)
    }

    /// The JSON value at the path written as `path`, if the cell has one.
    ///
    /// # Errors
    /// [`FormatError`](candler_schemas::primitives::FormatError) when `path` is not a field path.
    pub fn value_str(
        &self,
        path: &str,
    ) -> Result<Option<&'a Value>, candler_schemas::primitives::FormatError> {
        let field: FieldPath = path.parse()?;
        Ok(self.value(&field))
    }

    /// The table the cell holds at `root`, rebuilt from the values at and below it: each leaf is
    /// placed at its path under `root`, one object per segment, and an object at `root` itself is
    /// merged in whole. `None` when the cell sets nothing there.
    ///
    /// # Errors
    /// [`DriverError::InvalidValue`] at the offending path when a leaf lies below a value that is
    /// not a table, or the value at `root` itself is not an object.
    pub fn table_at(
        &self,
        root: &FieldPath,
    ) -> Result<Option<serde_json::Map<String, Value>>, DriverError> {
        let depth = root.segments().count();
        let mut table = serde_json::Map::new();
        for (path, value) in self.below(root) {
            let relative: Vec<&str> = path.segments().skip(depth).collect();
            let Some((leaf, parents)) = relative.split_last() else {
                match value {
                    Value::Object(inner) => table.extend(inner.clone()), // BORROW: .clone() copies the table out of the cell
                    Value::Null
                    | Value::Bool(_)
                    | Value::Number(_)
                    | Value::String(_)
                    | Value::Array(_) => {
                        return Err(DriverError::InvalidValue {
                            path: path.clone(),                    // BORROW: the error owns the path
                            detail: "is not an object".to_owned(), // BORROW: .to_owned() makes the owned text
                        });
                    }
                }
                continue;
            };
            let mut slot = &mut table;
            for segment in parents {
                match slot
                    .entry((*segment).to_owned()) // BORROW: .to_owned() makes the owned key
                    .or_insert_with(|| Value::Object(serde_json::Map::new()))
                {
                    Value::Object(inner) => slot = inner,
                    Value::Null
                    | Value::Bool(_)
                    | Value::Number(_)
                    | Value::String(_)
                    | Value::Array(_) => {
                        return Err(DriverError::InvalidValue {
                            path: path.clone(), // BORROW: the error owns the path
                            detail: "lies below a value that is not a table".to_owned(), // BORROW: .to_owned() makes the owned text
                        });
                    }
                }
            }
            // BORROW: .to_owned() makes the owned key; .clone() copies the value out of the cell
            slot.insert((*leaf).to_owned(), value.clone());
        }
        Ok((!table.is_empty()).then_some(table))
    }

    /// The values at `prefix` or below it, each with its path: a table of the cell reaches it as
    /// one value per leaf, so a table is read by its prefix.
    #[must_use]
    pub fn below(&self, prefix: &FieldPath) -> Vec<(&'a FieldPath, &'a Value)> {
        let root = prefix.as_str(); // BORROW: explicit .as_str() instead of Deref coercion
        self.values
            .iter()
            .filter(|resolved| resolved.path().relative_to(root).is_some())
            .filter_map(|resolved| {
                resolved
                    .value()
                    .map(|json| (resolved.path(), json.as_value()))
            })
            .collect()
    }

    /// The text at `path`, if the cell has a value there.
    ///
    /// # Errors
    /// [`DriverError::InvalidValue`] when the value is not text.
    pub fn text(&self, path: &FieldPath) -> Result<Option<&'a str>, DriverError> {
        self.value(path)
            .map(|value| {
                value.as_str().ok_or_else(|| DriverError::InvalidValue {
                    path: path.clone(),               // BORROW: .clone() gives the error its own path
                    detail: "is not text".to_owned(), // BORROW: .to_owned() makes the owned text
                })
            })
            .transpose()
    }

    /// The text at `path`, which the cell must have.
    ///
    /// # Errors
    /// [`DriverError::MissingValue`] when the cell has no value there, and
    /// [`DriverError::InvalidValue`] when the value is not text.
    pub fn required_text(&self, path: &FieldPath) -> Result<&'a str, DriverError> {
        self.text(path)?.ok_or_else(|| DriverError::MissingValue {
            path: path.clone(), // BORROW: .clone() gives the error its own path
        })
    }

    /// The model id, which the cell must have.
    ///
    /// # Errors
    /// [`DriverError`] as [`Cell::required_text`] returns it.
    pub fn model(&self) -> Result<&'a str, DriverError> {
        self.required_text(&field_path!(paths::MODEL_ID))
    }

    /// The permission mode, which the cell must have.
    ///
    /// # Errors
    /// [`DriverError`] as [`Cell::required_text`] returns it.
    pub fn permission_mode(&self) -> Result<&'a str, DriverError> {
        self.required_text(&field_path!(paths::PERMISSION_MODE))
    }

    /// The reasoning effort, if the cell sets one.
    ///
    /// # Errors
    /// [`DriverError::InvalidValue`] when the value is not text.
    pub fn effort(&self) -> Result<Option<&'a str>, DriverError> {
        self.text(&field_path!(paths::MODEL_EFFORT))
    }

    /// The prompt: the cell's template, filled by [`render_prompt`] from its values.
    ///
    /// # Errors
    /// [`DriverError::MissingValue`] when the cell has no template, and
    /// [`DriverError::InvalidValue`] when the template is not text, the values are not an object,
    /// or [`render_prompt`] refuses them.
    pub fn prompt(&self) -> Result<String, DriverError> {
        let template = self.required_text(&field_path!(paths::PROMPT_TEMPLATE))?;
        let values_path = field_path!(paths::PROMPT_VALUES);
        let values = match self.value(&values_path) {
            None => None,
            Some(Value::Object(map)) => Some(map),
            Some(
                Value::Null
                | Value::Bool(_)
                | Value::Number(_)
                | Value::String(_)
                | Value::Array(_),
            ) => {
                return Err(DriverError::InvalidValue {
                    path: values_path,
                    detail: "is not an object".to_owned(), // BORROW: .to_owned() makes the owned text
                });
            }
        };
        render_prompt(template, values)
    }
}

/// The scalar `value` as text, or `None` when it is not a scalar.
fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()), // BORROW: .clone() copies the text out of the cell
        Value::Number(number) => Some(number.to_string()), // BORROW: .to_string() gives owned text
        Value::Bool(flag) => Some(flag.to_string()), // BORROW: .to_string() gives owned text
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// The prompt: `template` with each `{key}` replaced by the scalar value of `values.<key>`.
///
/// A brace pair that does not hold a plain key stays as written.
///
/// # Errors
/// [`DriverError::InvalidValue`] at `prompt.values` when a key has no value or its value is not a
/// scalar.
pub fn render_prompt(
    template: &str,
    values: Option<&Map<String, Value>>,
) -> Result<String, DriverError> {
    let invalid = |detail: String| DriverError::InvalidValue {
        path: field_path!(paths::PROMPT_VALUES),
        detail,
    };
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let (before, from_open) = rest.split_at(open);
        out.push_str(before);
        let after_open = from_open.get(1..).unwrap_or_default();
        match after_open.split_once('}') {
            Some((key, tail)) if is_env_var_name(key) => {
                let value = values.and_then(|map| map.get(key)).ok_or_else(|| {
                    invalid(format!("the template names `{key}` and no value is given"))
                })?;
                let text = scalar_text(value)
                    .ok_or_else(|| invalid(format!("the value of `{key}` is not a scalar")))?;
                out.push_str(text.as_str()); // BORROW: explicit .as_str()
                rest = tail;
            }
            Some(_) | None => {
                out.push('{');
                rest = after_open;
            }
        }
    }
    out.push_str(rest);
    Ok(out)
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
