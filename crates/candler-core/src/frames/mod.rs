//! Raw frame files: writing a stream one line per frame and reading the frames back.
//!
//! A raw frame file (`raw-frames/1`) is JSONL: a header line, then one frame per line of the
//! archived stream. [`FrameWriter`] writes the file while the stream is produced: plain and
//! flushed after each frame under the name `<stream>.jsonl.tmp`, then gzipped with header mtime 0
//! and renamed to `<stream>.jsonl.gz` by [`FrameWriter::finish`]. [`frame_bytes`] frames a
//! finished stream after the fact. [`FrameReader`] reads a plain or gzipped file.
//!
//! Under [`Framing::Lines`] a frame is one line of the stream with its `\n` kept, and a trailing
//! line without `\n` is its own frame. Under [`Framing::Document`] the stream is one frame. An
//! empty stream has no frame in either mode. The data of the frames, concatenated, is the stream
//! byte for byte.

pub(crate) mod refusal;

use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use candler_harness::spec::Framing;
use candler_schemas::primitives::Digest;
use candler_schemas::raw::{Clock, FrameData, RawFrame, RawHeader, RawRecord, StreamName};
use flate2::read::MultiGzDecoder;
use flate2::{Compression, GzBuilder};
use sha2::{Digest as _, Sha256};

use crate::store::sync::{sync_dir, sync_file};

/// The two bytes that open a gzip member.
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// Why a raw frame file could not be read.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FramesError {
    /// The file could not be opened or read.
    #[error("raw frame file {path} cannot be read: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The operating-system error.
        #[source]
        source: io::Error,
    },
    /// A line that is not a record of the format.
    #[error("line {line} of the raw frame file is not a raw frame record: {detail}")]
    Record {
        /// The line, counted from 1.
        line: u64,
        /// What is wrong with it.
        detail: String,
    },
    /// A file whose first line is not a header.
    #[error("raw frame file has no header: its first line is {found}")]
    MissingHeader {
        /// What the first line is: `a frame`, `unknown` or `missing`.
        found: &'static str,
    },
    /// A header after the first line.
    #[error("line {line} of the raw frame file is a second header")]
    SecondHeader {
        /// The line, counted from 1.
        line: u64,
    },
}

/// What a finished frame file is: its path, the digest of its bytes, and its number of frames.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RawFileInfo {
    /// The gzipped file, `<dir>/<stream>.jsonl.gz`.
    pub path: PathBuf,
    /// The digest of the bytes of the gzipped file.
    pub digest: Digest,
    /// The number of frames, which excludes the header.
    pub frames: u64,
}

impl RawFileInfo {
    /// A file description.
    #[must_use]
    pub const fn new(path: PathBuf, digest: Digest, frames: u64) -> Self {
        Self {
            path,
            digest,
            frames,
        }
    }
}

/// Writes one JSON record and a newline, then flushes.
fn write_record(file: &mut File, record: &RawRecord) -> io::Result<()> {
    let mut line = serde_json::to_vec(record).map_err(io::Error::other)?;
    line.push(b'\n');
    file.write_all(line.as_slice())?; // BORROW: explicit .as_slice() instead of Deref coercion
    file.flush()
}

/// Writes one frame record and returns the frame with its index.
fn emit(
    file: &mut File,
    count: &mut u64,
    bytes: &[u8],
    t_mono_ns: Option<u64>,
) -> io::Result<(u64, RawFrame)> {
    let frame = RawFrame::new(t_mono_ns, FrameData::from_bytes(bytes));
    // BORROW: .clone() gives the record its own copy; the caller keeps the frame
    write_record(file, &RawRecord::Frame(frame.clone()))?;
    let index = *count;
    *count = count.saturating_add(1);
    Ok((index, frame))
}

/// Lowercase hex of `bytes`.
fn hex(bytes: impl IntoIterator<Item = u8>) -> String {
    let mut text = String::new();
    for byte in bytes {
        text.extend(char::from_digit(u32::from(byte >> 4), 16));
        text.extend(char::from_digit(u32::from(byte & 0xf), 16));
    }
    text
}

/// A writer that hashes what passes through it.
struct HashingWriter {
    file: File,
    hasher: Sha256,
}

impl Write for HashingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.file.write(buf)?;
        if let Some(done) = buf.get(..written) {
            self.hasher.update(done);
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

/// Writes the frames of one stream to `<dir>/<stream>.jsonl.tmp` as the stream is produced.
#[derive(Debug)]
pub struct FrameWriter {
    dir: PathBuf,
    stream: StreamName,
    mode: Framing,
    tmp_path: PathBuf,
    file: File,
    /// Bytes of a line not yet complete (`Lines`), or of the whole stream so far (`Document`).
    pending: Vec<u8>,
    /// How many leading bytes of `pending` are known to hold no `\n`.
    scanned: usize,
    /// The time of the latest write that carried bytes.
    last_t: Option<u64>,
    frames: u64,
    closed: bool,
}

impl FrameWriter {
    /// Creates `dir` if needed and writes the header to `<dir>/<stream>.jsonl.tmp`, replacing a
    /// file of that name.
    ///
    /// # Errors
    /// Returns the I/O error of creating the directory or writing the header.
    pub fn create(
        dir: &Path,
        stream: StreamName,
        mode: Framing,
        clock: Clock,
        source: Option<Digest>,
    ) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let tmp_path = dir.join(format!("{stream}.jsonl.tmp"));
        let mut file = File::create(&tmp_path)?;
        // BORROW: .clone() keeps the stream name for the writer; the header owns one
        let header = RawHeader::new(stream.clone(), clock, source);
        write_record(&mut file, &RawRecord::Header(header))?;
        Ok(Self {
            dir: dir.to_path_buf(), // BORROW: .to_path_buf() makes the owned path the writer keeps
            stream,
            mode,
            tmp_path,
            file,
            pending: Vec::new(),
            scanned: 0,
            last_t: None,
            frames: 0,
            closed: false,
        })
    }

    /// Takes the next bytes of the stream and returns the frames this call completed, each with
    /// its index, already flushed to the file.
    ///
    /// Under `Lines`, a frame completed by this call carries `t_mono_ns`; a partial line stays
    /// buffered. Under `Whole`, nothing is returned until the tail is flushed. The time of the
    /// trailing frame is that of the latest write that carried bytes.
    ///
    /// # Errors
    /// Returns the I/O error of writing a frame, and `InvalidInput` after the tail was flushed.
    pub fn write(
        &mut self,
        bytes: &[u8],
        t_mono_ns: Option<u64>,
    ) -> io::Result<Vec<(u64, RawFrame)>> {
        if self.closed {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "frame writer is closed",
            ));
        }
        if !bytes.is_empty() {
            self.last_t = t_mono_ns;
        }
        self.pending.extend_from_slice(bytes);
        let mut completed = Vec::new();
        if self.mode == Framing::Document {
            return Ok(completed);
        }
        let mut start = 0_usize;
        loop {
            let from = start.max(self.scanned);
            let Some(rest) = self.pending.get(from..) else {
                break;
            };
            let Some(offset) = rest.iter().position(|b| *b == b'\n') else {
                break;
            };
            let end = from.saturating_add(offset).saturating_add(1);
            let Some(line) = self.pending.get(start..end) else {
                break;
            };
            completed.push(emit(&mut self.file, &mut self.frames, line, t_mono_ns)?);
            start = end;
        }
        self.pending.drain(..start);
        self.scanned = self.pending.len();
        Ok(completed)
    }

    /// Emits the trailing partial line (`Lines`) or the whole stream (`Document`) as a frame and
    /// closes the writer to further bytes. It returns the frame, or `None` when nothing is
    /// pending or the tail was already flushed.
    ///
    /// # Errors
    /// Returns the I/O error of writing the frame.
    pub fn flush_tail(&mut self) -> io::Result<Option<(u64, RawFrame)>> {
        if self.closed {
            return Ok(None);
        }
        self.closed = true;
        if self.pending.is_empty() {
            return Ok(None);
        }
        let item = emit(
            &mut self.file,
            &mut self.frames,
            self.pending.as_slice(), // BORROW: explicit .as_slice() instead of Deref coercion
            self.last_t,
        )?;
        self.pending.clear();
        Ok(Some(item))
    }

    /// The number of frames written so far, which excludes the header.
    #[must_use]
    pub const fn frames(&self) -> u64 {
        self.frames
    }

    /// Removes the plain file of a stream that will never be finished, such as one whose child
    /// could not be started. A failure to remove it is ignored: nothing reads the file.
    pub fn discard(self) {
        let _ = fs::remove_file(&self.tmp_path);
    }

    /// Flushes the tail, gzips the file with header mtime 0 to `<stream>.jsonl.gz`, syncs it and
    /// renames it into place, then removes the plain file.
    ///
    /// # Errors
    /// Returns the I/O error of any of those steps; the plain file stays on disk.
    pub fn finish(mut self) -> io::Result<RawFileInfo> {
        self.flush_tail()?;
        let gz_path = self.dir.join(format!("{}.jsonl.gz", self.stream));
        let gz_tmp = self.dir.join(format!("{}.jsonl.gz.tmp", self.stream));
        let out = HashingWriter {
            file: File::create(&gz_tmp)?,
            hasher: Sha256::new(),
        };
        let mut encoder = GzBuilder::new().mtime(0).write(out, Compression::default());
        io::copy(&mut File::open(&self.tmp_path)?, &mut encoder)?;
        let HashingWriter { file, hasher } = encoder.finish()?;
        sync_file(&file)?;
        let digest = Digest::from_hex(hex(hasher.finalize()).as_str()) // BORROW: explicit .as_str()
            .map_err(io::Error::other)?;
        fs::rename(&gz_tmp, &gz_path)?;
        sync_dir(&self.dir)?;
        fs::remove_file(&self.tmp_path)?;
        Ok(RawFileInfo::new(gz_path, digest, self.frames))
    }
}

/// Frames a finished stream after the fact: the header (`clock: unrecorded`, `source` set), then
/// the frames, none of which has a time.
#[must_use]
pub fn frame_bytes(
    stream: StreamName,
    mode: Framing,
    bytes: &[u8],
    source: Digest,
) -> Vec<RawRecord> {
    let header = RawHeader::new(stream, Clock::Unrecorded, Some(source));
    let mut records = vec![RawRecord::Header(header)];
    let frame = |piece: &[u8]| RawRecord::Frame(RawFrame::new(None, FrameData::from_bytes(piece)));
    match mode {
        Framing::Lines => records.extend(bytes.split_inclusive(|b| *b == b'\n').map(frame)),
        Framing::Document if bytes.is_empty() => {}
        Framing::Document => records.push(frame(bytes)),
    }
    records
}

/// The bytes of a frame file, plain or gunzipped.
enum Source {
    Plain(File),
    Gzip(MultiGzDecoder<File>),
}

impl Read for Source {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(file) => file.read(buf),
            Self::Gzip(decoder) => decoder.read(buf),
        }
    }
}

impl fmt::Debug for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plain(_) => f.write_str("Plain"),
            Self::Gzip(_) => f.write_str("Gzip"),
        }
    }
}

/// Reads a raw frame file, plain or gzipped: the header first, then the frames.
#[derive(Debug)]
pub struct FrameReader {
    header: RawHeader,
    lines: io::Lines<BufReader<Source>>,
    path: PathBuf,
    line: u64,
    done: bool,
}

impl FrameReader {
    /// Opens `path` and reads its header.
    ///
    /// # Errors
    /// Returns [`FramesError::Io`] if the file cannot be read, [`FramesError::MissingHeader`] if
    /// its first line is not a header, and [`FramesError::Record`] if that line is not a record.
    pub fn open(path: &Path) -> Result<Self, FramesError> {
        let io_error = |source| FramesError::Io {
            path: path.to_path_buf(), // BORROW: .to_path_buf() makes the owned path the error keeps
            source,
        };
        let mut file = File::open(path).map_err(io_error)?;
        let mut magic = [0_u8; 2];
        let read = file.read(&mut magic).map_err(io_error)?;
        file.seek(SeekFrom::Start(0)).map_err(io_error)?;
        let source = if read == magic.len() && magic == GZIP_MAGIC {
            Source::Gzip(MultiGzDecoder::new(file))
        } else {
            Source::Plain(file)
        };
        let mut lines = BufReader::new(source).lines();
        let first = match lines.next() {
            Some(Ok(text)) => text,
            Some(Err(source)) => return Err(io_error(source)),
            None => return Err(FramesError::MissingHeader { found: "missing" }),
        };
        // BORROW: explicit .as_str() instead of Deref coercion
        let header = match parse_record(first.as_str(), 1)? {
            RawRecord::Header(header) => header,
            RawRecord::Frame(_) => return Err(FramesError::MissingHeader { found: "a frame" }),
            _ => return Err(FramesError::MissingHeader { found: "unknown" }),
        };
        Ok(Self {
            header,
            lines,
            path: path.to_path_buf(), // BORROW: .to_path_buf() makes the owned path the reader keeps
            line: 1,
            done: false,
        })
    }

    /// The header of the file.
    #[must_use]
    pub const fn header<'a>(&'a self) -> &'a RawHeader {
        &self.header
    }
}

/// Parses one line as a record.
fn parse_record(text: &str, line: u64) -> Result<RawRecord, FramesError> {
    serde_json::from_str::<RawRecord>(text).map_err(|e| FramesError::Record {
        line,
        detail: e.to_string(),
    })
}

impl Iterator for FrameReader {
    type Item = Result<RawFrame, FramesError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let text = match self.lines.next()? {
            Ok(text) => text,
            Err(source) => {
                self.done = true;
                return Some(Err(FramesError::Io {
                    path: self.path.clone(), // BORROW: .clone() gives the error its own path
                    source,
                }));
            }
        };
        self.line = self.line.saturating_add(1);
        // BORROW: explicit .as_str() instead of Deref coercion
        let item = match parse_record(text.as_str(), self.line) {
            Ok(RawRecord::Frame(frame)) => Ok(frame),
            Ok(RawRecord::Header(_)) => Err(FramesError::SecondHeader { line: self.line }),
            Ok(_) => Err(FramesError::Record {
                line: self.line,
                detail: String::from("a record of an unknown kind"),
            }),
            Err(error) => Err(error),
        };
        self.done = item.is_err();
        Some(item)
    }
}
