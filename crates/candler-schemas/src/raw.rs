//! Raw frame files (`raw-frames/1`) and stream names.
//!
//! A raw frame file is JSONL: a header record, then one frame record per line of the archived
//! stream. This module states the shapes; cutting a stream into frames belongs to the supervisor.
//!
//! An optional field is omitted when absent and never written as `null`; the reader refuses an
//! explicit `null`. Unknown fields are ignored on input, and every struct and every tagged enum is
//! read from an object only, never from a list.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use crate::generate::GeneratedSchema;
use crate::primitives::{Digest, Timestamp, object_only, present, schema_tag, text_type};

/// The version marker of a raw frame file, written in its header.
pub const RAW_FRAMES_SCHEMA: &str = "raw-frames/1";

/// Why a raw-frame text or record is not valid.
///
/// Each variant names the format, what is wrong with the text, and the offending text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RawError {
    /// A [`StreamName`] outside `[a-z0-9][a-z0-9._-]*`.
    #[error("stream name {problem} (got {value:?})")]
    StreamName {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
    /// A [`Base64Bytes`] text that is not canonical, padded, standard-alphabet base64.
    #[error("base64 {problem} (got {value:?})")]
    Base64 {
        /// What is wrong with the text.
        problem: &'static str,
        /// The text that was parsed.
        value: String,
    },
}

fn stream_name_problem(text: &str) -> Result<(), &'static str> {
    let mut bytes = text.bytes();
    let first = bytes
        .next()
        .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    let rest = bytes
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'));
    if first && rest {
        Ok(())
    } else {
        Err("does not match `[a-z0-9][a-z0-9._-]*`")
    }
}

const STREAM_NAME_PATTERN: &str = r"^[a-z0-9][a-z0-9._-]*(?![\s\S])";
const BASE64_PATTERN: &str =
    r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?(?![\s\S])";

text_type!(StreamName {
    description: "The name of an archived stream: `stdout`, `stderr`, or the name of a collected file, matching `[a-z0-9][a-z0-9._-]*`.",
    pattern: STREAM_NAME_PATTERN,
});

impl FromStr for StreamName {
    type Err = RawError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match stream_name_problem(text) {
            // BORROW: .to_owned() makes the owned text the value keeps
            Ok(()) => Ok(Self {
                text: text.to_owned(),
            }),
            // BORROW: .to_owned() makes the owned text the error keeps
            Err(problem) => Err(RawError::StreamName {
                problem,
                value: text.to_owned(),
            }),
        }
    }
}

/// Bytes that are written as standard-alphabet base64 with `=` padding; the bytes are held decoded,
/// so a value that exists is always a value that decodes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Base64Bytes {
    bytes: Vec<u8>,
}

impl Base64Bytes {
    /// Holds `bytes`.
    #[must_use]
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// The bytes the text encodes.
    #[must_use]
    pub const fn as_bytes<'a>(&'a self) -> &'a [u8] {
        self.bytes.as_slice()
    }
}

impl fmt::Display for Base64Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // BORROW: explicit .as_str() instead of Deref coercion
        f.write_str(STANDARD.encode(self.bytes.as_slice()).as_str())
    }
}

impl FromStr for Base64Bytes {
    type Err = RawError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let refuse = |problem| RawError::Base64 {
            problem,
            // BORROW: .to_owned() makes the owned text the error keeps
            value: text.to_owned(),
        };
        let bytes = STANDARD
            .decode(text)
            .map_err(|_| refuse("is not standard-alphabet base64"))?;
        if STANDARD.encode(bytes.as_slice()) == text {
            Ok(Self { bytes })
        } else {
            Err(refuse("is not canonical: padding or trailing bits differ"))
        }
    }
}

impl TryFrom<String> for Base64Bytes {
    type Error = RawError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        // BORROW: explicit .as_str() instead of Deref coercion
        text.as_str().parse()
    }
}

impl From<Base64Bytes> for String {
    fn from(value: Base64Bytes) -> Self {
        value.to_string()
    }
}

impl JsonSchema for Base64Bytes {
    fn schema_name() -> Cow<'static, str> {
        "Base64Bytes".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::Base64Bytes").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "Bytes as standard-alphabet base64 with `=` padding.",
            "pattern": BASE64_PATTERN,
        })
    }
}

schema_tag! {
    /// The version marker of a raw frame file: always `raw-frames/1`.
    pub RawFramesTag = RAW_FRAMES_SCHEMA
}

/// One wall-clock reading paired with the monotonic clock at the same moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct ClockAnchor {
    /// The wall-clock reading, an RFC 3339 timestamp.
    pub wall: Timestamp,
    /// The monotonic clock in nanoseconds at that reading.
    #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
    pub mono_ns: u64,
}

object_only!(ClockAnchor);

impl ClockAnchor {
    /// Pairs a wall-clock reading with a monotonic reading.
    #[must_use]
    pub const fn new(wall: Timestamp, mono_ns: u64) -> Self {
        Self { wall, mono_ns }
    }
}

/// The clock the frame times of a stream were taken from: an object whose `kind` is `monotonic`, with an `anchor`, or `unrecorded`, which has no frame times.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum Clock {
    /// Frame times are nanoseconds of a monotonic clock, anchored to one wall-clock reading.
    #[non_exhaustive]
    Monotonic {
        /// The wall-clock reading and the monotonic reading taken together.
        anchor: ClockAnchor,
    },
    /// The stream was framed after the fact and its frames have no times.
    Unrecorded,
}

object_only!(Clock);

impl Clock {
    /// A monotonic clock with its anchor.
    #[must_use]
    pub const fn monotonic(anchor: ClockAnchor) -> Self {
        Self::Monotonic { anchor }
    }
}

/// The first line of a raw frame file: the stream it holds and the clock of its frames, with the digest of the file the frames were cut from when they were cut after the fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct RawHeader {
    /// The version marker: always `raw-frames/1`.
    pub schema: RawFramesTag,
    /// The name of the archived stream.
    pub stream: StreamName,
    /// The clock of the frame times.
    pub clock: Clock,
    /// The digest of the file the frames were cut from, when they were cut after the fact.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub source: Option<Digest>,
}

object_only!(RawHeader);

impl RawHeader {
    /// Builds a header.
    #[must_use]
    pub const fn new(stream: StreamName, clock: Clock, source: Option<Digest>) -> Self {
        Self {
            schema: RawFramesTag,
            stream,
            clock,
            source,
        }
    }
}

/// The content of one frame: an object whose `kind` is `text`, with valid UTF-8 under `text`, or `bytes`, with standard base64 under `base64`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum FrameData {
    /// The frame's bytes are valid UTF-8, written as text.
    Text {
        /// The frame's text.
        text: String,
    },
    /// The frame's bytes are not valid UTF-8, written as base64.
    Bytes {
        /// The frame's bytes in standard base64 with padding.
        base64: Base64Bytes,
    },
}

object_only!(FrameData);

impl FrameData {
    /// The data of a frame holding `bytes`: `text` exactly when the bytes are valid UTF-8.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        match std::str::from_utf8(bytes) {
            // BORROW: .to_owned() makes the owned text the frame keeps
            Ok(text) => Self::Text {
                text: text.to_owned(),
            },
            Err(_) => Self::Bytes {
                base64: Base64Bytes::new(bytes.to_vec()),
            },
        }
    }

    /// The bytes of the frame.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            // BORROW: .as_bytes().to_vec() copies the bytes of the text
            Self::Text { text } => text.as_bytes().to_vec(),
            Self::Bytes { base64 } => base64.as_bytes().to_vec(),
        }
    }
}

/// One frame of a stream: its data and, when the clock records times, the monotonic time at which it was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct RawFrame {
    /// The monotonic time of the frame in nanoseconds; absent when the clock is unrecorded.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
    pub t_mono_ns: Option<u64>,
    /// What the frame holds.
    pub data: FrameData,
}

object_only!(RawFrame);

impl RawFrame {
    /// Builds a frame.
    #[must_use]
    pub const fn new(t_mono_ns: Option<u64>, data: FrameData) -> Self {
        Self { t_mono_ns, data }
    }
}

/// One line of a raw frame file: an object whose `kind` is `header` or `frame`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum RawRecord {
    /// The first line of the file.
    Header(RawHeader),
    /// A following line.
    Frame(RawFrame),
}

object_only!(RawRecord);

/// A pointer to one frame: the stream and the index of the frame, counting the lines after the header from 0.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct RawRef {
    /// The stream the frame belongs to.
    pub stream: StreamName,
    /// The index of the frame, from 0.
    #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
    pub frame: u64,
}

object_only!(RawRef);

impl RawRef {
    /// Points at frame `frame` of `stream`.
    #[must_use]
    pub const fn new(stream: StreamName, frame: u64) -> Self {
        Self { stream, frame }
    }
}

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    Vec::new()
}
