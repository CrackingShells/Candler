//! Two fake harnesses of opposite shapes drive both traits.
//!
//! The Claude Code-shaped fake streams one JSON object per line, has a live reader, stops on a
//! signal alone and collects a transcript file. The OpenCode-shaped fake reads a whole exported
//! document, has no live reader, runs its sessions outside the child, stops with an HTTP call
//! that carries a body and collects through an export command that names the observed session.
//! Every expected session, id, event and end is written here as a literal from the architecture
//! document (§Harness drivers, §Ends, §`OpenCode` v2), never taken from a fake's own output.
#![cfg(test)]

use std::path::Path;
use std::time::Duration;

use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::reader::{FrameIn, ReaderError, TraceReader};
use candler_harness::spec::{
    Action, BudgetCaps, Capabilities, Cell, CollectSpec, CommandSpec, CommandSpecError, Confidence,
    EnvEntry, FormatVersion, Framing, HarnessId, HttpMethod, Observed, Prepared, ProcessEnd,
    ProcessExit, StopSignal, StopSpec, Workspace,
};
use candler_schemas::primitives::{FieldPath, Name, NameVersion, Timestamp};
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::{FrameData, RawFrame, RawRef, StreamName};
use candler_schemas::trace::{
    Cap, EndedBy, Event, EventKind, Session, SessionEvent, SessionId, SessionSource,
};
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::Value;

/// The error every test helper returns.
type TestError = Box<dyn std::error::Error>; // TRAIT_OBJECT: helpers return the error of whichever call failed

type TestResult = Result<(), TestError>;

fn sid(text: &str) -> Result<SessionId, TestError> {
    Ok(text.parse()?)
}

fn ts(text: &str) -> Result<Timestamp, TestError> {
    Ok(text.parse()?)
}

fn stream(text: &str) -> Result<StreamName, TestError> {
    Ok(text.parse()?)
}

fn source(
    harness: &str,
    harness_version: &str,
    reader: &str,
    reader_version: &str,
) -> Result<SessionSource, TestError> {
    Ok(SessionSource::new(
        harness.parse::<Name>()?,
        harness_version.parse()?,
        reader.parse::<Name>()?,
        reader_version.parse()?,
    ))
}

/// The text of a frame's data as JSON.
fn frame_json(frame: &RawFrame, index: u64) -> Result<Value, ReaderError> {
    serde_json::from_slice(&frame.data.to_bytes()).map_err(|e| ReaderError::Frame {
        frame: index,
        detail: e.to_string(),
    })
}

// ---------------------------------------------------------------------------------------------
// The Claude Code-shaped fake: lines, a live reader, a signal-only stop, a file collect.
// ---------------------------------------------------------------------------------------------

struct Pending {
    id: SessionId,
    parent: Option<SessionId>,
    model: Option<String>,
    started_at: Option<Timestamp>,
    ended_at: Option<Timestamp>,
    ended_by: EndedBy,
    raw_ref: RawRef,
    next_seq: u64,
}

struct LinesReader {
    pending: Vec<Pending>,
}

impl TraceReader for LinesReader {
    fn id(&self) -> NameVersion {
        "fake-lines-reader@0.1.0"
            .parse()
            .unwrap_or_else(|_| unreachable!())
    }

    fn framing(&self) -> Framing {
        Framing::Lines
    }

    fn detect(&self, sample: &[u8]) -> Option<(FormatVersion, Confidence)> {
        let text = String::from_utf8_lossy(sample);
        // BORROW: explicit .as_ref() on the lossy text
        if text.as_ref().contains(r#""subtype":"init""#) {
            Some((
                FormatVersion::new(semver::Version::new(1, 0, 0)),
                Confidence::Certain,
            ))
        } else {
            None
        }
    }

    fn feed(&mut self, frame: FrameIn<'_>) -> Result<Vec<SessionEvent>, ReaderError> {
        let index = frame.raw_ref.frame;
        let fail = |detail: &str| ReaderError::Frame {
            frame: index,
            detail: String::from(detail),
        };
        let value = frame_json(frame.frame, index)?;
        let field = |key: &str| value.get(key).and_then(Value::as_str);
        let kind = field("type").ok_or_else(|| fail("no type"))?;
        let id: SessionId = field("session_id")
            .ok_or_else(|| fail("no session_id"))?
            .parse()
            .map_err(|_| fail("bad session_id"))?;
        let parent: Option<SessionId> = field("parent")
            .map(str::parse)
            .transpose()
            .map_err(|_| fail("bad parent"))?;
        let FrameIn { raw_ref, at, .. } = frame;
        let known = self.pending.iter().position(|p| p.id == id);
        let slot = if let Some(slot) = known {
            slot
        } else {
            self.pending.push(Pending {
                // BORROW: .clone() keeps the id for the event below
                id: id.clone(),
                parent,
                model: None,
                started_at: None,
                ended_at: None,
                ended_by: EndedBy::Unknown,
                // BORROW: .clone() keeps the frame's place for the event below
                raw_ref: raw_ref.clone(),
                next_seq: 0,
            });
            self.pending.len() - 1
        };
        let pending = self
            .pending
            .get_mut(slot)
            .ok_or_else(|| fail("no session"))?;
        let event_kind = match kind {
            "assistant" => EventKind::assistant_message(field("text").map(String::from)),
            "system" => {
                // BORROW: .clone() copies the frame time into the session
                pending.started_at = at.clone();
                pending.model = field("model").map(String::from);
                EventKind::notification()
            }
            "result" => {
                // BORROW: .clone() copies the frame time into the session
                pending.ended_at = at.clone();
                pending.ended_by = if value.get("is_error").and_then(Value::as_bool) == Some(true) {
                    EndedBy::agent_error(None)
                } else {
                    EndedBy::Completed
                };
                EventKind::notification()
            }
            other => return Err(fail(&format!("unknown type {other}"))),
        };
        let mut event = Event::new(pending.next_seq, event_kind, raw_ref);
        pending.next_seq += 1;
        if let Some(at) = at {
            event = event.with_at(at);
        }
        Ok(vec![SessionEvent::new(id, event)])
    }

    fn observed(&self) -> Observed {
        // BORROW: .clone() copies each id out of the pending sessions
        Observed::new(self.pending.iter().map(|p| p.id.clone()).collect())
    }

    fn finish(self: Box<Self>) -> Result<Vec<Session>, ReaderError> {
        let mut pending = self.pending;
        pending.sort_by_key(|p| p.parent.is_some());
        let session_source = source("claude-code", "2.1.304", "fake-lines-reader", "0.1.0")
            .map_err(|e| ReaderError::Incomplete {
                detail: e.to_string(),
            })?;
        pending
            .into_iter()
            .map(|p| {
                let mut builder = Session::builder()
                    .id(p.id)
                    // BORROW: .clone() gives each session its own source
                    .source(session_source.clone())
                    .ended_by(p.ended_by)
                    .raw_ref(p.raw_ref);
                if let Some(model) = p.model {
                    builder = builder.model(model);
                }
                if let Some(at) = p.started_at {
                    builder = builder.started_at(at);
                }
                if let Some(at) = p.ended_at {
                    builder = builder.ended_at(at);
                }
                if let Some(parent) = p.parent {
                    builder = builder.parent_session(parent);
                }
                Ok(builder.build()?)
            })
            .collect()
    }
}

struct LinesDriver;

impl HarnessDriver for LinesDriver {
    fn id(&self) -> HarnessId {
        HarnessId::new("claude-code".parse().unwrap_or_else(|_| unreachable!()))
    }

    fn supported(&self) -> semver::VersionReq {
        semver::VersionReq::parse(">=2.1, <3").unwrap_or_else(|_| unreachable!())
    }

    fn version_probe(&self) -> CommandSpec {
        CommandSpec::builder()
            .argv(vec![String::from("claude"), String::from("--version")])
            .build()
            .unwrap_or_else(|_| unreachable!())
    }

    fn parse_version(&self, output: &[u8]) -> Result<semver::Version, DriverError> {
        let text = String::from_utf8_lossy(output);
        // BORROW: explicit .as_ref() on the lossy text
        text.as_ref()
            .split_whitespace()
            .next()
            .and_then(|word| word.parse().ok())
            .ok_or_else(|| DriverError::VersionOutput {
                detail: String::from("no version"),
            })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::builder()
            .graceful_stop(true)
            .live_events(true)
            .budgets(BudgetCaps::new(true, true))
            .in_harness_guard(true)
            .census(true)
            .build()
    }

    fn prepare(
        &self,
        _cell: &Cell<'_>,
        _workspace: &Workspace<'_>,
    ) -> Result<Prepared, DriverError> {
        Ok(Prepared::default())
    }

    fn command(
        &self,
        _cell: &Cell<'_>,
        _workspace: &Workspace<'_>,
    ) -> Result<CommandSpec, DriverError> {
        Ok(CommandSpec::builder()
            .argv(vec![
                String::from("claude"),
                String::from("-p"),
                String::from("--output-format"),
                String::from("stream-json"),
            ])
            .build()?)
    }

    fn stop(&self) -> StopSpec {
        StopSpec::new(None, StopSignal::Term, Duration::from_secs(10))
    }

    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        // TRAIT_OBJECT: the trait returns readers as objects
        Some(Box::new(LinesReader {
            pending: Vec::new(),
        }))
    }

    fn offline_reader(&self, _stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        // TRAIT_OBJECT: the trait returns readers as objects
        Some(Box::new(LinesReader {
            pending: Vec::new(),
        }))
    }

    fn classify_exit(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy {
        match exit.ended {
            ProcessEnd::Deadline => EndedBy::cap(Cap::Wall),
            ProcessEnd::Exited | ProcessEnd::Stopped => sessions
                .first()
                // BORROW: .clone() returns the root session's end by value
                .map_or(EndedBy::Unknown, |root| root.ended_by().clone()),
            _ => EndedBy::Unknown,
        }
    }

    fn collect(&self, _workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec> {
        observed
            .session_ids
            .iter()
            .filter(|id| !id.as_str().contains('/')) // BORROW: explicit .as_str() instead of Deref coercion
            .map(|id| CollectSpec::file(format!("home/.claude/projects/*/{id}.jsonl")))
            .collect()
    }
}

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

// ---------------------------------------------------------------------------------------------
// The OpenCode-shaped fake: a document, no live reader, execution outside the child, an HTTP
// pre-stop with a body, an export command that carries the observed session id.
// ---------------------------------------------------------------------------------------------

struct DocumentReader {
    sessions: Vec<Session>,
}

impl TraceReader for DocumentReader {
    fn id(&self) -> NameVersion {
        "fake-document-reader@0.1.0"
            .parse()
            .unwrap_or_else(|_| unreachable!())
    }

    fn framing(&self) -> Framing {
        Framing::Document
    }

    fn detect(&self, sample: &[u8]) -> Option<(FormatVersion, Confidence)> {
        let text = String::from_utf8_lossy(sample);
        // BORROW: explicit .as_ref() on the lossy text
        let text = text.as_ref();
        let version = FormatVersion::new(semver::Version::new(1, 0, 0));
        if text.contains(r#""format":"opencode-export""#) {
            Some((version, Confidence::Certain))
        } else if text.contains(r#""sessions""#) {
            Some((version, Confidence::Likely))
        } else {
            None
        }
    }

    fn feed(&mut self, frame: FrameIn<'_>) -> Result<Vec<SessionEvent>, ReaderError> {
        let index = frame.raw_ref.frame;
        let fail = |detail: &str| ReaderError::Frame {
            frame: index,
            detail: String::from(detail),
        };
        let document = frame_json(frame.frame, index)?;
        let entries = document
            .get("sessions")
            .and_then(Value::as_array)
            .ok_or_else(|| fail("no sessions"))?;
        let session_source =
            source("opencode", "2.0.0", "fake-document-reader", "0.1.0").map_err(|e| {
                ReaderError::Incomplete {
                    detail: e.to_string(),
                }
            })?;
        let mut events = Vec::new();
        for entry in entries {
            let field = |key: &str| entry.get(key).and_then(Value::as_str);
            let id: SessionId = field("id")
                .ok_or_else(|| fail("no id"))?
                .parse()
                .map_err(|_| fail("bad id"))?;
            let ended_by = match field("result") {
                Some("error") => EndedBy::agent_error(None),
                Some("ok") => EndedBy::Completed,
                Some(_) | None => EndedBy::Unknown,
            };
            let mut builder = Session::builder()
                // BORROW: .clone() keeps the id for the event
                .id(id.clone())
                // BORROW: .clone() gives each session its own source
                .source(session_source.clone())
                .ended_by(ended_by)
                // BORROW: .clone() keeps the frame's place for the event
                .raw_ref(frame.raw_ref.clone());
            if let Some(model) = field("model") {
                builder = builder.model(String::from(model));
            }
            if let Some(parent) = field("parent") {
                builder = builder.parent_session(parent.parse().map_err(|_| fail("bad parent"))?);
            }
            self.sessions.push(builder.build()?);
            let kind = EventKind::assistant_message(field("text").map(String::from));
            // BORROW: .clone() keeps the frame's place for the next entry
            let mut event = Event::new(0, kind, frame.raw_ref.clone());
            // BORROW: .clone() copies the frame time, absent for a whole document
            if let Some(at) = frame.at.clone() {
                event = event.with_at(at);
            }
            events.push(SessionEvent::new(id, event));
        }
        Ok(events)
    }

    fn observed(&self) -> Observed {
        // BORROW: .clone() copies each id out of the sessions
        Observed::new(self.sessions.iter().map(|s| s.id().clone()).collect())
    }

    fn finish(self: Box<Self>) -> Result<Vec<Session>, ReaderError> {
        if self.sessions.is_empty() {
            return Err(ReaderError::Incomplete {
                detail: String::from("the document holds no session"),
            });
        }
        Ok(self.sessions)
    }
}

struct DocumentDriver;

impl HarnessDriver for DocumentDriver {
    fn id(&self) -> HarnessId {
        HarnessId::new("opencode".parse().unwrap_or_else(|_| unreachable!()))
    }

    fn supported(&self) -> semver::VersionReq {
        semver::VersionReq::parse(">=2, <3").unwrap_or_else(|_| unreachable!())
    }

    fn version_probe(&self) -> CommandSpec {
        CommandSpec::builder()
            .argv(vec![String::from("opencode"), String::from("--version")])
            .build()
            .unwrap_or_else(|_| unreachable!())
    }

    fn parse_version(&self, output: &[u8]) -> Result<semver::Version, DriverError> {
        let text = String::from_utf8_lossy(output);
        // BORROW: explicit .as_ref() on the lossy text
        text.as_ref()
            .trim()
            .parse()
            .map_err(|_| DriverError::VersionOutput {
                detail: String::from("not a version"),
            })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::builder()
            .graceful_stop(true)
            .execution_outside_child(true)
            .build()
    }

    fn prepare(
        &self,
        _cell: &Cell<'_>,
        _workspace: &Workspace<'_>,
    ) -> Result<Prepared, DriverError> {
        Ok(Prepared::default())
    }

    fn command(
        &self,
        cell: &Cell<'_>,
        _workspace: &Workspace<'_>,
    ) -> Result<CommandSpec, DriverError> {
        let path: FieldPath = "task.prompt"
            .parse()
            .map_err(|_| DriverError::VersionOutput {
                detail: String::from("bad path"),
            })?;
        let prompt = cell
            .get(&path)
            .ok_or_else(|| DriverError::MissingValue { path: path.clone() })? // BORROW: .clone() names the path in the error
            .as_value()
            .as_str()
            .map(String::from)
            .ok_or_else(|| DriverError::InvalidValue {
                path,
                detail: String::from("not a string"),
            })?;
        Ok(CommandSpec::builder()
            .argv(vec![
                String::from("opencode"),
                String::from("run"),
                String::from("--standalone"),
                prompt,
            ])
            .build()?)
    }

    fn stop(&self) -> StopSpec {
        StopSpec::new(
            Some(Action::http(
                HttpMethod::Post,
                String::from("http://127.0.0.1:4096/session/abort"),
                Some(String::from(r#"{"reason":"candler-stop"}"#)),
            )),
            StopSignal::Term,
            Duration::from_secs(5),
        )
    }

    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        None
    }

    fn offline_reader(&self, stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        // BORROW: explicit .as_str() instead of Deref coercion
        if stream.as_str() == "export" {
            // TRAIT_OBJECT: the trait returns readers as objects
            Some(Box::new(DocumentReader {
                sessions: Vec::new(),
            }))
        } else {
            None
        }
    }

    fn classify_exit(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy {
        let root = sessions.first().map(Session::ended_by);
        match (exit.ended, root) {
            (ProcessEnd::Deadline, _) => EndedBy::cap(Cap::Wall),
            // BORROW: .clone() returns the root session's end by value
            (ProcessEnd::Exited | ProcessEnd::Stopped, Some(ended)) => ended.clone(),
            _ => EndedBy::Unknown,
        }
    }

    fn collect(&self, _workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec> {
        observed
            .session_ids
            .iter()
            .filter(|id| !id.as_str().contains('/')) // BORROW: explicit .as_str() instead of Deref coercion
            .filter_map(|id| {
                let spec = CommandSpec::builder()
                    .argv(vec![
                        String::from("opencode"),
                        String::from("session"),
                        String::from("export"),
                        id.to_string(),
                    ])
                    .build()
                    .ok()?;
                Some(CollectSpec::command(spec, "export".parse().ok()?))
            })
            .collect()
    }
}

/// What a reader gave back for one document: events, observed sessions, sessions.
type Export = (Vec<SessionEvent>, Observed, Vec<Session>);

/// Feeds one whole document to the driver's offline reader for the export stream.
fn read_export(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the core selects drivers at runtime
    document: &str,
) -> Result<Export, TestError> {
    let export = stream("export")?;
    let Some(mut reader) = driver.offline_reader(&export) else {
        return Err("the OpenCode-shaped driver reads the export stream".into());
    };
    assert_eq!(reader.framing(), Framing::Document);
    let frame = RawFrame::new(None, FrameData::from_bytes(document.as_bytes()));
    let events = reader.feed(FrameIn::new(RawRef::new(export, 0), None, &frame))?;
    let observed = reader.observed();
    Ok((events, observed, reader.finish()?))
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
