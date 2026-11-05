//! Candler harness for Claude Code: a [`TraceReader`] for the stream-json output of Claude Code.
//!
//! [`ClaudeCodeReader`] reads the frames of one `agent_stream.jsonl` and turns them into the
//! sessions and events of the normalised trace. [`detect`] recognises a supported stream from its
//! first line.

pub mod detect;
mod parse;

use std::sync::LazyLock;

use candler_harness::reader::{FrameIn, ReaderError, TraceReader};
use candler_harness::spec::{Confidence, FormatVersion, Framing, Observed};
use candler_schemas::primitives::{Name, NameVersion, Timestamp};
use candler_schemas::raw::RawRef;
use candler_schemas::trace::{EndedBy, Event, Session, SessionEvent, SessionId, SessionSource};

use crate::parse::Draft;

/// The Claude Code versions the reader supports, as a `SemVer` requirement: `>=2.1.220, <2.2.0`.
pub const SUPPORTED: &str = ">=2.1.220, <2.2.0";

/// The name of the harness that produced the streams the reader reads.
const HARNESS: &str = "claude-code";

/// The identity of this reader: `claude-code-stream-json@<reader version>`, whose version changes whenever the reader's output does.
#[allow(clippy::expect_used)] // EXPLICIT: the literal is a valid `name@version`, which no input can change
pub static READER: LazyLock<NameVersion> = LazyLock::new(|| {
    "claude-code-stream-json@0.1.0"
        .parse()
        .expect("the reader identity is a valid name@version")
});

/// An event that arrived before any session was known.
struct Pending {
    /// The frame of the event.
    raw_ref: RawRef,
    /// The time of the event.
    at: Option<Timestamp>,
    /// The event.
    draft: Draft,
}

/// A root session: one `session_id` of the stream.
struct Root {
    /// The id of the session.
    id: SessionId,
    /// The `seq` of its next event.
    next_seq: u64,
    /// The first frame that names the session.
    raw_ref: RawRef,
    /// The model the session ran, once a line names it.
    model: Option<String>,
}

/// Reads Claude Code stream-json frames into sessions and events.
#[derive(Default)]
#[non_exhaustive]
pub struct ClaudeCodeReader {
    /// The root sessions, in the order they appeared.
    roots: Vec<Root>,
    /// The root session of the latest line that named one.
    current: Option<usize>,
    /// The Claude Code version of the first init line.
    harness_version: Option<semver::Version>,
    /// Events that arrived before any session was known.
    pending: Vec<Pending>,
}

impl ClaudeCodeReader {
    /// A reader that has seen no frame.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The index of the root session `text`, which a frame at `raw_ref` names; the session is added when it is new.
    fn root_index(&mut self, text: &str, raw_ref: &RawRef) -> Result<usize, ReaderError> {
        let id = text.parse::<SessionId>()?;
        if let Some(index) = self.roots.iter().position(|root| root.id == id) {
            return Ok(index);
        }
        self.roots.push(Root {
            id,
            next_seq: 0,
            // BORROW: .clone() keeps the frame of the first mention in the session
            raw_ref: raw_ref.clone(),
            model: None,
        });
        Ok(self.roots.len().saturating_sub(1))
    }
}

/// The event `draft` of `root`, read from the frame `raw_ref` at time `at`.
fn emit(root: &mut Root, raw_ref: &RawRef, at: Option<&Timestamp>, draft: Draft) -> SessionEvent {
    let seq = root.next_seq;
    root.next_seq = seq.saturating_add(1);
    // BORROW: .clone() keeps the frame of the event
    let mut event = Event::new(seq, draft.kind, raw_ref.clone()).with_ext(draft.ext);
    if let Some(span) = draft.span {
        event = event.with_span(span);
    }
    if let Some(at) = at {
        // BORROW: .clone() keeps the time of the event
        event = event.with_at(at.clone());
    }
    // BORROW: .clone() keeps the id of the session in the record
    SessionEvent::new(root.id.clone(), event)
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
        let parsed = parse::parse(frame.frame);
        let at = parsed.at.or(frame.at);
        if self.harness_version.is_none() {
            self.harness_version = parsed.harness_version;
        }
        let index = match parsed.session_id {
            Some(text) => {
                let index = self.root_index(&text, &frame.raw_ref)?;
                self.current = Some(index);
                Some(index)
            }
            None => self.current,
        };
        let Some(root) = index.and_then(|index| self.roots.get_mut(index)) else {
            self.pending
                .extend(parsed.drafts.into_iter().map(|draft| Pending {
                    // BORROW: .clone() keeps the frame of the event
                    raw_ref: frame.raw_ref.clone(),
                    // BORROW: .clone() keeps the time of the event
                    at: at.clone(),
                    draft,
                }));
            return Ok(Vec::new());
        };
        if root.model.is_none() {
            root.model = parsed.model;
        }
        let mut events: Vec<SessionEvent> = self
            .pending
            .drain(..)
            .map(|held| emit(root, &held.raw_ref, held.at.as_ref(), held.draft))
            .collect();
        events.extend(
            parsed
                .drafts
                .into_iter()
                .map(|draft| emit(root, &frame.raw_ref, at.as_ref(), draft)),
        );
        Ok(events)
    }

    fn observed(&self) -> Observed {
        // BORROW: .clone() copies the ids out of the reader
        Observed::new(self.roots.iter().map(|root| root.id.clone()).collect())
    }

    fn finish(self: Box<Self>) -> Result<Vec<Session>, ReaderError> {
        let incomplete = |detail: &str| ReaderError::Incomplete {
            // BORROW: .to_owned() makes the owned text the error keeps
            detail: detail.to_owned(),
        };
        let version = self
            .harness_version
            .ok_or_else(|| incomplete("the stream has no init line"))?;
        let name = |text: &str| {
            text.parse::<Name>()
                .map_err(|error| incomplete(&error.to_string()))
        };
        let reader = READER.clone(); // BORROW: copies the reader identity out of its static
        let source = SessionSource::new(
            name(HARNESS)?,
            version,
            name(reader.name())?,
            // BORROW: .clone() copies the reader version
            reader.version().clone(),
        );
        self.roots
            .into_iter()
            .map(|root| {
                let mut session = Session::builder()
                    .id(root.id)
                    // BORROW: .clone() gives each session its own source
                    .source(source.clone())
                    .ended_by(EndedBy::Unknown)
                    .raw_ref(root.raw_ref);
                if let Some(model) = root.model {
                    session = session.model(model);
                }
                session.build().map_err(ReaderError::from)
            })
            .collect()
    }
}
