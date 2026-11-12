//! The `OpenCode` driver (a stand-in the fixture suite fails on).

use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::TraceReader;
use candler_harness::spec::{
    Capabilities, Cell, CollectSpec, CommandSpec, HarnessId, Observed, Prepared, ProcessExit,
    StopSignal, StopSpec, Workspace,
};
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session};

/// The `OpenCode` driver.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct OpenCodeDriver;

impl OpenCodeDriver {
    /// A driver.
    ///
    /// # Errors
    /// Returns [`DriverError`] if the harness name or the probe cannot be built.
    pub fn new() -> Result<Self, DriverError> {
        Ok(Self)
    }
}

impl HarnessDriver for OpenCodeDriver {
    fn id(&self) -> HarnessId {
        HarnessId::new("opencode".parse().unwrap_or_else(|_| unreachable!()))
    }
    fn supported(&self) -> semver::VersionReq {
        semver::VersionReq::STAR
    }
    fn version_probe(&self) -> CommandSpec {
        CommandSpec::builder()
            .argv(vec![String::from("opencode")])
            .build()
            .unwrap_or_else(|_| unreachable!())
    }
    fn parse_version(&self, _output: &[u8]) -> Result<semver::Version, DriverError> {
        Ok(semver::Version::new(0, 0, 0))
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
    fn prepare(&self, _c: &Cell<'_>, _w: &Workspace<'_>) -> Result<Prepared, DriverError> {
        Ok(Prepared::default())
    }
    fn command(&self, _c: &Cell<'_>, _w: &Workspace<'_>) -> Result<CommandSpec, DriverError> {
        Ok(self.version_probe())
    }
    fn stop(&self) -> StopSpec {
        StopSpec::new(None, StopSignal::Term, std::time::Duration::from_secs(1))
    }
    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        None
    }
    fn offline_reader(&self, _stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        None
    }
    fn classify_exit(&self, _exit: &ProcessExit, _sessions: &[Session]) -> EndedBy {
        EndedBy::Unknown
    }
    fn collect(&self, _w: &Workspace<'_>, _o: &Observed) -> Vec<CollectSpec> {
        Vec::new()
    }
}
