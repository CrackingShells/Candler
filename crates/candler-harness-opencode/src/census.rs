//! The reader of the census stream: the tool names of each model request the gateway relayed.
//!
//! `OpenCode` reports its tool inventory nowhere the driver can read, so the inventory is what its
//! model requests carry. The gateway records one JSON line per request, `{"seq":<n>,"tools":[...]}`,
//! and the core stores the lines as a raw stream. The inventory is the sorted set of tool names of
//! the first request that carries any; a request with no tools says nothing, and a run none of
//! whose requests carry tools leaves the inventory unrecorded, never empty.

use candler_harness::reader::{FrameIn, ReaderError, TraceReader};
use candler_harness::spec::{
    CensusSource, Confidence, FormatVersion, Framing, InitReport, Observed,
};
use candler_schemas::primitives::NameVersion;
use candler_schemas::trace::{Session, SessionEvent};
use serde_json::Value;

use crate::{CENSUS_READER, OpenCodeError};

/// One request of the stream: its place in the run and the names of its tools.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Request {
    seq: u64,
    tools: Vec<String>,
}

/// Reads the census stream for the tool inventory.
#[non_exhaustive]
pub struct OpenCodeCensusReader {
    identity: NameVersion,
    requests: Vec<Request>,
}

impl OpenCodeCensusReader {
    /// A reader of the census stream.
    ///
    /// # Errors
    /// Returns [`OpenCodeError::Identity`] if the reader identity is not a valid name.
    pub fn new() -> Result<Self, OpenCodeError> {
        let identity =
            CENSUS_READER
                .parse::<NameVersion>()
                .map_err(|error| OpenCodeError::Identity {
                    detail: error.to_string(), // BORROW: .to_string() gives owned text
                })?;
        Ok(Self {
            identity,
            requests: Vec::new(),
        })
    }
}

/// The request a line states: an object whose `seq` is a number and whose `tools` is a list of
/// strings. Nothing else of the line is read.
fn request_of(line: &[u8]) -> Option<Request> {
    let Value::Object(map) = serde_json::from_slice::<Value>(line).ok()? else {
        return None;
    };
    let seq = map.get("seq")?.as_u64()?;
    let tools = map
        .get("tools")?
        .as_array()?
        .iter()
        .map(|name| name.as_str().map(str::to_owned)) // BORROW: .to_owned() makes the owned name
        .collect::<Option<Vec<String>>>()?;
    Some(Request { seq, tools })
}

impl TraceReader for OpenCodeCensusReader {
    fn id(&self) -> NameVersion {
        self.identity.clone() // BORROW: .clone() copies the reader identity
    }

    fn framing(&self) -> Framing {
        Framing::Lines
    }

    fn detect(&self, sample: &[u8]) -> Option<(FormatVersion, Confidence)> {
        let line = sample
            .split(|byte| *byte == b'\n')
            .find(|line| !line.iter().all(u8::is_ascii_whitespace))?;
        request_of(line).map(|_| {
            (
                FormatVersion::new(semver::Version::new(0, 1, 0)),
                Confidence::Likely,
            )
        })
    }

    fn feed(&mut self, frame: FrameIn<'_>) -> Result<Vec<SessionEvent>, ReaderError> {
        let bytes = frame.frame.data.to_bytes();
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Ok(Vec::new());
        }
        let request = request_of(&bytes).ok_or_else(|| ReaderError::Frame {
            frame: frame.raw_ref.frame,
            detail: "a census line holds a sequence number and a list of tool names".to_owned(), // BORROW: .to_owned() makes the owned text
        })?;
        self.requests.push(request);
        Ok(Vec::new())
    }

    fn observed(&self) -> Observed {
        let first = self
            .requests
            .iter()
            .filter(|request| !request.tools.is_empty())
            .min_by_key(|request| request.seq);
        let observed = Observed::new(Vec::new());
        let Some(request) = first else {
            return observed;
        };
        let tools = Value::Array(
            request
                .tools
                .iter()
                .map(|name| Value::String(name.clone())) // BORROW: .clone() copies the name into the report
                .collect(),
        );
        match InitReport::new(tools, None, CensusSource::Gateway) {
            Ok(init) => observed.with_init(init),
            // EXPLICIT: a list that is not a census leaves the inventory unrecorded
            Err(_) => observed,
        }
    }

    fn finish(self: Box<Self>) -> Result<Vec<Session>, ReaderError> {
        Ok(Vec::new())
    }
}
