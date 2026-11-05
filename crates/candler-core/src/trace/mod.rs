//! Normalisation of stored raw frame files into a trace file through a harness trace reader.
//!
//! [`normalise`] opens a framed stream, asks the reader whether it reads the stream's format,
//! feeds it every frame with the frame's place and wall time, and ends the stream. The wall time of
//! a frame is the anchor of a monotonic clock plus the distance of the frame's monotonic time from
//! the anchor; an unrecorded clock gives no time.

use std::str::FromStr as _;

use candler_harness::reader::{FrameIn, ReaderError, TraceReader};
use candler_schemas::primitives::{FormatError, Timestamp};
use candler_schemas::raw::{Clock, RawRef};
use candler_schemas::trace::{RawFileRef, TraceError, TraceFile, TraceHeader};
use jiff::SignedDuration;

use crate::frames::{FrameReader, FramesError, RawFileInfo};

/// How many bytes of the start of a stream the format detection sees at most.
const SAMPLE_BYTES: usize = 64 * 1024;

/// Why a raw frame file could not be normalised into a trace file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum NormaliseError {
    /// The raw frame file cannot be read.
    #[error("raw frame file cannot be normalised: {0}")]
    Frames(#[from] FramesError),
    /// No format the reader reads was recognised at the start of the stream.
    #[error("the reader does not recognise the format of stream `{stream}`")]
    Undetected {
        /// The stream.
        stream: String,
    },
    /// The reader refused a frame.
    #[error("the reader refused frame {frame} of stream `{stream}`: {detail}")]
    Frame {
        /// The stream.
        stream: String,
        /// The index of the frame.
        frame: u64,
        /// The reader's explanation.
        detail: String,
    },
    /// A frame time that names no instant.
    #[error("frame {frame} of stream `{stream}` has no valid wall time: {detail}")]
    Time {
        /// The stream.
        stream: String,
        /// The index of the frame.
        frame: u64,
        /// What is wrong with the time.
        detail: String,
    },
    /// The reader could not end the stream.
    #[error("the reader could not end stream `{stream}`: {detail}")]
    Finish {
        /// The stream.
        stream: String,
        /// The reader's explanation.
        detail: String,
    },
    /// The sessions and events form no valid trace file.
    #[error("the trace read from stream `{stream}` is not a valid trace file: {source}")]
    Trace {
        /// The stream.
        stream: String,
        /// The schema's refusal.
        #[source]
        source: TraceError,
    },
}

/// The wall time of a frame at `t_mono_ns` under `clock`; `None` for an unrecorded clock or a frame without time.
fn wall_time(clock: &Clock, t_mono_ns: Option<u64>) -> Result<Option<Timestamp>, String> {
    let (Clock::Monotonic { anchor, .. }, Some(t)) = (clock, t_mono_ns) else {
        return Ok(None);
    };
    let delta = i128::from(t).saturating_sub(i128::from(anchor.mono_ns));
    let secs = i64::try_from(delta.div_euclid(1_000_000_000)).map_err(|e| e.to_string())?;
    let nanos = i32::try_from(delta.rem_euclid(1_000_000_000)).map_err(|e| e.to_string())?;
    let instant = anchor
        .wall
        .instant()
        .checked_add(SignedDuration::new(secs, nanos))
        .map_err(|e| e.to_string())?;
    // BORROW: explicit .as_str()
    Timestamp::from_str(instant.to_string().as_str())
        .map(Some)
        .map_err(|e: FormatError| e.to_string())
}

/// The first bytes of the stream, whole frames up to [`SAMPLE_BYTES`].
fn sample(path: &std::path::Path) -> Result<Vec<u8>, NormaliseError> {
    let mut bytes = Vec::new();
    for frame in FrameReader::open(path)? {
        bytes.extend(frame?.data.to_bytes());
        if bytes.len() >= SAMPLE_BYTES {
            break;
        }
    }
    Ok(bytes)
}

/// Reads the stored stream `raw` through `reader` into a trace file.
///
/// The format is detected on the first bytes of the stream; a stream the reader does not
/// recognise is an error. Every frame is fed with its place and its wall time, then the stream is
/// ended and the sessions and events are checked as a trace file whose header names the reader and
/// the raw file.
///
/// # Errors
/// Returns [`NormaliseError::Frames`] if the file cannot be read, [`NormaliseError::Undetected`]
/// if the reader does not recognise the format, [`NormaliseError::Frame`] or
/// [`NormaliseError::Time`] naming the frame the reader or the clock refuses,
/// [`NormaliseError::Finish`] if the reader cannot end the stream, and
/// [`NormaliseError::Trace`] if the result is not a valid trace file.
pub fn normalise(
    // TRAIT_OBJECT: the reader's concrete type is chosen by the import's caller
    mut reader: Box<dyn TraceReader>,
    raw: &RawFileInfo,
) -> Result<TraceFile, NormaliseError> {
    // BORROW: explicit .as_path()
    let head = sample(raw.path.as_path())?;
    // BORROW: explicit .as_path()
    let frames = FrameReader::open(raw.path.as_path())?;
    // BORROW: .clone() gives the refs their own name
    let stream = frames.header().stream.clone();
    // BORROW: .clone() releases the borrow of the reader
    let clock = frames.header().clock.clone();
    let name = stream.to_string();
    // BORROW: explicit .as_slice() instead of Deref coercion
    if reader.detect(head.as_slice()).is_none() {
        return Err(NormaliseError::Undetected { stream: name });
    }
    let mut events = Vec::new();
    for (index, frame) in frames.enumerate() {
        let frame = frame?;
        let index = u64::try_from(index).unwrap_or(u64::MAX);
        let at = wall_time(&clock, frame.t_mono_ns).map_err(|detail| NormaliseError::Time {
            // BORROW: .clone() gives the error its own name
            stream: name.clone(),
            frame: index,
            detail,
        })?;
        // BORROW: .clone() gives the frame its own reference to the stream
        let input = FrameIn::new(RawRef::new(stream.clone(), index), at, &frame);
        let fed = reader.feed(input).map_err(|e| {
            let (frame, detail) = match e {
                ReaderError::Frame { frame, detail } => (frame, detail),
                // EXPLICIT: a reader that refuses a frame with any other error names the frame it was fed
                ReaderError::Incomplete { .. } | ReaderError::Trace(_) | _ => {
                    (index, e.to_string())
                }
            };
            NormaliseError::Frame {
                // BORROW: .clone() gives the error its own name
                stream: name.clone(),
                frame,
                detail,
            }
        })?;
        events.extend(fed);
    }
    let id = reader.id();
    let sessions = reader.finish().map_err(|e| NormaliseError::Finish {
        // BORROW: .clone() gives the error its own name
        stream: name.clone(),
        detail: e.to_string(),
    })?;
    // BORROW: .clone() copies the digest
    let header = TraceHeader::new(id, vec![RawFileRef::new(stream, raw.digest.clone())]);
    TraceFile::new(header, sessions, events).map_err(|source| NormaliseError::Trace {
        stream: name,
        source,
    })
}
