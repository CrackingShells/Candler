//! Write-once blobs: content stored under its own digest.

use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use candler_schemas::primitives::Digest;
use sha2::Digest as _;
use sha2::Sha256;

use super::sync::sync_file;
use super::{Store, StoreError, io_error, sync_dir, temp_sibling, write_new_synced};

/// The read-only mode of a stored blob.
const BLOB_MODE: u32 = 0o444;

/// The digest a finished hasher holds.
fn finish(hasher: Sha256) -> Result<Digest, StoreError> {
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        hex.extend(char::from_digit(u32::from(byte >> 4), 16));
        hex.extend(char::from_digit(u32::from(byte & 0xf), 16));
    }
    Ok(Digest::from_hex(&hex)?)
}

/// The digest of the bytes of the file at `path`, read in a stream.
pub(crate) fn digest_of_file(path: &Path) -> Result<Digest, StoreError> {
    let mut file = File::open(path).map_err(io_error("open a file to hash", path))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(io_error("read a file to hash", path))?;
        if read == 0 {
            return finish(hasher);
        }
        hasher.update(buffer.get(..read).unwrap_or_default());
    }
}

/// Refuses a digest that is not the expected one.
fn check_expected(expected: Option<&Digest>, actual: &Digest) -> Result<(), StoreError> {
    match expected {
        Some(want) if want != actual => Err(StoreError::DigestMismatch {
            expected: want.to_string(),
            actual: actual.to_string(),
        }),
        Some(_) | None => Ok(()),
    }
}

impl Store {
    /// The path a blob has, or would have, in this store.
    #[must_use]
    pub fn blob_path(&self, digest: &Digest) -> PathBuf {
        self.layout().blob(digest)
    }

    /// Stores `bytes` as a blob and returns their digest. The blob appears under its final name
    /// only once its bytes are synced and its mode is 0444; a blob that exists is not rewritten.
    ///
    /// # Errors
    /// Returns [`StoreError::DigestMismatch`] when `expected` is given and is not the digest of
    /// `bytes`, and [`StoreError::Io`] when a write fails.
    pub fn put_blob(&self, bytes: &[u8], expected: Option<&Digest>) -> Result<Digest, StoreError> {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = finish(hasher)?;
        check_expected(expected, &digest)?;
        let target = self.blob_path(&digest);
        if target.exists() {
            return Ok(digest);
        }
        let dir = self.layout().blobs_dir();
        fs::create_dir_all(&dir).map_err(io_error("create the blob directory", &dir))?;
        let temp = temp_sibling(&target);
        write_new_synced(&temp, bytes)?;
        Self::publish_blob(&temp, &target)?;
        Ok(digest)
    }

    /// Stores the bytes of the file at `path` as a blob by streaming it, with the guarantees of
    /// [`Store::put_blob`].
    ///
    /// # Errors
    /// Returns [`StoreError::DigestMismatch`] when `expected` is given and is not the digest of
    /// the file, and [`StoreError::Io`] when a read or a write fails.
    pub fn put_blob_from_path(
        &self,
        path: &Path,
        expected: Option<&Digest>,
    ) -> Result<Digest, StoreError> {
        let digest = digest_of_file(path)?;
        check_expected(expected, &digest)?;
        let target = self.blob_path(&digest);
        if target.exists() {
            return Ok(digest);
        }
        let dir = self.layout().blobs_dir();
        fs::create_dir_all(&dir).map_err(io_error("create the blob directory", &dir))?;
        let temp = temp_sibling(&target);
        let copied = copy_synced(path, &temp);
        if let Err(error) = copied {
            // EXPLICIT: the temporary file may not exist; the copy error is the one to report
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
        // The file may have changed between the hash and the copy: the copy is hashed again.
        let stored = digest_of_file(&temp)?;
        if stored != digest {
            // EXPLICIT: the mismatch is the error to report
            let _ = fs::remove_file(&temp);
            return Err(StoreError::DigestMismatch {
                expected: digest.to_string(),
                actual: stored.to_string(),
            });
        }
        Self::publish_blob(&temp, &target)?;
        Ok(digest)
    }

    /// Makes a synced temporary file read-only and renames it to the blob name.
    fn publish_blob(temp: &Path, target: &Path) -> Result<(), StoreError> {
        fs::set_permissions(temp, Permissions::from_mode(BLOB_MODE))
            .map_err(io_error("make a blob read-only", temp))?;
        fs::rename(temp, target).map_err(io_error("rename a blob into place", target))?;
        match target.parent() {
            Some(dir) => sync_dir(dir),
            None => Ok(()),
        }
    }
}

/// Copies `from` to the new file `to` and syncs it.
pub(crate) fn copy_synced(from: &Path, to: &Path) -> Result<(), StoreError> {
    let mut source = File::open(from).map_err(io_error("open a blob source", from))?;
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
        .map_err(io_error("create a temporary blob", to))?;
    std::io::copy(&mut source, &mut target).map_err(io_error("copy a blob", to))?;
    target.flush().map_err(io_error("flush a blob", to))?;
    sync_file(&target).map_err(io_error("sync a blob", to))
}
