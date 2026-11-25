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
    AgentSetting, Answer, BaseImage, CLAUDE_CODE, Cell, CheckVerdict, CollectSpec, CommandSpec,
    DriverFact, EffortLevel, GatewayEnv, HarnessId, InstructionScope, LoadedFile, Observed,
    PermissionMode, Prepared, ProcessExit, SessionTracker, StopSignal, StopSpec, Workspace,
};
use candler_schemas::execution::paths;
use candler_schemas::name;
use candler_schemas::primitives::FieldPath;
use candler_schemas::raw::StreamName;
use candler_schemas::servers::McpServerKind;
use candler_schemas::trace::{EndedBy, Session};
use semver::{Version, VersionReq};
use serde_json::Value;

use crate::{ClaudeCodeReader, detect, discovery, prepare, session};

/// The first line of every `claude doctor` report.
const DOCTOR_TITLE: &str = "Claude Code doctor";
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

/// The turn or cost cap of the cell as flag text, when the cell sets it.
pub(crate) fn cap_text(
    cell: &Cell<'_>,
    path: &str,
    whole: bool,
) -> Result<Option<String>, DriverError> {
    match cell
        .value_str(path)
        .map_err(|error| invalid(path, error.to_string()))?
    {
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
/// model, the stream output, the setting sources (with `user` when the cell sets a global text),
/// the permission mode, the effort and caps the cell sets, and the observation hooks' settings
/// file.
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
    // `user` also loads the home's `settings.json` and `rules/`, so only a cell that needs the
    // home's `CLAUDE.md` adds it.
    let sources = if prepare::has_global(cell)? {
        "user,project,local"
    } else {
        "project,local"
    };
    argv.extend(
        [
            "--setting-sources",
            sources,
            "--permission-mode",
            mode.as_str(),
        ]
        .map(str::to_owned), // BORROW: str::to_owned makes the owned arguments
    );
    if let Some(effort) = cell.effort()? {
        argv.extend(["--effort", effort.as_str()].map(str::to_owned)); // BORROW: str::to_owned makes the owned arguments
    }
    if let Some(turns) = cap_text(cell, paths::BUDGET_PER_RUN_TURNS, true)? {
        argv.push("--max-turns".to_owned()); // BORROW: .to_owned() makes the owned argument
        argv.push(turns);
    }
    if let Some(cost) = cap_text(cell, paths::BUDGET_PER_RUN_MAX_COST_USD, false)? {
        argv.push("--max-budget-usd".to_owned()); // BORROW: .to_owned() makes the owned argument
        argv.push(cost);
    }
    if candler_harness::mcp::servers(cell)?.is_some() {
        // Only the servers of the cell: no other MCP configuration reaches the agent.
        argv.extend(["--mcp-config", prepare::MCP_FILE, "--strict-mcp-config"].map(str::to_owned)); // BORROW: str::to_owned makes the owned arguments
    }
    argv.push("--settings".to_owned()); // BORROW: .to_owned() makes the owned argument
    argv.push(session::settings_argument());
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
            id: HarnessId::new(name!(CLAUDE_CODE)),
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

    fn answer(&self, setting: AgentSetting) -> Answer {
        match setting {
            AgentSetting::ModelId => Answer::Mapped {
                construct: "`--model` on the command line",
            },
            AgentSetting::ModelEndpoint => Answer::Candler {
                how: "the gateway forwards model requests to the endpoint",
            },
            AgentSetting::Effort(
                EffortLevel::Low
                | EffortLevel::Medium
                | EffortLevel::High
                | EffortLevel::Xhigh
                | EffortLevel::Max,
            ) => Answer::Mapped {
                construct: "`--effort <level>` on the command line",
            },
            AgentSetting::Effort(EffortLevel::Ultracode) => Answer::Mapped {
                construct: "`--effort ultracode` on the command line, which Claude Code reads as `xhigh` with its ultracode mode on",
            },
            AgentSetting::PermissionMode(
                PermissionMode::Default
                | PermissionMode::AcceptEdits
                | PermissionMode::Plan
                | PermissionMode::BypassPermissions
                | PermissionMode::Auto
                | PermissionMode::DontAsk,
            ) => Answer::Mapped {
                construct: "`--permission-mode <mode>` on the command line",
            },
            AgentSetting::ToolsAllowed => Answer::Mapped {
                construct: "`permissions.allow` of `.claude/settings.json`",
            },
            AgentSetting::ToolsDisallowed => Answer::Mapped {
                construct: "`permissions.deny` of `.claude/settings.json`",
            },
            AgentSetting::Env => Answer::Candler {
                how: "the core sets the cell's variables on the agent's process",
            },
            AgentSetting::Inputs => Answer::Candler {
                how: "the core places the inputs in the workspace",
            },
            AgentSetting::Skills => Answer::Mapped {
                construct: "the project skill directory",
            },
            AgentSetting::McpTransport(McpServerKind::Stdio | McpServerKind::Local) => {
                Answer::Mapped {
                    construct: "an entry of `mcpServers` in `.mcp.json`, of type `stdio`",
                }
            }
            AgentSetting::McpTransport(McpServerKind::Http | McpServerKind::Remote) => {
                Answer::Mapped {
                    construct: "an entry of `mcpServers` in `.mcp.json`, of type `http`",
                }
            }
            AgentSetting::McpTransport(McpServerKind::Sse) => Answer::Mapped {
                construct: "an entry of `mcpServers` in `.mcp.json`, of type `sse`",
            },
            AgentSetting::Codemode(true) => Answer::Refused {
                reason: "Claude Code offers MCP tools directly; it has no code mode",
            },
            AgentSetting::Codemode(false) => Answer::Mapped {
                construct: "no key: Claude Code offers the server's tools directly",
            },
            AgentSetting::McpName => Answer::Mapped {
                construct: "the key of the server's `.mcp.json` entry",
            },
            AgentSetting::WallBudget => Answer::Candler {
                how: "the core ends the run at the wall budget",
            },
            AgentSetting::TurnBudget => Answer::Mapped {
                construct: "`--max-turns` on the command line",
            },
            AgentSetting::CostBudget => Answer::Mapped {
                construct: "`--max-budget-usd` on the command line",
            },
            AgentSetting::SessionMode => Answer::Mapped {
                construct: "stream-json over an open stdin, one turn per message",
            },
            AgentSetting::Instructions(InstructionScope::Global) => Answer::Mapped {
                construct: "`.claude/CLAUDE.md` in the agent's home, read through `user` in `--setting-sources`, which also loads the home's `.claude/settings.json` and `.claude/rules/*.md` (Candler's home holds neither)",
            },
            AgentSetting::Instructions(InstructionScope::Workspace) => Answer::Mapped {
                construct: "`CLAUDE.md` at the workspace root, mounted read-only",
            },
            AgentSetting::Hooks => Answer::Mapped {
                construct: "the `hooks` key of `.claude/settings.json`, with the hook files under `.claude/hooks/`",
            },
        }
    }

    fn fact(&self, fact: DriverFact) -> Answer {
        match fact {
            DriverFact::GracefulStop => Answer::Mapped {
                construct: "SIGTERM ends the sessions cleanly",
            },
            DriverFact::LiveEvents => Answer::Mapped {
                construct: "stream-json events on stdout while the run lasts",
            },
            DriverFact::InHarnessGuard => Answer::Refused {
                reason: "the driver configures no Claude Code hook",
            },
            DriverFact::ExecutionOutsideChild => Answer::Refused {
                reason: "tools run in the child process",
            },
            DriverFact::Census => Answer::Mapped {
                construct: "Claude Code's `system/init` event lists the tools",
            },
            DriverFact::ConfigCheck => Answer::Mapped {
                construct: "`claude doctor` in the workspace, whose `Invalid settings` section lists a rejected file or a skipped entry",
            },
            DriverFact::UnsetPermissionMode => Answer::Mapped {
                construct: "`--permission-mode default` on the command line, which in a headless run denies every tool call that needs approval unless an allow rule or a hook approves it",
            },
        }
    }

    fn config_check(&self, plan: &Plan) -> Option<CommandSpec> {
        let _ = plan;
        // The exit status is 0 whatever it finds, and four install-method warnings always print:
        // only the `Invalid settings` section of a report, which opens with its title, says something of the files.
        Some(CommandSpec::new(
            PROGRAM.to_owned(),
            vec!["doctor".to_owned()],
        )) // BORROW: .to_owned() makes the owned arguments
    }

    fn read_config_check(&self, output: &[u8]) -> CheckVerdict {
        let text = String::from_utf8_lossy(output);
        // Every report opens with its title; output without it is not a report, so a check that
        // never ran (empty output, an error, a changed format) never passes.
        if text.lines().find(|line| !line.trim().is_empty()) != Some(DOCTOR_TITLE) {
            return CheckVerdict::Rejected {
                output: text.into_owned(),
            };
        }
        let mut section = text
            .lines()
            .skip_while(|line| !line.contains("Invalid settings"))
            .take_while(|line| !line.trim().is_empty())
            .peekable();
        if section.peek().is_none() {
            return CheckVerdict::Clean;
        }
        CheckVerdict::Rejected {
            output: section.collect::<Vec<_>>().join("\n"),
        }
    }

    fn loaded_instructions(&self, stream: &StreamName, bytes: &[u8]) -> Vec<LoadedFile> {
        session::loaded(stream, bytes)
    }

    fn workspace_instruction_file(&self) -> &'static str {
        "CLAUDE.md"
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

    fn check_settings(&self, cell: &Cell<'_>) -> Result<(), DriverError> {
        cap_text(cell, paths::BUDGET_PER_RUN_TURNS, true)?;
        cap_text(cell, paths::BUDGET_PER_RUN_MAX_COST_USD, false)?;
        candler_harness::agent_config::hooks::<candler_schemas::agent_config::ClaudeCodeHooks>(
            cell,
            CLAUDE_CODE,
        )?;
        prepare::has_global(cell)?;
        candler_harness::mcp::servers(cell).map(|_| ())
    }

    fn plan(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Plan, DriverError> {
        let _ = workspace;
        self.check_settings(cell)?;
        let mut files = vec![prepare::render_settings(cell)?, session::observers()?];
        files.extend(prepare::render_mcp(cell)?);
        files.extend(prepare::render_hook_files(cell)?);
        let (global, workspace_file) = prepare::render_instructions(cell)?;
        files.extend(global);
        let argv = argv(cell, Launch::Headless(cell.prompt()?))?;
        let mut plan = Plan::new(
            Prepared::new(files, Vec::new()),
            CommandSpec::builder().argv(argv).build()?,
        )
        .with_session(session::spec(cell)?);
        plan.workspace_files.extend(workspace_file);
        Ok(plan)
    }

    fn stop(&self) -> StopSpec {
        StopSpec::new(None, StopSignal::Term, STOP_GRACE)
    }

    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        Some(Box::new(ClaudeCodeReader::new())) // TRAIT_OBJECT: the core selects readers at runtime
    }

    fn offline_reader(
        &self,
        stream: &StreamName,
        servers: &[String],
    ) -> Option<Box<dyn TraceReader>> {
        let _ = (stream, servers);
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
        // EXPLICIT: the log's path and stream name are constants that parse; were they refused, the log would go uncollected
        specs.extend(session::log_collect().ok());
        specs
    }
}
