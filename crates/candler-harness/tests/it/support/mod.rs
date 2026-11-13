//! The fake harnesses of the shared fixture suite and of the two-shapes guard.
#![cfg(test)]
#![cfg_attr(
    not(feature = "conformance"),
    allow(
        dead_code,
        reason = "the fake harnesses of the conformance suite are used only by `conformance_shapes`, \
                  which is built with the `conformance` feature"
    )
)]

use std::time::Duration;

use candler_harness::driver::{DriverError, HarnessDriver, Plan};
use candler_harness::reader::{FrameIn, ReaderError, TraceReader};
use candler_harness::spec::{
    Action, BudgetCaps, Capabilities, Cell, CensusSource, CollectSpec, CommandSpec, Confidence,
    FormatVersion, Framing, GatewayEnv, HarnessId, HttpMethod, InitReport, Observed, ObservedEnd,
    Prepared, PreparedFile, ProcessEnd, ProcessExit, StateKind, StatePath, StopSignal, StopSpec,
    Workspace,
};
use candler_schemas::primitives::{FieldPath, Name, NameVersion, Timestamp};
use candler_schemas::raw::{FrameData, RawFrame, RawRef, StreamName};
use candler_schemas::trace::{
    Cap, EndedBy, Event, EventKind, Session, SessionEvent, SessionId, SessionSource,
};
use serde_json::Value;

/// The error every test helper returns.
pub type TestError = Box<dyn std::error::Error>; // TRAIT_OBJECT: helpers return the error of whichever call failed

pub type TestResult = Result<(), TestError>;

pub fn sid(text: &str) -> Result<SessionId, TestError> {
    Ok(text.parse()?)
}

pub fn ts(text: &str) -> Result<Timestamp, TestError> {
    Ok(text.parse()?)
}

pub fn stream(text: &str) -> Result<StreamName, TestError> {
    Ok(text.parse()?)
}

pub fn source(
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
pub fn frame_json(frame: &RawFrame, index: u64) -> Result<Value, ReaderError> {
    serde_json::from_slice(&frame.data.to_bytes()).map_err(|e| ReaderError::Frame {
        frame: index,
        detail: e.to_string(),
    })
}

// ---------------------------------------------------------------------------------------------
// The Claude Code-shaped fake: lines, a live reader, a signal-only stop, a file collect.
// ---------------------------------------------------------------------------------------------

pub struct Pending {
    id: SessionId,
    parent: Option<SessionId>,
    model: Option<String>,
    started_at: Option<Timestamp>,
    ended_at: Option<Timestamp>,
    ended_by: EndedBy,
    raw_ref: RawRef,
    next_seq: u64,
}

pub struct LinesReader {
    pub pending: Vec<Pending>,
    pub init: Option<InitReport>,
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
                if self.init.is_none()
                    && let Some(tools) = value.get("tools")
                {
                    // BORROW: .cloned() copies the lists out of the frame
                    self.init = InitReport::new(
                        tools.clone(),
                        value.get("mcp_servers").cloned(),
                        CensusSource::Harness,
                    )
                    .ok();
                }
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
        let observed = Observed::new(self.pending.iter().map(|p| p.id.clone()).collect());
        match &self.init {
            // BORROW: .clone() copies the inventory out of the reader
            Some(init) => observed.with_init(init.clone()),
            None => observed,
        }
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

pub struct LinesDriver;

impl HarnessDriver for LinesDriver {
    fn skills(&self) -> candler_harness::skills::SkillTree {
        candler_harness::skills::SkillTree::new(".claude/skills", ".claude/skills")
    }

    fn base_image(&self) -> candler_harness::spec::BaseImage {
        candler_harness::spec::BaseImage::new("base", Vec::new())
    }

    fn detect(&self, _sample: &[u8]) -> Option<semver::Version> {
        None
    }

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

    fn gateway(&self) -> Option<GatewayEnv> {
        Some(GatewayEnv::new(
            String::from("ANTHROPIC_BASE_URL"),
            String::from("ANTHROPIC_AUTH_TOKEN"),
            vec![
                String::from("ANTHROPIC_BASE_URL"),
                String::from("ANTHROPIC_AUTH_TOKEN"),
                String::from("ANTHROPIC_API_KEY"),
            ],
        ))
    }

    fn plan(&self, _cell: &Cell<'_>, _workspace: &Workspace<'_>) -> Result<Plan, DriverError> {
        Ok(Plan::new(
            Prepared::default(),
            CommandSpec::builder()
                .argv(vec![
                    String::from("claude"),
                    String::from("-p"),
                    String::from("--output-format"),
                    String::from("stream-json"),
                ])
                .build()?,
        ))
    }

    fn stop(&self) -> StopSpec {
        StopSpec::new(None, StopSignal::Term, Duration::from_secs(10))
    }

    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        // TRAIT_OBJECT: the trait returns readers as objects
        Some(Box::new(LinesReader {
            pending: Vec::new(),
            init: None,
        }))
    }

    fn offline_reader(&self, _stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        // TRAIT_OBJECT: the trait returns readers as objects
        Some(Box::new(LinesReader {
            pending: Vec::new(),
            init: None,
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

// ---------------------------------------------------------------------------------------------
// The OpenCode-shaped fake: a document, no live reader, execution outside the child, an HTTP
// pre-stop with a body, an export command that carries the observed session id.
// ---------------------------------------------------------------------------------------------

pub struct DocumentReader {
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
                Some("interrupted") => EndedBy::Interrupted,
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

pub struct DocumentDriver;

impl HarnessDriver for DocumentDriver {
    fn skills(&self) -> candler_harness::skills::SkillTree {
        candler_harness::skills::SkillTree::new(".claude/skills", ".claude/skills")
    }

    fn base_image(&self) -> candler_harness::spec::BaseImage {
        candler_harness::spec::BaseImage::new("base", Vec::new())
    }

    fn detect(&self, _sample: &[u8]) -> Option<semver::Version> {
        None
    }

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

    fn gateway(&self) -> Option<GatewayEnv> {
        Some(GatewayEnv::new(
            String::from("FAKE_GATEWAY_URL"),
            String::from("FAKE_GATEWAY_TOKEN"),
            vec![
                String::from("FAKE_GATEWAY_URL"),
                String::from("FAKE_GATEWAY_TOKEN"),
            ],
        ))
    }

    fn plan(&self, cell: &Cell<'_>, _workspace: &Workspace<'_>) -> Result<Plan, DriverError> {
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
        Ok(Plan::new(
            Prepared::default(),
            CommandSpec::builder()
                .argv(vec![
                    String::from("opencode"),
                    String::from("run"),
                    String::from("--standalone"),
                    prompt,
                ])
                .build()?,
        ))
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
pub type Export = (Vec<SessionEvent>, Observed, Vec<Session>);

/// Feeds one whole document to the driver's offline reader for the export stream.
pub fn read_export(
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

// ---------------------------------------------------------------------------------------------
// The third fake takes the OpenCode facts one by one: provider configuration through variable
// names, state paths, no init event (the census is a collected gateway record), an in-container
// interrupt that needs the session id, an exit 1 that means three endings and an authoritative
// export.
// ---------------------------------------------------------------------------------------------

/// A live reader that emits no events: it reports the session id and the last word of the
/// stream, the way a reader of `OpenCode`'s NDJSON does.
pub struct IdsReader {
    ids: Vec<SessionId>,
    end: Option<ObservedEnd>,
}

impl IdsReader {
    pub const fn new() -> Self {
        Self {
            ids: Vec::new(),
            end: None,
        }
    }
}

impl TraceReader for IdsReader {
    fn id(&self) -> NameVersion {
        "fake-ids-reader@0.1.0"
            .parse()
            .unwrap_or_else(|_| unreachable!())
    }

    fn framing(&self) -> Framing {
        Framing::Lines
    }

    fn detect(&self, sample: &[u8]) -> Option<(FormatVersion, Confidence)> {
        let text = String::from_utf8_lossy(sample);
        // BORROW: explicit .as_ref() on the lossy text
        text.as_ref().contains(r#""sessionID""#).then(|| {
            (
                FormatVersion::new(semver::Version::new(1, 0, 0)),
                Confidence::Certain,
            )
        })
    }

    fn feed(&mut self, frame: FrameIn<'_>) -> Result<Vec<SessionEvent>, ReaderError> {
        let index = frame.raw_ref.frame;
        let value = frame_json(frame.frame, index)?;
        if let Some(id) = value.get("sessionID").and_then(Value::as_str) {
            let id: SessionId = id.parse().map_err(|_| ReaderError::Frame {
                frame: index,
                detail: String::from("bad sessionID"),
            })?;
            if !self.ids.contains(&id) {
                self.ids.push(id);
            }
        }
        match value.get("type").and_then(Value::as_str) {
            Some("error") => {
                let kind = value
                    .get("error")
                    .and_then(|e| e.get("type"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                self.end = Some(if kind == "aborted" {
                    ObservedEnd::Aborted
                } else {
                    ObservedEnd::Error {
                        kind: String::from(kind),
                    }
                });
            }
            Some("step_finish") => {
                let stop = value
                    .get("part")
                    .and_then(|p| p.get("reason"))
                    .and_then(Value::as_str)
                    == Some("stop");
                if stop && self.end.is_none() {
                    self.end = Some(ObservedEnd::Finished);
                }
            }
            Some(_) | None => {}
        }
        Ok(Vec::new())
    }

    fn observed(&self) -> Observed {
        // BORROW: .clone() copies the ids and the end out of the reader
        let observed = Observed::new(self.ids.clone());
        match &self.end {
            Some(end) => observed.with_end(end.clone()),
            None => observed,
        }
    }

    fn finish(self: Box<Self>) -> Result<Vec<Session>, ReaderError> {
        Err(ReaderError::Incomplete {
            detail: String::from("the live stream names no session content"),
        })
    }
}

/// A reader of the gateway's record of model requests, one JSON line per request with the names of
/// the tools it carried. Its census is the tools of the first request that carries any (a title
/// request carries none); a record with no such request leaves the census unrecorded.
pub struct CensusReader {
    tools: Option<Value>,
}

impl TraceReader for CensusReader {
    fn id(&self) -> NameVersion {
        "fake-census-reader@0.1.0"
            .parse()
            .unwrap_or_else(|_| unreachable!())
    }

    fn framing(&self) -> Framing {
        Framing::Lines
    }

    fn detect(&self, _sample: &[u8]) -> Option<(FormatVersion, Confidence)> {
        None
    }

    fn feed(&mut self, frame: FrameIn<'_>) -> Result<Vec<SessionEvent>, ReaderError> {
        let index = frame.raw_ref.frame;
        let value = frame_json(frame.frame, index)?;
        let carries_tools = value
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|tools| !tools.is_empty());
        if self.tools.is_none() && carries_tools {
            self.tools = value.get("tools").cloned();
        }
        Ok(Vec::new())
    }

    fn observed(&self) -> Observed {
        let observed = Observed::new(Vec::new());
        match self
            .tools
            .clone() // BORROW: .clone() copies the tools out of the reader
            .and_then(|tools| InitReport::new(tools, None, CensusSource::Gateway).ok())
        {
            Some(init) => observed.with_init(init),
            None => observed,
        }
    }

    fn finish(self: Box<Self>) -> Result<Vec<Session>, ReaderError> {
        Err(ReaderError::Incomplete {
            detail: String::from("a census record names no session"),
        })
    }
}

/// The in-container interrupt: finds the server child's port and password, then calls the API
/// with the session id the core fills in.
pub const INTERRUPT_SCRIPT: &str = "pid=; for d in /proc/[0-9]*; do \
case \"$(tr '\\0' ' ' < $d/cmdline 2>/dev/null)\" in \"opencode serve \"*) pid=${d#/proc/};; esac; done; \
pw=$(tr '\\0' '\\n' < /proc/$pid/environ | sed -n 's/^OPENCODE_PASSWORD=//p'); \
port=$(awk 'NR>1 && $4==\"0A\" {print $2}' /proc/net/tcp | head -n1 | cut -d: -f2); \
OPENCODE_PASSWORD=$pw opencode api --server http://127.0.0.1:$((16#$port)) \
session.interrupt --param sessionID={session_id}";

/// The `OpenCode`-shaped fake with every fact the traits must carry.
pub struct OpenShapedDriver(pub DocumentDriver);

impl HarnessDriver for OpenShapedDriver {
    fn skills(&self) -> candler_harness::skills::SkillTree {
        candler_harness::skills::SkillTree::new(".claude/skills", ".claude/skills")
    }

    fn base_image(&self) -> candler_harness::spec::BaseImage {
        candler_harness::spec::BaseImage::new("base", Vec::new())
    }

    fn detect(&self, _sample: &[u8]) -> Option<semver::Version> {
        None
    }

    fn id(&self) -> HarnessId {
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
            .build()
    }

    fn gateway(&self) -> Option<GatewayEnv> {
        Some(GatewayEnv::new(
            String::from("OC_BASE_URL"),
            String::from("OC_API_KEY"),
            vec![String::from("OC_BASE_URL"), String::from("OC_API_KEY")],
        ))
    }

    fn state_paths(&self) -> Vec<StatePath> {
        let rel = |text: &str| text.parse().unwrap_or_else(|_| unreachable!());
        let dir =
            |env: &str, path: &str| StatePath::new(String::from(env), StateKind::Dir, rel(path));
        vec![
            dir("XDG_DATA_HOME", ".xdg/data"),
            dir("XDG_CONFIG_HOME", ".xdg/config"),
            dir("XDG_CACHE_HOME", ".xdg/cache"),
            dir("XDG_STATE_HOME", ".xdg/state"),
            StatePath::new(
                String::from("OPENCODE_DB"),
                StateKind::File,
                rel(".xdg/data/oc.db"),
            ),
        ]
    }

    fn census_stream(&self) -> Option<StreamName> {
        "model_requests".parse().ok()
    }

    fn plan(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Plan, DriverError> {
        let mut plan = self.0.plan(cell, workspace)?;
        // The provider's address and key are references to the variables the core sets; no value
        // passes through the driver.
        let Some(gateway) = self.gateway() else {
            plan.prepared = Prepared::default();
            return Ok(plan);
        };
        let config = serde_json::json!({
            "provider": { "fake": {
                "npm": "@ai-sdk/anthropic",
                "options": {
                    "baseURL": format!("{{env:{}}}/v1", gateway.base_url),
                    "apiKey": format!("{{env:{}}}", gateway.auth_token),
                },
                "models": {},
            } }
        });
        let path = ".xdg/config/opencode/opencode.json".parse().map_err(|_| {
            DriverError::VersionOutput {
                detail: String::from("bad path"),
            }
        })?;
        plan.prepared = Prepared::new(
            vec![PreparedFile::in_home(
                path,
                config.to_string().into_bytes(),
                false,
            )],
            Vec::new(),
        );
        Ok(plan)
    }

    fn stop(&self) -> StopSpec {
        let script = CommandSpec::builder()
            .argv(vec![
                String::from("sh"),
                String::from("-c"),
                String::from(INTERRUPT_SCRIPT),
            ])
            .build()
            .unwrap_or_else(|_| unreachable!());
        StopSpec::new(
            Some(Action::exec(script)),
            StopSignal::Term,
            Duration::from_secs(5),
        )
    }

    fn live_reader(&self) -> Option<Box<dyn TraceReader>> {
        // TRAIT_OBJECT: the trait returns readers as objects
        Some(Box::new(IdsReader::new()))
    }

    fn offline_reader(&self, stream: &StreamName) -> Option<Box<dyn TraceReader>> {
        // BORROW: explicit .as_str() instead of Deref coercion
        match stream.as_str() {
            "model_requests" => Some(Box::new(CensusReader { tools: None })), // TRAIT_OBJECT: the trait returns readers as objects
            "export" => Some(Box::new(ExportReader { session: None })), // TRAIT_OBJECT: the trait returns readers as objects
            _ => self.0.offline_reader(stream),
        }
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
        if exit.ended == ProcessEnd::Deadline {
            return EndedBy::cap(Cap::Wall);
        }
        // The export is authoritative; the stream's last word stands in when there is none.
        if let Some(root) = sessions.first() {
            // BORROW: .clone() returns the root session's end by value
            return root.ended_by().clone();
        }
        match &observed.end {
            Some(ObservedEnd::Finished) => EndedBy::Completed,
            Some(ObservedEnd::Aborted) => EndedBy::Interrupted,
            Some(ObservedEnd::Error { .. }) => EndedBy::agent_error(None),
            Some(_) | None => EndedBy::Unknown,
        }
    }

    fn collect(&self, _workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec> {
        // The export needs `--standalone` and the run's `OPENCODE_DB` and XDG variables, which
        // only a command inside the isolation has.
        observed
            .session_ids
            .iter()
            .filter(|id| !id.as_str().contains('/')) // BORROW: explicit .as_str() instead of Deref coercion
            .filter_map(|id| {
                let spec = CommandSpec::builder()
                    .argv(
                        ["opencode", "session", "export", "--standalone"]
                            .map(String::from)
                            .into_iter()
                            .chain(std::iter::once(id.to_string()))
                            .collect(),
                    )
                    .build()
                    .ok()?;
                Some(CollectSpec::exec(spec, "export".parse().ok()?))
            })
            .collect()
    }
}

/// A reader of the recorded `session export` document: `{info, messages}`, one root session whose
/// end is the export's `outcome`.
pub struct ExportReader {
    session: Option<Session>,
}

impl TraceReader for ExportReader {
    fn id(&self) -> NameVersion {
        "fake-export-reader@0.1.0"
            .parse()
            .unwrap_or_else(|_| unreachable!())
    }

    fn framing(&self) -> Framing {
        Framing::Document
    }

    fn detect(&self, sample: &[u8]) -> Option<(FormatVersion, Confidence)> {
        let text = String::from_utf8_lossy(sample);
        // BORROW: explicit .as_ref() on the lossy text
        (text.as_ref().contains(r#""outcome""#) && text.as_ref().contains(r#""messages""#)).then(
            || {
                (
                    FormatVersion::new(semver::Version::new(1, 0, 0)),
                    Confidence::Likely,
                )
            },
        )
    }

    fn feed(&mut self, frame: FrameIn<'_>) -> Result<Vec<SessionEvent>, ReaderError> {
        let index = frame.raw_ref.frame;
        let fail = |detail: &str| ReaderError::Frame {
            frame: index,
            detail: String::from(detail),
        };
        let document = frame_json(frame.frame, index)?;
        let info = document.get("info").ok_or_else(|| fail("no info"))?;
        let id: SessionId = info
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| fail("no info.id"))?
            .parse()
            .map_err(|_| fail("bad info.id"))?;
        let ended_by = match info.get("outcome").and_then(Value::as_str) {
            Some("succeeded") => EndedBy::Completed,
            Some("interrupted") => EndedBy::Interrupted,
            Some("failed") => EndedBy::agent_error(None),
            Some(_) | None => EndedBy::Unknown,
        };
        let session_source =
            source("opencode", "2.2.7", "fake-export-reader", "0.1.0").map_err(|e| {
                ReaderError::Incomplete {
                    detail: e.to_string(),
                }
            })?;
        self.session = Some(
            Session::builder()
                .id(id)
                .source(session_source)
                .ended_by(ended_by)
                .raw_ref(frame.raw_ref)
                .build()?,
        );
        Ok(Vec::new())
    }

    fn observed(&self) -> Observed {
        // BORROW: .clone() copies the id out of the session
        Observed::new(self.session.iter().map(|s| s.id().clone()).collect())
    }

    fn finish(self: Box<Self>) -> Result<Vec<Session>, ReaderError> {
        self.session
            .map(|session| vec![session])
            .ok_or_else(|| ReaderError::Incomplete {
                detail: String::from("the export holds no session"),
            })
    }
}
