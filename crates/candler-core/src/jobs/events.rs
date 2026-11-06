//! The job's event log: a `JobEvents` that implements `EventSink` and owns the envelope.
//!
//! `events.jsonl` has one writer at a time across processes: every append holds an exclusive
//! `flock` on the file, so the supervisor, `candler kill` and a recovery never interleave and
//! `seq` is read from the log, not kept in memory.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Mutex, PoisonError};

use candler_schemas::execution::{Envelope, EventPayload};
use candler_schemas::primitives::{JobId, RunKey, Timestamp};
use rustix::fs::{FlockOperation, flock};

use super::dir::{JobDir, JobError, io_error};
use super::view::JobView;
use crate::supervise::EventSink;

/// The file name of the log.
const EVENTS_FILE: &str = "events.jsonl";
/// How many bytes are read backwards at a time to find the last line.
const CHUNK: u64 = 64 * 1024;

/// A byte offset into the event log, at the start of a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct TailPos(pub u64);

/// The complete events read from a position, and where to read from next.
#[derive(Debug, Clone, PartialEq)]
pub struct Tail {
    /// The events, in log order.
    pub events: Vec<Envelope>,
    /// The offset after the last complete line read; a torn last line starts here.
    pub next: TailPos,
}

/// The event log of one job.
#[derive(Debug)]
pub struct JobEvents {
    job: JobId,
    path: PathBuf,
    failure: Mutex<Option<JobError>>,
}

impl JobEvents {
    /// Opens the log of `dir`, creating an empty file when there is none.
    ///
    /// # Errors
    /// [`JobError::Io`] when the file cannot be opened.
    pub fn open(dir: &JobDir) -> Result<Self, JobError> {
        let path = dir.path().join(EVENTS_FILE);
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .map_err(io_error("open the event log", &path))?;
        Ok(Self {
            job: dir.job().clone(), // BORROW: the log owns its job id
            path,
            failure: Mutex::new(None),
        })
    }

    /// Appends one event and returns its envelope: `seq` is one past the last line's, `at` is now.
    /// A torn trailing line is cut off first. The line is fsynced before the lock is released.
    ///
    /// # Errors
    /// [`JobError::Io`] when the log cannot be written, [`JobError::Corrupt`] when its last line
    /// is not an event.
    pub fn append(
        &self,
        run: Option<&RunKey>,
        payload: EventPayload,
    ) -> Result<Envelope, JobError> {
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&self.path)
            .map_err(io_error("open the event log", &self.path))?;
        flock(&file, FlockOperation::LockExclusive).map_err(|e| JobError::Io {
            op: "lock the event log",
            path: self.path.clone(), // BORROW: the error owns the path
            detail: e.to_string(),
        })?;
        let (end, last) = last_complete_line(&mut file, &self.path)?;
        let len = file
            .metadata()
            .map_err(io_error("stat the event log", &self.path))?
            .len();
        if len > end {
            file.set_len(end)
                .map_err(io_error("cut a torn line", &self.path))?;
        }
        let seq = match last {
            Some((offset, line)) => {
                let envelope = parse_line(&line, &self.path, offset)?;
                envelope.seq().saturating_add(1)
            }
            None => 0,
        };
        let at = Timestamp::from_str(&jiff::Timestamp::now().to_string()).map_err(|e| {
            JobError::Parse {
                path: self.path.clone(), // BORROW: the error owns the path
                detail: e.to_string(),
            }
        })?;
        let mut builder = Envelope::builder()
            .seq(seq)
            .at(at)
            .job_id(self.job.clone()) // BORROW: the envelope owns its job id
            .event(payload);
        if let Some(run) = run {
            builder = builder.run(run.clone()); // BORROW: the envelope owns its run key
        }
        let envelope = builder.build().map_err(|e| JobError::Parse {
            path: self.path.clone(), // BORROW: the error owns the path
            detail: e.to_string(),
        })?;
        let mut line = serde_json::to_vec(&envelope).map_err(|e| JobError::Parse {
            path: self.path.clone(), // BORROW: the error owns the path
            detail: e.to_string(),
        })?;
        line.push(b'\n');
        file.write_all(&line)
            .and_then(|()| file.sync_all())
            .map_err(io_error("append to the event log", &self.path))?;
        // The lock is released when `file` is dropped.
        Ok(envelope)
    }

    /// Whether every `emit` so far was written; the first failure is kept.
    ///
    /// # Errors
    /// The first [`JobError`] an `emit` met.
    pub fn health(&self) -> Result<(), JobError> {
        match &*self.failure.lock().unwrap_or_else(PoisonError::into_inner) {
            Some(error) => Err(error.clone()), // BORROW: the caller gets its own copy
            None => Ok(()),
        }
    }

    /// The complete lines from `from` on. A torn last line is left for the next call; an unknown
    /// event kind is kept as read.
    ///
    /// # Errors
    /// [`JobError::Corrupt`] naming the offset of a line that is not an event, [`JobError::Io`]
    /// when the log cannot be read.
    pub fn tail(&self, from: TailPos) -> Result<Tail, JobError> {
        let mut file =
            File::open(&self.path).map_err(io_error("open the event log", &self.path))?;
        file.seek(SeekFrom::Start(from.0))
            .map_err(io_error("seek in the event log", &self.path))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(io_error("read the event log", &self.path))?;
        let mut events = Vec::new();
        let mut offset = from.0;
        // BORROW: `.as_slice()` views the bytes as a slice
        let mut rest = bytes.as_slice();
        while let Some(at) = rest.iter().position(|b| *b == b'\n') {
            let (line, after) = rest.split_at(at);
            events.push(parse_line(line, &self.path, offset)?);
            offset = offset.saturating_add(wide(at)).saturating_add(1);
            rest = after.get(1..).unwrap_or_default();
        }
        Ok(Tail {
            events,
            next: TailPos(offset),
        })
    }
}

impl EventSink for JobEvents {
    fn emit(&self, run: Option<&RunKey>, payload: EventPayload) {
        if let Err(error) = self.append(run, payload) {
            let mut failure = self.failure.lock().unwrap_or_else(PoisonError::into_inner);
            if failure.is_none() {
                *failure = Some(error);
            }
        }
    }
}

/// Parses one line, naming `offset` when it is not an event.
fn parse_line(line: &[u8], path: &std::path::Path, offset: u64) -> Result<Envelope, JobError> {
    serde_json::from_slice(line).map_err(|e| JobError::Corrupt {
        path: path.to_path_buf(),
        offset,
        detail: e.to_string(),
    })
}

/// The offset just past the last newline of a log, and its last complete line with its offset.
type LastLine = (u64, Option<(u64, Vec<u8>)>);

/// The offset just past the last newline of `file`, and the last complete line with its offset.
fn last_complete_line(file: &mut File, path: &std::path::Path) -> Result<LastLine, JobError> {
    let len = file
        .metadata()
        .map_err(io_error("stat the event log", path))?
        .len();
    let mut window = CHUNK;
    loop {
        let start = len.saturating_sub(window);
        file.seek(SeekFrom::Start(start))
            .map_err(io_error("seek in the event log", path))?;
        let mut buf = Vec::with_capacity(usize::try_from(len - start).unwrap_or(0));
        Read::take(&mut *file, len - start)
            .read_to_end(&mut buf)
            .map_err(io_error("read the event log", path))?;
        let Some(last_nl) = buf.iter().rposition(|b| *b == b'\n') else {
            if start == 0 {
                return Ok((0, None));
            }
            window = window.saturating_mul(2);
            continue;
        };
        let end = start + wide(last_nl) + 1;
        let body = buf.get(..last_nl).unwrap_or_default();
        match body.iter().rposition(|b| *b == b'\n') {
            Some(prev) => {
                let line = body.get(prev + 1..).unwrap_or_default().to_vec();
                return Ok((end, Some((start + wide(prev) + 1, line))));
            }
            None if start == 0 => return Ok((end, Some((0, body.to_vec())))),
            None => window = window.saturating_mul(2),
        }
    }
}

/// A buffer length as a file offset.
fn wide(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}
