//! The harness driver: what the runner needs from one agent harness, as data.
//!
//! A driver returns commands, stop and collect specs and readers, and never acts on a process:
//! the core runs every command and HTTP call a driver describes. No method is `async`, so drivers
//! are usable as `dyn` trait objects.

use candler_schemas::primitives::FieldPath;
use candler_schemas::raw::StreamName;
use candler_schemas::refusal::pointer_of;
use candler_schemas::trace::{EndedBy, Session};

use crate::reader::TraceReader;
use crate::skills::SkillTree;
use crate::spec::{
    BaseImage, Capabilities, Cell, CollectSpec, CommandSpec, CommandSpecError, GatewayEnv,
    HarnessId, Observed, Prepared, ProcessExit, SessionSpec, SessionTracker, StatePath, StopSpec,
    Workspace,
};

/// Why a driver could not answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DriverError {
    /// A cell that lacks a value the driver needs.
    #[error("cell has no value for `{path}`")]
    MissingValue {
        /// The field the driver needs.
        path: FieldPath,
    },
    /// A cell value the driver cannot use.
    #[error("cell value for `{path}` cannot be used: {detail}")]
    InvalidValue {
        /// The field that holds the value.
        path: FieldPath,
        /// What is wrong with it.
        detail: String,
    },
    /// A setting the harness has no way to honour, refused rather than dropped.
    #[error("{harness} {version} cannot honour {path}: {detail}")]
    Unsupported {
        /// The pointer into the experiment file of the setting.
        path: String,
        /// The harness's name, as its users write it.
        harness: String,
        /// The harness version the driver was validated against.
        version: semver::Version,
        /// Why the harness cannot honour it, and what it accepts instead.
        detail: String,
    },
    /// The output of the version probe, which names no version.
    #[error("version output cannot be read: {detail}")]
    VersionOutput {
        /// What is wrong with the output.
        detail: String,
    },
    /// A command the driver could not build.
    #[error(transparent)]
    Command(#[from] CommandSpecError),
    /// A record of a session that a tracker cannot read.
    #[error("session record cannot be read: {detail}")]
    Record {
        /// What is wrong with the record.
        detail: String,
    },
}

impl DriverError {
    /// The refusal of `setting`, which the harness `harness` at `version` cannot honour for the
    /// reason `detail`.
    ///
    /// The pointer is the setting's place in the experiment file: a `budget` field sits under
    /// `/budget`, and any other setting is a key of `[fixed]`, written as its dotted path.
    #[must_use]
    pub fn unsupported(
        setting: &FieldPath,
        harness: &str,
        version: &semver::Version,
        detail: &str,
    ) -> Self {
        let text = setting.to_string(); // BORROW: .to_string() gives owned text
        let path = if setting.segments().next() == Some("budget") {
            pointer_of(setting.segments())
        } else {
            pointer_of(["fixed", text.as_str()])
        };
        Self::Unsupported {
            path,
            harness: harness.to_owned(), // BORROW: .to_owned() makes the owned text
            version: version.clone(),    // BORROW: .clone() copies the version
            detail: detail.to_owned(),   // BORROW: .to_owned() makes the owned text
        }
    }
}

/// Whether a driver honours one cell setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Disposition {
    /// The harness has a construct with the setting's meaning, and the driver writes it.
    Mapped,
    /// The harness has no construct with the setting's meaning; the driver refuses the setting
    /// with [`DriverError::Unsupported`].
    Refused,
}

/// One row of a driver's settings table: a cell setting (or one value or name of it), whether the
/// driver honours it, and in either case the harness construct or the reason, in a few words.
///
/// The table is data a capability page can read; the driver builds its command and its files from
/// the same rows, so a row and what the driver does cannot differ.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SettingRow {
    /// The cell path, such as `permission_mode` or `tools.allowed`.
    pub setting: String,
    /// The value or name the row is about (`bypassPermissions`, `Bash`, `high`), or `*` when the
    /// row covers the whole setting.
    pub value: String,
    /// Whether the driver honours the setting.
    pub disposition: Disposition,
    /// The harness construct a mapped row writes (empty for a refused row).
    pub construct: String,
    /// How a mapped row maps, or why a refused row is refused.
    pub detail: String,
}

impl SettingRow {
    /// A row the driver honours, writing `construct`.
    #[must_use]
    pub fn mapped(setting: &str, value: &str, construct: &str, detail: &str) -> Self {
        Self {
            setting: setting.to_owned(), // BORROW: .to_owned() makes the owned text the row keeps
            value: value.to_owned(),     // BORROW: .to_owned() makes the owned text the row keeps
            disposition: Disposition::Mapped,
            construct: construct.to_owned(), // BORROW: .to_owned() makes the owned text the row keeps
            detail: detail.to_owned(), // BORROW: .to_owned() makes the owned text the row keeps
        }
    }

    /// A row the driver refuses, for `why`.
    #[must_use]
    pub fn refused(setting: &str, value: &str, why: &str) -> Self {
        Self {
            setting: setting.to_owned(), // BORROW: .to_owned() makes the owned text the row keeps
            value: value.to_owned(),     // BORROW: .to_owned() makes the owned text the row keeps
            disposition: Disposition::Refused,
            construct: String::new(),
            detail: why.to_owned(), // BORROW: .to_owned() makes the owned text the row keeps
        }
    }
}

/// What a driver does for one cell in one workspace, built once so every part reads the same
/// settings.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Plan {
    /// The files and the environment the driver adds before the launch.
    pub prepared: Prepared,
    /// The headless command that runs the cell.
    pub command: CommandSpec,
    /// How the harness runs the cell as a session, or `None` for a harness that runs none.
    pub session: Option<SessionSpec>,
}

impl Plan {
    /// A plan that prepares `prepared` and runs `command` headless, with no session.
    #[must_use]
    pub const fn new(prepared: Prepared, command: CommandSpec) -> Self {
        Self {
            prepared,
            command,
            session: None,
        }
    }

    /// This plan with `session` as the session that runs the cell, in place of any it held.
    #[must_use]
    pub fn with_session(self, session: SessionSpec) -> Self {
        Self {
            session: Some(session),
            ..self
        }
    }
}

/// What the runner needs from one harness, returned as data for the core to act on.
pub trait HarnessDriver: Send + Sync {
    /// The harness's name and version token.
    fn id(&self) -> HarnessId;

    /// The harness versions this driver supports.
    fn supported(&self) -> semver::VersionReq;

    /// The command whose output names the installed harness version.
    fn version_probe(&self) -> CommandSpec;

    /// The harness version named by the output of the version probe.
    ///
    /// # Errors
    /// Returns [`DriverError::VersionOutput`] if `output` names no version.
    fn parse_version(&self, output: &[u8]) -> Result<semver::Version, DriverError>;

    /// What the harness can do.
    fn capabilities(&self) -> Capabilities;

    /// What the driver does for `cell` in `workspace`: the files and the environment it adds
    /// before the launch, the headless command, and the session, if the harness runs one. The core
    /// asks once per run and reads every part from the one plan.
    ///
    /// # Errors
    /// Returns [`DriverError::MissingValue`] if the cell lacks a value the driver needs.
    /// Returns [`DriverError::InvalidValue`] if a value cannot be used.
    /// Returns [`DriverError::Unsupported`] for a setting the harness cannot honour.
    /// Returns [`DriverError::Command`] if a command cannot be built.
    fn plan(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Plan, DriverError>;

    /// The settings table: one row per cell setting, value or name the driver knows, mapped or
    /// refused. A driver that declares nothing keeps the default, an empty table, and answers
    /// each setting in [`HarnessDriver::plan`].
    ///
    /// No two rows name the same `(setting, value)`, a refused row says why, and a mapped row
    /// says what it writes.
    fn settings_table(&self) -> Vec<SettingRow> {
        Vec::new()
    }

    /// Refuses a cell that sets something the driver cannot honour, with the error
    /// [`HarnessDriver::plan`] gives for it, so the core can refuse the cell before any job
    /// starts. The default accepts every cell.
    ///
    /// # Errors
    /// [`DriverError`] for the first setting of the cell the driver cannot honour.
    fn check_settings(&self, cell: &Cell<'_>) -> Result<(), DriverError> {
        let _ = cell;
        Ok(())
    }

    /// The names of the variables through which the core hands the gateway to the agent, or
    /// `None` for a driver that names none. The core sets the values; a driver whose
    /// configuration needs the address or the token writes a reference to the variable's name
    /// into it (`prepare`), so no value passes through the driver. A driver the core runs against
    /// the gateway names its variables: there is no default set of names.
    fn gateway(&self) -> Option<GatewayEnv> {
        None
    }

    /// The places the harness keeps state inside the agent's home. The core creates each one per
    /// run and sets its variable, so a run never reads or leaves another run's state. A harness
    /// with no state of its own beyond the home keeps the default, an empty list.
    ///
    /// No two paths are the same path, and a [`crate::spec::StateKind::File`] is never another
    /// path's parent. A file inside a directory the driver also lists is fine: the core creates
    /// parents, and no state path is mounted on its own.
    fn state_paths(&self) -> Vec<StatePath> {
        Vec::new()
    }

    /// The raw stream the gateway's census is stored under when the harness has no init event, or
    /// `None` when the census comes from the live reader's init report. The core reads the tool
    /// names the gateway saw in the model requests and reports them as the inventory, labelled
    /// [`crate::spec::CensusSource::Gateway`]; a run none of whose requests carry tools leaves it
    /// unrecorded, never an empty set.
    fn census_stream(&self) -> Option<StreamName> {
        None
    }

    /// Where a treatment's skill tree comes from and where the harness reads it.
    fn skills(&self) -> SkillTree;

    /// The build context of the harness's base image and the build arguments that pin its
    /// version.
    fn base_image(&self) -> BaseImage;

    /// The streams the harness collects after a run, each read offline by
    /// [`HarnessDriver::offline_reader`]. The default is none.
    fn offline_streams(&self) -> Vec<StreamName> {
        Vec::new()
    }

    /// The harness version the start of a raw stream names, or `None` when the stream is not the
    /// harness's. A version outside [`HarnessDriver::supported`] is still the harness's: the core
    /// alone decides whether it is supported.
    fn detect(&self, sample: &[u8]) -> Option<semver::Version>;

    /// A tracker that follows a session from its stream frames and hook records, or `None` for a
    /// harness that runs none.
    fn session_tracker(&self) -> Option<Box<dyn SessionTracker>> {
        // TRAIT_OBJECT: the core selects drivers and trackers at runtime
        None
    }

    /// How the core stops the harness.
    fn stop(&self) -> StopSpec;

    /// The reader of the harness's live output, or `None` if the harness streams none. The core
    /// feeds the child's stdout to it whenever there is one. A driver whose harness writes its
    /// events elsewhere may return a Lines reader that emits no events and only reports the
    /// session ids it sees; `live_events` then means only that events arrive before the exit.
    fn live_reader(&self) -> Option<Box<dyn TraceReader>>; // TRAIT_OBJECT: the core selects drivers and readers at runtime

    /// The reader of a stream the core collected after the run, such as an export, or `None` if
    /// the driver reads no such stream.
    fn offline_reader(&self, stream: &StreamName) -> Option<Box<dyn TraceReader>>; // TRAIT_OBJECT: the core selects drivers and readers at runtime

    /// How the run ended, read from the exit and the parsed sessions rather than from the exit
    /// status alone: an exit 0 with an error result is an agent error. A run the experimenter
    /// stopped (job budget, `candler kill`) is classified by the core, whatever this returns.
    fn classify_exit(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy;

    /// How the run ended, from the exit, the parsed sessions and the live reader's last word.
    /// The core calls this one. The default is [`HarnessDriver::classify_exit`], which ignores
    /// `observed`. A harness whose exit status covers several endings overrides it: only the
    /// stream's last error event or the export tells them apart.
    fn classify_end(
        &self,
        exit: &ProcessExit,
        sessions: &[Session],
        observed: &Observed,
    ) -> EndedBy {
        let _ = observed;
        self.classify_exit(exit, sessions)
    }

    /// What the core collects after the run. `observed` is what the live reader reported, so the
    /// ids a collect command needs come from the live reader's `observed()`.
    fn collect(&self, workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec>;
}
