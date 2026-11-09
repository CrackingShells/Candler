//! The child record on disk: one `<pid>.json` file per child, written so that a reader sees the
//! whole record or none of it.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use candler_schemas::process::ChildRecord;

use crate::store::sync::{sync_dir, sync_file};

/// The path of the record of the child `pid` under `dir`.
#[must_use]
pub fn record_path(dir: &Path, pid: u32) -> PathBuf {
    dir.join(format!("{pid}.json"))
}

/// Writes the record to `<dir>/<pid>.json`: to a temporary file first, synced, then renamed into
/// place, then the directory is synced. `dir` is created if it does not exist.
///
/// # Errors
/// Returns the I/O error of creating the directory, or of writing, syncing or renaming the file.
pub fn write_record(dir: &Path, record: &ChildRecord) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = record_path(dir, record.pid());
    let tmp = dir.join(format!("{}.json.tmp", record.pid()));
    let mut bytes = serde_json::to_vec(record).map_err(io::Error::other)?;
    bytes.push(b'\n');
    let mut file = File::create(&tmp)?;
    file.write_all(&bytes)?;
    sync_file(&file)?;
    drop(file);
    fs::rename(&tmp, &path)?;
    sync_dir(dir)?;
    Ok(path)
}
