//! The public items other crates consume, used from outside the crate: usage only, with no
//! behaviour asserted. It is the contract the API review diffs.
#![cfg(test)]

use std::path::Path;
use std::time::Duration;

use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::{FrameIn, ReaderError, TraceReader};
use candler_harness::spec::{
    Action, BudgetCaps, Capabilities, CapabilitiesBuilder, Cell, CollectSpec, CommandSpec,
    CommandSpecBuilder, CommandSpecError, Confidence, EnvEntry, EnvValue, FormatVersion, Framing,
    HarnessId, HttpMethod, Observed, Prepared, PreparedFile, ProcessEnd, ProcessExit, StopSignal,
    StopSpec, Workspace,
};
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session, SessionEvent};

const fn assert_send<T: ?Sized + Send>() {}
const fn assert_send_sync<T: ?Sized + Send + Sync>() {}

const _: () = assert_send_sync::<Box<dyn HarnessDriver>>(); // TRAIT_OBJECT: the core holds drivers as objects
const _: () = assert_send::<Box<dyn TraceReader>>(); // TRAIT_OBJECT: the core holds readers as objects

/// Calls every driver method the core uses.
fn drive(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the core selects drivers at runtime
    cell: &Cell<'_>,
    workspace: &Workspace<'_>,
    exit: &ProcessExit,
    sessions: &[Session],
    stream: &StreamName,
) -> Result<(), DriverError> {
    let _: HarnessId = driver.id();
    let _: semver::VersionReq = driver.supported();
    let _: CommandSpec = driver.version_probe();
    let _: semver::Version = driver.parse_version(b"")?;
    let _: Capabilities = driver.capabilities();
    let _: Prepared = driver.prepare(cell, workspace)?;
    let _: CommandSpec = driver.command(cell, workspace)?;
    let _: StopSpec = driver.stop();
    let _: Option<Box<dyn TraceReader>> = driver.live_reader(); // TRAIT_OBJECT: the core selects readers at runtime
    let _: Option<Box<dyn TraceReader>> = driver.offline_reader(stream); // TRAIT_OBJECT: the core selects readers at runtime
    let _: EndedBy = driver.classify_exit(exit, sessions);
    let _: Vec<CollectSpec> = driver.collect(workspace, &Observed::default());
    Ok(())
}

/// Calls every reader method the core uses.
fn read(
    mut reader: Box<dyn TraceReader>, // TRAIT_OBJECT: the core selects readers at runtime
    frame: FrameIn<'_>,
) -> Result<(), ReaderError> {
    let _: candler_schemas::primitives::NameVersion = reader.id();
    let _: Framing = reader.framing();
    let _: Option<(FormatVersion, Confidence)> = reader.detect(b"");
    let _: Vec<SessionEvent> = reader.feed(frame)?;
    let _: Observed = reader.observed();
    let _: Vec<Session> = reader.finish()?;
    Ok(())
}

/// Builds and reads every spec type the core consumes.
fn specs() -> Result<(), CommandSpecError> {
    let command: CommandSpec = CommandSpecBuilder::default()
        .argv(vec!["tool".to_owned()]) // BORROW: .to_owned() makes the owned argument
        .env(EnvEntry::literal("A".to_owned(), "1".to_owned())) // BORROW: .to_owned() makes owned text
        .build()?;
    let _: &[String] = command.argv();
    let _: &[EnvEntry] = command.env_overlay();
    let _: Option<&candler_schemas::primitives::RelPath> = command.cwd();
    let _ = CommandSpec::builder();
    let _ = CapabilitiesBuilder::default().build();
    let _ = Capabilities::builder()
        .budgets(BudgetCaps::new(true, false))
        .build();
    let _ = EnvValue::Literal(String::new());
    let _ = StopSpec::new(
        Some(Action::http(HttpMethod::Post, String::new(), None)),
        StopSignal::Term,
        Duration::from_secs(1),
    );
    let _ = StopSignal::Int;
    let _ = StopSignal::Hup;
    let _ = CollectSpec::file(String::new());
    let _ = ProcessExit::new(ExitStatus::Code { code: 0 }, ProcessEnd::Exited, Vec::new());
    let _ = ProcessEnd::Deadline;
    let _ = ProcessEnd::Stopped;
    let _ = Prepared::new(Vec::<PreparedFile>::new(), Vec::new());
    let _ = Workspace::new(Path::new("."));
    let _ = Cell::new(&[]);
    let _ = Framing::Lines;
    let _ = Framing::Document;
    Ok(())
}

/// The surface compiles; nothing runs.
#[test]
fn public_surface_compiles() {
    let _ = (drive, read, specs);
}
