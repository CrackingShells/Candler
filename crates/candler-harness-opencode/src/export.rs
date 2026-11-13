//! The reader of `opencode session export`: one JSON document, `{info, messages}`, read into one
//! root session whose end is the export's `outcome`.
//!
//! The export is the authoritative trace. The live NDJSON holds a tool call only after it
//! completed and none while it runs, and its token counts leave out the requests of the title
//! agent, so the events come from here and the session's usage total is `info.tokens`.

use std::str::FromStr as _;

use candler_harness::reader::{FrameIn, ReaderError, TraceReader};
use candler_harness::spec::{Confidence, FormatVersion, Framing, Observed, ObservedEnd};
use candler_schemas::outcome::Finite;
use candler_schemas::primitives::{NameVersion, ReasonCode, Timestamp};
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{
    EndedBy, Event, EventKind, Session, SessionEvent, SessionId, SpanId, ToolOrigin, UsageScope,
};
use serde_json::{Map, Value};

use crate::{VALIDATED, session_source};
use candler_schemas::name;

/// The tools `OpenCode` offers the model by itself, as `opencode_facts` listed them.
const BUILTIN_TOOLS: [&str; 12] = [
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
    "execute",
];

/// The outcomes an export names.
const OUTCOMES: [&str; 3] = ["succeeded", "interrupted", "failed"];

/// A reason code from an error type: lowercase letters, digits and `_`, so `provider.no-route` is
/// `provider_no_route`.
fn reason_of(kind: &str) -> Option<ReasonCode> {
    let text: String = kind
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    ReasonCode::from_str(&text).ok()
}

/// How a run that ended with the error of type `kind` ended.
///
/// An interrupt (`aborted`) is a stop that the core's own stop, wall cap or kill explains, so it is
/// `unknown` here. A provider that refused or could not be reached is a gateway or provider error;
/// a model that no provider lists never sent a request, so it is an agent error with a reason.
pub(crate) fn ended_by_of_error(kind: &str) -> EndedBy {
    if kind == "aborted" {
        EndedBy::Unknown
    } else if kind == "provider.no-route" {
        EndedBy::agent_error(reason_of(kind))
    } else if kind.starts_with("provider.") {
        EndedBy::GatewayError
    } else {
        EndedBy::agent_error(reason_of(kind))
    }
}

/// How a run ended, from the last word of its live stream.
pub(crate) fn ended_by_of_observed(end: &ObservedEnd) -> EndedBy {
    match end {
        ObservedEnd::Finished => EndedBy::Completed,
        ObservedEnd::Error { kind } => ended_by_of_error(kind.as_str()), // BORROW: explicit .as_str()
        ObservedEnd::Aborted => EndedBy::Unknown,
        #[expect(
            clippy::match_same_arms,
            reason = "ObservedEnd is non-exhaustive in another crate; a word this version does not know says nothing"
        )]
        _ => EndedBy::Unknown,
    }
}

fn timestamp(millis: Option<u64>) -> Option<Timestamp> {
    let millis = i64::try_from(millis?).ok()?;
    let at = jiff::Timestamp::from_millisecond(millis).ok()?;
    Timestamp::from_str(&at.to_string()).ok() // BORROW: .to_string() gives owned text
}

fn field<'v>(value: &'v Value, key: &str) -> Option<&'v Value> {
    value.get(key)
}

fn text<'v>(value: &'v Value, key: &str) -> Option<&'v str> {
    field(value, key).and_then(Value::as_str)
}

fn millis(value: &Value, path: &[&str]) -> Option<u64> {
    let mut at = value;
    for key in path {
        at = at.get(*key)?;
    }
    at.as_u64()
}

/// Token counts of an `info` or an assistant message, and the cost beside them.
struct Usage {
    kind: EventKind,
    ext: Map<String, Value>,
}

fn usage_of(scope: UsageScope, holder: &Value) -> Option<Usage> {
    let tokens = field(holder, "tokens")?;
    let count = |key: &str| field(tokens, key).and_then(Value::as_u64).unwrap_or(0);
    let cache = |key: &str| {
        field(tokens, "cache")
            .and_then(|cache| field(cache, key))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let cost = field(holder, "cost")
        .and_then(Value::as_f64)
        .and_then(|number| Finite::new(number).ok());
    let mut ext = Map::new();
    if let Some(reasoning) = field(tokens, "reasoning").and_then(Value::as_u64) {
        ext.insert("reasoning_tokens".to_owned(), Value::from(reasoning)); // BORROW: .to_owned() makes the owned key
    }
    Some(Usage {
        kind: EventKind::usage(
            scope,
            count("input"),
            count("output"),
            cache("read"),
            cache("write"),
            cost,
        ),
        ext,
    })
}

/// One event the reader has decided on, before it gets its number and place.
struct Draft {
    kind: EventKind,
    at: Option<Timestamp>,
    span: Option<SpanId>,
    ext: Map<String, Value>,
}

impl Draft {
    fn new(kind: EventKind, at: Option<Timestamp>) -> Self {
        Self {
            kind,
            at,
            span: None,
            ext: Map::new(),
        }
    }

    fn with(mut self, key: &str, value: Value) -> Self {
        self.ext.insert(key.to_owned(), value); // BORROW: .to_owned() makes the owned key
        self
    }
}

fn tool_drafts(block: &Value, message_at: Option<&Timestamp>) -> Vec<Draft> {
    let name = text(block, "name").unwrap_or_default();
    let state = field(block, "state");
    let input = state.and_then(|state| field(state, "input"));
    let argv = (name == "shell")
        .then(|| {
            input
                .and_then(|input| text(input, "command"))
                .map(|command| vec![command.to_owned()]) // BORROW: .to_owned() makes owned text
        })
        .flatten();
    let origin = if BUILTIN_TOOLS.contains(&name) {
        ToolOrigin::Builtin
    } else {
        ToolOrigin::Other
    };
    let time = |key: &str| timestamp(block.get("time").and_then(|t| millis(t, &[key])));
    let mut call = Draft::new(
        // BORROW: .to_owned() makes the owned name the event keeps
        EventKind::tool_call(name.to_owned(), origin, argv, Vec::new()),
        time("created").or_else(|| message_at.cloned()), // BORROW: .cloned() copies the time
    );
    let span = text(block, "id").and_then(|id| id.parse::<SpanId>().ok());
    if span.is_none()
        && let Some(id) = text(block, "id")
    {
        call = call.with("tool_use_id", Value::String(id.to_owned())); // BORROW: .to_owned() makes owned text
    }
    call.span.clone_from(&span); // BORROW: copies the id for the result
    if let Some(input) = input {
        call = call.with("input", input.clone()); // BORROW: .clone() keeps the input in the event
    }
    let mut drafts = vec![call];
    let status = state.and_then(|state| text(state, "status"));
    if matches!(status, Some("completed" | "error")) {
        let mut result = Draft::new(
            EventKind::tool_result(status == Some("error"), None),
            time("completed"),
        );
        result.span = span;
        drafts.push(result);
    }
    drafts
}

fn assistant_drafts(message: &Value, error_seen: &mut bool) -> Vec<Draft> {
    let at = timestamp(
        message
            .get("time")
            .and_then(|time| millis(time, &["created"])),
    );
    let mut drafts = Vec::new();
    for block in field(message, "content")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        match text(block, "type") {
            Some("text") => drafts.push(Draft::new(
                // BORROW: .to_owned() makes owned text
                EventKind::assistant_message(text(block, "text").map(str::to_owned)),
                at.clone(), // BORROW: .clone() copies the time
            )),
            Some("tool") => drafts.extend(tool_drafts(block, at.as_ref())),
            Some("reasoning") => {
                let mut body = Map::new();
                // BORROW: .to_owned() makes the owned key and text
                body.insert(
                    "text".to_owned(),
                    Value::String(text(block, "text").unwrap_or_default().to_owned()),
                );
                // BORROW: .to_owned() makes the owned kind
                if let Ok(kind) = EventKind::unknown("reasoning".to_owned(), body) {
                    drafts.push(Draft::new(kind, at.clone())); // BORROW: .clone() copies the time
                }
            }
            Some(_) | None => drafts.push(
                Draft::new(EventKind::notification(), at.clone()) // BORROW: .clone() copies the time
                    .with("block", block.clone()), // BORROW: .clone() keeps the block in the event
            ),
        }
    }
    if let Some(error) = field(message, "error") {
        *error_seen = true;
        drafts.push(
            Draft::new(EventKind::notification(), at.clone()) // BORROW: .clone() copies the time
                .with("error", error.clone()), // BORROW: .clone() keeps the error in the event
        );
    }
    if let Some(usage) = usage_of(UsageScope::Message, message) {
        let mut draft = Draft::new(usage.kind, at);
        draft.ext = usage.ext;
        if let Some(id) = text(message, "id") {
            draft = draft.with("message_id", Value::String(id.to_owned())); // BORROW: .to_owned() makes owned text
        }
        drafts.push(draft);
    }
    drafts
}

/// The type of the error that ended the run: the closing `idle` message's, else the last
/// assistant message's.
fn terminal_error(messages: &[Value]) -> Option<&str> {
    fn of(message: &Value) -> Option<&str> {
        field(message, "error").and_then(|error| text(error, "type"))
    }
    messages
        .iter()
        .rfind(|m| text(m, "type") == Some("idle"))
        .and_then(of)
        .or_else(|| messages.iter().rev().find_map(of))
}

/// How the session ended, from `info.outcome` and the error beside it.
#[expect(
    clippy::match_same_arms,
    reason = "an interrupt is a stop the core's own stop, wall cap or kill explains, listed apart from the outcomes that name nothing"
)]
fn ended_by_of_outcome(outcome: Option<&str>, error: Option<&str>) -> EndedBy {
    match outcome {
        Some("succeeded") => EndedBy::Completed,
        Some("interrupted") => EndedBy::Unknown,
        Some("failed") => error.map_or_else(|| EndedBy::agent_error(None), ended_by_of_error),
        Some(_) | None => EndedBy::Unknown,
    }
}

/// Reads one export document into a root session and its events.
#[non_exhaustive]
pub struct OpenCodeExportReader {
    identity: NameVersion,
    read: Option<(Session, SessionId)>,
}

impl Default for OpenCodeExportReader {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenCodeExportReader {
    /// A reader of the export.
    #[must_use]
    pub fn new() -> Self {
        Self {
            identity: NameVersion::of(&name!("opencode-export"), semver::Version::new(0, 1, 0)),
            read: None,
        }
    }

    fn read(
        &mut self,
        frame: &FrameIn<'_>,
        document: &Value,
    ) -> Result<Vec<SessionEvent>, ReaderError> {
        let fail = |detail: &str| ReaderError::Frame {
            frame: frame.raw_ref.frame,
            detail: detail.to_owned(), // BORROW: .to_owned() makes the owned text
        };
        let info = field(document, "info").ok_or_else(|| fail("the document has no `info`"))?;
        let messages = field(document, "messages")
            .and_then(Value::as_array)
            .ok_or_else(|| fail("the document has no `messages` list"))?;
        let own = text(info, "id").ok_or_else(|| fail("`info` has no `id`"))?;
        // A child session's export is a child: its id is its parent's, `/`, its own, as the trace
        // requires, so it can never be taken for a root.
        let parent = match text(info, "parentID") {
            Some(parent) => Some(
                parent
                    .parse::<SessionId>()
                    .map_err(|_| fail("`info.parentID` is not a session id"))?,
            ),
            None => None,
        };
        let id: SessionId = match &parent {
            Some(parent) => SessionId::child(
                parent,
                &own.parse::<SpanId>()
                    .map_err(|_| fail("`info.id` is not a call id"))?,
            ),
            None => own
                .parse()
                .map_err(|_| fail("`info.id` is not a session id"))?,
        };
        let mut drafts = Vec::new();
        let mut error_seen = false;
        for message in messages {
            let at = || {
                timestamp(
                    message
                        .get("time")
                        .and_then(|time| millis(time, &["created"])),
                )
            };
            match text(message, "type") {
                Some("user") => drafts.push(
                    Draft::new(
                        // BORROW: .to_owned() makes owned text
                        EventKind::user_message(text(message, "text").map(str::to_owned)),
                        at(),
                    )
                    .with(
                        "message_id",
                        text(message, "id").map_or(Value::Null, |i| Value::String(i.to_owned())),
                    ), // BORROW: .to_owned() makes owned text
                ),
                Some("assistant") => drafts.extend(assistant_drafts(message, &mut error_seen)),
                Some("idle") => {
                    if let (Some(error), false) = (field(message, "error"), error_seen) {
                        drafts.push(
                            Draft::new(EventKind::notification(), at())
                                .with("error", error.clone()), // BORROW: .clone() keeps the error in the event
                        );
                    }
                }
                Some(_) | None => drafts.push(
                    Draft::new(EventKind::notification(), at()).with("message", message.clone()), // BORROW: .clone() keeps the message in the event
                ),
            }
        }
        if let Some(total) = usage_of(UsageScope::SessionTotal, info) {
            let mut draft = Draft::new(total.kind, None);
            draft.ext = total.ext;
            drafts.push(draft);
        }
        let mut events = Vec::with_capacity(drafts.len());
        for (seq, draft) in (0_u64..).zip(drafts) {
            // BORROW: .clone() keeps the frame of the event
            let mut event = Event::new(seq, draft.kind, frame.raw_ref.clone()).with_ext(draft.ext);
            if let Some(at) = draft.at {
                event = event.with_at(at);
            }
            if let Some(span) = draft.span {
                event = event.with_span(span);
            }
            events.push(SessionEvent::new(id.clone(), event)); // BORROW: .clone() copies the id
        }
        let source = session_source(&self.identity)?;
        let mut session = Session::builder()
            .id(id.clone()) // BORROW: .clone() copies the id
            .source(source)
            .ended_by(ended_by_of_outcome(
                text(info, "outcome"),
                terminal_error(messages),
            ))
            .raw_ref(frame.raw_ref.clone()); // BORROW: .clone() copies the frame place
        if let Some(parent) = parent {
            session = session.parent_session(parent);
        }
        if let Some(model) = field(info, "model").and_then(|m| text(m, "id")) {
            session = session.model(model.to_owned()); // BORROW: .to_owned() makes owned text
        }
        if let Some(at) = timestamp(millis(info, &["time", "created"])) {
            session = session.started_at(at);
        }
        if let Some(at) = timestamp(millis(info, &["time", "updated"])) {
            session = session.ended_at(at);
        }
        self.read = Some((session.build()?, id));
        Ok(events)
    }
}

impl TraceReader for OpenCodeExportReader {
    fn id(&self) -> NameVersion {
        self.identity.clone() // BORROW: .clone() copies the reader identity
    }

    fn framing(&self) -> Framing {
        Framing::Document
    }

    fn detect(&self, sample: &[u8]) -> Option<(FormatVersion, Confidence)> {
        // No export names the version that wrote it, so the most a sample can earn is `Likely`, and
        // only when it holds an `info` with a session id and an outcome this reader knows.
        let known = |outcome: &str| OUTCOMES.contains(&outcome);
        let looks = if let Ok(document) = serde_json::from_slice::<Value>(sample) {
            field(&document, "info").is_some_and(|info| {
                text(info, "id").is_some() && text(info, "outcome").is_some_and(known)
            })
        } else {
            // A sample cut inside a long document: read the text for the pair.
            let text = String::from_utf8_lossy(sample);
            // BORROW: explicit .as_ref() on the lossy text
            let text = text.as_ref();
            text.contains("\"info\"")
                && OUTCOMES.iter().any(|outcome| {
                    text.contains(&format!("\"outcome\": \"{outcome}\""))
                        || text.contains(&format!("\"outcome\":\"{outcome}\""))
                })
        };
        looks.then(|| (FormatVersion::new(VALIDATED), Confidence::Likely))
    }

    fn feed(&mut self, frame: FrameIn<'_>) -> Result<Vec<SessionEvent>, ReaderError> {
        if self.read.is_some() {
            return Err(ReaderError::Frame {
                frame: frame.raw_ref.frame,
                detail: "an export is one document".to_owned(), // BORROW: .to_owned() makes the owned text
            });
        }
        let bytes = frame.frame.data.to_bytes();
        let document: Value =
            serde_json::from_slice(&bytes).map_err(|error| ReaderError::Frame {
                frame: frame.raw_ref.frame,
                detail: error.to_string(), // BORROW: .to_string() gives owned text
            })?;
        self.read(&frame, &document)
    }

    fn observed(&self) -> Observed {
        // BORROW: .clone() copies the id out of the reader
        Observed::new(self.read.iter().map(|(_, id)| id.clone()).collect())
    }

    fn finish(self: Box<Self>) -> Result<Vec<Session>, ReaderError> {
        self.read
            .map(|(session, _)| vec![session])
            .ok_or_else(|| ReaderError::Incomplete {
                detail: "the export holds no session".to_owned(), // BORROW: .to_owned() makes the owned text
            })
    }
}

/// The stream the export is collected as.
pub(crate) fn export_stream() -> Option<StreamName> {
    crate::EXPORT_STREAM.parse().ok()
}
