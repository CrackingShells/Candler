//! The one place that flushes a file or a directory to disk.
//!
//! "Synced" means `fsync(2)`: the data and the metadata of the file reach the device's cache
//! controller, which is the guarantee `File::sync_all` gives on Linux. On macOS the standard
//! library's `sync_all` and `sync_data` issue `F_FULLFSYNC` instead, which costs about 4 ms for a
//! small file against 0.06 ms for `fsync(2)`. Every site of the core goes through these functions,
//! so the cost and the guarantee are the same everywhere.

use std::fs::File;
use std::io;
use std::path::Path;

use rustix::fs::fsync;

/// Flushes the data and the metadata of `file` with `fsync(2)`.
///
/// # Errors
/// Returns the I/O error of the call.
pub(crate) fn sync_file(file: &File) -> io::Result<()> {
    fsync(file).map_err(io::Error::from)
}

/// Flushes the entries of the directory `dir`, so that a rename or a new name in it survives a
/// crash.
///
/// # Errors
/// Returns the I/O error of opening the directory or of the sync.
pub(crate) fn sync_dir(dir: &Path) -> io::Result<()> {
    sync_file(&File::open(dir)?)
}
