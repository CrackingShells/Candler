//! Candler harness for Claude Code: a [`TraceReader`] for the stream-json output of Claude Code.
//!
//! [`ClaudeCodeReader`] reads the frames of one `agent_stream.jsonl` and turns them into the
//! sessions and events of the normalised trace. [`detect`] recognises a supported stream from its
//! first line.

pub mod detect;
mod parse;
mod sessions;
mod usage;

use std::sync::LazyLock;

use candler_harness::reader::{FrameIn, ReaderError, TraceReader};
use candler_harness::spec::{Confidence, FormatVersion, Framing, Observed};
use candler_schemas::primitives::{Name, NameVersion, Timestamp};
use candler_schemas::raw::RawRef;
use candler_schemas::trace::{Session, SessionEvent, SessionSource, UsageScope};
use serde_json::{Map, Value};

use crate::parse::{Draft, Parsed};
use crate::sessions::Registry;

/// The Claude Code versions the reader supports, as a `SemVer` requirement: `>=2.1.220, <2.2.0`.
pub const SUPPORTED: &str = ">=2.1.220, <2.2.0";

/// The name of the harness that produced the streams the reader reads.
const HARNESS: &str = "claude-code";

/// The identity of this reader: `claude-code-stream-json@<reader version>`, whose version changes whenever the reader's output does.
#[allow(clippy::expect_used)] // EXPLICIT: the literal is a valid `name@version`, which no input can change
pub static READER: LazyLock<NameVersion> = LazyLock::new(|| {
    "claude-code-stream-json@0.2.0"
        .parse()
        .expect("the reader identity is a valid name@version")
});

/// A frame that arrived before any session was known.
struct Held {
    /// The parsed line.
    parsed: Parsed,
    /// The frame of the line.
    raw_ref: RawRef,
    /// The time of the line.
    at: Option<Timestamp>,
}

/// Reads Claude Code stream-json frames into sessions and events.
#[derive(Default)]
#[non_exhaustive]
pub struct ClaudeCodeReader {
    /// The sessions seen so far.
    registry: Registry,
    /// The Claude Code version of the first init line.
    harness_version: Option<semver::Version>,
    /// Frames that arrived before any session was known, replayed when the first session opens.
    pending: Vec<Held>,
}

impl ClaudeCodeReader {
    /// A reader that has seen no frame.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The events of one frame whose session is known.
    fn apply(&mut self, held: Held) -> Result<Vec<SessionEvent>, ReaderError> {
        let Held {
            parsed,
            raw_ref,
            at,
        } = held;
        let Parsed {
            session_id,
            parent,
            message_line,
            at: _,
            harness_version: _,
            model,
            drafts,
            message,
            result,
        } = parsed;
        let registry = &mut self.registry;
        let Some(target) = registry.resolve(
            session_id.as_deref(),
            parent.as_deref(),
            message_line,
            &raw_ref,
        )?
        else {
            return Err(ReaderError::Incomplete {
                // BORROW: .to_owned() makes the owned text the error keeps
                detail: "a frame names no session".to_owned(),
            });
        };
        registry.set_model(target, model)?;
        let mut events = Vec::new();
        for draft in drafts {
            events.push(registry.emit(target, &raw_ref, at.as_ref(), draft)?);
        }
        if let Some(message) = message
            && let (true, Some(tokens)) =
                (registry.first_line_of_message(&message.id), message.tokens)
        {
            let mut ext = Map::new();
            // BORROW: .to_owned() makes the owned key the map keeps
            ext.insert("message_id".to_owned(), Value::String(message.id));
            let draft = Draft::with_ext(tokens.event(UsageScope::Message, None), ext);
            events.push(registry.emit(target, &raw_ref, at.as_ref(), draft)?);
        }
        if let Some(result) = result {
            let draft = if result.reports_usage {
                let total = registry.add_turn(target, result.tokens)?;
                Draft::with_ext(
                    total.event(UsageScope::SessionTotal, result.cost),
                    result.line,
                )
            } else {
                Draft::notification(result.line)
            };
            events.push(registry.emit(target, &raw_ref, at.as_ref(), draft)?);
            registry.set_ended_by(target, result.ended_by)?;
        }
        Ok(events)
    }
}

impl TraceReader for ClaudeCodeReader {
    fn id(&self) -> NameVersion {
        // BORROW: .clone() copies the reader identity out of its static
        READER.clone()
    }

    fn framing(&self) -> Framing {
        Framing::Lines
    }

    fn detect(&self, sample: &[u8]) -> Option<(FormatVersion, Confidence)> {
        detect::detect(sample)
    }

    fn feed(&mut self, frame: FrameIn<'_>) -> Result<Vec<SessionEvent>, ReaderError> {
        let mut parsed = parse::parse(frame.frame);
        let at = parsed.at.take().or(frame.at);
        let version = parsed.harness_version.take();
        if self.harness_version.is_none() {
            self.harness_version = version;
        }
        let held = Held {
            parsed,
            raw_ref: frame.raw_ref,
            at,
        };
        if let Some(id) = held.parsed.session_id.as_deref() {
            let first = self
                .pending
                .first()
                .map_or(&held.raw_ref, |earlier| &earlier.raw_ref);
            self.registry.enter(id, first)?;
        } else if !self.registry.has_session() {
            self.pending.push(held);
            return Ok(Vec::new());
        }
        let mut events = Vec::new();
        for earlier in std::mem::take(&mut self.pending) {
            events.extend(self.apply(earlier)?);
        }
        events.extend(self.apply(held)?);
        Ok(events)
    }

    fn observed(&self) -> Observed {
        Observed::new(self.registry.root_ids())
    }

    fn finish(self: Box<Self>) -> Result<Vec<Session>, ReaderError> {
        let incomplete = |detail: &str| ReaderError::Incomplete {
            // BORROW: .to_owned() makes the owned text the error keeps
            detail: detail.to_owned(),
        };
        if !self.pending.is_empty() {
            return Err(incomplete("the stream has frames before any session"));
        }
        let version = self
            .harness_version
            .ok_or_else(|| incomplete("the stream has no init line"))?;
        let name = |text: &str| {
            text.parse::<Name>()
                .map_err(|error| incomplete(&error.to_string())) // BORROW: .to_string() gives owned text
        };
        let reader = READER.clone(); // BORROW: copies the reader identity out of its static
        let source = SessionSource::new(
            name(HARNESS)?,
            version,
            name(reader.name())?,
            // BORROW: .clone() copies the reader version
            reader.version().clone(),
        );
        self.registry.into_sessions(&source)
    }
}
