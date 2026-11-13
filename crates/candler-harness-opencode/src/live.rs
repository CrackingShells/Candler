//! The reader of the NDJSON on stdout of `opencode run --format json`.
//!
//! The stream is not the trace: a tool call appears only after it completed, and token counts leave
//! out the title agent's requests. The reader emits no event. It reports the session id, which is on
//! the first line (or on the `error` event of a run that fails before its first step), and the
//! stream's last word, so the core can interrupt a run by id, collect its export, and classify the
//! end when the export is lost.

use candler_harness::reader::{FrameIn, ReaderError, TraceReader};
use candler_harness::spec::{Confidence, FormatVersion, Framing, Observed, ObservedEnd};
use candler_schemas::primitives::NameVersion;
use candler_schemas::raw::RawRef;
use candler_schemas::trace::{EndedBy, Session, SessionEvent, SessionId};
use serde_json::Value;

use crate::export::ended_by_of_observed;
use crate::{VALIDATED, session_source};
use candler_schemas::name;
use semver::Version;

/// Reads the live NDJSON for session ids and the last word.
#[non_exhaustive]
pub struct OpenCodeLiveReader {
    identity: NameVersion,
    seen: Vec<(SessionId, RawRef)>,
    end: Option<ObservedEnd>,
}

impl OpenCodeLiveReader {
    /// A reader of the live NDJSON.
    #[must_use]
    pub fn new() -> Self {
        Self {
            identity: NameVersion::of(&name!("opencode-ndjson"), Version::new(0, 1, 0)),
            seen: Vec::new(),
            end: None,
        }
    }
}

impl Default for OpenCodeLiveReader {
    fn default() -> Self {
        Self::new()
    }
}

/// The kinds of event `opencode run --format json` writes, as `opencode_facts` recorded them.
const KINDS: [&str; 6] = [
    "step_start",
    "text",
    "tool_use",
    "step_finish",
    "retry",
    "error",
];

/// The first non-blank line of `sample` as an event of a known kind that names a session.
fn is_event(sample: &[u8]) -> bool {
    sample
        .split(|byte| *byte == b'\n')
        .find(|line| !line.iter().all(u8::is_ascii_whitespace))
        .and_then(|line| serde_json::from_slice::<Value>(line).ok())
        .is_some_and(|value| {
            value
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| KINDS.contains(&kind))
                && value.get("sessionID").is_some_and(Value::is_string)
        })
}

impl TraceReader for OpenCodeLiveReader {
    fn id(&self) -> NameVersion {
        self.identity.clone() // BORROW: .clone() copies the reader identity
    }

    fn framing(&self) -> Framing {
        Framing::Lines
    }

    fn detect(&self, sample: &[u8]) -> Option<(FormatVersion, Confidence)> {
        is_event(sample).then(|| (FormatVersion::new(VALIDATED), Confidence::Likely))
    }

    fn feed(&mut self, frame: FrameIn<'_>) -> Result<Vec<SessionEvent>, ReaderError> {
        let bytes = frame.frame.data.to_bytes();
        let Ok(Value::Object(line)) = serde_json::from_slice::<Value>(&bytes) else {
            // A blank or foreign line says nothing about the run; the raw frame keeps it.
            return Ok(Vec::new());
        };
        if let Some(text) = line.get("sessionID").and_then(Value::as_str) {
            let id: SessionId = text.parse().map_err(|_| ReaderError::Frame {
                frame: frame.raw_ref.frame,
                detail: "`sessionID` is not a session id".to_owned(), // BORROW: .to_owned() makes the owned text
            })?;
            if !self.seen.iter().any(|(known, _)| *known == id) {
                self.seen.push((id, frame.raw_ref.clone())); // BORROW: .clone() copies the place
            }
        }
        match line.get("type").and_then(Value::as_str) {
            Some("error") => {
                let kind = line
                    .get("error")
                    .and_then(|error| error.get("type"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                self.end = Some(if kind == "aborted" {
                    ObservedEnd::Aborted
                } else {
                    ObservedEnd::Error {
                        kind: kind.to_owned(), // BORROW: .to_owned() makes owned text
                    }
                });
            }
            Some("step_finish") => {
                let stop = line
                    .get("part")
                    .and_then(|part| part.get("reason"))
                    .and_then(Value::as_str)
                    == Some("stop");
                if stop && self.end.is_none() {
                    self.end = Some(ObservedEnd::Finished);
                }
            }
            // EXPLICIT: any other event says nothing about the end
            Some(_) | None => {}
        }
        Ok(Vec::new())
    }

    fn observed(&self) -> Observed {
        // BORROW: .clone() copies the ids out of the reader
        let observed = Observed::new(self.seen.iter().map(|(id, _)| id.clone()).collect());
        match &self.end {
            Some(end) => observed.with_end(end.clone()), // BORROW: .clone() copies the end
            None => observed,
        }
    }

    fn finish(self: Box<Self>) -> Result<Vec<Session>, ReaderError> {
        if self.seen.is_empty() {
            return Err(ReaderError::Incomplete {
                detail: "the stream names no session".to_owned(), // BORROW: .to_owned() makes the owned text
            });
        }
        let ended_by = self
            .end
            .as_ref()
            .map_or(EndedBy::Unknown, ended_by_of_observed);
        let source = session_source(&self.identity)?;
        self.seen
            .into_iter()
            .map(|(id, raw_ref)| {
                Session::builder()
                    .id(id)
                    .source(source.clone()) // BORROW: .clone() gives each session its source
                    .ended_by(ended_by.clone()) // BORROW: .clone() gives each session its end
                    .raw_ref(raw_ref)
                    .build()
                    .map_err(ReaderError::from)
            })
            .collect()
    }
}
