//! The files-first run store: blobs, runs, outcome files and the rebuildable index.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use candler_schemas::primitives::FormatError;
use serde_json::Value;

pub mod blob;
pub mod index;
pub mod layout;
pub mod outcome;
pub mod run;
pub(crate) mod sync;

use layout::StoreLayout;

/// The `schema` value of the marker file.
const STORE_SCHEMA: &str = "candler-store/1";

/// Why a store operation failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StoreError {
    /// A file system call failed.
    #[error("{op} failed on {}: {source}", path.display())]
    Io {
        /// What the store was doing.
        op: &'static str,
        /// The path the call named.
        path: PathBuf,
        /// The error of the call.
        source: io::Error,
    },
    /// The directory has no valid `store.json` marker.
    #[error("{} is not a store: {detail}", path.display())]
    NotAStore {
        /// The directory that was opened.
        path: PathBuf,
        /// What is wrong with the marker.
        detail: String,
    },
    /// Bytes do not have the digest that was expected of them.
    #[error("content digest {actual} differs from the expected {expected}")]
    DigestMismatch {
        /// The digest the caller named.
        expected: String,
        /// The digest of the bytes.
        actual: String,
    },
    /// A run directory exists and the caller did not ask to replace it, or a finished run cannot
    /// be replaced.
    #[error("run {run} already exists")]
    RunExists {
        /// The key of the run.
        run: String,
    },
    /// The run is sealed by `run.done` and cannot change.
    #[error("run {run} is sealed")]
    RunSealed {
        /// The key of the run.
        run: String,
    },
    /// A manifest does not belong to the run it was written for.
    #[error("manifest of run {run} {detail}")]
    ManifestMismatch {
        /// The key of the run.
        run: String,
        /// How the manifest differs.
        detail: &'static str,
    },
    /// A raw stream is already stored in the run.
    #[error("stream {stream} of run {run} is already stored")]
    StreamExists {
        /// The key of the run.
        run: String,
        /// The stream name.
        stream: String,
    },
    /// The final outcome file of a request exists and is never replaced.
    #[error("outcome file {} already exists", path.display())]
    OutcomeExists {
        /// The final file.
        path: PathBuf,
    },
    /// A record does not belong to the request its writer was opened for.
    #[error("record does not belong to this outcome file: {detail}")]
    RecordMismatch {
        /// What differs.
        detail: &'static str,
    },
    /// A line of the index other than the last does not parse; `rebuild_index` repairs it.
    #[error("{} line {line} does not parse", path.display())]
    IndexCorrupt {
        /// The index file.
        path: PathBuf,
        /// The line number, from 1.
        line: usize,
    },
    /// A stored file is not a valid record.
    #[error("{} does not parse: {detail}", path.display())]
    Parse {
        /// The file.
        path: PathBuf,
        /// What the reader said.
        detail: String,
    },
    /// A digest or key could not be formed.
    #[error(transparent)]
    Format(#[from] FormatError),
}

impl Store {
    /// Marks `root` as a store, creating the directory when it is absent. A directory that already
    /// holds the marker is opened as it is.
    ///
    /// # Errors
    /// Returns [`StoreError::Io`] when the directory or the marker cannot be written, and
    /// [`StoreError::NotAStore`] when a marker is present but is not `candler-store/1`.
    pub fn init(root: &Path) -> Result<Self, StoreError> {
        fs::create_dir_all(root).map_err(io_error("create the store root", root))?;
        let layout = StoreLayout::new(root);
        if layout.marker().exists() {
            return Self::open(root);
        }
        let marker = format!("{{\"schema\":\"{STORE_SCHEMA}\"}}\n");
        write_atomic(&layout.marker(), marker.as_bytes())?;
        sync_dir(root)?;
        Ok(Self { layout })
    }

    /// Opens the store at `root`.
    ///
    /// # Errors
    /// Returns [`StoreError::NotAStore`] when `root` has no `store.json` marker of schema
    /// `candler-store/1`.
    pub fn open(root: &Path) -> Result<Self, StoreError> {
        let layout = StoreLayout::new(root);
        let not_a_store = |detail: String| StoreError::NotAStore {
            path: root.to_path_buf(),
            detail,
        };
        let bytes = fs::read(layout.marker()).map_err(|e| not_a_store(e.to_string()))?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|e| not_a_store(e.to_string()))?;
        match value.get("schema").and_then(Value::as_str) {
            Some(STORE_SCHEMA) => Ok(Self { layout }),
            Some(other) => Err(not_a_store(format!("schema is `{other}`"))),
            None => Err(not_a_store(String::from("the marker has no `schema`"))),
        }
    }

    /// The paths of this store.
    #[must_use]
    pub const fn layout<'a>(&'a self) -> &'a StoreLayout {
        &self.layout
    }
}

/// A store: a directory marked by `store.json`, read and written through its layout.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Store {
    layout: StoreLayout,
}

/// Wraps a failed call on `path` as a [`StoreError::Io`] of `op`.
pub(crate) fn io_error<'a>(
    op: &'static str,
    path: &'a Path,
) -> impl FnOnce(io::Error) -> StoreError + 'a {
    move |source| StoreError::Io {
        op,
        path: path.to_path_buf(),
        source,
    }
}

/// A hidden name beside `target` that no reader lists and no two writers share.
pub(crate) fn temp_sibling(target: &Path) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let stem = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = format!(".tmp-{}-{n}-{stem}", std::process::id());
    target.with_file_name(name)
}

/// Reports whether a directory entry name belongs to a reader: not hidden and not a partial.
pub(crate) fn is_visible(name: &str) -> bool {
    !name.starts_with('.') && !name.ends_with(".partial")
}

/// The sorted names of the visible entries of `dir`; a missing directory has none.
pub(crate) fn visible_names(dir: &Path) -> Result<Vec<String>, StoreError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(io_error("list a directory", dir)(source)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(io_error("list a directory", dir))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_visible(&name) {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// Flushes the entries of a directory to disk.
pub(crate) fn sync_dir(dir: &Path) -> Result<(), StoreError> {
    sync::sync_dir(dir).map_err(io_error("sync a directory", dir))
}

/// Creates the new file `path`, writes `bytes` and syncs them; a failed write leaves no file.
pub(crate) fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let written = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            sync::sync_file(&file)
        });
    if let Err(source) = written {
        // EXPLICIT: the file may not exist; the write error is the one to report
        let _ = fs::remove_file(path);
        return Err(io_error("write a file", path)(source));
    }
    Ok(())
}

/// Writes `bytes` to a temporary name beside `target`, syncs them and renames them onto `target`.
/// The directory is not synced.
pub(crate) fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let temp = temp_sibling(target);
    write_new_synced(&temp, bytes)?;
    fs::rename(&temp, target).map_err(io_error("rename a file into place", target))
}
