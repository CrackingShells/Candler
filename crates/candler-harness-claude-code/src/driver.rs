//! The Claude Code harness driver: the command line, the stop spec and the exit classification.
//!
//! The driver is data only. It spawns nothing, reads no endpoint and names no token: the core
//! owns the environment and the secrets of the agent.

use std::str::FromStr as _;
use std::time::Duration;

use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::TraceReader;
use candler_harness::skills::SkillTree;
use candler_harness::spec::{
    BudgetCaps, Capabilities, Cell, CollectSpec, CommandSpec, GatewayEnv, HarnessId, Observed,
    Prepared, ProcessExit, SessionSpec, SessionTracker, StopSignal, StopSpec, Workspace,
};
use candler_schemas::name;
use candler_schemas::primitives::FieldPath;
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};
use semver::{Version, VersionReq};
use serde_json::{Map, Value};

use crate::{ClaudeCodeReader, detect, discovery, prepare, session};

/// The cell path of the model id.
pub(crate) const MODEL_ID: &str = "model.id";
/// The cell path of the reasoning effort.
pub(crate) const MODEL_EFFORT: &str = "model.effort";
/// The cell path of the permission mode.
pub(crate) const PERMISSION_MODE: &str = "permission_mode";
/// The cell path of the prompt template.
pub(crate) const PROMPT_TEMPLATE: &str = "prompt.template";
/// The cell path of the prompt values.
pub(crate) const PROMPT_VALUES: &str = "prompt.values";
/// The cell path of the per-run turn cap.
pub(crate) const BUDGET_TURNS: &str = "budget.per_run.turns";
/// The cell path of the per-run cost cap.
pub(crate) const BUDGET_COST: &str = "budget.per_run.max_cost_usd";

/// The program the driver runs.
pub(crate) const PROGRAM: &str = "claude";
/// The variable that carries the gateway's address.
const GATEWAY_BASE_URL: &str = "ANTHROPIC_BASE_URL";
/// The variable that carries the token the agent presents.
const GATEWAY_AUTH_TOKEN: &str = "ANTHROPIC_AUTH_TOKEN";
/// Every variable the core owns for this harness.
const GATEWAY_RESERVED: [&str; 3] = [GATEWAY_BASE_URL, GATEWAY_AUTH_TOKEN, "ANTHROPIC_API_KEY"];
/// How long the core waits after the stop signal before it escalates: the five seconds the core
/// gave every harness before it applied a driver's own stop (`DEFAULT_GRACE` of
/// `supervise/spawn.rs` at 40ef2fe).
const STOP_GRACE: Duration = Duration::from_secs(5);

/// The error for a value of the cell that cannot be used.
pub(crate) fn invalid(path: &str, detail: String) -> DriverError {
    match FieldPath::from_str(path) {
        Ok(path) => DriverError::InvalidValue { path, detail },
        Err(error) => DriverError::VersionOutput {
            // BORROW: .to_string() makes the owned text the error keeps
            detail: error.to_string(),
        },
    }
}

/// The error for a value the cell lacks.
pub(crate) fn missing(path: &str) -> DriverError {
    match FieldPath::from_str(path) {
        Ok(path) => DriverError::MissingValue { path },
        Err(error) => DriverError::VersionOutput {
            // BORROW: .to_string() makes the owned text the error keeps
            detail: error.to_string(),
        },
    }
}

/// The value of the cell at `path`, if it has one.
pub(crate) fn value_at<'c>(cell: &Cell<'c>, path: &str) -> Result<Option<&'c Value>, DriverError> {
    let field = FieldPath::from_str(path).map_err(|error| invalid(path, error.to_string()))?; // BORROW: .to_string() gives owned text
    Ok(cell
        .get(&field)
        .map(candler_schemas::values::JsonValue::as_value))
}

/// The text of the cell at `path`; the cell must have it.
pub(crate) fn required_text<'c>(cell: &Cell<'c>, path: &str) -> Result<&'c str, DriverError> {
    match value_at(cell, path)? {
        None => Err(missing(path)),
        Some(Value::String(text)) => Ok(text.as_str()), // BORROW: explicit .as_str()
        Some(
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_),
        ) => {
            Err(invalid(path, "is not text".to_owned())) // BORROW: .to_owned() makes the owned text
        }
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

/// Whether `text` is the name of a prompt placeholder.
fn is_key(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The prompt: `template` with each `{key}` replaced by the value of `values.<key>`.
///
/// A brace pair that does not hold a plain key stays as written.
///
/// # Errors
/// [`DriverError::InvalidValue`] when a key has no value or its value is not a scalar.
pub fn render_prompt(
    template: &str,
    values: Option<&Map<String, Value>>,
) -> Result<String, DriverError> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let (before, from_open) = rest.split_at(open);
        out.push_str(before);
        let after_open = from_open.get(1..).unwrap_or_default();
        match after_open.split_once('}') {
            Some((key, tail)) if is_key(key) => {
                let value = values.and_then(|map| map.get(key)).ok_or_else(|| {
                    invalid(
                        PROMPT_VALUES,
                        format!("the template names `{key}` and no value is given"),
                    )
                })?;
                let text = scalar_text(value).ok_or_else(|| {
                    invalid(
                        PROMPT_VALUES,
                        format!("the value of `{key}` is not a scalar"),
                    )
                })?;
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

/// The turn or cost cap of the cell as flag text, when the cell sets it.
pub(crate) fn cap_text(
    cell: &Cell<'_>,
    path: &str,
    whole: bool,
) -> Result<Option<String>, DriverError> {
    match value_at(cell, path)? {
        None => Ok(None),
        Some(Value::Number(number)) => {
            if whole && number.as_u64().is_none() {
                return Err(invalid(path, "is not a whole number".to_owned())); // BORROW: .to_owned() makes the owned text
            }
            Ok(Some(number.to_string())) // BORROW: .to_string() gives owned text
        }
        Some(
            Value::Null | Value::Bool(_) | Value::String(_) | Value::Array(_) | Value::Object(_),
        ) => {
            Err(invalid(path, "is not a number".to_owned())) // BORROW: .to_owned() makes the owned text
        }
    }
}

/// The driver of Claude Code.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ClaudeCodeDriver {
    /// The harness's name and version token.
    id: HarnessId,
    /// The command whose output names the installed version.
    probe: CommandSpec,
}

impl ClaudeCodeDriver {
    /// A driver, with its harness id and version probe built once.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: HarnessId::new(name!("claude-code")),
            // BORROW: .to_owned() makes owned arguments
            probe: CommandSpec::new(PROGRAM.to_owned(), vec!["--version".to_owned()]),
        }
    }
}

impl Default for ClaudeCodeDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl HarnessDriver for ClaudeCodeDriver {
    fn id(&self) -> HarnessId {
        self.id.clone() // BORROW: the trait returns an owned id
    }

    fn supported(&self) -> VersionReq {
        crate::supported()
    }

    fn skills(&self) -> SkillTree {
        SkillTree::new(".claude/skills", ".claude/skills")
    }

    fn base_context(&self) -> &'static str {
        "base"
    }

    fn detect(&self, sample: &[u8]) -> Option<Version> {
        detect::sample_version(sample)
    }

    fn version_probe(&self) -> CommandSpec {
        self.probe.clone() // BORROW: the trait returns an owned command
    }

    fn parse_version(&self, output: &[u8]) -> Result<Version, DriverError> {
        let text = std::str::from_utf8(output).map_err(|error| DriverError::VersionOutput {
            detail: error.to_string(), // BORROW: .to_string() gives owned text
        })?;
        let token = text.split_whitespace().next().unwrap_or_default();
        Version::parse(token.trim_start_matches('v')).map_err(|error| DriverError::VersionOutput {
            detail: format!("`{token}`: {error}"),
        })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::builder()
            .graceful_stop(true)
            .live_events(true)
            .budgets(BudgetCaps::new(true, true))
            .census(true)
            .session(true)
            .build()
    }

    fn gateway(&self) -> Option<GatewayEnv> {
        Some(GatewayEnv::new(
            // BORROW: .to_owned() makes the owned names the spec keeps
            GATEWAY_BASE_URL.to_owned(),
            GATEWAY_AUTH_TOKEN.to_owned(),
            GATEWAY_RESERVED.map(str::to_owned).to_vec(),
        ))
    }

    fn session(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<Option<SessionSpec>, DriverError> {
        let _ = workspace;
        session::spec(cell).map(Some)
    }

    fn session_tracker(&self) -> Option<Box<dyn SessionTracker>> {
        Some(Box::new(session::Tracker::default())) // TRAIT_OBJECT: the core selects trackers at runtime
    }

    fn prepare(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Prepared, DriverError> {
        let _ = workspace;
        let mut files = vec![prepare::render_settings(cell)?];
        files.extend(prepare::render_mcp(cell)?);
        Ok(Prepared::new(files, Vec::new()))
    }

    fn command(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<CommandSpec, DriverError> {
        let model = required_text(cell, MODEL_ID)?;
        let mode = required_text(cell, PERMISSION_MODE)?;
        let template = required_text(cell, PROMPT_TEMPLATE)?;
        let values = match value_at(cell, PROMPT_VALUES)? {
            None => None,
            Some(Value::Object(map)) => Some(map),
            Some(
                Value::Null
                | Value::Bool(_)
                | Value::Number(_)
                | Value::String(_)
                | Value::Array(_),
            ) => {
                return Err(invalid(PROMPT_VALUES, "is not an object".to_owned())); // BORROW: .to_owned() makes the owned text
            }
        };
        let prompt = render_prompt(template, values)?;
        let _ = workspace;
        // BORROW: .to_owned() makes the owned arguments the command keeps
        let mut argv = vec![
            PROGRAM.to_owned(),
            "--print".to_owned(),
            prompt,
            "--model".to_owned(),
            model.to_owned(),
            "--output-format".to_owned(),
            "stream-json".to_owned(),
            "--verbose".to_owned(),
            "--setting-sources".to_owned(),
            "project,local".to_owned(),
            "--permission-mode".to_owned(),
            mode.to_owned(),
        ];
        if let Some(effort) = value_at(cell, MODEL_EFFORT)? {
            let Value::String(effort) = effort else {
                return Err(invalid(MODEL_EFFORT, "is not text".to_owned())); // BORROW: .to_owned() makes the owned text
            };
            argv.push("--effort".to_owned()); // BORROW: .to_owned() makes the owned argument
            argv.push(effort.clone()); // BORROW: .clone() copies the text out of the cell
        }
        if let Some(turns) = cap_text(cell, BUDGET_TURNS, true)? {
            argv.push("--max-turns".to_owned()); // BORROW: .to_owned() makes the owned argument
            argv.push(turns);
        }
        if let Some(cost) = cap_text(cell, BUDGET_COST, false)? {
            argv.push("--max-budget-usd".to_owned()); // BORROW: .to_owned() makes the owned argument
            argv.push(cost);
        }
        Ok(CommandSpec::builder().argv(argv).build()?)
    }

    fn stop(&self) -> StopSpec {
        StopSpec::new(None, StopSignal::Term, STOP_GRACE)
    }

    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        Some(Box::new(ClaudeCodeReader::new())) // TRAIT_OBJECT: the core selects readers at runtime
    }

    fn offline_reader(&self, stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        let _ = stream;
        None
    }

    fn classify_exit(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy {
        let _ = exit;
        let ended_by = sessions
            .iter()
            .rfind(|session| session.parent_session().is_none())
            .map(Session::ended_by);
        match ended_by {
            Some(EndedBy::Completed) => EndedBy::Completed,
            Some(EndedBy::AgentError { reason, .. }) => EndedBy::agent_error(reason.clone()), // BORROW: .clone() copies the reason out of the session
            Some(EndedBy::Cap { cap, .. }) => EndedBy::cap(*cap),
            // The core decides these from facts of its own (R2), and gateway failures from the
            // gateway signals and the trace, so the driver reports none of them.
            Some(
                EndedBy::GatewayError
                | EndedBy::BudgetStop
                | EndedBy::Killed
                | EndedBy::Interrupted
                | EndedBy::Unknown,
            )
            | None => EndedBy::Unknown,
            #[expect(
                clippy::match_same_arms,
                reason = "EndedBy is non-exhaustive; a kind this driver does not know is unknown, as the kinds listed above are"
            )]
            Some(_) => EndedBy::Unknown,
        }
    }

    fn collect(&self, workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec> {
        let _ = (workspace, observed);
        // A glob that starts with `!` leaves out the paths it matches.
        let mut specs = vec![CollectSpec::file("**/*".to_owned())]; // BORROW: .to_owned() makes the owned glob
        specs.extend(
            discovery::exclusion_patterns()
                .into_iter()
                .map(|pattern| CollectSpec::file(format!("!{pattern}"))),
        );
        specs
    }
}
