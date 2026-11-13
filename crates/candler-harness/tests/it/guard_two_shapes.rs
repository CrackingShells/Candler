//! Two fake harnesses of opposite shapes drive both traits.
//!
//! The Claude Code-shaped fake streams one JSON object per line, has a live reader, stops on a
//! signal alone and collects a transcript file. The OpenCode-shaped fake reads a whole exported
//! document, has no live reader, runs its sessions outside the child, stops with an HTTP call
//! that carries a body and collects through an export command that names the observed session.
//! Every expected session, id, event and end is written here as a literal from the architecture
//! document (§Harness drivers, §Ends, §`OpenCode` v2), never taken from a fake's own output.
#![cfg(test)]

use crate::support;

use std::path::Path;
use std::time::Duration;

use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::FrameIn;
use candler_harness::spec::{
    Action, Cell, CollectSpec, CommandSpec, CommandSpecError, Confidence, EnvEntry, FormatVersion,
    Framing, HttpMethod, Observed, ProcessEnd, ProcessExit, StopSignal, StopSpec, Workspace,
};
use candler_schemas::primitives::FieldPath;
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::{FrameData, RawFrame, RawRef};
use candler_schemas::trace::{Cap, EndedBy, Event, EventKind, Session, SessionEvent};
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::Value;

use support::{
    DocumentDriver, LinesDriver, TestError, TestResult, read_export, sid, source, stream, ts,
};

/// Guards: Cond 4 — a harness with line framing, a live reader, a signal-only stop and a file
/// collect runs through every driver and reader method and yields the sessions, ids, events and
/// ends R01 states for it, with the root first, the child linked to it, and an exit 0 read from
/// the root session. Oracle: hand-written literals from R01 §Harness drivers and §Ends.
#[test]
fn lines_shaped_harness_drives_both_traits() -> TestResult {
    let driver: Box<dyn HarnessDriver> = Box::new(LinesDriver); // TRAIT_OBJECT: the core holds drivers as objects
    assert_eq!(driver.id().to_string(), "claude-code");
    assert!(driver.capabilities().live_events);
    assert!(!driver.capabilities().execution_outside_child);
    assert_eq!(driver.stop().pre_stop, None);
    assert_eq!(driver.stop().signal, StopSignal::Term);

    let Some(mut reader) = driver.live_reader() else {
        return Err("the Claude Code-shaped driver has a live reader".into());
    };
    assert_eq!(reader.framing(), Framing::Lines);
    assert_eq!(reader.id().to_string(), "fake-lines-reader@0.1.0");
    assert_eq!(
        reader.detect(br#"{"type":"system","subtype":"init","session_id":"cc-1"}"#),
        Some((
            FormatVersion::new(semver::Version::new(1, 0, 0)),
            Confidence::Certain
        )),
    );
    assert_eq!(reader.detect(br#"{"format":"opencode-export"}"#), None);

    // One frame per line: the time the core gives each frame, and the line.
    let input: [(&str, &str); 5] = [
        (
            "2026-11-05T10:00:00Z",
            r#"{"type":"system","session_id":"cc-1","model":"m-1"}"#,
        ),
        (
            "2026-11-05T10:00:01Z",
            r#"{"type":"assistant","session_id":"cc-1","text":"working"}"#,
        ),
        (
            "2026-11-05T10:00:02Z",
            r#"{"type":"system","session_id":"cc-1/toolu-9","parent":"cc-1"}"#,
        ),
        (
            "2026-11-05T10:00:03Z",
            r#"{"type":"result","session_id":"cc-1/toolu-9","is_error":true}"#,
        ),
        (
            "2026-11-05T10:00:04Z",
            r#"{"type":"result","session_id":"cc-1","is_error":false}"#,
        ),
    ];
    let mut events = Vec::new();
    for (index, (at, line)) in (0_u64..).zip(input) {
        let raw_ref = RawRef::new(stream("stdout")?, index);
        let frame = RawFrame::new(None, FrameData::from_bytes(format!("{line}\n").as_bytes()));
        events.extend(reader.feed(FrameIn::new(raw_ref, Some(ts(at)?), &frame))?);
    }

    let event = |session: &str, seq: u64, kind: EventKind, at: &str, frame: u64| {
        let raw_ref = RawRef::new(stream("stdout")?, frame);
        Ok::<_, TestError>(SessionEvent::new(
            sid(session)?,
            Event::new(seq, kind, raw_ref).with_at(ts(at)?),
        ))
    };
    let expected_events = vec![
        event(
            "cc-1",
            0,
            EventKind::notification(),
            "2026-11-05T10:00:00Z",
            0,
        )?,
        event(
            "cc-1",
            1,
            EventKind::assistant_message(Some(String::from("working"))),
            "2026-11-05T10:00:01Z",
            1,
        )?,
        event(
            "cc-1/toolu-9",
            0,
            EventKind::notification(),
            "2026-11-05T10:00:02Z",
            2,
        )?,
        event(
            "cc-1/toolu-9",
            1,
            EventKind::notification(),
            "2026-11-05T10:00:03Z",
            3,
        )?,
        event(
            "cc-1",
            2,
            EventKind::notification(),
            "2026-11-05T10:00:04Z",
            4,
        )?,
    ];
    assert_eq!(events, expected_events);

    let observed = reader.observed();
    assert_eq!(
        observed,
        Observed::new(vec![sid("cc-1")?, sid("cc-1/toolu-9")?])
    );

    let sessions = reader.finish()?;
    let root_source = source("claude-code", "2.1.304", "fake-lines-reader", "0.1.0")?;
    let expected_sessions = vec![
        Session::builder()
            .id(sid("cc-1")?)
            // BORROW: .clone() gives the second session its own source
            .source(root_source.clone())
            .model(String::from("m-1"))
            .started_at(ts("2026-11-05T10:00:00Z")?)
            .ended_at(ts("2026-11-05T10:00:04Z")?)
            .ended_by(EndedBy::Completed)
            .raw_ref(RawRef::new(stream("stdout")?, 0))
            .build()?,
        Session::builder()
            .id(sid("cc-1/toolu-9")?)
            .source(root_source)
            .started_at(ts("2026-11-05T10:00:02Z")?)
            .ended_at(ts("2026-11-05T10:00:03Z")?)
            .ended_by(EndedBy::agent_error(None))
            .parent_session(sid("cc-1")?)
            .raw_ref(RawRef::new(stream("stdout")?, 2))
            .build()?,
    ];
    assert_eq!(sessions, expected_sessions);

    // The run ended by its root session, not by the child's error.
    let exited = ProcessExit::new(ExitStatus::Code { code: 0 }, ProcessEnd::Exited, Vec::new());
    assert_eq!(driver.classify_exit(&exited, &sessions), EndedBy::Completed);
    let deadline = ProcessExit::new(
        ExitStatus::Signal { signal: 15 },
        ProcessEnd::Deadline,
        Vec::new(),
    );
    assert_eq!(
        driver.classify_exit(&deadline, &sessions),
        EndedBy::cap(Cap::Wall)
    );

    let workspace = Workspace::new(Path::new("."));
    assert_eq!(
        driver.collect(&workspace, &observed),
        vec![CollectSpec::file(String::from(
            "home/.claude/projects/*/cc-1.jsonl"
        ))],
    );
    Ok(())
}

/// Guards: Cond 4 — a harness with document framing, no live reader, execution outside the child,
/// an HTTP pre-stop with a body and an export collect runs through every driver and reader method:
/// the export command carries the observed session id, and an error result in the sessions
/// classifies the run as an agent error although the exit status is 0, while the same exit with an
/// ok result is completed. Oracle: hand-written literals from R01 §Ends and §`OpenCode` v2.
#[test]
fn document_shaped_harness_drives_both_traits() -> TestResult {
    let driver: Box<dyn HarnessDriver> = Box::new(DocumentDriver); // TRAIT_OBJECT: the core holds drivers as objects
    assert_eq!(driver.id().to_string(), "opencode");
    assert!(driver.live_reader().is_none());
    assert!(driver.offline_reader(&stream("stdout")?).is_none());
    assert!(driver.capabilities().execution_outside_child);
    assert!(!driver.capabilities().live_events);
    assert_eq!(
        driver.stop(),
        StopSpec::new(
            Some(Action::http(
                HttpMethod::Post,
                String::from("http://127.0.0.1:4096/session/abort"),
                Some(String::from(r#"{"reason":"candler-stop"}"#)),
            )),
            StopSignal::Term,
            Duration::from_secs(5),
        ),
    );

    // The driver forces `--standalone`; the prompt comes from the cell.
    let prompt_path: FieldPath = "task.prompt".parse()?;
    let values = [ResolvedValue::recorded(
        prompt_path,
        JsonValue::new(Value::String(String::from("fix it")))?,
    )];
    let workspace = Workspace::new(Path::new("."));
    let command = driver.command(&Cell::new(&values), &workspace)?;
    assert_eq!(
        command.argv(),
        ["opencode", "run", "--standalone", "fix it"]
    );
    assert!(matches!(
        driver.command(&Cell::new(&[]), &workspace),
        Err(DriverError::MissingValue { .. })
    ));

    let document = r#"{"format":"opencode-export","sessions":[
        {"id":"ses_01","model":"m-2","result":"error","text":"boom"},
        {"id":"ses_01/call_5","parent":"ses_01","result":"ok","text":"sub"}]}"#;
    let Some(reader) = driver.offline_reader(&stream("export")?) else {
        return Err("the OpenCode-shaped driver reads the export stream".into());
    };
    assert_eq!(reader.id().to_string(), "fake-document-reader@0.1.0");
    assert_eq!(
        reader.detect(document.as_bytes()),
        Some((
            FormatVersion::new(semver::Version::new(1, 0, 0)),
            Confidence::Certain
        )),
    );
    assert_eq!(
        reader.detect(br#"{"sessions":[]}"#),
        Some((
            FormatVersion::new(semver::Version::new(1, 0, 0)),
            Confidence::Likely
        )),
    );

    // Here the ids come from the offline reader over the export. The core takes the ids `collect`
    // receives from the live reader's `observed()`; an OpenCode driver returns a Lines reader over
    // stdout that only reports session ids, and this fake has none.
    let (events, observed, sessions) = read_export(driver.as_ref(), document)?;
    let export_ref = || -> Result<RawRef, TestError> { Ok(RawRef::new(stream("export")?, 0)) };
    assert_eq!(
        events,
        vec![
            SessionEvent::new(
                sid("ses_01")?,
                Event::new(
                    0,
                    EventKind::assistant_message(Some(String::from("boom"))),
                    export_ref()?
                ),
            ),
            SessionEvent::new(
                sid("ses_01/call_5")?,
                Event::new(
                    0,
                    EventKind::assistant_message(Some(String::from("sub"))),
                    export_ref()?
                ),
            ),
        ],
    );
    assert_eq!(
        observed,
        Observed::new(vec![sid("ses_01")?, sid("ses_01/call_5")?])
    );

    let opencode = source("opencode", "2.0.0", "fake-document-reader", "0.1.0")?;
    let expected_sessions = vec![
        Session::builder()
            .id(sid("ses_01")?)
            // BORROW: .clone() gives the second session its own source
            .source(opencode.clone())
            .model(String::from("m-2"))
            .ended_by(EndedBy::agent_error(None))
            .raw_ref(export_ref()?)
            .build()?,
        Session::builder()
            .id(sid("ses_01/call_5")?)
            .source(opencode)
            .ended_by(EndedBy::Completed)
            .parent_session(sid("ses_01")?)
            .raw_ref(export_ref()?)
            .build()?,
    ];
    assert_eq!(sessions, expected_sessions);

    // The export command carries the session id the reader observed.
    let collected = driver.collect(&workspace, &observed);
    let expected_export = CommandSpec::builder()
        .argv(
            ["opencode", "session", "export", "ses_01"]
                .map(String::from)
                .to_vec(),
        )
        .build()?;
    assert_eq!(
        collected,
        vec![CollectSpec::command(expected_export, stream("export")?)]
    );

    // Exit status 0, and an error result in the sessions: an agent error.
    let exit_zero = ProcessExit::new(ExitStatus::Code { code: 0 }, ProcessEnd::Exited, Vec::new());
    assert_eq!(
        driver.classify_exit(&exit_zero, &sessions),
        EndedBy::agent_error(None)
    );

    // Positive control: the same exit over a document whose root session is ok.
    let ok_document = r#"{"format":"opencode-export","sessions":[
        {"id":"ses_02","result":"ok","text":"done"}]}"#;
    let (_, _, ok_sessions) = read_export(driver.as_ref(), ok_document)?;
    assert_eq!(
        driver.classify_exit(&exit_zero, &ok_sessions),
        EndedBy::Completed
    );
    Ok(())
}

/// Guards: Cond 4 — a command with an empty argv or a variable set twice is refused, and the same
/// command with one argument and distinct variable names builds (positive control). Oracle: R01
/// §Harness drivers (`CommandSpec`: a non-empty argv and an overlay naming each variable once),
/// hand-written inputs.
#[test]
fn command_spec_refuses_empty_argv_and_duplicate_variables() -> TestResult {
    let env = |name: &str, value: &str| EnvEntry::literal(String::from(name), String::from(value));
    let argv = || vec![String::from("tool")];

    let valid = CommandSpec::builder()
        .argv(argv())
        .env(env("A", "1"))
        .env(env("B", "1"))
        .build()?;
    assert_eq!(valid.argv(), ["tool"]);
    assert_eq!(valid.env_overlay().len(), 2);

    assert_eq!(
        CommandSpec::builder()
            .argv(argv())
            .env(env("A", "1"))
            .env(env("B", "2"))
            .env(env("A", "3"))
            .build(),
        Err(CommandSpecError::DuplicateEnv {
            name: String::from("A")
        }),
    );
    assert_eq!(
        CommandSpec::builder().argv(Vec::new()).build(),
        Err(CommandSpecError::EmptyArgv),
    );
    assert_eq!(
        CommandSpec::builder().build(),
        Err(CommandSpecError::MissingField { field: "argv" }),
    );
    Ok(())
}
