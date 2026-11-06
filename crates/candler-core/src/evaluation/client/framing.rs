//! Reading the evaluator's standard output as protocol frames.
//!
//! One frame is one line. A line that is not JSON is counted and never ends a request; a JSON line
//! that is no protocol message is malformed; a last chunk with no newline that is not JSON is a
//! partial frame. A line longer than [`MAX_FRAME`] is dropped whole.

use std::io;

use candler_schemas::evaluation::rpc::Message;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt as _, AsyncRead, BufReader};

/// The longest frame, in bytes, that is read; a longer line is dropped.
pub(crate) const MAX_FRAME: usize = 8 * 1024 * 1024;

/// What one read of the stream found.
#[derive(Debug)]
pub(crate) enum Frame {
    /// A protocol message.
    Message(Box<Message>),
    /// A complete line that is not JSON.
    NotJson,
    /// A JSON line that is no protocol message, with what the reader said.
    Malformed(String),
    /// A line longer than [`MAX_FRAME`].
    Oversize,
    /// The end of the stream cut a frame short.
    Partial,
    /// The end of the stream.
    Eof,
    /// The stream failed to read, with the system's error.
    Broken(String),
}

/// Reads frames from a stream. Its state is in the struct, so a read that is dropped half way
/// loses nothing: [`FrameReader::next`] is safe to use in a `select!`.
pub(crate) struct FrameReader<R> {
    reader: BufReader<R>,
    line: Vec<u8>,
    oversize: bool,
}

impl<R: AsyncRead + Unpin> FrameReader<R> {
    /// A reader over `stream`.
    pub(crate) fn new(stream: R) -> Self {
        Self {
            reader: BufReader::new(stream),
            line: Vec::new(),
            oversize: false,
        }
    }

    /// Adds `bytes` to the line in progress, or marks the line oversize.
    fn push(&mut self, bytes: &[u8]) {
        if self.oversize || self.line.len().saturating_add(bytes.len()) > MAX_FRAME {
            self.oversize = true;
            self.line.clear();
        } else {
            self.line.extend_from_slice(bytes);
        }
    }

    /// The next frame. A blank line is skipped.
    pub(crate) async fn next(&mut self) -> Frame {
        loop {
            let available = match self.reader.fill_buf().await {
                Ok(bytes) => bytes.to_vec(),
                Err(error) => return Frame::Broken(describe(&error)),
            };
            if available.is_empty() {
                return self.at_end();
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let take = newline.map_or(available.len(), |at| at + 1);
            let content = available
                .get(..newline.unwrap_or(available.len()))
                .unwrap_or_default();
            self.push(content);
            self.reader.consume(take);
            if newline.is_some()
                && let Some(frame) = self.finish(true)
            {
                return frame;
            }
        }
    }

    /// The frame the end of the stream leaves, if any.
    fn at_end(&mut self) -> Frame {
        self.finish(false).unwrap_or(Frame::Eof)
    }

    /// Classifies the line in progress and starts the next one; `None` for a blank line.
    fn finish(&mut self, terminated: bool) -> Option<Frame> {
        let line = std::mem::take(&mut self.line);
        let oversize = std::mem::take(&mut self.oversize);
        if oversize {
            return Some(Frame::Oversize);
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            return None;
        }
        Some(classify(&line, terminated))
    }
}

/// What one complete or cut-short line is.
fn classify(line: &[u8], terminated: bool) -> Frame {
    // BORROW: the slice is read in place
    let Ok(value) = serde_json::from_slice::<Value>(line) else {
        return if terminated {
            Frame::NotJson
        } else {
            Frame::Partial
        };
    };
    match serde_json::from_value::<Message>(value) {
        Ok(message) => Frame::Message(Box::new(message)),
        Err(error) => Frame::Malformed(error.to_string()),
    }
}

/// The system's error as a phrase.
fn describe(error: &io::Error) -> String {
    format!("cannot read the evaluator's output: {error}")
}
