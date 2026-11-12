//! The public items other crates consume, used from outside the crate: usage only, with no
//! behaviour asserted. It is the contract the API review diffs.
#![cfg(test)]

use std::path::Path;
use std::time::Duration;

use candler_harness::driver::{Disposition, DriverError, HarnessDriver, SettingRow};
use candler_harness::reader::{FrameIn, ReaderError, TraceReader};
use candler_harness::skills::SkillTree;
use candler_harness::spec::{
    Action, BudgetCaps, Capabilities, CapabilitiesBuilder, Cell, CensusSource, CollectSpec,
    CommandSpec, CommandSpecBuilder, CommandSpecError, Confidence, EnvEntry, EnvValue, FileRoot,
    FormatVersion, Framing, GatewayEnv, HarnessId, HookSpec, HttpMethod, InitReport,
    InitReportError, Observed, ObservedEnd, PendingItem, PendingKind, PendingState, Prepared,
    PreparedFile, ProcessEnd, ProcessExit, RunTotals, SESSION_ID_PLACEHOLDER, SessionClose,
    SessionRecord, SessionSpec, SessionTracker, StateKind, StatePath, StopSignal, StopSpec,
    Workspace,
};
use candler_schemas::execution::TreatmentRef;
use candler_schemas::primitives::FieldPath;
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session, SessionEvent, SessionId};
use serde_json::json;

// TRAIT_OBJECT: a usage-only function returns any error its steps raise
type Res = Result<(), Box<dyn std::error::Error>>;

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

/// Calls the driver methods the session runner added.
fn drive_session_runner(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the core selects drivers at runtime
    cell: &Cell<'_>,
    workspace: &Workspace<'_>,
    exit: &ProcessExit,
    sessions: &[Session],
    observed: &Observed,
) -> Result<(), DriverError> {
    let rows: Vec<SettingRow> = driver.settings_table();
    for row in &rows {
        let _: (&String, &String, &String, &String) =
            (&row.setting, &row.value, &row.construct, &row.detail);
        let _: Disposition = row.disposition;
    }
    let _ = SettingRow::mapped("a", "b", "c", "d");
    let _ = SettingRow::refused("a", "b", "c");
    let _ = (Disposition::Mapped, Disposition::Refused);
    let _: Option<GatewayEnv> = driver.gateway();
    let _: Vec<StatePath> = driver.state_paths();
    let _: Option<StreamName> = driver.census_stream();
    let _: Option<SessionSpec> = driver.session(cell, workspace)?;
    let _: Option<Box<dyn SessionTracker>> = driver.session_tracker(); // TRAIT_OBJECT: the core selects trackers at runtime
    let _: EndedBy = driver.classify_end(exit, sessions, observed);
    Ok(())
}

/// Names the refusals a driver and a spec return.
fn refusals(error: &DriverError, spec_error: &CommandSpecError) -> Res {
    let version = semver::Version::new(1, 0, 0);
    let _: DriverError = DriverError::unsupported(
        &"permission_mode".parse::<FieldPath>()?,
        "H",
        &version,
        "why",
    );
    if let DriverError::Unsupported {
        path,
        harness,
        version,
        detail,
        ..
    } = error
    {
        let _: (&String, &String, &semver::Version, &String) = (path, harness, version, detail);
    }
    if let DriverError::Record { detail, .. } = error {
        let _: &String = detail;
    }
    if let CommandSpecError::UnsafeSessionId { id, .. } = spec_error {
        let _: &String = id;
    }
    Ok(())
}

/// Calls the skill tree a harness lays out.
fn skills(
    tree: &dyn SkillTree, // TRAIT_OBJECT: the core selects skill trees at runtime
    treatment: &TreatmentRef,
) -> Result<(), DriverError> {
    let _: CommandSpec = tree.archive_command(treatment, Path::new("."))?;
    let _: Vec<PreparedFile> = tree.files(treatment, b"")?;
    Ok(())
}

/// Builds and reads the spec types the session runner added.
fn session_runner_specs(
    tracker: &mut dyn SessionTracker, // TRAIT_OBJECT: the core holds trackers as objects
) -> Res {
    let id: SessionId = "ses_1".parse()?;
    let _: &str = SESSION_ID_PLACEHOLDER;
    let command = CommandSpec::builder()
        .argv(vec![format!("tool {SESSION_ID_PLACEHOLDER}")])
        .build()?;
    let _: bool = command.uses_session_id();
    let _: CommandSpec = command.with_session_id(&id)?;
    let action = Action::exec(command.clone()); // BORROW: .clone() copies the command
    let _: bool = action.uses_session_id();
    let _: Option<Action> = action.resolve(Some(&id));
    let _: Action = action.with_session_id(&id)?;
    let _ = Action::Command(command.clone()); // BORROW: .clone() copies the command
    let stream: StreamName = "census".parse()?;
    let _ = CollectSpec::exec(command.clone(), stream.clone()); // BORROW: .clone() copies the command and the name
    let capabilities = Capabilities::builder().session(true).build();
    let _: bool = capabilities.session;

    let totals = RunTotals::new(None, None, None);
    let _ = (&totals.usage, &totals.cost_usd, &totals.num_turns);
    let init = InitReport::new(json!(["Bash"]), None, CensusSource::Harness)?;
    let _ = (&init.tools, &init.mcp_servers, init.source);
    let _ = (CensusSource::Gateway, CensusSource::Collected);
    let _: Option<InitReportError> = InitReport::new(json!(1), None, CensusSource::Harness).err();
    let _ = InitReportError::ToolsNotNames;
    let _ = InitReportError::EmptyToolName;
    let _ = InitReportError::ServersNotList;
    let _ = InitReportError::NoTools;
    let observed = Observed::new(vec![id.clone()]) // BORROW: .clone() copies the id
        .with_init(init)
        .with_end(ObservedEnd::Finished)
        .with_totals(totals);
    let _: Option<&SessionId> = observed.root_session_id();
    let _ = (
        &observed.session_ids,
        &observed.init,
        &observed.end,
        &observed.totals,
    );
    let _ = ObservedEnd::Error {
        kind: String::new(),
    };
    let _ = ObservedEnd::Aborted;

    let gateway = GatewayEnv::new(String::new(), String::new(), Vec::new());
    let _ = (&gateway.base_url, &gateway.auth_token, &gateway.reserved);
    let _ = (StateKind::Dir, StateKind::File);
    let state = StatePath::new(String::new(), StateKind::Dir, ".state".parse()?);
    let _ = (&state.env, state.kind, &state.subpath);

    let _ = (
        PendingKind::Task,
        PendingKind::Monitor,
        PendingKind::Cron,
        PendingKind::WakeUp,
    );
    let item = PendingItem::new(PendingKind::Task, String::new());
    let _ = (item.kind, &item.id);
    let pending = PendingState::new(0, true, true, vec![item]);
    let _ = (
        pending.turns,
        pending.turn_ended,
        pending.known,
        &pending.pending,
    );
    tracker.feed(SessionRecord::Stream(b""))?;
    tracker.feed(SessionRecord::Hook(b""))?;
    let _: PendingState = tracker.state();

    let _ = (FileRoot::Workspace, FileRoot::Home);
    let file = PreparedFile::in_home(".f".parse()?, Vec::new(), false);
    let _: FileRoot = file.root;
    let hooks = HookSpec::new(vec![file], ".log".parse()?, stream);
    let _ = (&hooks.files, &hooks.log, &hooks.stream);
    let _ = SessionClose::Eof;
    let spec =
        SessionSpec::new(command, Vec::new(), SessionClose::Action(action)).with_hooks(hooks);
    let _ = (&spec.command, &spec.input, &spec.close, &spec.hooks);
    Ok(())
}

/// The shared fixture suite, used from outside the crate.
#[cfg(feature = "conformance")]
fn conformance_suite(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the suite checks any driver
    cell: &Cell<'_>,
    workspace: &Workspace<'_>,
) -> Res {
    use candler_harness::conformance::{
        Case, ExpectedSession, Failure, ReaderSource, RecordBytes, Setup, TrackStep, run,
    };
    let id: SessionId = "ses_1".parse()?;
    let stream: StreamName = "stream".parse()?;
    let expected = ExpectedSession::new(id.clone(), None, EndedBy::Completed); // BORROW: .clone() copies the id
    let _ = (&expected.id, &expected.parent, &expected.ended_by);
    let step = TrackStep::new(
        RecordBytes::Stream(Vec::new()),
        Some(PendingState::new(0, false, true, Vec::new())),
    );
    let _ = RecordBytes::Hook(Vec::new());
    let _ = (&step.record, &step.state);
    let exit = ProcessExit::new(ExitStatus::Code { code: 0 }, ProcessEnd::Exited, Vec::new());
    let case = Case::new(
        "case".to_owned(),                     // BORROW: .to_owned() makes the owned name
        ReaderSource::Offline(stream.clone()), // BORROW: .clone() copies the name
        stream.clone(),                        // BORROW: .clone() copies the name
        Vec::new(),
    )
    .with_sessions(vec![expected.clone()], vec![id.clone()]) // BORROW: .clone() copies the expectation and the id
    .with_observed_ids(vec![id])
    .with_init(InitReport::new(
        json!(["Bash"]),
        None,
        CensusSource::Harness,
    )?)
    .with_end(exit, EndedBy::Completed)
    .with_observed_end(ObservedEnd::Finished)
    .detects(true)
    .with_export(stream, Vec::new(), vec![expected])
    .with_collect(vec![CollectSpec::file(String::new())])
    .with_track(vec![step]);
    let _ = ReaderSource::Live;
    let _ = (
        &case.name,
        &case.reader,
        &case.stream,
        &case.bytes,
        case.detects,
        &case.sessions,
        &case.observed_ids,
        &case.init,
        &case.observed_end,
        &case.end,
        &case.collect,
        &case.export,
        &case.track,
    );
    let canaries: Vec<String> = Vec::new();
    let setup = Setup::new(cell, workspace, &canaries);
    let _ = (setup.cell, setup.workspace, setup.canaries);
    let result: Result<(), Failure> = run(driver, &setup, &[case]);
    if let Err(failure) = result {
        let _ = (&failure.case, failure.check, &failure.detail);
    }
    Ok(())
}

/// The surface compiles; nothing runs.
#[test]
fn public_surface_compiles() {
    let _ = (
        drive,
        read,
        specs,
        drive_session_runner,
        refusals,
        skills,
        session_runner_specs,
    );
    #[cfg(feature = "conformance")]
    let _ = conformance_suite;
}
