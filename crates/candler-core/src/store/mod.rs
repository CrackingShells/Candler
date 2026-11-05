//! The files-first run store: blobs, runs, outcome files and the rebuildable index.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use candler_schemas::primitives::FormatError;
use serde_json::Value;

pub mod blob;
pub mod layout;

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

/// Flushes the entries of a directory to disk.
pub(crate) fn sync_dir(dir: &Path) -> Result<(), StoreError> {
    File::open(dir)
        .and_then(|handle| handle.sync_all())
        .map_err(io_error("sync a directory", dir))
}

/// Creates the new file `path`, writes `bytes` and syncs them; a failed write leaves no file.
pub(crate) fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let written = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            file.sync_all()
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
