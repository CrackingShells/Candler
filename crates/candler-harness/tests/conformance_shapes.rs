//! The shared fixture suite on the fake shapes: a Lines reader with an init event, a Document
//! reader with an export, and a Document-framed fake that takes the `OpenCode` facts one by one.
//!
//! The suite makes no assumption that belongs to one harness, and these cases are what shows it:
//! the Document cases have no live reader and no init event, and the census of the third fake is
//! a collected gateway record. The negative tests break one thing in a working driver and expect
//! the suite to refuse it, with the unbroken driver as the positive control.
#![cfg(test)]

mod support;

use std::path::Path;
use std::time::Duration;

use candler_harness::conformance::{
    Case, ExpectedSession, ReaderSource, RecordBytes, Setup, TrackStep, run,
};
use candler_harness::driver::{DriverError, HarnessDriver, SettingRow};
use candler_harness::reader::TraceReader;
use candler_harness::spec::{
    Action, Capabilities, Cell, CensusSource, CollectSpec, CommandSpec, EnvEntry, GatewayEnv,
    InitReport, Observed, ObservedEnd, PendingItem, PendingKind, PendingState, Prepared,
    PreparedFile, ProcessEnd, ProcessExit, SessionClose, SessionSpec, SessionTracker, StateKind,
    StatePath, StopSignal, StopSpec, Workspace,
};
use candler_schemas::primitives::FieldPath;
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{EndedBy, Session, SessionId};
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::{Value, json};

use support::{DocumentDriver, LinesDriver, OpenShapedDriver, TestError, TestResult};

/// A value that stands for the gateway's token; it must appear in no command.
const CANARY: &str = "sk-conformance-canary-7f3a";

/// A file of a recorded `OpenCode` case of `opencode_facts`.
fn fixture(case: &str, file: &str) -> Result<Vec<u8>, TestError> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../candler-harness-opencode/tests/fixtures/streams")
        .join(case)
        .join(file);
    Ok(std::fs::read(path)?)
}

/// The value of `key = <value>` in a hand-written fixture note: a quoted text or a number, with
/// any trailing comment cut.
fn note(case: &str, file: &str, key: &str) -> Result<String, TestError> {
    let text = String::from_utf8(fixture(case, file)?)?;
    let line = text
        .lines()
        .find(|line| line.starts_with(&format!("{key} = ")))
        .ok_or_else(|| format!("{case}/{file} has no `{key}`"))?;
    let value = line.split_once(" = ").map_or("", |(_, v)| v);
    let value = value.split(" #").next().unwrap_or_default().trim();
    Ok(value.trim_matches('"').to_owned())
}

/// The quoted value after `key = ` inside the inline table of `expected.toml`'s terminal event.
fn terminal_error_type(case: &str) -> Result<String, TestError> {
    let text = String::from_utf8(fixture(case, "expected.toml")?)?;
    let tail = text
        .split("error_type = \"")
        .nth(1)
        .ok_or("no error_type")?;
    Ok(tail.split('"').next().unwrap_or_default().to_owned())
}

/// How the export's `outcome` classifies a run, as `opencode_facts` row 15 states it.
fn class_of(outcome: &str) -> Result<EndedBy, TestError> {
    match outcome {
        "succeeded" => Ok(EndedBy::Completed),
        "interrupted" => Ok(EndedBy::Interrupted),
        "failed" => Ok(EndedBy::agent_error(None)),
        other => Err(format!("unknown outcome {other}").into()),
    }
}

fn sid(text: &str) -> Result<SessionId, TestError> {
    Ok(text.parse()?)
}

fn name(text: &str) -> Result<StreamName, TestError> {
    Ok(text.parse()?)
}

fn exited(code: i32) -> ProcessExit {
    ProcessExit::new(ExitStatus::Code { code }, ProcessEnd::Exited, Vec::new())
}

/// The cell every fake builds its command from.
fn cell_values() -> Result<Vec<ResolvedValue>, TestError> {
    let path: FieldPath = "task.prompt".parse()?;
    Ok(vec![ResolvedValue::recorded(
        path,
        JsonValue::new(Value::String(String::from("fix it")))?,
    )])
}

/// Runs the suite on `driver` with the standard cell and canary.
fn run_suite(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the suite checks any driver
    cases: &[Case],
) -> Result<(), candler_harness::conformance::Failure> {
    let values = cell_values().unwrap_or_default();
    let cell = Cell::new(&values);
    let workspace = Workspace::new(Path::new("."));
    let canaries = [String::from(CANARY)];
    run(driver, &Setup::new(&cell, &workspace, &canaries), cases)
}

/// The Lines-shaped case: the sessions of `guard_two_shapes`, with an init event naming the tools.
fn lines_cases() -> Result<Vec<Case>, TestError> {
    let text = [
        r#"{"type":"system","subtype":"init","session_id":"cc-1","model":"m-1","tools":["Bash","Read"],"mcp_servers":[]}"#,
        r#"{"type":"assistant","session_id":"cc-1","text":"working"}"#,
        r#"{"type":"system","session_id":"cc-1/toolu-9","parent":"cc-1"}"#,
        r#"{"type":"result","session_id":"cc-1/toolu-9","is_error":true}"#,
        r#"{"type":"result","session_id":"cc-1","is_error":false}"#,
    ]
    .join("\n")
        + "\n";
    let init = InitReport::new(
        json!(["Bash", "Read"]),
        Some(json!([])),
        CensusSource::Harness,
    )?;
    Ok(vec![
        Case::new(
            String::from("lines_with_init"),
            ReaderSource::Live,
            name("stdout")?,
            text.into_bytes(),
        )
        .with_sessions(
            vec![
                ExpectedSession::new(sid("cc-1")?, None, EndedBy::Completed),
                ExpectedSession::new(
                    sid("cc-1/toolu-9")?,
                    Some(sid("cc-1")?),
                    EndedBy::agent_error(None),
                ),
            ],
            vec![sid("cc-1")?, sid("cc-1/toolu-9")?],
        )
        .with_init(init)
        .with_end(exited(0), EndedBy::Completed)
        .with_collect(vec![CollectSpec::file(String::from(
            "home/.claude/projects/*/cc-1.jsonl",
        ))]),
    ])
}

/// The Document-shaped cases: an export read whole, with no live reader and no init event.
fn document_cases() -> Result<Vec<Case>, TestError> {
    let failed = r#"{"format":"opencode-export","sessions":[
        {"id":"ses_01","model":"m-2","result":"error","text":"boom"},
        {"id":"ses_01/call_5","parent":"ses_01","result":"ok","text":"sub"}]}"#;
    let ok = r#"{"format":"opencode-export","sessions":[
        {"id":"ses_02","result":"ok","text":"done"}]}"#;
    let export = name("export")?;
    let export_command = |id: &str| -> Result<CollectSpec, TestError> {
        let spec = CommandSpec::builder()
            .argv(
                ["opencode", "session", "export", id]
                    .map(String::from)
                    .to_vec(),
            )
            .build()?;
        Ok(CollectSpec::command(spec, name("export")?))
    };
    Ok(vec![
        Case::new(
            String::from("document_error_result"),
            ReaderSource::Offline(export.clone()),
            export.clone(),
            failed.as_bytes().to_vec(),
        )
        .with_sessions(
            vec![
                ExpectedSession::new(sid("ses_01")?, None, EndedBy::agent_error(None)),
                ExpectedSession::new(
                    sid("ses_01/call_5")?,
                    Some(sid("ses_01")?),
                    EndedBy::Completed,
                ),
            ],
            vec![sid("ses_01")?, sid("ses_01/call_5")?],
        )
        .with_end(exited(0), EndedBy::agent_error(None))
        .with_collect(vec![export_command("ses_01")?]),
        Case::new(
            String::from("document_ok_result"),
            ReaderSource::Offline(export.clone()),
            export,
            ok.as_bytes().to_vec(),
        )
        .with_sessions(
            vec![ExpectedSession::new(
                sid("ses_02")?,
                None,
                EndedBy::Completed,
            )],
            vec![sid("ses_02")?],
        )
        .with_end(exited(0), EndedBy::Completed),
    ])
}

/// The census record: one JSON line per model request, with the names of the tools it carried.
/// The twelve names are the built-in inventory `opencode_facts` read from a request.
fn census_record() -> Vec<u8> {
    let tools = json!([
        "edit",
        "glob",
        "grep",
        "question",
        "read",
        "shell",
        "skill",
        "subagent",
        "webfetch",
        "websearch",
        "write",
        "execute"
    ]);
    let mut line = json!({"tools": tools}).to_string();
    line.push('\n');
    line.into_bytes()
}

/// The cases of the third fake: the recorded `OpenCode` streams and exports, read with the facts
/// each case's `expected.toml` and `exit.toml` state, and the census records.
fn open_cases() -> Result<Vec<Case>, TestError> {
    let ses = |text: &str| sid(text);
    let stdout = name("stdout")?;
    let export = name("export")?;
    let record = name("model_requests")?;
    let tools = InitReport::new(
        json!([
            "edit",
            "glob",
            "grep",
            "question",
            "read",
            "shell",
            "skill",
            "subagent",
            "webfetch",
            "websearch",
            "write",
            "execute"
        ]),
        None,
        CensusSource::Gateway,
    )?;
    let exit_of = |case: &str| -> Result<ProcessExit, TestError> {
        Ok(exited(note(case, "exit.toml", "exit_status")?.parse()?))
    };
    let id_of = |case: &str| -> Result<SessionId, TestError> {
        ses(&note(case, "expected.toml", "session_id")?)
    };
    let outcome_of = |case: &str| -> Result<EndedBy, TestError> {
        class_of(&note(case, "expected.toml", "export_outcome")?)
    };

    // Exit 1 is three endings; the live stream's last error event tells them apart.
    let interrupted = {
        let exit = exit_of("interrupt_api")?;
        let kind = terminal_error_type("interrupt_api")?;
        assert_eq!(kind, "aborted");
        Case::new(
            String::from("exit1_interrupted_through_the_api"),
            ReaderSource::Live,
            stdout.clone(),
            fixture("interrupt_api", "stdout.jsonl")?,
        )
        .with_observed_ids(vec![id_of("interrupt_api")?])
        .with_observed_end(ObservedEnd::Aborted)
        .with_end(exit, outcome_of("interrupt_api")?)
    };
    let refused = {
        let kind = terminal_error_type("refused_key")?;
        Case::new(
            String::from("exit1_refused_key"),
            ReaderSource::Live,
            stdout.clone(),
            fixture("refused_key", "stdout.jsonl")?,
        )
        .with_observed_ids(vec![id_of("refused_key")?])
        .with_observed_end(ObservedEnd::Error { kind })
        .with_end(exit_of("refused_key")?, outcome_of("refused_key")?)
    };
    let completed = Case::new(
        String::from("exit0_completed"),
        ReaderSource::Live,
        stdout.clone(),
        fixture("completed_o1", "stdout.jsonl")?,
    )
    .with_observed_ids(vec![id_of("completed_o1")?])
    .with_observed_end(ObservedEnd::Finished)
    .with_end(exit_of("completed_o1")?, outcome_of("completed_o1")?);

    // The recorded exports are read whole, and the run is classified from them.
    let export_case = |case: &str| -> Result<Case, TestError> {
        let id = id_of(case)?;
        Ok(Case::new(
            format!("export_{case}"),
            ReaderSource::Offline(export.clone()),
            export.clone(),
            fixture(case, "export.json")?,
        )
        .with_sessions(
            vec![ExpectedSession::new(id.clone(), None, outcome_of(case)?)],
            vec![id],
        )
        .with_end(exit_of(case)?, outcome_of(case)?))
    };

    // The live stream ended finished and the export says interrupted: the export wins.
    let mixed = Case::new(
        String::from("export_wins_over_the_live_end"),
        ReaderSource::Live,
        stdout.clone(),
        fixture("completed_o1", "stdout.jsonl")?,
    )
    .with_observed_ids(vec![id_of("completed_o1")?])
    .with_observed_end(ObservedEnd::Finished)
    .with_export(
        export.clone(),
        fixture("interrupt_api", "export.json")?,
        vec![ExpectedSession::new(
            id_of("interrupt_api")?,
            None,
            outcome_of("interrupt_api")?,
        )],
    )
    .with_end(exit_of("interrupt_api")?, EndedBy::Interrupted);

    Ok(vec![
        interrupted,
        refused,
        Case::new(
            String::from("exit1_stream_cut_short"),
            ReaderSource::Live,
            stdout,
            br#"{"type":"step_start","sessionID":"ses_cut"}
"#
            .to_vec(),
        )
        .with_observed_ids(vec![ses("ses_cut")?])
        .with_end(exited(1), EndedBy::Unknown),
        completed,
        export_case("completed_o1")?,
        export_case("interrupt_api")?,
        export_case("refused_key")?,
        mixed,
        // The census has no init event: it is read from a collected record, and labelled so.
        Case::new(
            String::from("census_from_the_gateway_record"),
            ReaderSource::Offline(record.clone()),
            record.clone(),
            census_record(),
        )
        .detects(false)
        .with_init(tools.clone()),
        // OpenCode sends a tool-less title request per session; it may come first.
        Case::new(
            String::from("census_skips_a_tool_less_first_request"),
            ReaderSource::Offline(record.clone()),
            record.clone(),
            [
                String::from("{\"model\":\"title\"}\n"),
                String::from("{\"tools\":[]}\n"),
                String::from_utf8(census_record())?,
            ]
            .concat()
            .into_bytes(),
        )
        .detects(false)
        .with_init(tools),
        // A record with no request that carries a tool is unrecorded, not an empty inventory.
        Case::new(
            String::from("census_record_without_a_request"),
            ReaderSource::Offline(record.clone()),
            record.clone(),
            Vec::new(),
        )
        .detects(false),
        Case::new(
            String::from("census_record_of_tool_less_requests"),
            ReaderSource::Offline(record.clone()),
            record,
            b"{\"model\":\"title\"}\n{\"tools\":[]}\n".to_vec(),
        )
        .detects(false),
    ])
}

/// Guards: R01 §Harness drivers — a harness with line framing, a live reader and an init event
/// passes the shared suite: its sessions, its inventory labelled as the harness's own, its end and
/// its collect are what the recorded lines say. Oracle: hand-written literals from R01 §Ends and
/// the fake's recorded input.
#[test]
fn conformance_lines_shape() -> TestResult {
    run_suite(&LinesDriver, &lines_cases()?)?;
    Ok(())
}

/// Guards: R01 §Harness drivers — a harness with document framing, no live reader and no init
/// event passes the same suite: the export is fed whole, its sessions and the end class come from
/// it, and the census stays off. Oracle: hand-written literals from R01 §Ends and §`OpenCode` v2.
#[test]
fn conformance_document_shape() -> TestResult {
    run_suite(&DocumentDriver, &document_cases()?)?;
    Ok(())
}

/// Guards: R01 §`OpenCode` v2 — a Document-framed harness that takes provider configuration through
/// variable names, keeps state in per-run paths, has no init event (its census is a collected
/// gateway record), stops with an in-container call that needs the session id and exits 1 for three
/// endings passes the suite, and a record with no request leaves the census unrecorded. Oracle: the
/// streams recorded by `opencode_facts` and its report's tool list.
#[test]
fn conformance_opencode_facts_shape() -> TestResult {
    run_suite(&OpenShapedDriver(DocumentDriver), &open_cases()?)?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Negative tests: one thing broken in a working driver, which the suite must refuse.
// ---------------------------------------------------------------------------------------------

/// A working driver with one thing changed.
struct Patched {
    inner: OpenShapedDriver,
    capabilities: Option<Capabilities>,
    stop: Option<StopSpec>,
    extra_arg: Option<String>,
    census_stream_removed: bool,
    eager_census: bool,
    session: Option<SessionSpec>,
    extra_file: Option<PreparedFile>,
    host_collect: bool,
    state_override: Option<Vec<StatePath>>,
    gateway_override: Option<GatewayEnv>,
    settings_override: Option<Vec<SettingRow>>,
}

impl Patched {
    fn new() -> Self {
        Self {
            inner: OpenShapedDriver(DocumentDriver),
            capabilities: None,
            stop: None,
            extra_arg: None,
            census_stream_removed: false,
            eager_census: false,
            session: None,
            extra_file: None,
            host_collect: false,
            state_override: None,
            gateway_override: None,
            settings_override: None,
        }
    }
}

/// A reader of the census record that reports an empty inventory when it saw no request.
struct EagerCensus;

impl TraceReader for EagerCensus {
    fn id(&self) -> candler_schemas::primitives::NameVersion {
        "eager-census@0.1.0"
            .parse()
            .unwrap_or_else(|_| unreachable!())
    }
    fn framing(&self) -> candler_harness::spec::Framing {
        candler_harness::spec::Framing::Lines
    }
    fn detect(
        &self,
        _sample: &[u8],
    ) -> Option<(
        candler_harness::spec::FormatVersion,
        candler_harness::spec::Confidence,
    )> {
        None
    }
    fn feed(
        &mut self,
        _frame: candler_harness::reader::FrameIn<'_>,
    ) -> Result<Vec<candler_schemas::trace::SessionEvent>, candler_harness::reader::ReaderError>
    {
        Ok(Vec::new())
    }
    fn observed(&self) -> Observed {
        // A phantom inventory, whatever the record holds.
        let init = InitReport::new(json!(["shell"]), None, CensusSource::Gateway)
            .unwrap_or_else(|_| unreachable!());
        Observed::new(Vec::new()).with_init(init)
    }
    fn finish(self: Box<Self>) -> Result<Vec<Session>, candler_harness::reader::ReaderError> {
        Err(candler_harness::reader::ReaderError::Incomplete {
            detail: String::from("no session"),
        })
    }
}

impl HarnessDriver for Patched {
    fn id(&self) -> candler_harness::spec::HarnessId {
        self.inner.id()
    }
    fn supported(&self) -> semver::VersionReq {
        self.inner.supported()
    }
    fn version_probe(&self) -> CommandSpec {
        self.inner.version_probe()
    }
    fn parse_version(&self, output: &[u8]) -> Result<semver::Version, DriverError> {
        self.inner.parse_version(output)
    }
    fn capabilities(&self) -> Capabilities {
        self.capabilities
            .unwrap_or_else(|| self.inner.capabilities())
    }
    fn gateway(&self) -> Option<GatewayEnv> {
        self.gateway_override
            .clone() // BORROW: .clone() copies the override
            .or_else(|| self.inner.gateway())
    }
    fn settings_table(&self) -> Vec<SettingRow> {
        self.settings_override
            .clone() // BORROW: .clone() copies the override
            .unwrap_or_else(|| self.inner.settings_table())
    }
    fn state_paths(&self) -> Vec<StatePath> {
        self.state_override
            .clone() // BORROW: .clone() copies the override
            .unwrap_or_else(|| self.inner.state_paths())
    }
    fn census_stream(&self) -> Option<StreamName> {
        if self.census_stream_removed {
            None
        } else {
            self.inner.census_stream()
        }
    }
    fn session(
        &self,
        _cell: &Cell<'_>,
        _workspace: &Workspace<'_>,
    ) -> Result<Option<SessionSpec>, DriverError> {
        Ok(self.session.clone())
    }
    fn session_tracker(&self) -> Option<Box<dyn SessionTracker>> {
        None
    }
    fn prepare(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Prepared, DriverError> {
        let mut prepared = self.inner.prepare(cell, workspace)?;
        if let Some(file) = &self.extra_file {
            prepared.files.push(file.clone()); // BORROW: .clone() copies the file
        }
        Ok(prepared)
    }
    fn command(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<CommandSpec, DriverError> {
        let command = self.inner.command(cell, workspace)?;
        let Some(extra) = &self.extra_arg else {
            return Ok(command);
        };
        let mut argv = command.argv().to_vec();
        argv.push(extra.clone()); // BORROW: .clone() copies the argument
        Ok(CommandSpec::builder().argv(argv).build()?)
    }
    fn stop(&self) -> StopSpec {
        self.stop.clone().unwrap_or_else(|| self.inner.stop())
    }
    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        self.inner.live_reader()
    }
    fn offline_reader(&self, stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        // BORROW: explicit .as_str() instead of Deref coercion
        if self.eager_census && stream.as_str() == "model_requests" {
            return Some(Box::new(EagerCensus)); // TRAIT_OBJECT: the trait returns readers as objects
        }
        self.inner.offline_reader(stream)
    }
    fn classify_exit(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy {
        self.inner.classify_exit(exit, sessions)
    }
    fn classify_end(
        &self,
        exit: &ProcessExit,
        sessions: &[Session],
        observed: &Observed,
    ) -> EndedBy {
        self.inner.classify_end(exit, sessions, observed)
    }
    fn collect(&self, workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec> {
        let collected = self.inner.collect(workspace, observed);
        if !self.host_collect {
            return collected;
        }
        // The same commands, run on the host.
        collected
            .into_iter()
            .map(|collected| {
                if let CollectSpec::Exec { spec, name, .. } = &collected {
                    // BORROW: .clone() copies the command and the stream name
                    CollectSpec::command(spec.clone(), name.clone())
                } else {
                    collected
                }
            })
            .collect()
    }
}

/// The failure the suite gives a broken driver, or an error when it passes.
fn refusal(driver: &Patched, cases: &[Case]) -> Result<String, TestError> {
    match run_suite(driver, cases) {
        Err(failure) => Ok(failure.to_string()),
        Ok(()) => Err("the suite passed a broken driver".into()),
    }
}

/// Guards: R01 §Harness drivers, §Agent environment and secrets, §`OpenCode` v2, §Session runner —
/// the checks that refuse a broken driver, each its own section below: a session capability
/// without a session spec; a census capability with neither a census stream nor an init report;
/// a census record with no request read as an empty inventory (unrecorded, never an empty set);
/// execution outside the child with no pre-stop; a secret in the argument vector, or a variable
/// the core owns; a driver that names no gateway variables; a config naming an undeclared
/// variable; a host collect for a driver with per-run state; two state paths that are one path,
/// or a file that is another's parent; a census stream with no reader; a stop script that matches
/// itself (`pgrep`/`pkill` with the full-command-line option, in any cluster); `Action::resolve`
/// falling back for an unknown or refused id; an inventory from a source that saw no tool; two state
/// paths sharing a variable or named like a gateway variable or not a variable at all; gateway names
/// that are no variable, one variable for two uses, unreserved or listed twice; a settings table
/// with a row twice, a refusal with no reason or a mapping with no construct.
/// Removing any one check from `candler-harness/src/conformance.rs` turns the matching section
/// below red alone.
/// Oracle: the capability and census rules of the leaf, the hand-written canary and the driver's
/// own reserved names, `opencode_facts` F6 and row 6, the `pgrep` manual's options, hand-written
/// ids and documents — each cited again at its section.
#[test]
fn conformance_refuses_a_broken_driver() -> TestResult {
    use candler_harness::spec::InitReportError;

    // A session capability declared without a session spec is refused, and the same driver
    // without the capability passes.
    let cases = open_cases()?;
    assert!(run_suite(&Patched::new(), &cases).is_ok());
    let mut broken = Patched::new();
    broken.capabilities = Some(
        Capabilities::builder()
            .graceful_stop(true)
            .live_events(true)
            .execution_outside_child(true)
            .census(true)
            .session(true)
            .build(),
    );
    let message = refusal(&broken, &cases)?;
    assert!(
        message.contains("session capability is declared and the driver returns no session spec"),
        "{message}"
    );

    // A census capability with neither a census stream nor an init report is refused.
    let mut broken = Patched::new();
    broken.census_stream_removed = true;
    // Only the cases that expect no inventory remain, so no init report backs the capability.
    let no_inventory_cases: Vec<Case> = open_cases()?
        .into_iter()
        .filter(|case| case.init.is_none() && case.name.starts_with("exit"))
        .collect();
    assert!(!no_inventory_cases.is_empty());
    let message = refusal(&broken, &no_inventory_cases)?;
    assert!(
        message.contains("neither a census stream nor an init report"),
        "{message}"
    );

    // A census record with no request that is read as an empty inventory is refused: nothing
    // seen is unrecorded, never an empty set.
    let census_cases: Vec<Case> = open_cases()?
        .into_iter()
        .filter(|case| case.name.starts_with("census_record_"))
        .collect();
    assert_eq!(census_cases.len(), 2);
    assert!(run_suite(&Patched::new(), &census_cases).is_ok());
    let mut broken = Patched::new();
    broken.eager_census = true;
    let message = refusal(&broken, &census_cases)?;
    assert!(
        message.contains("census_record_without_a_request"),
        "{message}"
    );

    // A harness whose execution lives outside the child must carry a pre-stop.
    let mut broken = Patched::new();
    broken.stop = Some(StopSpec::new(
        None,
        StopSignal::Term,
        Duration::from_secs(5),
    ));
    let message = refusal(&broken, &cases)?;
    assert!(message.contains("no pre-stop"), "{message}");

    // A command that carries the token in its argument vector, or one that sets a variable the
    // core owns, is refused.
    let mut leaky = Patched::new();
    leaky.extra_arg = Some(format!("--api-key={CANARY}"));
    let message = refusal(&leaky, &cases)?;
    assert!(
        message.contains("secret value is in the argument vector"),
        "{message}"
    );
    let owned = CommandSpec::builder()
        .argv(vec![String::from("sh")])
        .env(EnvEntry::literal(
            String::from("OC_BASE_URL"),
            String::from("http://elsewhere"),
        ))
        .build()?;
    let mut setting = Patched::new();
    setting.stop = Some(StopSpec::new(
        Some(Action::exec(owned)),
        StopSignal::Term,
        Duration::from_secs(5),
    ));
    let message = refusal(&setting, &cases)?;
    assert!(message.contains("which the core owns"), "{message}");

    // A driver that does not name its gateway variables gets no set of names from the trait: the
    // suite refuses it, and the same driver with names passes.
    let lines_cases = lines_cases()?;
    assert!(run_suite(&LinesDriver, &lines_cases).is_ok());
    let Err(failure) = run_suite(&Silent(LinesDriver), &lines_cases) else {
        return Err("the suite passed a driver with no gateway names".into());
    };
    assert!(
        failure.to_string().contains("names no gateway variables"),
        "{failure}"
    );

    // A written configuration may name only variables the driver declared: a file that refers to
    // an undeclared variable is refused, and the driver's own config passes.
    assert!(run_suite(&Patched::new(), &cases).is_ok());
    let mut broken = Patched::new();
    broken.extra_file = Some(PreparedFile::in_home(
        "extra.json".parse()?,
        br#"{"apiKey":"{env:OPENAI_API_KEY}"}"#.to_vec(),
        false,
    ));
    let message = refusal(&broken, &cases)?;
    assert!(
        message.contains("{env:OPENAI_API_KEY}, which the driver does not declare"),
        "{message}"
    );

    // A harness that keeps per-run state collects through a command inside the isolation; a host
    // command is refused.
    let mut broken = Patched::new();
    broken.host_collect = true;
    let message = refusal(&broken, &cases)?;
    assert!(message.contains("host command"), "{message}");

    // Two state paths that are one path, or a file that is another path's parent, are refused; a
    // file inside a directory the driver also lists, and sibling paths, pass.
    let path = |env: &str, kind: StateKind, text: &str| -> Result<StatePath, TestError> {
        Ok(StatePath::new(String::from(env), kind, text.parse()?))
    };
    let with = |paths: Vec<StatePath>| {
        let mut driver = Patched::new();
        driver.state_override = Some(paths);
        driver
    };
    let same = with(vec![
        path("A_DIR", StateKind::Dir, ".xdg/data")?,
        path("B_DIR", StateKind::Dir, ".xdg/data")?,
    ]);
    assert!(refusal(&same, &cases)?.contains("one path, or a file is the other's parent"));
    let parent = with(vec![
        path("OPENCODE_DB", StateKind::File, ".xdg/db")?,
        path("XDG_DATA_HOME", StateKind::Dir, ".xdg/db/data")?,
    ]);
    assert!(refusal(&parent, &cases)?.contains("one path, or a file is the other's parent"));
    // The same pair in the other order.
    let reversed = with(vec![
        path("XDG_DATA_HOME", StateKind::Dir, ".xdg/db/data")?,
        path("OPENCODE_DB", StateKind::File, ".xdg/db")?,
    ]);
    assert!(refusal(&reversed, &cases)?.contains("one path, or a file is the other's parent"));
    let file_in_dir = with(vec![
        path("XDG_DATA_HOME", StateKind::Dir, ".xdg/data")?,
        path("OPENCODE_DB", StateKind::File, ".xdg/data/oc.db")?,
    ]);
    assert!(run_suite(&file_in_dir, &cases).is_ok());
    let siblings = with(vec![
        path("A_DIR", StateKind::Dir, ".xdg/data")?,
        path("B_DIR", StateKind::Dir, ".xdg/database")?,
    ]);
    assert!(run_suite(&siblings, &cases).is_ok());

    // Two state paths that share one variable, a variable that is also a gateway name, and a name
    // that is no variable are refused.
    let shared = with(vec![
        path("A_DIR", StateKind::Dir, ".xdg/data")?,
        path("A_DIR", StateKind::Dir, ".xdg/other")?,
    ]);
    assert!(refusal(&shared, &cases)?.contains("two state paths share a variable"));
    let gateway_named = with(vec![path("OC_BASE_URL", StateKind::Dir, ".xdg/data")?]);
    assert!(refusal(&gateway_named, &cases)?.contains("OC_BASE_URL is also a gateway name"));
    let not_a_name = with(vec![path("not-a-var", StateKind::Dir, ".xdg/data")?]);
    assert!(refusal(&not_a_name, &cases)?.contains("is not a variable name"));

    // The gateway names: a name that is no variable, one variable for the address and the token,
    // a name the core sets but the driver does not reserve, and a reserved name listed twice.
    let gateway = |base: &str, token: &str, reserved: &[&str]| {
        let mut driver = Patched::new();
        driver.gateway_override = Some(GatewayEnv::new(
            String::from(base),
            String::from(token),
            reserved.iter().map(|name| String::from(*name)).collect(),
        ));
        driver
    };
    assert!(
        run_suite(
            &gateway("OC_BASE_URL", "OC_API_KEY", &["OC_BASE_URL", "OC_API_KEY"]),
            &cases
        )
        .is_ok()
    );
    let bad_name = gateway(
        "OC_BASE_URL",
        "OC_API_KEY",
        &["OC_BASE_URL", "OC_API_KEY", "a b"],
    );
    assert!(refusal(&bad_name, &cases)?.contains("is not a variable name"));
    let one_variable = gateway("OC_BASE_URL", "OC_BASE_URL", &["OC_BASE_URL"]);
    assert!(refusal(&one_variable, &cases)?.contains("share one variable"));
    let unreserved = gateway("OC_BASE_URL", "OC_API_KEY", &["OC_BASE_URL"]);
    assert!(
        refusal(&unreserved, &cases)?.contains("OC_API_KEY is set by the core but not reserved")
    );
    let twice = gateway(
        "OC_BASE_URL",
        "OC_API_KEY",
        &["OC_BASE_URL", "OC_API_KEY", "OC_API_KEY"],
    );
    assert!(refusal(&twice, &cases)?.contains("a reserved name is listed twice"));

    // A settings table that names a setting and value twice, a refused row that gives no reason
    // and a mapped row that names no construct are refused; a sound table passes.
    let table = |rows: Vec<SettingRow>| {
        let mut driver = Patched::new();
        driver.settings_override = Some(rows);
        driver
    };
    let mapped = SettingRow::mapped("permission_mode", "bypassPermissions", "`--auto`", "all");
    let refused = SettingRow::refused("permission_mode", "plan", "reads and read-only shell");
    assert!(run_suite(&table(vec![mapped.clone(), refused.clone()]), &cases).is_ok()); // BORROW: .clone() copies the row
    let duplicate = table(vec![mapped.clone(), mapped.clone()]); // BORROW: .clone() copies the row
    assert!(
        refusal(&duplicate, &cases)?.contains("two rows name permission_mode bypassPermissions")
    );
    let no_reason = table(vec![SettingRow::refused("permission_mode", "plan", "")]);
    assert!(refusal(&no_reason, &cases)?.contains("gives no reason"));
    let no_construct = table(vec![SettingRow::mapped(
        "permission_mode",
        "plan",
        "",
        "all",
    )]);
    assert!(refusal(&no_construct, &cases)?.contains("names no construct"));

    // An in-container script that finds a process with the full-command-line option of `pgrep`
    // or `pkill` is refused, in any option cluster; exact-name and `/proc` forms pass.
    let driver_with = |script: &str| -> Result<Patched, TestError> {
        let command = CommandSpec::builder()
            .argv(vec![
                String::from("sh"),
                String::from("-c"),
                format!("{script}; echo {{session_id}}"),
            ])
            .build()?;
        let mut driver = Patched::new();
        driver.stop = Some(StopSpec::new(
            Some(Action::exec(command)),
            StopSignal::Term,
            Duration::from_secs(5),
        ));
        Ok(driver)
    };
    for script in [
        "pid=$(pgrep -f 'opencode serve')",
        "pid=$(pgrep -af opencode)",
        "pid=$(pgrep -fl opencode)",
        "pkill -9 -f opencode",
        "pid=`pgrep -9f opencode`",
        "pgrep --full opencode",
        "echo a && pgrep -n -f opencode",
    ] {
        let message = refusal(&driver_with(script)?, &cases)?;
        assert!(message.contains("pgrep -f"), "{script}: {message}");
    }
    for script in [
        "pid=$(pgrep -x opencode)",
        "pgrep -l opencode",
        "pkill -9 -x opencode",
        "echo -f pgrep",
    ] {
        assert!(run_suite(&driver_with(script)?, &cases).is_ok(), "{script}");
    }

    // `Action::resolve` gives the action to run now: itself when it needs no id, the filled
    // action for a plain id, and `None` for an unknown or refused id.
    let needs = Action::exec(
        CommandSpec::builder()
            .argv(vec![String::from("x"), String::from("{session_id}")])
            .build()?,
    );
    let plain = Action::exec(
        CommandSpec::builder()
            .argv(vec![String::from("x")])
            .build()?,
    );
    let id = sid("ses_ok")?;
    let filled = needs.resolve(Some(&id));
    assert!(filled.as_ref().is_some_and(|a| !a.uses_session_id()));
    assert_eq!(needs.resolve(None), None);
    assert_eq!(needs.resolve(Some(&sid("a;b")?)), None);
    assert_eq!(needs.resolve(Some(&sid("p/child")?)), None);
    assert_eq!(plain.resolve(None), Some(plain.clone()));

    // An inventory from a source that saw no tool is refused (it is unrecorded), the harness's
    // own empty init list is a recorded one, and the three sources never compare equal.
    for source in [CensusSource::Gateway, CensusSource::Collected] {
        assert_eq!(
            InitReport::new(json!([]), None, source),
            Err(InitReportError::NoTools)
        );
    }
    assert!(InitReport::new(json!([]), Some(json!([])), CensusSource::Harness).is_ok());
    for source in [
        CensusSource::Harness,
        CensusSource::Gateway,
        CensusSource::Collected,
    ] {
        assert_eq!(
            InitReport::new(json!(["read", ""]), None, source),
            Err(InitReportError::EmptyToolName)
        );
    }
    let tools = || json!(["read"]);
    let by = |source| InitReport::new(tools(), None, source);
    assert_ne!(by(CensusSource::Harness)?, by(CensusSource::Gateway)?);
    assert_ne!(by(CensusSource::Gateway)?, by(CensusSource::Collected)?);
    assert_ne!(by(CensusSource::Harness)?, by(CensusSource::Collected)?);
    Ok(())
}

/// A working driver that leaves `gateway` to the trait's default.
struct Silent(LinesDriver);

impl HarnessDriver for Silent {
    fn id(&self) -> candler_harness::spec::HarnessId {
        self.0.id()
    }
    fn supported(&self) -> semver::VersionReq {
        self.0.supported()
    }
    fn version_probe(&self) -> CommandSpec {
        self.0.version_probe()
    }
    fn parse_version(&self, output: &[u8]) -> Result<semver::Version, DriverError> {
        self.0.parse_version(output)
    }
    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }
    fn prepare(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Prepared, DriverError> {
        self.0.prepare(cell, workspace)
    }
    fn command(
        &self,
        cell: &Cell<'_>,
        workspace: &Workspace<'_>,
    ) -> Result<CommandSpec, DriverError> {
        self.0.command(cell, workspace)
    }
    fn stop(&self) -> StopSpec {
        self.0.stop()
    }
    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        self.0.live_reader()
    }
    fn offline_reader(&self, stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        self.0.offline_reader(stream)
    }
    fn classify_exit(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy {
        self.0.classify_exit(exit, sessions)
    }
    fn collect(&self, workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec> {
        self.0.collect(workspace, observed)
    }
}

/// Guards: R01 §Session runner — a session spec whose close action is checked like a stop (a
/// placeholder resolves) and a tracker that reports turns and pending items passes; the suite
/// feeds records to the tracker and compares the state. Oracle: hand-written records and states.
#[test]
fn conformance_session_surface_on_a_fake() -> TestResult {
    struct Fake {
        turns: u64,
        pending: Vec<PendingItem>,
        ended: bool,
    }
    impl SessionTracker for Fake {
        fn feed(
            &mut self,
            record: candler_harness::spec::SessionRecord<'_>,
        ) -> Result<(), DriverError> {
            match record {
                candler_harness::spec::SessionRecord::Stream(line) => {
                    if line == b"result" {
                        self.turns += 1;
                        self.ended = true;
                    } else {
                        self.ended = false;
                    }
                }
                candler_harness::spec::SessionRecord::Hook(line) => {
                    self.pending = if line == b"one task" {
                        vec![PendingItem::new(PendingKind::Task, String::from("t1"))]
                    } else {
                        Vec::new()
                    };
                }
                _ => {
                    return Err(DriverError::Record {
                        detail: String::from("unknown record"),
                    });
                }
            }
            Ok(())
        }
        fn state(&self) -> PendingState {
            // BORROW: .clone() copies the pending list out of the tracker
            PendingState::new(self.turns, self.ended, true, self.pending.clone())
        }
    }
    struct Sessioned(Patched);
    impl HarnessDriver for Sessioned {
        fn id(&self) -> candler_harness::spec::HarnessId {
            self.0.id()
        }
        fn supported(&self) -> semver::VersionReq {
            self.0.supported()
        }
        fn version_probe(&self) -> CommandSpec {
            self.0.version_probe()
        }
        fn parse_version(&self, output: &[u8]) -> Result<semver::Version, DriverError> {
            self.0.parse_version(output)
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities::builder()
                .graceful_stop(true)
                .live_events(true)
                .execution_outside_child(true)
                .census(true)
                .session(true)
                .build()
        }
        fn gateway(&self) -> Option<GatewayEnv> {
            self.0.gateway()
        }
        fn state_paths(&self) -> Vec<StatePath> {
            self.0.state_paths()
        }
        fn census_stream(&self) -> Option<StreamName> {
            self.0.census_stream()
        }
        fn session(
            &self,
            cell: &Cell<'_>,
            workspace: &Workspace<'_>,
        ) -> Result<Option<SessionSpec>, DriverError> {
            let command = self.0.command(cell, workspace)?;
            let close = CommandSpec::builder()
                .argv(vec![
                    String::from("opencode"),
                    String::from("api"),
                    String::from("session.interrupt"),
                    String::from("sessionID={session_id}"),
                ])
                .build()?;
            Ok(Some(SessionSpec::new(
                command,
                b"fix it\n".to_vec(),
                SessionClose::Action(Action::exec(close)),
            )))
        }
        fn session_tracker(&self) -> Option<Box<dyn SessionTracker>> {
            Some(Box::new(Fake {
                turns: 0,
                pending: Vec::new(),
                ended: false,
            })) // TRAIT_OBJECT: the trait returns trackers as objects
        }
        fn prepare(
            &self,
            cell: &Cell<'_>,
            workspace: &Workspace<'_>,
        ) -> Result<Prepared, DriverError> {
            self.0.prepare(cell, workspace)
        }
        fn command(
            &self,
            cell: &Cell<'_>,
            workspace: &Workspace<'_>,
        ) -> Result<CommandSpec, DriverError> {
            self.0.command(cell, workspace)
        }
        fn stop(&self) -> StopSpec {
            self.0.stop()
        }
        fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
            self.0.live_reader()
        }
        fn offline_reader(&self, stream: &StreamName) -> Option<Box<dyn TraceReader>> {
            self.0.offline_reader(stream)
        }
        fn classify_exit(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy {
            self.0.classify_exit(exit, sessions)
        }
        fn classify_end(
            &self,
            exit: &ProcessExit,
            sessions: &[Session],
            observed: &Observed,
        ) -> EndedBy {
            self.0.classify_end(exit, sessions, observed)
        }
        fn collect(&self, workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec> {
            self.0.collect(workspace, observed)
        }
    }
    let record = |bytes: &[u8]| RecordBytes::Stream(bytes.to_vec());
    let hook = |bytes: &[u8]| RecordBytes::Hook(bytes.to_vec());
    let task = vec![PendingItem::new(PendingKind::Task, String::from("t1"))];
    let track = vec![
        TrackStep::new(
            record(b"init"),
            Some(PendingState::new(0, false, true, vec![])),
        ),
        TrackStep::new(hook(b"one task"), None),
        TrackStep::new(
            record(b"result"),
            Some(PendingState::new(1, true, true, task.clone())),
        ),
        TrackStep::new(
            record(b"init"),
            Some(PendingState::new(1, false, true, task)),
        ),
    ];
    let mut cases = open_cases()?;
    cases.push(
        Case::new(
            String::from("session_tracker"),
            ReaderSource::Live,
            name("stdout")?,
            br#"{"type":"step_start","sessionID":"ses_t"}
"#
            .to_vec(),
        )
        .with_observed_ids(vec![sid("ses_t")?])
        .with_track(track),
    );
    run_suite(&Sessioned(Patched::new()), &cases)?;

    // A close action that holds a placeholder is resolved and an unsafe id is refused.
    let closing = Action::exec(
        CommandSpec::builder()
            .argv(vec![String::from("x"), String::from("{session_id}")])
            .build()?,
    );
    assert!(closing.uses_session_id());
    let filled = closing.with_session_id(&sid("ses_ok")?)?;
    assert!(!filled.uses_session_id());
    assert!(closing.with_session_id(&sid("a;b")?).is_err());
    Ok(())
}
