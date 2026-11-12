//! One stream-json line, read into the events it holds.

use candler_harness::spec::{CensusSource, InitReport};
use candler_schemas::outcome::Finite;
use candler_schemas::primitives::Timestamp;
use candler_schemas::raw::{FrameData, RawFrame};
use candler_schemas::trace::{Denial, DeniedBy, EndedBy, EventKind, SpanId, ToolOrigin};
use serde_json::{Map, Value};

use crate::detect;
use crate::sessions::ended_by_of;
use crate::usage::Tokens;

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

    /// A draft of `kind` holding `ext`.
    pub(crate) const fn with_ext(kind: EventKind, ext: Map<String, Value>) -> Self {
        Self {
            kind,
            span: None,
            ext,
        }
    }

    /// A notification holding `ext`.
    pub(crate) fn notification(ext: Map<String, Value>) -> Self {
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

/// The usage of an assistant message, on the first line that carries the message id.
#[derive(Debug)]
pub(crate) struct MessageLine {
    /// The `message.id`.
    pub(crate) id: String,
    /// The `message.usage`, when the line has one.
    pub(crate) tokens: Option<Tokens>,
}

/// A `result` line.
#[derive(Debug)]
pub(crate) struct ResultLine {
    /// The tokens of the turn the line ends.
    pub(crate) tokens: Tokens,
    /// Whether the line reports usage or a cost.
    pub(crate) reports_usage: bool,
    /// The running cost of the session.
    pub(crate) cost: Option<Finite>,
    /// How the line says the session ended.
    pub(crate) ended_by: EndedBy,
    /// The whole line.
    pub(crate) line: Map<String, Value>,
}

/// What one frame holds, as far as the reader needs it.
#[derive(Debug)]
pub(crate) struct Parsed {
    /// The `session_id` of the line.
    pub(crate) session_id: Option<String>,
    /// The `parent_tool_use_id` of the line.
    pub(crate) parent: Option<String>,
    /// Whether the line is an `assistant` or `user` line, the only lines that start a child session.
    pub(crate) message_line: bool,
    /// The `timestamp` of the line, when it is a valid RFC 3339 time.
    pub(crate) at: Option<Timestamp>,
    /// The Claude Code version, when the line is an init line.
    pub(crate) harness_version: Option<semver::Version>,
    /// The tools and MCP servers the line lists, when it is an init line that lists tools.
    pub(crate) init: Option<InitReport>,
    /// The model the line names.
    pub(crate) model: Option<String>,
    /// The events of the line, in order.
    pub(crate) drafts: Vec<Draft>,
    /// The message id and usage, when the line is an assistant line with a message id.
    pub(crate) message: Option<MessageLine>,
    /// The result, when the line is a `result` line.
    pub(crate) result: Option<ResultLine>,
}

impl Parsed {
    /// A frame that holds no event.
    fn empty() -> Self {
        Self::of(Vec::new())
    }

    /// A frame with no session, time, version, model, message or result that holds `drafts`.
    fn of(drafts: Vec<Draft>) -> Self {
        Self {
            session_id: None,
            parent: None,
            message_line: false,
            at: None,
            harness_version: None,
            init: None,
            model: None,
            drafts,
            message: None,
            result: None,
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
            Value::String(base64.to_string()), // BORROW: .to_string() gives owned text
        )]),
        // A frame form this version does not know is kept as a marker; the frame itself stays in the raw file.
        _ => Parsed::of(vec![Draft::notification_of(
            "unsupported_frame",
            Value::Bool(true),
        )]),
    }
}

/// The inventory an init line lists: its `tools` and, when it has them, its `mcp_servers`, as the
/// line gives them. A line that is not an init line, or lists no tool names, gives none.
fn init_report(line: &Map<String, Value>) -> Option<InitReport> {
    let is_init = line.get("type").and_then(Value::as_str) == Some("system")
        && line.get("subtype").and_then(Value::as_str) == Some("init");
    if !is_init {
        return None;
    }
    let tools = line.get("tools")?;
    InitReport::new(
        tools.clone(), // BORROW: .clone() copies the list out of the line
        // BORROW: .cloned() copies the list out of the line
        line.get("mcp_servers").filter(|v| v.is_array()).cloned(),
        CensusSource::Harness,
    )
    .ok()
}

/// Reads one JSON object line.
fn object_line(line: Map<String, Value>) -> Parsed {
    let text_of = |key: &str| line.get(key).and_then(Value::as_str);
    // BORROW: .to_owned() makes the owned id the result keeps
    let session_id = text_of("session_id").map(str::to_owned);
    let at = text_of("timestamp").and_then(|time| time.parse::<Timestamp>().ok());
    let kind = text_of("type");
    let message = line.get("message");
    let message_id = message
        .and_then(|message| message.get("id"))
        .and_then(Value::as_str);
    // BORROW: .to_owned() makes the owned model the result keeps
    let model = match kind {
        Some("system") if text_of("subtype") == Some("init") => text_of("model").map(str::to_owned),
        Some("assistant") => message
            .and_then(|message| message.get("model"))
            .and_then(Value::as_str)
            .map(str::to_owned), // BORROW: str::to_owned gives owned copy of the text
        _ => None,
    };
    let mut parsed = Parsed {
        session_id,
        at,
        harness_version: detect::init_version(&line),
        init: init_report(&line),
        model,
        ..Parsed::empty()
    };
    // BORROW: .to_owned() makes the owned id the result keeps
    let parent = text_of("parent_tool_use_id").map(str::to_owned);
    match kind {
        Some("assistant") => {
            parsed.drafts = message_drafts(&line, Role::Assistant);
            parsed.message_line = true;
            parsed.parent = parent;
            parsed.message = message_id.map(|id| MessageLine {
                // BORROW: .to_owned() makes the owned id the result keeps
                id: id.to_owned(),
                tokens: message
                    .and_then(|message| message.get("usage"))
                    .map(Tokens::from_usage),
            });
            if let Some(id) = message_id {
                for draft in &mut parsed.drafts {
                    // BORROW: .to_owned() makes the owned key and id the map keeps
                    draft
                        .ext
                        .insert("message_id".to_owned(), Value::String(id.to_owned()));
                }
            }
        }
        Some("user") => {
            parsed.drafts = message_drafts(&line, Role::User);
            parsed.message_line = true;
            parsed.parent = parent;
        }
        Some("result") => parsed.result = Some(result_line(line)),
        _ => {
            parsed.parent = parent;
            parsed.drafts = vec![Draft::notification(line)];
        }
    }
    parsed
}

/// Reads a `result` line.
fn result_line(line: Map<String, Value>) -> ResultLine {
    let usage = line.get("usage");
    let cost = line.get("total_cost_usd");
    ResultLine {
        tokens: usage.map(Tokens::from_usage).unwrap_or_default(),
        reports_usage: usage.is_some() || cost.is_some(),
        cost: cost
            .and_then(Value::as_f64)
            .and_then(|number| Finite::new(number).ok()),
        ended_by: ended_by_of(&line),
        line,
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
            .map(|block| block_draft(role, block, line))
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
fn block_draft(role: Role, block: &Value, line: &Map<String, Value>) -> Draft {
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
                let mut draft = Draft::new(EventKind::tool_result(is_error, denial_of(line, id)));
                draft.span = span_of(id, &mut draft.ext);
                draft
            }
            None => unknown_block(block),
        },
        _ => unknown_block(block),
    }
}

/// The refusal of the call `id`, when the line's `tool_result_meta` says the call did not run: the harness refused it, and the kind it gives is the reason.
fn denial_of(line: &Map<String, Value>, id: &str) -> Option<Denial> {
    line.get("tool_result_meta")
        .and_then(Value::as_array)?
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
        .and_then(|entry| entry.get("non_execution_kind"))
        .and_then(Value::as_str)
        // BORROW: .to_owned() makes the owned reason the denial keeps
        .map(|kind| Denial::new(DeniedBy::Harness, Some(kind.to_owned())))
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
