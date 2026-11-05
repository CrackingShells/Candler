//! The trace reader: turns the frames of one harness stream into normalised events and sessions.
//!
//! A reader is synchronous and holds no clock. The core cuts the stream into frames, supplies each
//! frame's place and its wall time from the raw header's clock, and calls [`TraceReader::feed`]
//! once per frame.

use candler_schemas::primitives::{NameVersion, Timestamp};
use candler_schemas::raw::{RawFrame, RawRef};
use candler_schemas::trace::{Session, SessionEvent, TraceError};

use crate::spec::{Confidence, FormatVersion, Framing, Observed};

/// Why a reader could not read a frame or finish its sessions.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ReaderError {
    /// A frame the reader cannot parse.
    #[error("frame {frame} cannot be read: {detail}")]
    Frame {
        /// The index of the frame in its stream, from 0.
        frame: u64,
        /// What is wrong with the frame.
        detail: String,
    },
    /// A stream that ended before the reader had what it needs to name its sessions.
    #[error("stream is incomplete: {detail}")]
    Incomplete {
        /// What is missing.
        detail: String,
    },
    /// Sessions that do not form a valid trace.
    #[error(transparent)]
    Trace(#[from] TraceError),
}

/// One frame handed to a reader: its place in the stream, its wall time when the clock gives one, and its content.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct FrameIn<'a> {
    /// The stream and the index of the frame.
    pub raw_ref: RawRef,
    /// The wall time of the frame from the raw header's clock, absent when the clock records none.
    pub at: Option<Timestamp>,
    /// The frame: one line under [`Framing::Lines`], the whole stream under [`Framing::Document`].
    pub frame: &'a RawFrame,
}

impl<'a> FrameIn<'a> {
    /// A frame for a reader.
    #[must_use]
    pub const fn new(raw_ref: RawRef, at: Option<Timestamp>, frame: &'a RawFrame) -> Self {
        Self { raw_ref, at, frame }
    }
}

/// Reads one harness stream into events and sessions; it can exist without a driver.
pub trait TraceReader: Send {
    /// The reader's name and version, written into the trace header. The version is bumped
    /// whenever the reader's output changes.
    fn id(&self) -> NameVersion;

    /// How the reader takes frames.
    fn framing(&self) -> Framing;

    /// Whether `sample`, the start of a stream, is in a format this reader reads, and which
    /// version of it.
    fn detect(&self, sample: &[u8]) -> Option<(FormatVersion, Confidence)>;

    /// Reads one frame and returns the events it holds. A reader fills each event's `raw_ref`
    /// from `frame.raw_ref` and `at` from `frame.at`.
    ///
    /// # Errors
    /// Returns [`ReaderError::Frame`] if the frame cannot be parsed.
    fn feed(&mut self, frame: FrameIn<'_>) -> Result<Vec<SessionEvent>, ReaderError>;

    /// The sessions seen so far.
    fn observed(&self) -> Observed;

    /// Ends the stream and returns its sessions, the root first, each child linked to its parent
    /// by `parent_session`.
    ///
    /// # Errors
    /// Returns [`ReaderError::Incomplete`] if the stream ended without naming a session.
    /// Returns [`ReaderError::Trace`] if the sessions do not form a valid trace.
    fn finish(self: Box<Self>) -> Result<Vec<Session>, ReaderError>;
}
