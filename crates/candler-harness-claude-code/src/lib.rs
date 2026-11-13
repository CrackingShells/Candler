//! Candler harness for Claude Code: a [`TraceReader`] for the stream-json output of Claude Code.
//!
//! [`ClaudeCodeReader`] reads the frames of one `agent_stream.jsonl` and turns them into the
//! sessions and events of the normalised trace. [`detect`] recognises a supported stream from its
//! first line.

#![deny(clippy::allow_attributes)]

pub mod detect;
pub mod discovery;
pub mod driver;
mod parse;
pub mod prepare;
pub mod session;
mod sessions;
mod usage;

use candler_harness::reader::{FrameIn, ReaderError, TraceReader};
use candler_harness::spec::{Confidence, FormatVersion, Framing, InitReport, Observed, RunTotals};
use candler_schemas::name;
use candler_schemas::primitives::{Name, NameVersion, Timestamp};
use candler_schemas::raw::RawRef;
use candler_schemas::trace::{Session, SessionEvent, SessionSource, UsageScope};
use serde_json::{Map, Value};

use crate::parse::{Draft, Parsed};
use crate::sessions::Registry;

/// The Claude Code versions the reader supports, as a `SemVer` requirement: `>=2.1.220, <2.2.0`.
pub const SUPPORTED: &str = ">=2.1.220, <2.2.0";

/// The Claude Code versions the reader supports, as a requirement: [`SUPPORTED`] written out, so
/// no text has to parse.
#[must_use]
pub fn supported() -> semver::VersionReq {
    let at = |op, minor, patch| semver::Comparator {
        op,
        major: 2,
        minor: Some(minor),
        patch: Some(patch),
        pre: semver::Prerelease::EMPTY,
    };
    semver::VersionReq {
        comparators: vec![
            at(semver::Op::GreaterEq, 1, 220),
            at(semver::Op::Less, 2, 0),
        ],
    }
}

/// The Claude Code version the base image installs, which [`supported`] accepts.
pub const PINNED: semver::Version = semver::Version::new(2, 1, 311);

/// The name of the harness that produced the streams the reader reads.
const HARNESS: &str = "claude-code";

/// The identity of this reader as text, `claude-code-stream-json@<reader version>`; the version changes whenever the reader's output does.
pub const READER: &str = "claude-code-stream-json@0.2.0";

/// The version of [`READER`].
const READER_VERSION: semver::Version = semver::Version::new(0, 2, 0);

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
#[non_exhaustive]
pub struct ClaudeCodeReader {
    /// The identity of the reader, parsed from [`READER`].
    identity: NameVersion,
    /// The sessions seen so far.
    registry: Registry,
    /// The Claude Code version of the first init line.
    harness_version: Option<semver::Version>,
    /// The tools and MCP servers of the first init line that lists tools.
    init: Option<InitReport>,
    /// What the last `result` line reported: usage, cost and turns.
    totals: RunTotals,
    /// Frames that arrived before any session was known, replayed when the first session opens.
    pending: Vec<Held>,
}

impl Default for ClaudeCodeReader {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeCodeReader {
    /// A reader that has seen no frame.
    #[must_use]
    pub fn new() -> Self {
        Self {
            identity: NameVersion::of(&name!("claude-code-stream-json"), READER_VERSION),
            registry: Registry::default(),
            harness_version: None,
            init: None,
            totals: RunTotals::default(),
            pending: Vec::new(),
        }
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
            init: _,
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
            self.totals = totals_of(&result.line);
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

/// What a `result` line reports: its `usage`, `total_cost_usd` and `num_turns`, each as written.
fn totals_of(line: &Map<String, Value>) -> RunTotals {
    // BORROW: .cloned() gives the totals their own copy of each value
    RunTotals::new(
        line.get("usage").cloned(),
        line.get("total_cost_usd").cloned(),
        line.get("num_turns").cloned(),
    )
}

impl TraceReader for ClaudeCodeReader {
    fn id(&self) -> NameVersion {
        // BORROW: .clone() copies the reader identity out of the reader
        self.identity.clone()
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
        let init = parsed.init.take();
        if self.init.is_none() {
            self.init = init;
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
        // BORROW: .clone() copies the totals out of the reader
        let observed = Observed::new(self.registry.root_ids()).with_totals(self.totals.clone());
        match &self.init {
            // BORROW: .clone() copies the inventory out of the reader
            Some(init) => observed.with_init(init.clone()),
            None => observed,
        }
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
        let reader = self.identity;
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
