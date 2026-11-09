//! The index: a cache of the sealed runs and the finished outcome files, appended under a lock,
//! rebuilt atomically from the files and read through a torn last line.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

use candler_schemas::primitives::{NameVersion, RequestId, RunKey};
use rustix::fs::{FlockOperation, flock};
use serde_json::{Value, json};

use super::outcome::OutcomeFileRef;
use super::sync::sync_data;
use super::{Store, StoreError, io_error, sync_dir, visible_names, write_atomic};

/// An exclusive lock on `index/lock`, released when it is dropped.
struct IndexLock {
    _file: File,
}

impl Store {
    /// Takes the index lock, creating the index directory and the lock file when absent.
    fn lock_index(&self) -> Result<IndexLock, StoreError> {
        let dir = self.layout().index_dir();
        fs::create_dir_all(&dir).map_err(io_error("create the index directory", &dir))?;
        let path = self.layout().index_lock();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(io_error("open the index lock", &path))?;
        flock(&file, FlockOperation::LockExclusive)
            .map_err(|errno| io_error("lock the index", &path)(io::Error::from(errno)))?;
        Ok(IndexLock { _file: file })
    }

    /// Appends the line of a sealed run.
    pub(crate) fn index_run(&self, key: &RunKey) -> Result<(), StoreError> {
        let line = run_line(key);
        let _lock = self.lock_index()?;
        append_line(&self.layout().index_runs(), &line)
    }

    /// Appends the line of a finished outcome file.
    pub(crate) fn index_outcome(&self, file: &OutcomeFileRef) -> Result<(), StoreError> {
        let line = outcome_line(file);
        let _lock = self.lock_index()?;
        append_line(&self.layout().index_outcomes(), &line)
    }

    /// Rewrites both index files from the sealed runs and the finished outcome files on disk.
    /// Each file is written to a temporary name and renamed, under the index lock.
    ///
    /// # Errors
    /// Returns [`StoreError::Io`] when a directory cannot be listed or a file cannot be written.
    pub fn rebuild_index(&self) -> Result<(), StoreError> {
        let _lock = self.lock_index()?;
        let layout = self.layout();
        let mut runs = String::new();
        for entry in self.runs()?.iter().filter(|entry| entry.done) {
            runs.push_str(&run_line(&entry.key));
        }
        let mut outcomes = String::new();
        for name in visible_names(&layout.outcomes_dir())? {
            // BORROW: explicit .as_str() instead of Deref coercion
            let Ok(run) = name.as_str().parse::<RunKey>() else {
                // BORROW: explicit .as_str() instead of Deref coercion
                continue;
            };
            for file in self.outcome_files(&run)? {
                outcomes.push_str(&outcome_line(&file));
            }
        }
        write_atomic(&layout.index_runs(), runs.as_bytes())?;
        write_atomic(&layout.index_outcomes(), outcomes.as_bytes())?;
        sync_dir(&layout.index_dir())
    }

    /// The sealed runs the index lists, skipping a last line that does not parse.
    ///
    /// # Errors
    /// Returns [`StoreError::IndexCorrupt`] when a line other than the last does not parse, and
    /// [`StoreError::Io`] when the file cannot be read. A missing index lists nothing.
    pub fn indexed_runs(&self) -> Result<Vec<RunKey>, StoreError> {
        read_lines(&self.layout().index_runs(), |value| {
            value.get("run")?.as_str()?.parse::<RunKey>().ok() // BORROW: explicit .as_str() instead of Deref coercion
        })
    }

    /// The finished outcome files the index lists, skipping a last line that does not parse.
    ///
    /// # Errors
    /// Returns [`StoreError::IndexCorrupt`] when a line other than the last does not parse, and
    /// [`StoreError::Io`] when the file cannot be read. A missing index lists nothing.
    pub fn indexed_outcomes(&self) -> Result<Vec<OutcomeFileRef>, StoreError> {
        read_lines(&self.layout().index_outcomes(), |value| {
            let run = value.get("run")?.as_str()?.parse::<RunKey>().ok()?; // BORROW: explicit .as_str() instead of Deref coercion
            let evaluator = value
                .get("evaluator")?
                .as_str()? // BORROW: explicit .as_str() instead of Deref coercion
                .parse::<NameVersion>()
                .ok()?;
            let request = value.get("request")?.as_str()?.parse::<RequestId>().ok()?; // BORROW: explicit .as_str() instead of Deref coercion
            Some(OutcomeFileRef::new(run, evaluator, request))
        })
    }
}

/// The index line of a run, with its line feed.
fn run_line(key: &RunKey) -> String {
    format!("{}\n", json!({ "run": key.to_string() }))
}

/// The index line of an outcome file, with its line feed.
fn outcome_line(file: &OutcomeFileRef) -> String {
    format!(
        "{}\n",
        json!({
            "run": file.run.to_string(),
            "evaluator": file.evaluator.to_string(),
            "request": file.request.to_string(),
        })
    )
}

/// Appends `line` to the index file at `path`, which the caller has locked. A torn last line of
/// an earlier writer is completed when it parses and cut off when it does not.
fn append_line(path: &Path, line: &str) -> Result<(), StoreError> {
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(path)
        .map_err(io_error("open an index file", path))?;
    let fail = io_error("append to an index file", path);
    let length = file
        .metadata()
        .map_err(io_error("stat an index file", path))?
        .len();
    if length > 0 {
        let mut last = [0_u8; 1];
        file.seek(SeekFrom::Start(length.saturating_sub(1)))
            .and_then(|_| file.read_exact(&mut last))
            .map_err(io_error("read an index file", path))?;
        if last != *b"\n" {
            let mut bytes = Vec::new();
            file.seek(SeekFrom::Start(0))
                .and_then(|_| file.read_to_end(&mut bytes))
                .map_err(io_error("read an index file", path))?;
            let keep = bytes
                .iter()
                .rposition(|byte| *byte == b'\n')
                .map_or(0, |at| at.saturating_add(1));
            let tail = bytes.get(keep..).unwrap_or_default();
            if serde_json::from_slice::<Value>(tail).is_ok() {
                file.write_all(b"\n")
                    .map_err(io_error("append to an index file", path))?;
            } else {
                // The cut is made at a line start, so the complete lines are untouched.
                file.set_len(u64::try_from(keep).unwrap_or(length))
                    .map_err(io_error("cut a torn index line", path))?;
            }
        }
    }
    file.write_all(line.as_bytes())
        .and_then(|()| sync_data(&file))
        .map_err(fail)
}

/// Reads the lines of the index file at `path` through `parse`, skipping a last line that does not
/// parse.
fn read_lines<T>(path: &Path, parse: impl Fn(&Value) -> Option<T>) -> Result<Vec<T>, StoreError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(io_error("read an index file", path)(source)),
    };
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.split('\n').filter(|line| !line.is_empty()).collect();
    let mut items = Vec::new();
    for (position, line) in lines.iter().enumerate() {
        let item = serde_json::from_str::<Value>(line)
            .ok()
            .and_then(|value| parse(&value));
        match item {
            Some(item) => items.push(item),
            None if position.saturating_add(1) == lines.len() => {}
            None => {
                return Err(StoreError::IndexCorrupt {
                    path: path.to_path_buf(),
                    line: position.saturating_add(1),
                });
            }
        }
    }
    Ok(items)
}
