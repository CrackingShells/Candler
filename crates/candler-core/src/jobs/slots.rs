//! The slots that bound how many runs of all jobs execute at once.
//!
//! A slot is a pair of files under the slots directory: `<n>.lock`, the target of an exclusive
//! `flock`, and `<n>.json`, who holds it. The kernel releases the lock when the holder dies, so a
//! free lock with a leftover `.json` is a crashed holder: reported as stale, never as held.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::future::BoxFuture;
use rustix::fs::{FlockOperation, flock};
use rustix::io::Errno;
use serde::de::value::MapAccessDeserializer;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::dir::write_atomic;

/// How often [`Slots::acquire`] tries again.
const POLL: Duration = Duration::from_millis(50);

/// The directory of the evaluator slot pool under the user state directory `state`: a directory of
/// its own, apart from the agent pool at `<state>/candler/slots`, so that a held agent slot never
/// delays an evaluator and the reverse.
#[must_use]
pub fn evaluator_slots_dir(state: &Path) -> PathBuf {
    state.join("candler").join("evaluator-slots")
}

/// The size of the evaluator pool when `slots.evaluators` is not set: half of `cpus`, rounded down,
/// and at least 1. Evaluators are short and CPU-bound, while the agents mostly wait on the model.
#[must_use]
pub fn default_evaluator_slots(cpus: usize) -> u32 {
    u32::try_from(cpus / 2).unwrap_or(u32::MAX).max(1)
}

/// Why a slot could not be taken.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SlotError {
    /// The slots are not implemented.
    #[error("slots are not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
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
}

fn io_error<'a>(op: &'static str, path: &'a Path) -> impl FnOnce(io::Error) -> SlotError + 'a {
    move |source| SlotError::Io {
        op,
        path: path.to_path_buf(),
        detail: source.to_string(),
    }
}

/// Who holds a slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    /// The job that holds it.
    pub job: String,
    /// The run that holds it, when a run does.
    pub run: Option<String>,
}

/// The content of `<n>.json`: an object, with `run` omitted when no run holds the slot.
///
/// The derive is `remote = "Self"`, which makes its functions inherent; the impls below wrap them
/// so the reader takes an object and nothing else, as the wire shapes of `candler-schemas` do.
#[derive(Debug, Serialize, Deserialize)]
#[serde(remote = "Self")]
struct HolderFile {
    job: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    run: Option<String>,
    pid: u32,
    since: String,
}

impl Serialize for HolderFile {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for HolderFile {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectOnly;

        impl<'de> Visitor<'de> for ObjectOnly {
            type Value = HolderFile;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<HolderFile, A::Error> {
                HolderFile::deserialize(MapAccessDeserializer::new(map))
            }
        }

        deserializer.deserialize_map(ObjectOnly)
    }
}

/// A held slot; dropping it releases the slot.
#[derive(Debug)]
pub struct Slot {
    /// The slot's number.
    pub index: u32,
    json: PathBuf,
    // The descriptor holds the lock; dropping it releases it.
    _lock: File,
}

impl Drop for Slot {
    fn drop(&mut self) {
        // The holder file goes first, while the lock is still held, so no other holder's file is
        // ever removed. A failure leaves a stale file, which `held` reports.
        let _ = fs::remove_file(&self.json);
    }
}

/// What a slot looks like from outside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotInfo {
    /// The slot's number.
    pub index: u32,
    /// Who holds it, if anyone.
    pub holder: Option<Holder>,
    /// Whether the holder is gone and the slot can be taken over.
    pub stale: bool,
}

/// The slots of a store.
#[derive(Debug, Clone)]
pub struct Slots {
    dir: PathBuf,
    count: u32,
}

/// Tries the exclusive lock on `file` without waiting; `false` when another descriptor holds it.
fn try_lock(file: &File, path: &Path) -> Result<bool, SlotError> {
    match flock(file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(true),
        Err(Errno::WOULDBLOCK) => Ok(false),
        Err(error) => Err(SlotError::Io {
            op: "lock a slot",
            path: path.to_path_buf(),
            detail: error.to_string(),
        }),
    }
}

impl Slots {
    /// Opens the slots under `dir`, `count` of them: the caller passes the smaller of
    /// `slots.count` and the job's `max_parallel`.
    ///
    /// # Errors
    /// [`SlotError`] when the slot files cannot be opened.
    pub fn open(dir: &Path, count: u32) -> Result<Self, SlotError> {
        fs::create_dir_all(dir).map_err(io_error("create the slots directory", dir))?;
        for n in 0..count {
            let path = lock_path(dir, n);
            open_lock(&path)?;
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            count,
        })
    }

    /// How many slots there are.
    #[must_use]
    pub const fn count(&self) -> u32 {
        self.count
    }

    /// Waits for a free slot and takes it, trying every 50 ms.
    ///
    /// # Errors
    /// [`SlotError`] when the slots cannot be read.
    pub fn acquire<'a>(&'a self, holder: &'a Holder) -> BoxFuture<'a, Result<Slot, SlotError>> {
        Box::pin(async move {
            loop {
                if let Some(slot) = self.try_acquire(holder)? {
                    return Ok(slot);
                }
                tokio::time::sleep(POLL).await;
            }
        })
    }

    /// Takes the first free slot, or returns `None` when there is none.
    ///
    /// # Errors
    /// [`SlotError`] when the slots cannot be read.
    pub fn try_acquire(&self, holder: &Holder) -> Result<Option<Slot>, SlotError> {
        for n in 0..self.count {
            let path = lock_path(&self.dir, n);
            let file = open_lock(&path)?;
            if !try_lock(&file, &path)? {
                continue;
            }
            let json = json_path(&self.dir, n);
            let body = HolderFile {
                job: holder.job.clone(), // BORROW: the file owns its text
                run: holder.run.clone(), // BORROW: the file owns its text
                pid: std::process::id(),
                since: jiff::Timestamp::now().to_string(),
            };
            let mut bytes = serde_json::to_vec(&body).map_err(|e| SlotError::Io {
                op: "write a slot holder",
                path: json.clone(), // BORROW: the error owns the path
                detail: e.to_string(),
            })?;
            bytes.push(b'\n');
            write_atomic(&json, &bytes).map_err(|e| SlotError::Io {
                op: "write a slot holder",
                path: json.clone(), // BORROW: the error owns the path
                detail: e.to_string(),
            })?;
            return Ok(Some(Slot {
                index: n,
                json,
                _lock: file,
            }));
        }
        Ok(None)
    }

    /// Every slot, with who holds it. A free lock with a leftover holder file is stale.
    ///
    /// # Errors
    /// [`SlotError`] when a slot cannot be read.
    pub fn held(&self) -> Result<Vec<SlotInfo>, SlotError> {
        let mut infos = Vec::new();
        for n in 0..self.count {
            let path = lock_path(&self.dir, n);
            let file = open_lock(&path)?;
            let free = try_lock(&file, &path)?;
            let holder = read_holder(&json_path(&self.dir, n));
            infos.push(SlotInfo {
                index: n,
                stale: free && holder.is_some(),
                holder,
            });
            // The probe lock, if taken, is released with `file`.
        }
        Ok(infos)
    }
}

fn lock_path(dir: &Path, n: u32) -> PathBuf {
    dir.join(format!("{n}.lock"))
}

fn json_path(dir: &Path, n: u32) -> PathBuf {
    dir.join(format!("{n}.json"))
}

fn open_lock(path: &Path) -> Result<File, SlotError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(io_error("open a slot lock", path))
}

/// The holder a `<n>.json` names, if it is there and readable.
fn read_holder(path: &Path) -> Option<Holder> {
    let bytes = fs::read(path).ok()?;
    let file: HolderFile = serde_json::from_slice(&bytes).ok()?;
    Some(Holder {
        job: file.job,
        run: file.run,
    })
}
