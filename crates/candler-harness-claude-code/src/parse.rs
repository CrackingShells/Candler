//! One stream-json line, read into the events it holds.

use candler_schemas::primitives::Timestamp;
use candler_schemas::raw::{FrameData, RawFrame};
use candler_schemas::trace::{EventKind, SpanId, ToolOrigin};
use serde_json::{Map, Value};

use crate::detect;

/// An event before it has a session, a sequence number or a frame.
#[derive(Debug)]
pub(crate) struct Draft {
    /// What happened.
    pub(crate) kind: EventKind,
    /// The id that pairs a call with its result.
    pub(crate) span: Option<SpanId>,
    /// The data of the line that no other field holds.
    pub(crate) ext: Map<String, Value>,
}

impl Draft {
    /// A draft of `kind` with no span and an empty `ext`.
    fn new(kind: EventKind) -> Self {
        Self {
            kind,
            span: None,
            ext: Map::new(),
        }
    }

    /// A notification holding `ext`.
    fn notification(ext: Map<String, Value>) -> Self {
        Self {
            kind: EventKind::notification(),
            span: None,
            ext,
        }
    }

    /// A notification holding the one entry `key`.
    fn notification_of(key: &str, value: Value) -> Self {
        let mut ext = Map::new();
        // BORROW: .to_owned() makes the owned key the map keeps
        ext.insert(key.to_owned(), value);
        Self::notification(ext)
    }
}

/// What one frame holds, as far as the reader needs it.
#[derive(Debug)]
pub(crate) struct Parsed {
    /// The `session_id` of the line.
    pub(crate) session_id: Option<String>,
    /// The `timestamp` of the line, when it is a valid RFC 3339 time.
    pub(crate) at: Option<Timestamp>,
    /// The Claude Code version, when the line is an init line.
    pub(crate) harness_version: Option<semver::Version>,
    /// The model the line names.
    pub(crate) model: Option<String>,
    /// The events of the line, in order.
    pub(crate) drafts: Vec<Draft>,
}

impl Parsed {
    /// A frame that holds no event.
    fn empty() -> Self {
        Self::of(Vec::new())
    }

    /// A frame with no session, time, version or model that holds `drafts`.
    fn of(drafts: Vec<Draft>) -> Self {
        Self {
            session_id: None,
            at: None,
            harness_version: None,
            model: None,
            drafts,
        }
    }
}

/// Who spoke on a message line.
#[derive(Clone, Copy)]
enum Role {
    /// An `assistant` line.
    Assistant,
    /// A `user` line.
    User,
}

/// Reads one frame. A blank frame holds no event; text that is not a JSON object becomes a notification.
pub(crate) fn parse(frame: &RawFrame) -> Parsed {
    match &frame.data {
        FrameData::Text { text } => {
            if text.trim().is_empty() {
                return Parsed::empty();
            }
            match serde_json::from_str::<Value>(text) {
                Ok(Value::Object(line)) => object_line(line),
                // BORROW: .clone() keeps the text of the frame in the event
                Err(_) => Parsed::of(vec![Draft::notification_of(
                    "text",
                    Value::String(text.clone()),
                )]),
                Ok(other) => Parsed::of(vec![Draft::notification_of("json", other)]),
            }
        }
        FrameData::Bytes { base64 } => Parsed::of(vec![Draft::notification_of(
            "base64",
            Value::String(base64.to_string()),
        )]),
        // A frame form this version does not know is kept as a marker; the frame itself stays in the raw file.
        _ => Parsed::of(vec![Draft::notification_of(
            "unsupported_frame",
            Value::Bool(true),
        )]),
    }
}

/// Reads one JSON object line.
fn object_line(line: Map<String, Value>) -> Parsed {
    let text_of = |key: &str| line.get(key).and_then(Value::as_str);
    // BORROW: .to_owned() makes the owned id the result keeps
    let session_id = text_of("session_id").map(str::to_owned);
    let at = text_of("timestamp").and_then(|time| time.parse::<Timestamp>().ok());
    let kind = text_of("type");
    let harness_version = detect::init_version(&line);
    let model = match kind {
        // BORROW: .to_owned() makes the owned model the result keeps
        Some("system") if text_of("subtype") == Some("init") => text_of("model").map(str::to_owned),
        Some("assistant") => line
            .get("message")
            .and_then(|message| message.get("model"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        _ => None,
    };
    let drafts = match kind {
        Some("assistant") => message_drafts(&line, Role::Assistant),
        Some("user") => message_drafts(&line, Role::User),
        _ => vec![Draft::notification(line)],
    };
    Parsed {
        session_id,
        at,
        harness_version,
        model,
        drafts,
    }
}

/// The events of an `assistant` or `user` line: one per content block, or the line itself as a notification when it has no content.
fn message_drafts(line: &Map<String, Value>, role: Role) -> Vec<Draft> {
    let content = line
        .get("message")
        .and_then(|message| message.get("content"));
    let mut drafts = match content {
        // BORROW: .clone() keeps the text of the message in the event
        Some(Value::String(text)) => vec![message_draft(role, text.clone())],
        Some(Value::Array(blocks)) => blocks
            .iter()
            .map(|block| block_draft(role, block))
            .collect(),
        Some(Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_)) | None => {
            Vec::new()
        }
    };
    if drafts.is_empty() {
        // BORROW: .clone() keeps the whole line in the event
        drafts.push(Draft::notification(line.clone()));
    }
    drafts
}

/// A message event of `role` holding `text`.
fn message_draft(role: Role, text: String) -> Draft {
    Draft::new(match role {
        Role::Assistant => EventKind::assistant_message(Some(text)),
        Role::User => EventKind::user_message(Some(text)),
    })
}

/// The event of one content block.
fn block_draft(role: Role, block: &Value) -> Draft {
    let text_of = |key: &str| block.get(key).and_then(Value::as_str);
    match (role, text_of("type")) {
        // BORROW: .to_owned() makes the owned text the event keeps
        (Role::Assistant | Role::User, Some("text")) => match text_of("text") {
            Some(text) => message_draft(role, text.to_owned()),
            None => unknown_block(block),
        },
        (Role::Assistant, Some("tool_use")) => match (text_of("id"), text_of("name")) {
            (Some(id), Some(name)) => tool_call(id, name, block.get("input")),
            _ => unknown_block(block),
        },
        (Role::User, Some("tool_result")) => match text_of("tool_use_id") {
            Some(id) => {
                let is_error = block
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let mut draft = Draft::new(EventKind::tool_result(is_error, None));
                draft.span = span_of(id, &mut draft.ext);
                draft
            }
            None => unknown_block(block),
        },
        _ => unknown_block(block),
    }
}

/// A notification that keeps a content block this reader has no event for.
fn unknown_block(block: &Value) -> Draft {
    // BORROW: .clone() keeps the whole block in the event
    Draft::notification_of("block", block.clone())
}

/// The span for the call id `id`; an id that cannot be a span is kept in `ext` instead.
fn span_of(id: &str, ext: &mut Map<String, Value>) -> Option<SpanId> {
    // BORROW: .to_owned() makes the owned id the map keeps when the id is no span
    id.parse::<SpanId>().map_or_else(
        |_| {
            ext.insert("tool_use_id".to_owned(), Value::String(id.to_owned()));
            None
        },
        Some,
    )
}

/// The call of the tool `name` with the id `id`.
fn tool_call(id: &str, name: &str, input: Option<&Value>) -> Draft {
    let argv = (name == "Bash")
        .then(|| {
            input
                .and_then(|input| input.get("command"))
                .and_then(Value::as_str)
        })
        .flatten()
        // BORROW: .to_owned() makes the owned command line the event keeps
        .map(|command| vec![command.to_owned()]);
    // BORROW: .to_owned() makes the owned name the event keeps
    let mut draft = Draft::new(EventKind::tool_call(
        name.to_owned(),
        origin_of(name),
        argv,
        Vec::new(),
    ));
    draft.span = span_of(id, &mut draft.ext);
    if let Some(input) = input {
        // BORROW: .to_owned() and .clone() make the owned key and input the map keeps
        draft.ext.insert("input".to_owned(), input.clone());
    }
    draft
}

/// Where the tool `name` is served from: `mcp__<server>__<tool>` is the tool `<tool>` of the MCP server `<server>`.
fn origin_of(name: &str) -> ToolOrigin {
    match name
        .strip_prefix("mcp__")
        .and_then(|rest| rest.split_once("__"))
    {
        // BORROW: .to_owned() makes the owned server name the origin keeps
        Some((server, tool)) if !server.is_empty() && !tool.is_empty() => {
            ToolOrigin::mcp(server.to_owned())
        }
        Some(_) | None => ToolOrigin::Builtin,
    }
}
