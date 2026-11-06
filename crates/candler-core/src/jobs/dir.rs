//! The directory of one job.
//!
//! Layout under `<store>/jobs/<id>/`: `job.json`, `events.jsonl`, `lock`, `children/` and
//! `control/`. A record is written by temp file, fsync, rename, so a reader sees all of a record or
//! none of it.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use candler_schemas::container::{CONTAINER_RECORD_SCHEMA, ContainerRecord};
use candler_schemas::execution::{JobRecord, JobState};
use candler_schemas::primitives::JobId;
use candler_schemas::process::{CHILD_RECORD_SCHEMA, ChildRecord};
use serde_json::Value;

use super::lost;
use super::view::{ChildEntry, JobView, JobViewError};
use crate::isolation::gateway::ChildRecorder;
use crate::store::layout::StoreLayout;

/// The file that holds the job's [`JobRecord`].
const RECORD_FILE: &str = "job.json";
/// The directory of the records of the processes and containers the job started.
const CHILDREN_DIR: &str = "children";
/// The directory of the records of the control calls the job made.
const CONTROL_DIR: &str = "control";
/// The suffix of a record that is still being written.
const TEMP_SUFFIX: &str = ".tmp";

/// Why a job directory operation failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum JobError {
    /// The operation is not implemented.
    #[error("job directory is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
    /// Another process holds the job's lock.
    #[error("the job is held by another process")]
    Held,
    /// A file system call failed.
    #[error("{op} failed on {}: {detail}", path.display())]
    Io {
        /// What was being done.
        op: &'static str,
        /// The path.
        path: PathBuf,
        /// What the system reported.
        detail: String,
    },
    /// A file did not hold what it should.
    #[error("{} is not readable: {detail}", path.display())]
    Parse {
        /// The path.
        path: PathBuf,
        /// What is wrong.
        detail: String,
    },
    /// The directory of the job already exists.
    #[error("the job directory {} already exists", path.display())]
    Exists {
        /// The path.
        path: PathBuf,
    },
    /// The record does not belong to the job.
    #[error("the record names job {record}, not {job}")]
    Mismatch {
        /// The job the directory is for.
        job: String,
        /// The job the record names.
        record: String,
    },
    /// A line of the event log is not an event.
    #[error("the event log {} has a line that is not an event at byte {offset}: {detail}", path.display())]
    Corrupt {
        /// The log.
        path: PathBuf,
        /// The byte offset of the line.
        offset: u64,
        /// What is wrong.
        detail: String,
    },
}

/// Wraps a failed call on `path` as a [`JobError::Io`].
pub(crate) fn io_error<'a>(
    op: &'static str,
    path: &'a Path,
) -> impl FnOnce(io::Error) -> JobError + 'a {
    move |source| JobError::Io {
        op,
        path: path.to_path_buf(),
        detail: source.to_string(),
    }
}

/// Syncs the directory `dir`, so a rename or a new entry in it survives a crash.
pub(crate) fn sync_dir(dir: &Path) -> Result<(), JobError> {
    File::open(dir)
        .and_then(|handle| handle.sync_all())
        .map_err(io_error("sync a directory", dir))
}

/// Writes `bytes` to `target` by temp file, fsync of the file, rename and fsync of the directory.
pub(crate) fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), JobError> {
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = target.with_file_name(format!("{name}.{}{TEMP_SUFFIX}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temp)
        .map_err(io_error("create a temp file", &temp))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(io_error("write a temp file", &temp))?;
    drop(file);
    fs::rename(&temp, target).map_err(io_error("rename a file into place", target))?;
    if let Some(parent) = target.parent() {
        sync_dir(parent)?;
    }
    Ok(())
}

/// A job's directory and the records in it.
#[derive(Debug, Clone)]
pub struct JobDir {
    job: JobId,
    path: PathBuf,
}

impl JobDir {
    /// The directory `path` of the job `job`.
    #[must_use]
    pub const fn new(job: JobId, path: PathBuf) -> Self {
        Self { job, path }
    }

    /// Creates `<store>/jobs/<id>/` with `children/` and `control/`, and writes `job.json`.
    ///
    /// # Errors
    /// [`JobError::Exists`] when the directory is there, [`JobError::Mismatch`] when `rec` names
    /// another job, [`JobError::Io`] when a write fails.
    pub fn create(store: &StoreLayout, id: JobId, rec: &JobRecord) -> Result<Self, JobError> {
        if rec.id() != &id {
            return Err(JobError::Mismatch {
                job: id.to_string(),
                record: rec.id().to_string(),
            });
        }
        let path = store.job_dir(&id);
        let jobs = path.parent().map(Path::to_path_buf).unwrap_or_default();
        fs::create_dir_all(&jobs).map_err(io_error("create the jobs directory", &jobs))?;
        match fs::create_dir(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(JobError::Exists { path });
            }
            Err(error) => return Err(io_error("create the job directory", &path)(error)),
        }
        let dir = Self { job: id, path };
        for sub in [dir.children_dir(), dir.control_dir()] {
            fs::create_dir(&sub).map_err(io_error("create a job subdirectory", &sub))?;
        }
        dir.write_record(rec)?;
        sync_dir(&jobs)?;
        Ok(dir)
    }

    /// Opens the directory of the job `id`.
    ///
    /// # Errors
    /// [`JobError::Io`] when the directory is not there.
    pub fn open(store: &StoreLayout, id: &JobId) -> Result<Self, JobError> {
        let path = store.job_dir(id);
        let meta = fs::metadata(&path).map_err(io_error("open the job directory", &path))?;
        if !meta.is_dir() {
            return Err(JobError::Parse {
                path,
                detail: "not a directory".to_owned(),
            });
        }
        Ok(Self {
            job: id.clone(), // BORROW: the directory owns its job id
            path,
        })
    }

    /// The directory's path.
    #[must_use]
    pub fn path<'a>(&'a self) -> &'a Path {
        self.path.as_path()
    }

    /// The directory of the process and container records, which a supervisor takes as its
    /// `record_dir`.
    #[must_use]
    pub fn children_dir(&self) -> PathBuf {
        self.path.join(CHILDREN_DIR)
    }

    /// The directory of the records of control calls; the children reader never reads it.
    #[must_use]
    pub fn control_dir(&self) -> PathBuf {
        self.path.join(CONTROL_DIR)
    }

    /// The job's record.
    ///
    /// # Errors
    /// [`JobError::Io`] or [`JobError::Parse`] when `job.json` cannot be read.
    pub fn record(&self) -> Result<JobRecord, JobError> {
        let path = self.path.join(RECORD_FILE);
        let bytes = fs::read(&path).map_err(io_error("read the job record", &path))?;
        serde_json::from_slice(&bytes).map_err(|e| JobError::Parse {
            path,
            detail: e.to_string(),
        })
    }

    /// Rewrites the record with `state`. The state is a hint for `ps`; nothing decides on it.
    ///
    /// # Errors
    /// [`JobError::Io`] or [`JobError::Parse`] when the record cannot be read or written.
    pub fn set_state(&self, state: JobState) -> Result<(), JobError> {
        let old = self.record()?;
        let new = JobRecord::builder()
            .id(old.id().clone()) // BORROW: the new record owns its id
            .supervisor(old.supervisor().clone()) // BORROW: the new record owns its supervisor
            .spec_digest(old.spec_digest().clone()) // BORROW: the new record owns its digest
            .state(state)
            .build()
            .map_err(|e| JobError::Parse {
                path: self.path.join(RECORD_FILE),
                detail: e.to_string(),
            })?;
        self.write_record(&new)
    }

    fn write_record(&self, rec: &JobRecord) -> Result<(), JobError> {
        let path = self.path.join(RECORD_FILE);
        let mut bytes = serde_json::to_vec(rec).map_err(|e| JobError::Parse {
            path: path.clone(), // BORROW: the error owns the path
            detail: e.to_string(),
        })?;
        bytes.push(b'\n');
        write_atomic(&path, &bytes)
    }

    /// Writes `children/container-<cid12>.json` as soon as the container id is known.
    ///
    /// # Errors
    /// [`JobError::Io`] when the write fails.
    pub fn write_child(&self, rec: &ContainerRecord) -> Result<(), JobError> {
        let cid = rec.cid().as_str(); // BORROW: explicit .as_str() instead of Deref coercion
        let short = cid.get(..12).unwrap_or(cid);
        let path = self.children_dir().join(format!("container-{short}.json"));
        let mut bytes = serde_json::to_vec(rec).map_err(|e| JobError::Parse {
            path: path.clone(), // BORROW: the error owns the path
            detail: e.to_string(),
        })?;
        bytes.push(b'\n');
        write_atomic(&path, &bytes)
    }

    /// The `*.json` files of `control/`, as records; one that cannot be read is listed apart.
    pub(crate) fn control_records(&self) -> Result<(Vec<ChildRecord>, Vec<PathBuf>), JobError> {
        let dir = self.control_dir();
        let mut records = Vec::new();
        let mut unreadable = Vec::new();
        for path in sorted_files(&dir).map_err(io_error("list the control directory", &dir))? {
            if path.extension().is_none_or(|ext| ext != "json") {
                unreadable.push(path);
                continue;
            }
            match fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<ChildRecord>(&bytes).ok())
            {
                Some(record) => records.push(record),
                None => unreadable.push(path),
            }
        }
        Ok((records, unreadable))
    }
}

/// The paths of the entries of `dir`, sorted by file name.
fn sorted_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir)? {
        paths.push(entry?.path());
    }
    paths.sort();
    Ok(paths)
}

/// Reads one file of `children/` as an entry; whatever goes wrong becomes `Unreadable`.
fn read_entry(path: PathBuf) -> ChildEntry {
    let unreadable = |path: PathBuf, error: String| ChildEntry::Unreadable { path, error };
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => return unreadable(path, error.to_string()),
    };
    let value: Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(error) => return unreadable(path, error.to_string()),
    };
    match value.get("schema").and_then(Value::as_str) {
        Some(CHILD_RECORD_SCHEMA) => match serde_json::from_value::<ChildRecord>(value) {
            Ok(record) => ChildEntry::Process(record),
            Err(error) => unreadable(path, error.to_string()),
        },
        Some(CONTAINER_RECORD_SCHEMA) => match serde_json::from_value::<ContainerRecord>(value) {
            Ok(record) => ChildEntry::Container(record),
            Err(error) => unreadable(path, error.to_string()),
        },
        Some(other) => unreadable(path, format!("unknown schema `{other}`")),
        None => unreadable(path, "no `schema` member".to_owned()),
    }
}

impl ChildRecorder for JobDir {
    fn record_child(&self, record: &ContainerRecord) -> io::Result<()> {
        self.write_child(record)
            .map_err(|error| io::Error::other(error.to_string()))
    }
}

impl JobView for JobDir {
    fn job<'a>(&'a self) -> &'a JobId {
        &self.job
    }

    fn children(&self) -> Result<Vec<ChildEntry>, JobViewError> {
        let dir = self.children_dir();
        let paths = sorted_files(&dir).map_err(|e| JobViewError::Read {
            detail: format!("{}: {e}", dir.display()),
        })?;
        // Only `*.json` files are records; any other name (a container's `.cid` file, a
        // leftover temp file) is neither a record nor unreadable.
        Ok(paths
            .into_iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .map(read_entry)
            .collect())
    }

    fn is_lost(&self, job: &JobId) -> Result<bool, JobViewError> {
        let read = |e: JobError| JobViewError::Read {
            detail: e.to_string(),
        };
        let other;
        let dir = if job == &self.job {
            self
        } else {
            // The directories of the jobs of a store sit side by side.
            let Some(path) = self.path.parent().map(|jobs| jobs.join(job.as_str()))
            // BORROW: explicit .as_str() instead of Deref coercion
            else {
                return Ok(false);
            };
            if !path.is_dir() {
                return Ok(false);
            }
            other = Self::new(job.clone(), path); // BORROW: the directory owns its job id
            &other
        };
        Ok(lost::classify(dir).map_err(read)? == lost::JobLiveness::Lost)
    }
}
