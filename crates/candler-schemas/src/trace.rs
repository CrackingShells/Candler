//! The normalised trace (`normalised-trace/1`): sessions, events and the trace file.
//!
//! An optional field is omitted when absent and never written as `null`; the reader refuses an
//! explicit `null` there, while `ext` and the body of an unknown event kind are open content that
//! keeps any `null` inside it. Unknown fields are ignored on input, and every struct and every
//! tagged enum is read from an object only, never from a list.

use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::generate::GeneratedSchema;
use crate::map_only::MapOnly;
use crate::primitives::{
    NAME_PATTERN, ReasonCode, SEMVER_PATTERN, Timestamp, object_only, present, text_type,
};
use crate::raw::RawRef;

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
        concat!(r"^[^#\t\n\v\f\r\x20\u0085   -     　﻿]+", r"(?![\s\S])")
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

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    Vec::new()
}
