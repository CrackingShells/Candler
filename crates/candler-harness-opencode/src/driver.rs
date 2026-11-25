//! The `OpenCode` driver: the command line, the configuration, the state paths, the stop, the
//! collect and the exit classification.
//!
//! The driver is data only. It spawns nothing, reads no endpoint and names no token: the core owns
//! the environment and the secrets of the agent, and the configuration refers to them by name.

use std::str::FromStr as _;
use std::time::Duration;

use candler_harness::driver::{DriverError, HarnessDriver, Plan};
use candler_harness::reader::TraceReader;
use candler_harness::skills::SkillTree;
use candler_harness::spec::{
    Action, AgentSetting, Answer, BaseImage, Cell, CheckVerdict, CollectSpec, CommandSpec,
    DriverFact, EnvEntry, GATEWAY_STREAM, GatewayEnv, GatewayRecord, HarnessId, LoadedFile,
    LoadedScope, OPENCODE, Observed, Prepared, ProcessExit, StateKind, StatePath, StopSignal,
    StopSpec, Workspace, gateway_records,
};
use candler_schemas::name;
use candler_schemas::primitives::{RelPath, plain_arg};
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};
use semver::{Version, VersionReq};

use crate::export::{OpenCodeExportReader, ended_by_of_observed, export_stream};
use crate::live::OpenCodeLiveReader;
use crate::prepare::{self, model_id};
use crate::{EXPORT_READER, VALIDATED};

const PROGRAM: &str = "opencode";

/// The variable that holds the gateway's address; the configuration refers to it.
const GATEWAY_BASE_URL: &str = "CANDLER_GATEWAY_URL";
/// The variable that holds the gateway's token; the configuration refers to it.
const GATEWAY_AUTH_TOKEN: &str = "CANDLER_GATEWAY_TOKEN";

/// The agent's home inside the container, which the state paths are relative to.
const AGENT_HOME: &str = "/home/agent";

/// The state directories and the database file, relative to the agent's home.
const STATE: [(&str, StateKind, &str); 5] = [
    ("XDG_DATA_HOME", StateKind::Dir, ".xdg/data"),
    ("XDG_CONFIG_HOME", StateKind::Dir, ".xdg/config"),
    ("XDG_CACHE_HOME", StateKind::Dir, ".xdg/cache"),
    ("XDG_STATE_HOME", StateKind::Dir, ".xdg/state"),
    ("OPENCODE_DB", StateKind::File, ".xdg/data/oc.db"),
];

/// The model of the config check: one `OpenCode` cannot select, so the start ends at the model.
const CHECK_MODEL: &str = "probe/none";
/// The directory the config check keeps its data home and database in, away from the run's.
const CHECK_DATA: &str = "/tmp/candler-config-check";

/// The text that opens an instruction file in the system text of a request, at the start of a
/// line: `Instructions from: <path>`, then the file's text.
const INSTRUCTIONS_MARKER: &str = "Instructions from: ";

/// How long a stopped run has to exit after the signal: `OpenCode` ended in under 100 ms on TERM.
const STOP_GRACE: Duration = Duration::from_secs(10);

/// The in-container interrupt, run as `python3 -I -c <script> {session_id}`.
const STOP_SCRIPT: &str = include_str!("stop.py");

fn argv(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect() // BORROW: .to_owned() makes owned arguments
}

/// The end of the root session: the export's when the sessions hold one, whatever their order,
/// and the live stream's only when no export session was read.
fn root_end(sessions: &[Session]) -> Option<&EndedBy> {
    let export = EXPORT_READER.split('@').next().unwrap_or_default();
    let mut roots = sessions
        .iter()
        .filter(|session| session.parent_session().is_none());
    let exported = roots
        .clone()
        .find(|session| session.source().reader().as_str() == export); // BORROW: explicit .as_str()
    exported.or_else(|| roots.next()).map(Session::ended_by)
}

/// The instruction files the starting context of the first request that offers tools names, in the
/// order the system text holds them, each once, as `OpenCode` printed their paths. `bytes` is the
/// stream `stream`; any other stream lists nothing. A file under the `XDG_CONFIG_HOME` the plan sets
/// is the user's own; any other is the workspace's. The census reads `OpenCode`'s `Instructions
/// from:` lines, which `OpenCode` does not escape, so a line of that shape inside an instruction
/// text counts too.
fn loaded(stream: &StreamName, bytes: &[u8]) -> Vec<LoadedFile> {
    if stream.as_str() != GATEWAY_STREAM {
        return Vec::new();
    }
    let first = gateway_records(bytes)
        .filter_map(|(_, record)| {
            let GatewayRecord::Context { seq, context } = record else {
                return None;
            };
            Some((seq, context))
        })
        .min_by_key(|(seq, _)| *seq);
    let Some((_, context)) = first else {
        return Vec::new();
    };
    let own = format!(
        "{AGENT_HOME}/{}/",
        OpenCodeDriver::state_subpath("XDG_CONFIG_HOME")
    );
    let mut files: Vec<LoadedFile> = Vec::new();
    for path in context
        .system
        .lines()
        .filter_map(|line| line.strip_prefix(INSTRUCTIONS_MARKER))
    {
        let scope = if path.starts_with(own.as_str()) {
            LoadedScope::User
        } else {
            LoadedScope::Project
        };
        let file = LoadedFile::new(path.to_owned(), scope); // BORROW: .to_owned() gives the file its own path
        if !path.is_empty() && !files.contains(&file) {
            files.push(file);
        }
    }
    files
}

/// The line that marks a check start that reached the model: the session drain failing on the model
/// that does not exist.
fn is_drain(line: &str) -> bool {
    line.contains("Failed to drain Session") && line.contains("ModelUnavailableError")
}

/// The `OpenCode` V2 driver.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct OpenCodeDriver {
    id: HarnessId,
    probe: CommandSpec,
}

impl Default for OpenCodeDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenCodeDriver {
    /// A driver.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: HarnessId::new(name!(OPENCODE)),
            probe: CommandSpec::new(PROGRAM.to_owned(), argv(&["--version"])), // BORROW: .to_owned() makes the owned program
        }
    }

    fn state_subpath(env: &str) -> &'static str {
        STATE
            .iter()
            .find(|(name, _, _)| *name == env)
            .map_or(".xdg/config", |(_, _, subpath)| *subpath)
    }
}

impl HarnessDriver for OpenCodeDriver {
    fn id(&self) -> HarnessId {
        self.id.clone() // BORROW: the trait returns an owned id
    }

    fn supported(&self) -> VersionReq {
        crate::supported()
    }

    fn skills(&self) -> SkillTree {
        // A treatment's skills live in `.claude/skills` of its repository; `OpenCode` reads them
        // from `.opencode/skills`.
        SkillTree::new(".claude/skills", ".opencode/skills")
    }

    fn base_image(&self) -> BaseImage {
        // BORROW: .to_owned() and .to_string() make the owned argument
        BaseImage::new(
            "base-opencode",
            vec![("OC_VERSION".to_owned(), VALIDATED.to_string())],
        )
    }

    fn offline_streams(&self) -> Vec<StreamName> {
        export_stream().into_iter().collect()
    }

    fn detect(&self, sample: &[u8]) -> Option<Version> {
        let (format, _) = TraceReader::detect(&OpenCodeLiveReader::new(), sample)?;
        Some(format.as_version().clone()) // BORROW: .clone() copies the version
    }

    fn version_probe(&self) -> CommandSpec {
        self.probe.clone() // BORROW: the trait returns an owned command
    }

    fn parse_version(&self, output: &[u8]) -> Result<Version, DriverError> {
        let text = std::str::from_utf8(output).map_err(|error| DriverError::VersionOutput {
            detail: error.to_string(), // BORROW: .to_string() gives owned text
        })?;
        let line = text.trim();
        let version = line
            .strip_prefix("opencode v")
            .and_then(|token| Version::parse(token).ok())
            .ok_or_else(|| DriverError::VersionOutput {
                detail: format!("`{line}` is not `opencode vX.Y.Z`"),
            })?;
        // The readers stand for one version, so another one is refused here (the supported range
        // says the same, and the output of a binary that is not the validated one is no version
        // this driver can read).
        if version != VALIDATED {
            return Err(DriverError::VersionOutput {
                detail: format!("opencode {version} is not the validated {VALIDATED}"),
            });
        }
        Ok(version)
    }

    fn answer(&self, setting: AgentSetting) -> Answer {
        prepare::answer(setting)
    }

    fn fact(&self, fact: DriverFact) -> Answer {
        match fact {
            DriverFact::GracefulStop => Answer::Mapped {
                construct: "an interrupt request to the `serve` child, then SIGTERM to the client",
            },
            DriverFact::LiveEvents => Answer::Refused {
                reason: "the live reader emits no event: the trace comes from the export",
            },
            DriverFact::InHarnessGuard => Answer::Refused {
                reason: "the driver configures no OpenCode guard",
            },
            DriverFact::ExecutionOutsideChild => Answer::Mapped {
                construct: "tools run in the `serve` child, which the stop interrupts first",
            },
            DriverFact::Census => Answer::Candler {
                how: "the tools the gateway offered the model in its first request",
            },
            DriverFact::ConfigCheck => Answer::Mapped {
                construct: "`opencode run --standalone --print-logs -m probe/none x` in the workspace, whose `level=WARN` and `level=ERROR` lines name a rejected `opencode.json` or a plugin that failed to load",
            },
            DriverFact::UnsetPermissionMode => Answer::Candler {
                how: "the run is under `--auto` and the baseline ruleset {*, *, allow}, so every tool is allowed: OpenCode has no headless ask mode",
            },
        }
    }

    fn config_check(&self, plan: &Plan) -> Option<CommandSpec> {
        let _ = plan;
        // A model that does not exist ends the start at the model, after the configuration and
        // the plugins have loaded, so no request leaves. The start keeps the plan's configuration
        // home and writes its database and data under a throwaway directory, not the run's.
        let argv = argv(&[
            PROGRAM,
            "run",
            "--standalone",
            "--print-logs",
            "-m",
            CHECK_MODEL,
            "x",
        ]);
        CommandSpec::builder()
            .argv(argv)
            // BORROW: .to_owned() makes the owned names and values
            .env(EnvEntry::literal(
                "XDG_DATA_HOME".to_owned(),
                CHECK_DATA.to_owned(),
            ))
            .env(EnvEntry::literal(
                "OPENCODE_DB".to_owned(),
                format!("{CHECK_DATA}/oc.db"),
            ))
            .build()
            .ok()
    }

    fn read_config_check(&self, output: &[u8]) -> CheckVerdict {
        let text = String::from_utf8_lossy(output);
        // The exit status is 1 whatever the start found. Output is a report only when it holds the
        // line that says the start reached the model; without it the check never got that far
        // (empty output, an error, a changed format), and never passes.
        let reached_model = text.lines().any(is_drain);
        if !reached_model {
            return CheckVerdict::Rejected {
                output: text.into_owned(),
            };
        }
        let findings: Vec<&str> = text
            .lines()
            .filter(|line| {
                line.contains("level=WARN") || (line.contains("level=ERROR") && !is_drain(line))
            })
            .collect();
        if findings.is_empty() {
            return CheckVerdict::Clean;
        }
        CheckVerdict::Rejected {
            output: findings.join("\n"),
        }
    }

    fn loaded_instructions(&self, stream: &StreamName, bytes: &[u8]) -> Vec<LoadedFile> {
        loaded(stream, bytes)
    }

    fn workspace_instruction_file(&self) -> &'static str {
        "AGENTS.md"
    }

    fn gateway(&self) -> Option<GatewayEnv> {
        Some(GatewayEnv::new(
            // BORROW: .to_owned() makes the owned names the spec keeps
            GATEWAY_BASE_URL.to_owned(),
            GATEWAY_AUTH_TOKEN.to_owned(),
            vec![GATEWAY_BASE_URL.to_owned(), GATEWAY_AUTH_TOKEN.to_owned()],
        ))
    }

    fn state_paths(&self) -> Vec<StatePath> {
        STATE
            .iter()
            .filter_map(|(env, kind, subpath)| {
                RelPath::from_str(subpath)
                    .ok()
                    // BORROW: .to_owned() makes the owned variable name
                    .map(|path| StatePath::new((*env).to_owned(), *kind, path))
            })
            .collect()
    }

    fn check_settings(&self, cell: &Cell<'_>) -> Result<(), DriverError> {
        prepare::settings(cell)?;
        prepare::check_agent_config(cell)?;
        let names: Vec<String> = candler_harness::mcp::servers(cell)?
            .map(|servers| servers.into_keys().collect())
            .unwrap_or_default();
        ambiguous_prefixes(&names)
    }

    fn plan(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Plan, DriverError> {
        let _ = workspace;
        let settings = prepare::settings(cell)?;
        let config_dir = Self::state_subpath("XDG_CONFIG_HOME");
        let model = model_id(cell)?;
        let model_arg = match &settings.variant {
            Some((id, _)) => format!("{}/{model}#{id}", prepare::PROVIDER),
            None => format!("{}/{model}", prepare::PROVIDER),
        };
        let mut files = vec![prepare::render_config(
            cell,
            settings,
            config_dir,
            GATEWAY_BASE_URL,
            GATEWAY_AUTH_TOKEN,
        )?];
        let (global, workspace_file) = prepare::render_instructions(cell, config_dir)?;
        files.extend(global);
        files.extend(prepare::render_plugins(cell, config_dir)?);
        let mut line = argv(&[PROGRAM, "run", "--standalone", "--format", "json"]);
        line.push("--auto".to_owned()); // BORROW: .to_owned() makes the owned argument
        line.push("-m".to_owned()); // BORROW: .to_owned() makes the owned argument
        line.push(model_arg);
        line.push("--".to_owned()); // BORROW: .to_owned() makes the owned argument
        line.push(cell.prompt()?);
        let mut plan = Plan::new(
            Prepared::new(files, Vec::new()),
            CommandSpec::builder().argv(line).build()?,
        );
        plan.workspace_files.extend(workspace_file);
        Ok(plan)
    }

    fn stop(&self) -> StopSpec {
        // The interrupt first: it ends the session in the server child with its tool, which a
        // signal to the client does not reach. Then TERM, which `OpenCode` honours (exit 143).
        let script = CommandSpec::new(
            "python3".to_owned(), // BORROW: .to_owned() makes the owned program
            // BORROW: str::to_owned makes the owned arguments
            Vec::from(["-I", "-c", STOP_SCRIPT, SESSION_ID].map(str::to_owned)),
        );
        StopSpec::new(Some(Action::exec(script)), StopSignal::Term, STOP_GRACE)
    }

    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        Some(Box::new(OpenCodeLiveReader::new())) // TRAIT_OBJECT: the core selects readers at runtime
    }

    fn census_from_gateway(&self) -> bool {
        true
    }

    fn offline_reader(
        &self,
        stream: &StreamName,
        servers: &[String],
    ) -> Option<Box<dyn TraceReader>> {
        if export_stream().as_ref() == Some(stream) {
            return Some(Box::new(OpenCodeExportReader::new(servers))); // TRAIT_OBJECT: the core selects readers at runtime
        }
        None
    }

    fn classify_exit(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy {
        // The exit status alone cannot tell the three endings of exit 1 apart, and a signal's
        // status says only that the core or a stop ended the run: the trace decides.
        let _ = exit;
        root_end(sessions).cloned().unwrap_or(EndedBy::Unknown) // BORROW: .cloned() copies the end
    }

    fn classify_end(
        &self,
        exit: &ProcessExit,
        sessions: &[Session],
        observed: &Observed,
    ) -> EndedBy {
        // The export is authoritative, even when it says nothing; the stream's last word stands in
        // only when no session was read.
        match root_end(sessions) {
            Some(end) => end.clone(), // BORROW: .clone() copies the end
            None => observed
                .end
                .as_ref()
                .map_or_else(|| self.classify_exit(exit, sessions), ended_by_of_observed),
        }
    }

    fn collect(&self, workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec> {
        let _ = workspace;
        // The files the agent wrote: everything in the workspace but the skills the harness
        // mounts. A glob that starts with `!` leaves out the paths it matches.
        let mut specs = vec![CollectSpec::file("**/*".to_owned())]; // BORROW: .to_owned() makes the owned glob
        specs.extend(
            [".opencode/**", "**/.opencode/**"]
                .map(|pattern| CollectSpec::file(format!("!{pattern}"))),
        );
        // The export needs `--standalone` and the run's own database and XDG variables, which only
        // a command inside the isolation has. One export, for the first root session the stream
        // named; an id that could pass as an option is not exported.
        let Some(id) = observed
            .session_ids
            .iter()
            .map(candler_schemas::trace::SessionId::as_str) // BORROW: explicit .as_str()
            .find(|id| plain_arg(id))
        else {
            return specs;
        };
        let mut line = argv(&[PROGRAM, "session", "export", "--standalone"]);
        line.push(id.to_owned()); // BORROW: .to_owned() makes owned text
        if let (Ok(spec), Some(name)) = (CommandSpec::builder().argv(line).build(), export_stream())
        {
            specs.push(CollectSpec::exec(spec, name));
        }
        specs
    }
}

/// Refuses two server names where one, then `_`, begins the other: `OpenCode` names a tool
/// `<server>_<tool>`, so `a_b_c` would be the tool `b_c` of `a` or the tool `c` of `a_b`.
fn ambiguous_prefixes(names: &[String]) -> Result<(), DriverError> {
    for first in names {
        let prefix = format!("{first}_");
        if let Some(second) = names.iter().find(|name| name.starts_with(prefix.as_str())) {
            return Err(DriverError::AmbiguousServers {
                first: first.clone(),   // BORROW: .clone() makes the owned name the error keeps
                second: second.clone(), // BORROW: .clone() makes the owned name the error keeps
            });
        }
    }
    Ok(())
}

/// The placeholder the core fills with the root session id.
const SESSION_ID: &str = candler_harness::spec::SESSION_ID_PLACEHOLDER;
