//! The Claude Code harness driver: the command line, the stop spec and the exit classification.
//!
//! The driver is data only. It spawns nothing, reads no endpoint and names no token: the core
//! owns the environment and the secrets of the agent.

use std::str::FromStr as _;
use std::time::Duration;

use candler_harness::driver::{DriverError, HarnessDriver, Plan};
use candler_harness::reader::TraceReader;
use candler_harness::skills::SkillTree;
use candler_harness::spec::{
    BaseImage, BudgetCaps, Capabilities, Cell, CollectSpec, CommandSpec, GatewayEnv, HarnessId,
    Observed, Prepared, ProcessExit, SessionTracker, StopSignal, StopSpec, Workspace,
};
use candler_schemas::execution::paths;
use candler_schemas::name;
use candler_schemas::primitives::FieldPath;
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};
use semver::{Version, VersionReq};
use serde_json::Value;

use crate::{ClaudeCodeReader, detect, discovery, prepare, session};

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

/// The value of the cell at `path`, if it has one.
pub(crate) fn value_at<'c>(cell: &Cell<'c>, path: &str) -> Result<Option<&'c Value>, DriverError> {
    let field = FieldPath::from_str(path).map_err(|error| invalid(path, error.to_string()))?; // BORROW: .to_string() gives owned text
    Ok(cell
        .get(&field)
        .map(candler_schemas::values::JsonValue::as_value))
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

/// How Claude Code takes the prompt.
#[derive(Debug)]
pub(crate) enum Launch {
    /// The prompt as the argument of `--print`.
    Headless(String),
    /// The prompt as the first message on stdin, which stays open.
    Session,
}

/// The command line of a cell: the program and `--print` with the prompt or the stdin format, the
/// model, the stream output, the setting sources, the permission mode, and the effort and caps the
/// cell sets.
pub(crate) fn argv(cell: &Cell<'_>, launch: Launch) -> Result<Vec<String>, DriverError> {
    let model = cell.model()?;
    let mode = cell.permission_mode()?;
    // BORROW: .to_owned() makes the owned arguments the command keeps
    let mut argv = vec![PROGRAM.to_owned(), "--print".to_owned()];
    // Each launch keeps the order of its recorded command line.
    let stream = ["--output-format", "stream-json", "--verbose"];
    match launch {
        Launch::Headless(prompt) => {
            argv.push(prompt);
            argv.extend(["--model", model].map(str::to_owned)); // BORROW: str::to_owned makes the owned arguments
            argv.extend(stream.map(str::to_owned)); // BORROW: str::to_owned makes the owned arguments
        }
        Launch::Session => {
            argv.extend(["--input-format", "stream-json"].map(str::to_owned)); // BORROW: str::to_owned makes the owned arguments
            argv.extend(stream.map(str::to_owned)); // BORROW: str::to_owned makes the owned arguments
            argv.extend(["--model", model].map(str::to_owned)); // BORROW: str::to_owned makes the owned arguments
        }
    }
    argv.extend(
        [
            "--setting-sources",
            "project,local",
            "--permission-mode",
            mode,
        ]
        .map(str::to_owned), // BORROW: str::to_owned makes the owned arguments
    );
    if let Some(effort) = cell.effort()? {
        argv.extend(["--effort", effort].map(str::to_owned)); // BORROW: str::to_owned makes the owned arguments
    }
    if let Some(turns) = cap_text(cell, paths::BUDGET_PER_RUN_TURNS, true)? {
        argv.push("--max-turns".to_owned()); // BORROW: .to_owned() makes the owned argument
        argv.push(turns);
    }
    if let Some(cost) = cap_text(cell, paths::BUDGET_PER_RUN_MAX_COST_USD, false)? {
        argv.push("--max-budget-usd".to_owned()); // BORROW: .to_owned() makes the owned argument
        argv.push(cost);
    }
    Ok(argv)
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

    fn base_image(&self) -> BaseImage {
        // BORROW: .to_owned() and .to_string() make the owned argument
        BaseImage::new(
            "base",
            vec![("CC_VERSION".to_owned(), crate::PINNED.to_string())],
        )
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

    fn session_tracker(&self) -> Option<Box<dyn SessionTracker>> {
        Some(Box::new(session::Tracker::default())) // TRAIT_OBJECT: the core selects trackers at runtime
    }

    fn plan(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Plan, DriverError> {
        let _ = workspace;
        let mut files = vec![prepare::render_settings(cell)?];
        files.extend(prepare::render_mcp(cell)?);
        let argv = argv(cell, Launch::Headless(cell.prompt()?))?;
        Ok(Plan::new(
            Prepared::new(files, Vec::new()),
            CommandSpec::builder().argv(argv).build()?,
        )
        .with_session(session::spec(cell)?))
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
