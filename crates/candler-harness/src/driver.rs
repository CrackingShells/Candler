//! The harness driver: what the runner needs from one agent harness, as data.
//!
//! A driver returns commands, stop and collect specs and readers, and never acts on a process:
//! the core runs every command and HTTP call a driver describes. No method is `async`, so drivers
//! are usable as `dyn` trait objects.

use candler_schemas::primitives::FieldPath;
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};

use crate::reader::TraceReader;
use crate::spec::{
    Capabilities, Cell, CollectSpec, CommandSpec, CommandSpecError, HarnessId, Observed, Prepared,
    ProcessExit, StopSpec, Workspace,
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
    /// The output of the version probe, which names no version.
    #[error("version output cannot be read: {detail}")]
    VersionOutput {
        /// What is wrong with the output.
        detail: String,
    },
    /// A command the driver could not build.
    #[error(transparent)]
    Command(#[from] CommandSpecError),
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

    /// The files and the environment the driver adds before the launch.
    ///
    /// # Errors
    /// Returns [`DriverError::MissingValue`] if the cell lacks a value the driver needs.
    /// Returns [`DriverError::InvalidValue`] if a value cannot be used.
    fn prepare(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Prepared, DriverError>;

    /// The headless command that runs the cell.
    ///
    /// # Errors
    /// Returns [`DriverError::MissingValue`] if the cell lacks a value the driver needs.
    /// Returns [`DriverError::InvalidValue`] if a value cannot be used.
    /// Returns [`DriverError::Command`] if the command cannot be built.
    fn command(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<CommandSpec, DriverError>;

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

    /// What the core collects after the run. `observed` is what the live reader reported, so the
    /// ids a collect command needs come from the live reader's `observed()`.
    fn collect(&self, workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec>;
}
