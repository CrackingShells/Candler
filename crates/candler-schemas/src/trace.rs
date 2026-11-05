//! The normalised trace (`normalised-trace/1`): sessions, events and the trace file.
//!
//! An optional field is omitted when absent and never written as `null`; the reader refuses an
//! explicit `null` there, while `ext` and the body of an unknown event kind are open content that
//! keeps any `null` inside it. Unknown fields are ignored on input, and every struct and every
//! tagged enum is read from an object only, never from a list.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::de::Error as _;
use serde::ser::SerializeMap as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::generate::GeneratedSchema;
use crate::map_only::MapOnly;
use crate::outcome::Finite;
use crate::primitives::{
    Digest, NAME_PATTERN, NameVersion, ReasonCode, SEMVER_PATTERN, Timestamp, object_only, present,
    schema_tag, text_type,
};
use crate::raw::{RawRef, StreamName};

/// The version marker of a trace file, written in its header.
pub const NORMALISED_TRACE_SCHEMA: &str = "normalised-trace/1";

/// Why a trace record or one of its members is not valid.
///
/// Each variant names what is wrong, and carries the session or text that is at fault.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TraceError {
    /// A [`SessionId`] that is empty or holds `#` or white space.
    #[error("session id {problem} (got {value:?})")]
    SessionId {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// A [`SpanId`] that is empty or holds `#` or white space.
    #[error("span id {problem} (got {value:?})")]
    SpanId {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// A [`SourceName`] that is empty or holds `@` or white space.
    #[error("source name {problem} (got {value:?})")]
    SourceName {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// A harness or reader version that is not `SemVer` 2.0.
    #[error("source version is not SemVer 2.0 (got {value:?})")]
    SourceVersion {
        /// The text that was parsed.
        value: String,
    },
    /// A kind given to [`EventKind::unknown`] that this version knows, or a body that has a `kind` key.
    #[error("unknown event kind {kind:?} {problem}")]
    UnknownKind {
        /// What is wrong with the kind.
        problem: &'static str,
        /// The kind that was given.
        kind: String,
    },
    /// A line of a trace file that is not a valid record.
    #[error("line {line} is not a valid record: {detail}")]
    Line {
        /// The line, counting from 1.
        line: usize,
        /// What the record reader reported.
        detail: String,
    },
    /// A trace file with no header, or a record before the header.
    #[error("the trace file has no header before its first session or event")]
    MissingHeader,
    /// A second header record.
    #[error("line {line} is a second header")]
    SecondHeader {
        /// The line, counting from 1.
        line: usize,
    },
    /// A session record after an event record.
    #[error("session {session:?} is declared after an event")]
    SessionAfterEvent {
        /// The session that is declared late.
        session: String,
    },
    /// A session declared twice.
    #[error("session {session:?} is declared twice")]
    DuplicateSession {
        /// The session that is declared again.
        session: String,
    },
    /// A child session declared before its parent.
    #[error("session {session:?} is declared before its parent {parent:?}")]
    ChildBeforeParent {
        /// The child session.
        session: String,
        /// The parent it names.
        parent: String,
    },
    /// An event of a session that no session record declares.
    #[error("event seq {seq} belongs to session {session:?}, which is not declared")]
    UndeclaredSession {
        /// The session the event names.
        session: String,
        /// The event's `seq`.
        seq: u64,
    },
    /// An event whose `seq` does not follow its predecessor in its session.
    #[error("session {session:?} has an event with seq {found} where seq {expected} is next")]
    SeqOrder {
        /// The session of the event.
        session: String,
        /// The `seq` that is next in the session.
        expected: u64,
        /// The `seq` that was found.
        found: u64,
    },
    /// A `raw_ref` whose stream the header's `raw` does not list.
    #[error(
        "session {session:?} (seq {seq:?}) points at stream {stream:?}, which the header does not list"
    )]
    StreamNotListed {
        /// The session of the record.
        session: String,
        /// The event's `seq`, or `None` for the session record itself.
        seq: Option<u64>,
        /// The stream that is not listed.
        stream: String,
    },
    /// A session whose `parent_session` is not the id of its parent followed by `/` and a span.
    #[error("session {session:?} is not `<parent>/<span>` of its parent session {parent:?}")]
    ParentNotPrefix {
        /// The session that is at fault.
        session: String,
        /// The parent session it names.
        parent: String,
    },
    /// A session builder that was asked to build before a required field was set.
    #[error("session field is missing (`{field}` was not set)")]
    MissingField {
        /// The name of the field.
        field: &'static str,
    },
}

/// Reports whether `c` is white space or the byte-order mark.
const fn is_blank(c: char) -> bool {
    c.is_whitespace() || c == '\u{FEFF}'
}

fn session_id_problem(text: &str) -> Result<(), &'static str> {
    if text.is_empty() {
        Err("is empty")
    } else if text.chars().any(|c| c == '#' || is_blank(c)) {
        Err("holds `#` or white space")
    } else {
        Ok(())
    }
}

fn source_name_problem(text: &str) -> Result<(), &'static str> {
    if text.is_empty() {
        Err("is empty")
    } else if text.chars().any(|c| c == '@' || is_blank(c)) {
        Err("holds `@` or white space")
    } else {
        Ok(())
    }
}

/// The pattern of a session or span id: non-empty, with neither `#` nor white space.
macro_rules! id_pattern {
    () => {
        concat!(
            r"^[^#\t\n\v\f\r\x20\u0085\u00A0\u1680\u2000-\u200A\u2028\u2029\u202F\u205F\u3000\uFEFF]+",
            r"(?![\s\S])"
        )
    };
}

text_type!(SessionId {
    description: "The id of a session: non-empty, with neither `#` nor white space, so that it reads as the session part of an `event:<session-id>#<seq>` evidence reference. A child session's id is its parent's id, `/`, and the id of the call that started it.",
    pattern: id_pattern!(),
});

impl FromStr for SessionId {
    type Err = TraceError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match session_id_problem(text) {
            // BORROW: .to_owned() makes the owned text the value keeps
            Ok(()) => Ok(Self {
                text: text.to_owned(),
            }),
            // BORROW: .to_owned() makes the owned text the error keeps
            Err(problem) => Err(TraceError::SessionId {
                problem,
                value: text.to_owned(),
            }),
        }
    }
}

impl SessionId {
    /// The id of the child session that the call `span` of `parent` started: `<parent>/<span>`.
    #[must_use]
    pub fn child(parent: &Self, span: &SpanId) -> Self {
        Self {
            // BORROW: explicit .as_str() instead of Deref coercion
            text: format!("{}/{}", parent.as_str(), span.as_str()),
        }
    }

    /// Reports whether this id is `<parent>/<span>` for some span.
    #[must_use]
    pub fn is_child_of(&self, parent: &Self) -> bool {
        // BORROW: explicit .as_str() instead of Deref coercion
        self.as_str()
            .strip_prefix(parent.as_str())
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some_and(|span| !span.is_empty())
    }
}

text_type!(SpanId {
    description: "The id of a harness call, which pairs a call with its result and names the child session the call started: non-empty, with neither `#` nor white space.",
    pattern: id_pattern!(),
});

impl FromStr for SpanId {
    type Err = TraceError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match session_id_problem(text) {
            // BORROW: .to_owned() makes the owned text the value keeps
            Ok(()) => Ok(Self {
                text: text.to_owned(),
            }),
            // BORROW: .to_owned() makes the owned text the error keeps
            Err(problem) => Err(TraceError::SpanId {
                problem,
                value: text.to_owned(),
            }),
        }
    }
}

text_type!(SourceName {
    description: "The name of a harness or a reader: non-empty, with neither `@` nor white space.",
    pattern: NAME_PATTERN,
});

impl FromStr for SourceName {
    type Err = TraceError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match source_name_problem(text) {
            // BORROW: .to_owned() makes the owned text the value keeps
            Ok(()) => Ok(Self {
                text: text.to_owned(),
            }),
            // BORROW: .to_owned() makes the owned text the error keeps
            Err(problem) => Err(TraceError::SourceName {
                problem,
                value: text.to_owned(),
            }),
        }
    }
}

const VERSION_COMMENT: &str =
    "The major, minor and patch numbers each fit in 64 bits; the Rust reader enforces it.";

/// The harness and the reader that produced a session, written as an object with the names and `SemVer` versions of both.
#[derive(Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "SessionSource", extend("$comment" = VERSION_COMMENT))]
struct RawSessionSource {
    /// The name of the harness that ran the session.
    harness: SourceName,
    /// The harness version, a `SemVer` 2.0 string.
    #[schemars(regex(pattern = SEMVER_PATTERN))]
    harness_version: String,
    /// The name of the reader that normalised the session.
    reader: SourceName,
    /// The reader version, a `SemVer` 2.0 string.
    #[schemars(regex(pattern = SEMVER_PATTERN))]
    reader_version: String,
}

impl TryFrom<MapOnly<RawSessionSource>> for SessionSource {
    type Error = TraceError;

    fn try_from(raw: MapOnly<RawSessionSource>) -> Result<Self, Self::Error> {
        let raw = raw.0;
        Ok(Self::new(
            raw.harness,
            // BORROW: explicit .as_str() instead of Deref coercion
            parse_version(raw.harness_version.as_str())?,
            raw.reader,
            // BORROW: explicit .as_str() instead of Deref coercion
            parse_version(raw.reader_version.as_str())?,
        ))
    }
}

impl From<SessionSource> for RawSessionSource {
    fn from(source: SessionSource) -> Self {
        Self {
            harness: source.harness,
            harness_version: source.harness_version.to_string(),
            reader: source.reader,
            reader_version: source.reader_version.to_string(),
        }
    }
}

fn parse_version(text: &str) -> Result<semver::Version, TraceError> {
    semver::Version::parse(text).map_err(|_| TraceError::SourceVersion {
        // BORROW: .to_owned() makes the owned text the error keeps
        value: text.to_owned(),
    })
}

/// The harness and the reader that produced a session, written as an object with the names and `SemVer` versions of both.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawSessionSource>", into = "RawSessionSource")]
#[non_exhaustive]
pub struct SessionSource {
    harness: SourceName,
    harness_version: semver::Version,
    reader: SourceName,
    reader_version: semver::Version,
}

impl SessionSource {
    /// Builds a source.
    #[must_use]
    pub const fn new(
        harness: SourceName,
        harness_version: semver::Version,
        reader: SourceName,
        reader_version: semver::Version,
    ) -> Self {
        Self {
            harness,
            harness_version,
            reader,
            reader_version,
        }
    }

    /// The name of the harness.
    #[must_use]
    pub const fn harness<'a>(&'a self) -> &'a SourceName {
        &self.harness
    }

    /// The version of the harness.
    #[must_use]
    pub const fn harness_version<'a>(&'a self) -> &'a semver::Version {
        &self.harness_version
    }

    /// The name of the reader.
    #[must_use]
    pub const fn reader<'a>(&'a self) -> &'a SourceName {
        &self.reader
    }

    /// The version of the reader.
    #[must_use]
    pub const fn reader_version<'a>(&'a self) -> &'a semver::Version {
        &self.reader_version
    }
}

/// Which per-run cap ended a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Cap {
    /// The cap on conversation turns.
    Turns,
    /// The cap on cost.
    Cost,
    /// The cap on wall-clock time.
    Wall,
}

/// How a session ended: an object whose `kind` is `completed`, `agent_error`, `cap`, `gateway_error`, `budget_stop`, `killed`, `interrupted` or `unknown`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum EndedBy {
    /// The agent finished its work.
    Completed,
    /// The agent reported an error, optionally with a reason code under `reason`.
    #[non_exhaustive]
    AgentError {
        /// Why the agent failed, when it says.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        reason: Option<ReasonCode>,
    },
    /// A per-run cap stopped the session; the key `cap` names it.
    #[non_exhaustive]
    Cap {
        /// The cap that was reached.
        cap: Cap,
    },
    /// The gateway or the provider failed.
    GatewayError,
    /// The job budget stopped the session.
    BudgetStop,
    /// The session was killed.
    Killed,
    /// The supervisor was lost while the session ran.
    Interrupted,
    /// The trace does not say how the session ended.
    Unknown,
}

object_only!(EndedBy);

impl EndedBy {
    /// An agent error, with its reason if it gave one.
    #[must_use]
    pub const fn agent_error(reason: Option<ReasonCode>) -> Self {
        Self::AgentError { reason }
    }

    /// An end caused by a per-run cap.
    #[must_use]
    pub const fn cap(cap: Cap) -> Self {
        Self::Cap { cap }
    }
}

/// One session of a run: a conversation of one agent, with its source, its end and the frame it starts at; when it has a parent session, its id is the parent's id, `/`, and a non-empty call id.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "Session")]
struct RawSession {
    /// The id of the session: non-empty, with neither `#` nor white space; a child's id is its parent's id, `/`, and the id of the call that started it.
    id: SessionId,
    /// The harness and the reader that produced the session.
    source: SessionSource,
    /// The model the session ran, as the harness names it, if it says.
    #[serde(default, deserialize_with = "present")]
    model: Option<String>,
    /// When the session started, if the harness or the frame times say.
    #[serde(default, deserialize_with = "present")]
    started_at: Option<Timestamp>,
    /// When the session ended, if the harness or the frame times say.
    #[serde(default, deserialize_with = "present")]
    ended_at: Option<Timestamp>,
    /// How the session ended.
    ended_by: EndedBy,
    /// The id of the session that started this one; the session's own id is that id, `/`, and a call id.
    #[serde(default, deserialize_with = "present")]
    #[schemars(extend(
        "$comment" = "The session id is this id followed by `/` and a non-empty call id; JSON Schema cannot state this, and the Rust reader enforces it."
    ))]
    parent_session: Option<SessionId>,
    /// The frame the session starts at.
    raw_ref: RawRef,
}

impl TryFrom<MapOnly<RawSession>> for Session {
    type Error = TraceError;

    fn try_from(raw: MapOnly<RawSession>) -> Result<Self, Self::Error> {
        let raw = raw.0;
        Self::checked(Self {
            id: raw.id,
            source: raw.source,
            model: raw.model,
            started_at: raw.started_at,
            ended_at: raw.ended_at,
            ended_by: raw.ended_by,
            parent_session: raw.parent_session,
            raw_ref: raw.raw_ref,
        })
    }
}

/// One session of a run: a conversation of one agent, with its source, its end and the frame it starts at; when it has a parent session, its id is the parent's id, `/`, and a non-empty call id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawSession>")]
#[non_exhaustive]
#[allow(clippy::struct_field_names)] // WIRE: `parent_session` is the field name of the record
pub struct Session {
    id: SessionId,
    source: SessionSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ended_at: Option<Timestamp>,
    ended_by: EndedBy,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_session: Option<SessionId>,
    raw_ref: RawRef,
}

impl Session {
    /// Starts a builder; `id`, `source`, `ended_by` and `raw_ref` must be set before it builds.
    #[must_use]
    pub fn builder() -> SessionBuilder {
        SessionBuilder::default()
    }

    /// Checks the rule that relates `parent_session` to the id.
    fn checked(session: Self) -> Result<Self, TraceError> {
        match &session.parent_session {
            Some(parent) if !session.id.is_child_of(parent) => Err(TraceError::ParentNotPrefix {
                session: session.id.to_string(),
                parent: parent.to_string(),
            }),
            Some(_) | None => Ok(session),
        }
    }

    /// The id of the session.
    #[must_use]
    pub const fn id<'a>(&'a self) -> &'a SessionId {
        &self.id
    }

    /// The harness and the reader that produced the session.
    #[must_use]
    pub const fn source<'a>(&'a self) -> &'a SessionSource {
        &self.source
    }

    /// The model the session ran, if the trace says.
    #[must_use]
    pub fn model<'a>(&'a self) -> Option<&'a str> {
        // BORROW: explicit .as_deref() instead of Deref coercion
        self.model.as_deref()
    }

    /// When the session started, if known.
    #[must_use]
    pub const fn started_at<'a>(&'a self) -> Option<&'a Timestamp> {
        self.started_at.as_ref()
    }

    /// When the session ended, if known.
    #[must_use]
    pub const fn ended_at<'a>(&'a self) -> Option<&'a Timestamp> {
        self.ended_at.as_ref()
    }

    /// How the session ended.
    #[must_use]
    pub const fn ended_by<'a>(&'a self) -> &'a EndedBy {
        &self.ended_by
    }

    /// The session that started this one, if it is a child.
    #[must_use]
    pub const fn parent_session<'a>(&'a self) -> Option<&'a SessionId> {
        self.parent_session.as_ref()
    }

    /// The frame the session starts at.
    #[must_use]
    pub const fn raw_ref<'a>(&'a self) -> &'a RawRef {
        &self.raw_ref
    }
}

/// Builds a [`Session`].
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct SessionBuilder {
    id: Option<SessionId>,
    source: Option<SessionSource>,
    model: Option<String>,
    started_at: Option<Timestamp>,
    ended_at: Option<Timestamp>,
    ended_by: Option<EndedBy>,
    parent_session: Option<SessionId>,
    raw_ref: Option<RawRef>,
}

impl SessionBuilder {
    /// Sets the id of the session.
    #[must_use]
    pub fn id(mut self, id: SessionId) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the harness and the reader that produced the session.
    #[must_use]
    pub fn source(mut self, source: SessionSource) -> Self {
        self.source = Some(source);
        self
    }

    /// Sets the model.
    #[must_use]
    pub fn model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    /// Sets when the session started.
    #[must_use]
    pub fn started_at(mut self, at: Timestamp) -> Self {
        self.started_at = Some(at);
        self
    }

    /// Sets when the session ended.
    #[must_use]
    pub fn ended_at(mut self, at: Timestamp) -> Self {
        self.ended_at = Some(at);
        self
    }

    /// Sets how the session ended.
    #[must_use]
    pub fn ended_by(mut self, ended_by: EndedBy) -> Self {
        self.ended_by = Some(ended_by);
        self
    }

    /// Sets the parent session.
    #[must_use]
    pub fn parent_session(mut self, parent: SessionId) -> Self {
        self.parent_session = Some(parent);
        self
    }

    /// Sets the frame the session starts at.
    #[must_use]
    pub fn raw_ref(mut self, raw_ref: RawRef) -> Self {
        self.raw_ref = Some(raw_ref);
        self
    }

    /// Builds the session.
    ///
    /// # Errors
    /// Returns [`TraceError::MissingField`] if `id`, `source`, `ended_by` or `raw_ref` was not set.
    /// Returns [`TraceError::ParentNotPrefix`] if the parent session is not the prefix of the id,
    /// that is, if the id is not the parent's id, `/` and a non-empty call id.
    pub fn build(self) -> Result<Session, TraceError> {
        let missing = |field| TraceError::MissingField { field };
        Session::checked(Session {
            id: self.id.ok_or_else(|| missing("id"))?,
            source: self.source.ok_or_else(|| missing("source"))?,
            model: self.model,
            started_at: self.started_at,
            ended_at: self.ended_at,
            ended_by: self.ended_by.ok_or_else(|| missing("ended_by"))?,
            parent_session: self.parent_session,
            raw_ref: self.raw_ref.ok_or_else(|| missing("raw_ref"))?,
        })
    }
}

/// Where a tool call is served from: an object whose `kind` is `builtin`, `mcp`, with the `server` that offers the tool, or `other`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum ToolOrigin {
    /// A tool the harness itself provides.
    Builtin,
    /// A tool an MCP server offers.
    #[non_exhaustive]
    Mcp {
        /// The name of the MCP server.
        server: String,
    },
    /// Any other source.
    Other,
}

object_only!(ToolOrigin);

impl ToolOrigin {
    /// An MCP tool of `server`.
    #[must_use]
    pub const fn mcp(server: String) -> Self {
        Self::Mcp { server }
    }
}

/// One MCP call that a bundling tool call contains: the tool it names and where the tool is served from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct WrappedCall {
    /// The name of the wrapped tool.
    pub name: String,
    /// Where the wrapped tool is served from.
    pub origin: ToolOrigin,
}

object_only!(WrappedCall);

impl WrappedCall {
    /// Builds a wrapped call.
    #[must_use]
    pub const fn new(name: String, origin: ToolOrigin) -> Self {
        Self { name, origin }
    }
}

/// Who refused a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DeniedBy {
    /// The harness refused the call by its own rules.
    Harness,
    /// A hook refused the call.
    Hook,
    /// The user refused the call.
    User,
}

/// A refusal of a tool call: who refused it and, when it says, why.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct Denial {
    /// Who refused the call.
    pub by: DeniedBy,
    /// The reason given, if any.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub reason: Option<String>,
}

object_only!(Denial);

impl Denial {
    /// Builds a denial.
    #[must_use]
    pub const fn new(by: DeniedBy, reason: Option<String>) -> Self {
        Self { by, reason }
    }
}

/// What a usage event counts: one message, or the running total of the session, where a later `session_total` of a session supersedes an earlier one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UsageScope {
    /// The usage of one message, reported once, on the first line that carries the message id.
    Message,
    /// The running total of the session so far; a later one supersedes an earlier one.
    SessionTotal,
}

/// What happened at one point of a session.
///
/// A kind this version does not know is read as [`EventKind::Unknown`], which keeps its whole
/// body, so that writing reproduces what was read.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum EventKind {
    /// The user said something.
    #[non_exhaustive]
    UserMessage {
        /// The text, if the harness records it.
        text: Option<String>,
    },
    /// The assistant said something.
    #[non_exhaustive]
    AssistantMessage {
        /// The text, if the harness records it.
        text: Option<String>,
    },
    /// The assistant called a tool.
    #[non_exhaustive]
    ToolCall {
        /// The name of the tool.
        name: String,
        /// Where the tool is served from.
        origin: ToolOrigin,
        /// The command line, if the tool runs one.
        argv: Option<Vec<String>>,
        /// The MCP calls this call bundles; empty when it bundles none.
        wrapped_calls: Vec<WrappedCall>,
    },
    /// A tool call came back.
    #[non_exhaustive]
    ToolResult {
        /// Whether the result is an error.
        is_error: bool,
        /// Who refused the call, when it was refused.
        denied: Option<Denial>,
    },
    /// A line of the stream that is none of the other kinds; its content is in the event's `ext`.
    #[non_exhaustive]
    Notification,
    /// Token counts and cost.
    #[non_exhaustive]
    Usage {
        /// Whether the counts are one message's or the session's running total.
        scope: UsageScope,
        /// Input tokens.
        input_tokens: u64,
        /// Output tokens.
        output_tokens: u64,
        /// Input tokens served from the cache.
        cache_read_tokens: u64,
        /// Input tokens written to the cache.
        cache_creation_tokens: u64,
        /// The cost in US dollars, if the harness reports it.
        cost_usd: Option<Finite>,
    },
    /// A kind this version does not know, with everything else that was written beside `kind`.
    #[non_exhaustive]
    Unknown {
        /// The kind as written.
        kind: String,
        /// The other keys of the object, as written.
        body: Map<String, Value>,
    },
}

/// The kinds this version knows, as written.
const KNOWN_KINDS: [&str; 6] = [
    "user_message",
    "assistant_message",
    "tool_call",
    "tool_result",
    "notification",
    "usage",
];

impl EventKind {
    /// A user message.
    #[must_use]
    pub const fn user_message(text: Option<String>) -> Self {
        Self::UserMessage { text }
    }

    /// An assistant message.
    #[must_use]
    pub const fn assistant_message(text: Option<String>) -> Self {
        Self::AssistantMessage { text }
    }

    /// A tool call.
    #[must_use]
    pub const fn tool_call(
        name: String,
        origin: ToolOrigin,
        argv: Option<Vec<String>>,
        wrapped_calls: Vec<WrappedCall>,
    ) -> Self {
        Self::ToolCall {
            name,
            origin,
            argv,
            wrapped_calls,
        }
    }

    /// A tool result.
    #[must_use]
    pub const fn tool_result(is_error: bool, denied: Option<Denial>) -> Self {
        Self::ToolResult { is_error, denied }
    }

    /// A notification.
    #[must_use]
    pub const fn notification() -> Self {
        Self::Notification
    }

    /// A usage event.
    #[must_use]
    pub const fn usage(
        scope: UsageScope,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_creation_tokens: u64,
        cost_usd: Option<Finite>,
    ) -> Self {
        Self::Usage {
            scope,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            cost_usd,
        }
    }

    /// A kind this version does not know.
    ///
    /// # Errors
    /// Returns [`TraceError::UnknownKind`] if `kind` is a kind this version knows, or if `body`
    /// has a key `kind`.
    pub fn unknown(kind: String, body: Map<String, Value>) -> Result<Self, TraceError> {
        // BORROW: explicit .as_str() instead of Deref coercion
        let problem = if KNOWN_KINDS.contains(&kind.as_str()) {
            Some("is a kind this version knows")
        } else if body.contains_key("kind") {
            Some("has a body with the key `kind`")
        } else {
            None
        };
        match problem {
            Some(problem) => Err(TraceError::UnknownKind { problem, kind }),
            None => Ok(Self::Unknown { kind, body }),
        }
    }
}

/// What happened at one point of a session: an object whose `kind` is `user_message`, `assistant_message`, `tool_call`, `tool_result`, `notification` or `usage`, with the members of that kind beside it.
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(remote = "EventKind", tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields, rename = "EventKind")]
enum KnownKind {
    /// The user said something.
    UserMessage {
        /// The text, if the harness records it.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        text: Option<String>,
    },
    /// The assistant said something.
    AssistantMessage {
        /// The text, if the harness records it.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        text: Option<String>,
    },
    /// The assistant called a tool.
    ToolCall {
        /// The name of the tool.
        name: String,
        /// Where the tool is served from.
        origin: ToolOrigin,
        /// The command line, if the tool runs one.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        argv: Option<Vec<String>>,
        /// The MCP calls this call bundles; an empty array when it bundles none.
        wrapped_calls: Vec<WrappedCall>,
    },
    /// A tool call came back.
    ToolResult {
        /// Whether the result is an error.
        is_error: bool,
        /// Who refused the call, when it was refused.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        denied: Option<Denial>,
    },
    /// A line of the stream that is none of the other kinds; its content is in the event's `ext`.
    Notification,
    /// Token counts and cost; a `session_total` is the running total of the session, so a later one supersedes an earlier one.
    Usage {
        /// Whether the counts are one message's or the session's running total.
        scope: UsageScope,
        /// Input tokens.
        #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
        input_tokens: u64,
        /// Output tokens.
        #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
        output_tokens: u64,
        /// Input tokens served from the cache.
        #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
        cache_read_tokens: u64,
        /// Input tokens written to the cache.
        #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
        cache_creation_tokens: u64,
        /// The cost in US dollars, a finite number, if the harness reports it.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        cost_usd: Option<Finite>,
    },
    /// A kind this version does not know; it is not part of the schema.
    #[serde(skip)]
    Unknown {
        kind: String,
        body: Map<String, Value>,
    },
}

impl Serialize for EventKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Unknown { kind, body } => {
                let mut map = serializer.serialize_map(Some(body.len() + 1))?;
                map.serialize_entry("kind", kind)?;
                for (key, value) in body {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
            Self::UserMessage { .. }
            | Self::AssistantMessage { .. }
            | Self::ToolCall { .. }
            | Self::ToolResult { .. }
            | Self::Notification
            | Self::Usage { .. } => KnownKind::serialize(self, serializer),
        }
    }
}

impl<'de> Deserialize<'de> for EventKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut map = Map::<String, Value>::deserialize(deserializer)?;
        let kind = match map.get("kind") {
            Some(Value::String(kind)) => kind.clone(),
            Some(_) => return Err(D::Error::custom("`kind` is not a string")),
            None => return Err(D::Error::custom("missing field `kind`")),
        };
        // BORROW: explicit .as_str() instead of Deref coercion
        if KNOWN_KINDS.contains(&kind.as_str()) {
            KnownKind::deserialize(Value::Object(map)).map_err(D::Error::custom)
        } else {
            map.remove("kind");
            Ok(Self::Unknown { kind, body: map })
        }
    }
}

impl JsonSchema for EventKind {
    fn schema_name() -> Cow<'static, str> {
        KnownKind::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        KnownKind::schema_id()
    }

    fn inline_schema() -> bool {
        KnownKind::inline_schema()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        KnownKind::json_schema(generator)
    }
}

/// One event of a session: its place in the session, what happened, the frame it came from, and the keys of the line that no field holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct Event {
    /// The position of the event in its session, counting from 0 without gaps.
    #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
    pub seq: u64,
    /// When the event happened, if the harness or the frame time says.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub at: Option<Timestamp>,
    /// What happened.
    pub kind: EventKind,
    /// The harness's id that pairs a call with its result, if the event has one.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub span: Option<SpanId>,
    /// The frame the event was read from.
    pub raw_ref: RawRef,
    /// Data of the harness that no other field holds: an open JSON object, empty when there is none, whose content is open, so a `null` inside it is kept and written back.
    pub ext: Map<String, Value>,
}

object_only!(Event);

impl Event {
    /// Builds an event with no time, no span and an empty `ext`.
    #[must_use]
    pub fn new(seq: u64, kind: EventKind, raw_ref: RawRef) -> Self {
        Self {
            seq,
            at: None,
            kind,
            span: None,
            raw_ref,
            ext: Map::new(),
        }
    }

    /// Sets when the event happened.
    #[must_use]
    pub fn with_at(mut self, at: Timestamp) -> Self {
        self.at = Some(at);
        self
    }

    /// Sets the span.
    #[must_use]
    pub fn with_span(mut self, span: SpanId) -> Self {
        self.span = Some(span);
        self
    }

    /// Sets the harness data.
    #[must_use]
    pub fn with_ext(mut self, ext: Map<String, Value>) -> Self {
        self.ext = ext;
        self
    }
}

/// An event with the session it belongs to: the record a reader's `feed` returns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct SessionEvent {
    /// The id of the session the event belongs to.
    pub session: SessionId,
    /// The event.
    pub event: Event,
}

object_only!(SessionEvent);

impl SessionEvent {
    /// Pairs an event with its session.
    #[must_use]
    pub const fn new(session: SessionId, event: Event) -> Self {
        Self { session, event }
    }
}

/// The streams the raw files of a trace archive, each with the digest of its file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct RawFileRef {
    /// The name of the archived stream.
    pub stream: StreamName,
    /// The digest of the framed raw file of the stream.
    pub digest: Digest,
}

object_only!(RawFileRef);

impl RawFileRef {
    /// Names a raw file by its stream and digest.
    #[must_use]
    pub const fn new(stream: StreamName, digest: Digest) -> Self {
        Self { stream, digest }
    }
}

schema_tag! {
    /// The version marker of a trace file: always `normalised-trace/1`.
    pub NormalisedTraceTag = NORMALISED_TRACE_SCHEMA
}

/// The first line of a trace file: the reader that produced it and the raw files it was read from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct TraceHeader {
    /// The version marker: always `normalised-trace/1`.
    pub schema: NormalisedTraceTag,
    /// The reader and its version, as `name@version` with a `SemVer` version.
    pub reader: NameVersion,
    /// The raw files the trace was read from; every `raw_ref` of the file names one of their streams.
    pub raw: Vec<RawFileRef>,
}

object_only!(TraceHeader);

impl TraceHeader {
    /// Builds a header.
    #[must_use]
    pub const fn new(reader: NameVersion, raw: Vec<RawFileRef>) -> Self {
        Self {
            schema: NormalisedTraceTag,
            reader,
            raw,
        }
    }
}

/// One line of a trace file: an object whose `kind` is `header`, `session` or `event`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum TraceRecord {
    /// The first line of the file.
    Header(TraceHeader),
    /// A session, after the header and before every event.
    Session(Session),
    /// An event with the session it belongs to.
    Event(SessionEvent),
}

object_only!(TraceRecord);

/// A record borrowed for writing; it is written exactly as the [`TraceRecord`] it stands for.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RecordRef<'a> {
    Header(&'a TraceHeader),
    Session(&'a Session),
    Event(&'a SessionEvent),
}

/// A trace file: its header, its sessions with each parent before its children, and its events in feed order.
///
/// A file is valid however it was made: every session id is declared once, a child follows its
/// parent, every event belongs to a declared session and counts its `seq` from 0 without gaps in
/// that session, and every `raw_ref` names a stream of the header.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct TraceFile {
    header: TraceHeader,
    sessions: Vec<Session>,
    events: Vec<SessionEvent>,
}

impl TraceFile {
    /// Builds a file, checking the rules that relate its records.
    ///
    /// # Errors
    /// Returns the [`TraceError`] of the first rule the records break: a repeated session, a child
    /// before its parent, an event for an undeclared session, a `seq` that does not follow its
    /// predecessor in the session, or a `raw_ref.stream` that the header does not list.
    pub fn new(
        header: TraceHeader,
        sessions: Vec<Session>,
        events: Vec<SessionEvent>,
    ) -> Result<Self, TraceError> {
        check_records(&header, &sessions, &events)?;
        Ok(Self {
            header,
            sessions,
            events,
        })
    }

    /// The header.
    #[must_use]
    pub const fn header<'a>(&'a self) -> &'a TraceHeader {
        &self.header
    }

    /// The sessions, root first, each parent before its children.
    #[must_use]
    pub const fn sessions<'a>(&'a self) -> &'a [Session] {
        self.sessions.as_slice()
    }

    /// The events in feed order.
    #[must_use]
    pub const fn events<'a>(&'a self) -> &'a [SessionEvent] {
        self.events.as_slice()
    }

    /// Reads a trace file: one JSON object per line, the header first, then the sessions, then the events.
    ///
    /// # Errors
    /// Returns [`TraceError::MissingHeader`] if a record comes before the header or there is no
    /// header, [`TraceError::SecondHeader`] for a second header,
    /// [`TraceError::SessionAfterEvent`] for a session after an event,
    /// [`TraceError::Line`] for a line that is not a valid record (a child whose id is not
    /// `<parent>/<span>` is such a line), and the errors of [`TraceFile::new`] for the rest.
    pub fn read(bytes: &[u8]) -> Result<Self, TraceError> {
        if bytes.is_empty() {
            return Err(TraceError::MissingHeader);
        }
        let mut header = None;
        let mut sessions = Vec::new();
        let mut events = Vec::new();
        let text = bytes.strip_suffix(b"\n").unwrap_or(bytes);
        for (index, line) in text.split(|byte| *byte == b'\n').enumerate() {
            let number = index + 1;
            let record =
                serde_json::from_slice::<TraceRecord>(line).map_err(|e| TraceError::Line {
                    line: number,
                    detail: e.to_string(),
                })?;
            match record {
                TraceRecord::Header(first) if header.is_none() => header = Some(first),
                TraceRecord::Header(_) => return Err(TraceError::SecondHeader { line: number }),
                TraceRecord::Session(session) if header.is_some() && events.is_empty() => {
                    sessions.push(session);
                }
                TraceRecord::Session(session) if header.is_some() => {
                    return Err(TraceError::SessionAfterEvent {
                        session: session.id().to_string(),
                    });
                }
                TraceRecord::Event(event) if header.is_some() => events.push(event),
                TraceRecord::Session(_) | TraceRecord::Event(_) => {
                    return Err(TraceError::MissingHeader);
                }
            }
        }
        let header = header.ok_or(TraceError::MissingHeader)?;
        Self::new(header, sessions, events)
    }

    /// Writes the file: one compact JSON object per line, fields in declaration order, `\n` after each.
    #[must_use]
    pub fn write(&self) -> Vec<u8> {
        let mut out = Vec::new();
        push_line(&mut out, &RecordRef::Header(&self.header));
        for session in &self.sessions {
            push_line(&mut out, &RecordRef::Session(session));
        }
        for event in &self.events {
            push_line(&mut out, &RecordRef::Event(event));
        }
        out
    }
}

/// Appends `record` and a line feed.
fn push_line<T: Serialize>(out: &mut Vec<u8>, record: &T) {
    // EXPLICIT: serialisation fails only for a map with non-string keys, and these records hold
    // none, so there is no failure to report and `write` keeps returning bytes
    if serde_json::to_writer(&mut *out, record).is_ok() {
        out.push(b'\n');
    }
}

/// Checks the rules that relate the sessions and events of a file to its header and to each other.
fn check_records(
    header: &TraceHeader,
    sessions: &[Session],
    events: &[SessionEvent],
) -> Result<(), TraceError> {
    let listed = |stream: &StreamName| header.raw.iter().any(|raw| raw.stream == *stream);
    let mut next_seq: BTreeMap<&SessionId, u64> = BTreeMap::new();
    for session in sessions {
        let id = session.id();
        if next_seq.contains_key(id) {
            return Err(TraceError::DuplicateSession {
                session: id.to_string(),
            });
        }
        if let Some(parent) = session.parent_session()
            && !next_seq.contains_key(parent)
        {
            return Err(TraceError::ChildBeforeParent {
                session: id.to_string(),
                parent: parent.to_string(),
            });
        }
        if !listed(&session.raw_ref().stream) {
            return Err(TraceError::StreamNotListed {
                session: id.to_string(),
                seq: None,
                stream: session.raw_ref().stream.to_string(),
            });
        }
        next_seq.insert(id, 0);
    }
    for record in events {
        let event = &record.event;
        let Some(expected) = next_seq.get_mut(&record.session) else {
            return Err(TraceError::UndeclaredSession {
                session: record.session.to_string(),
                seq: event.seq,
            });
        };
        if event.seq != *expected {
            return Err(TraceError::SeqOrder {
                session: record.session.to_string(),
                expected: *expected,
                found: event.seq,
            });
        }
        *expected += 1;
        if !listed(&event.raw_ref.stream) {
            return Err(TraceError::StreamNotListed {
                session: record.session.to_string(),
                seq: Some(event.seq),
                stream: event.raw_ref.stream.to_string(),
            });
        }
    }
    Ok(())
}

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    vec![GeneratedSchema::of::<TraceRecord>("normalised-trace")]
}
