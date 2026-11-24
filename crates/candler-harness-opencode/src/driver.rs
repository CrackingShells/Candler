//! The `OpenCode` driver: the command line, the configuration, the state paths, the stop, the
//! collect and the exit classification.
//!
//! The driver is data only. It spawns nothing, reads no endpoint and names no token: the core owns
//! the environment and the secrets of the agent, and the configuration refers to them by name.

use std::str::FromStr as _;
use std::time::Duration;

use candler_harness::driver::{DriverError, HarnessDriver, Plan, check_answers};
use candler_harness::reader::TraceReader;
use candler_harness::skills::SkillTree;
use candler_harness::spec::{
    Action, AgentSetting, Answer, BaseImage, Cell, CollectSpec, CommandSpec, DriverFact,
    GatewayEnv, HarnessId, Observed, Prepared, ProcessExit, StateKind, StatePath, StopSignal,
    StopSpec, Workspace,
};
use candler_schemas::name;
use candler_schemas::primitives::{RelPath, plain_arg};
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};
use semver::{Version, VersionReq};

use crate::export::{OpenCodeExportReader, ended_by_of_observed, export_stream};
use crate::live::OpenCodeLiveReader;
use crate::prepare::{self, model_id};
use crate::{CENSUS_STREAM, EXPORT_READER, VALIDATED};

const PROGRAM: &str = "opencode";

/// The variable that holds the gateway's address; the configuration refers to it.
const GATEWAY_BASE_URL: &str = "CANDLER_GATEWAY_URL";
/// The variable that holds the gateway's token; the configuration refers to it.
const GATEWAY_AUTH_TOKEN: &str = "CANDLER_GATEWAY_TOKEN";

/// The state directories and the database file, relative to the agent's home.
const STATE: [(&str, StateKind, &str); 5] = [
    ("XDG_DATA_HOME", StateKind::Dir, ".xdg/data"),
    ("XDG_CONFIG_HOME", StateKind::Dir, ".xdg/config"),
    ("XDG_CACHE_HOME", StateKind::Dir, ".xdg/cache"),
    ("XDG_STATE_HOME", StateKind::Dir, ".xdg/state"),
    ("OPENCODE_DB", StateKind::File, ".xdg/data/oc.db"),
];

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
            id: HarnessId::new(name!("opencode")),
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
                how: "the gateway's first model request with tools",
            },
        }
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
        check_answers(self, cell, prepare::DISPLAY_NAME, &VALIDATED)?;
        prepare::settings(cell, &VALIDATED)?;
        prepare::mcp_servers(cell).map(|_| ())
    }

    fn plan(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Plan, DriverError> {
        let _ = workspace;
        check_answers(self, cell, prepare::DISPLAY_NAME, &VALIDATED)?;
        let settings = prepare::settings(cell, &VALIDATED)?;
        let model = model_id(cell)?;
        let model_arg = match &settings.variant {
            Some((id, _)) => format!("{}/{model}#{id}", prepare::PROVIDER),
            None => format!("{}/{model}", prepare::PROVIDER),
        };
        let file = prepare::render_config(
            cell,
            settings,
            Self::state_subpath("XDG_CONFIG_HOME"),
            GATEWAY_BASE_URL,
            GATEWAY_AUTH_TOKEN,
        )?;
        let mut line = argv(&[PROGRAM, "run", "--standalone", "--format", "json"]);
        line.push("--auto".to_owned()); // BORROW: .to_owned() makes the owned argument
        line.push("-m".to_owned()); // BORROW: .to_owned() makes the owned argument
        line.push(model_arg);
        line.push("--".to_owned()); // BORROW: .to_owned() makes the owned argument
        line.push(cell.prompt()?);
        Ok(Plan::new(
            Prepared::new(vec![file], Vec::new()),
            CommandSpec::builder().argv(line).build()?,
        ))
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

    fn census_stream(&self) -> Option<StreamName> {
        census_stream()
    }

    fn offline_reader(&self, stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        if export_stream().as_ref() == Some(stream) {
            return Some(Box::new(OpenCodeExportReader::new())); // TRAIT_OBJECT: the core selects readers at runtime
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

/// The stream the gateway's census is stored as.
fn census_stream() -> Option<StreamName> {
    CENSUS_STREAM.parse().ok()
}

/// The placeholder the core fills with the root session id.
const SESSION_ID: &str = candler_harness::spec::SESSION_ID_PLACEHOLDER;
