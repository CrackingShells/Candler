//! Pumps that move a child's output from its pipe into a raw frame file.
//!
//! A pump reads one pipe until end of file or until the cut-off is raised, frames the bytes into a
//! [`FrameWriter`], tells the stream's observer about each frame once it is flushed, and keeps the
//! last [`TAIL_LIMIT`] bytes it saw. A pump never stops reading because a write failed: it drops
//! the file, keeps reading and discards, so a full disk cannot block the child on a full pipe. The
//! failure is reported when the pump ends.

use std::io;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use candler_schemas::raw::{Clock, ClockAnchor, StreamName};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::watch;

use super::SuperviseError;
use super::spawn::FrameObserver;
use crate::frames::{FrameMode, FrameWriter, RawFileInfo};

/// How many trailing bytes of an output the supervisor keeps.
pub const TAIL_LIMIT: usize = 64 * 1024;

/// The size of one read from a pipe.
const CHUNK: usize = 8 * 1024;

/// Nanoseconds since the first reading in this process: the monotonic clock of frame times.
pub(crate) fn mono_ns() -> u64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    let origin = ORIGIN.get_or_init(Instant::now);
    u64::try_from(origin.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// The last [`TAIL_LIMIT`] bytes of a stream.
#[derive(Debug, Default)]
pub(crate) struct Tail {
    bytes: Vec<u8>,
}

impl Tail {
    /// Appends `chunk` and drops the oldest bytes beyond the limit.
    pub(crate) fn push(&mut self, chunk: &[u8]) {
        self.bytes.extend_from_slice(chunk);
        let excess = self.bytes.len().saturating_sub(TAIL_LIMIT);
        if excess > 0 {
            self.bytes.drain(..excess);
        }
    }

    /// The bytes kept.
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// A raw frame file that is receiving a stream, and the observer that sees its frames.
pub(crate) struct FramedStream {
    writer: FrameWriter,
    // TRAIT_OBJECT: the observer is supplied by the caller of the supervisor
    observer: Option<Box<dyn FrameObserver>>,
}

impl FramedStream {
    /// Creates the file `<dir>/<stream>.jsonl.tmp` with a monotonic clock anchored now.
    pub(crate) fn create(
        dir: &Path,
        stream: StreamName,
        mode: FrameMode,
        // TRAIT_OBJECT: the observer is supplied by the caller of the supervisor
        observer: Option<Box<dyn FrameObserver>>,
    ) -> Result<Self, SuperviseError> {
        let anchor = ClockAnchor::new(super::now()?, mono_ns());
        let writer = FrameWriter::create(dir, stream, mode, Clock::monotonic(anchor), None)
            .map_err(|source| SuperviseError::Io {
                context: "cannot create the raw frame file",
                source,
            })?;
        Ok(Self { writer, observer })
    }

    /// Frames `bytes` and tells the observer about each frame that is complete.
    fn accept(&mut self, bytes: &[u8]) -> io::Result<()> {
        let frames = self.writer.write(bytes, Some(mono_ns()))?;
        if let Some(observer) = self.observer.as_mut() {
            for (index, frame) in &frames {
                observer.frame(*index, frame);
            }
        }
        Ok(())
    }

    /// Emits the trailing frame, then gzips the file into place.
    fn finish(mut self) -> io::Result<RawFileInfo> {
        if let Some((index, frame)) = self.writer.flush_tail()?
            && let Some(observer) = self.observer.as_mut()
        {
            observer.frame(index, &frame);
        }
        self.writer.finish()
    }
}

/// What a pump leaves behind when it ends.
#[derive(Debug)]
#[non_exhaustive]
#[expect(dead_code, reason = "read by Child::wait")]
pub(crate) struct Pumped {
    /// The finished raw frame file, or the error that lost it; `None` for a pump with no file.
    pub(crate) file: Option<io::Result<RawFileInfo>>,
    /// The last bytes the pump read.
    pub(crate) tail: Vec<u8>,
}

/// Reads `reader` until end of file, an error or the cut-off, framing what it reads into `stream`
/// when there is one.
pub(crate) async fn pump<R: AsyncRead + Unpin>(
    mut reader: R,
    mut stream: Option<FramedStream>,
    mut cutoff: watch::Receiver<bool>,
) -> Pumped {
    let mut tail = Tail::default();
    let mut failure: Option<io::Error> = None;
    let mut chunk = vec![0_u8; CHUNK];
    loop {
        let read = tokio::select! {
            read = reader.read(&mut chunk) => read,
            _ = cutoff.wait_for(|stop| *stop) => break,
        };
        let Ok(count) = read else { break };
        let Some(bytes) = chunk.get(..count).filter(|bytes| !bytes.is_empty()) else {
            break;
        };
        tail.push(bytes);
        if let Some(open) = stream.as_mut()
            && let Err(error) = open.accept(bytes)
        {
            failure = Some(error);
            stream = None;
        }
    }
    let file = match stream {
        Some(open) => Some(open.finish()),
        None => failure.map(Err),
    };
    Pumped {
        file,
        tail: tail.into_bytes(),
    }
}
