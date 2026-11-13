//! Runs: a directory per attempt, written step by step and sealed by `run.done` last.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use candler_schemas::execution::{Integrity, InvalidReason, RunManifest, RunTree};
use candler_schemas::primitives::{Digest, RunKey};
use candler_schemas::raw::StreamName;
use candler_schemas::trace::TraceFile;
use serde_json::Value;

use super::blob::{copy_synced, digest_of_file};
use super::layout::StoreLayout;
use super::sync::sync_file;
use super::{Store, StoreError, io_error, sync_dir, temp_sibling, visible_names, write_atomic};

/// What `RunWriter::create` does when the run directory exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Existing {
    /// Refuse.
    Refuse,
    /// Move an unfinished run aside to `runs/.replaced/<key>-<n>/` and write a new one; a finished
    /// run is still refused.
    ReplaceUnfinished,
}

/// One run directory: its key and whether `run.done` seals it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RunEntry {
    /// The key of the run.
    pub key: RunKey,
    /// Whether `run.done` exists.
    pub done: bool,
}

/// Writes one run, step by step. The run counts as complete only once [`RunWriter::done`] has
/// written `run.done`; a writer dropped before that leaves an unsealed run.
#[derive(Debug)]
#[non_exhaustive]
pub struct RunWriter {
    store: Store,
    key: RunKey,
}

/// The JSON text of a record with a trailing line feed.
fn json_bytes<T: serde::Serialize>(path: &Path, record: &T) -> Result<Vec<u8>, StoreError> {
    let mut bytes = serde_json::to_vec(record).map_err(|e| StoreError::Parse {
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Reads and parses the JSON file at `path`.
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, StoreError> {
    let bytes = fs::read(path).map_err(io_error("read a stored file", path))?;
    serde_json::from_slice(&bytes).map_err(|e| StoreError::Parse {
        path: path.to_path_buf(),
        detail: e.to_string(),
    })
}

impl RunWriter {
    /// Starts a run: its directory appears by renaming a temporary directory that already holds the
    /// manifest.
    ///
    /// # Errors
    /// Returns [`StoreError::RunExists`] when the directory exists and `existing` is
    /// [`Existing::Refuse`] or the run is finished, [`StoreError::ManifestMismatch`] when the key's
    /// cell is not the manifest's cell digest, and [`StoreError::Io`] when a write fails.
    pub fn create(
        store: &Store,
        manifest: &RunManifest,
        existing: Existing,
    ) -> Result<Self, StoreError> {
        let key = manifest.run();
        if key.cell() != &manifest.cell_digest() {
            return Err(StoreError::ManifestMismatch {
                run: key.to_string(),
                detail: "names a cell other than the digest of its comparable fields",
            });
        }
        let layout = store.layout();
        let dir = layout.run_dir(key);
        if layout.done(key).exists() {
            return Err(StoreError::RunExists {
                run: key.to_string(),
            });
        }
        if dir.exists() {
            match existing {
                Existing::Refuse => {
                    return Err(StoreError::RunExists {
                        run: key.to_string(),
                    });
                }
                Existing::ReplaceUnfinished => move_aside(layout, key)?,
            }
        }
        let runs = layout.runs_dir();
        fs::create_dir_all(&runs).map_err(io_error("create the runs directory", &runs))?;
        let temp = temp_sibling(&dir);
        fs::create_dir(&temp).map_err(io_error("create a temporary run directory", &temp))?;
        let built = write_atomic(&temp.join("manifest.json"), &json_bytes(&temp, manifest)?)
            .and_then(|()| sync_dir(&temp));
        if let Err(error) = built {
            // EXPLICIT: the temporary directory is removed best-effort; the write error is the one to report
            let _ = fs::remove_dir_all(&temp);
            return Err(error);
        }
        if let Err(source) = fs::rename(&temp, &dir) {
            // EXPLICIT: the temporary directory is removed best-effort; the rename error is the one to report
            let _ = fs::remove_dir_all(&temp);
            return Err(if dir.exists() {
                StoreError::RunExists {
                    run: key.to_string(),
                }
            } else {
                io_error("rename a run directory into place", &dir)(source)
            });
        }
        sync_dir(&runs)?;
        Ok(Self {
            store: store.clone(), // BORROW: the writer owns a handle on the store
            key: key.clone(),     // BORROW: the writer owns the key
        })
    }

    /// The key of the run being written.
    #[must_use]
    pub const fn key<'a>(&'a self) -> &'a RunKey {
        &self.key
    }

    /// Replaces the manifest; the run key stays the same.
    ///
    /// # Errors
    /// Returns [`StoreError::ManifestMismatch`] when the manifest names another run, and
    /// [`StoreError::Io`] when the write fails.
    pub fn manifest(&mut self, manifest: &RunManifest) -> Result<(), StoreError> {
        if manifest.run() != &self.key {
            return Err(StoreError::ManifestMismatch {
                run: self.key.to_string(),
                detail: "names another run",
            });
        }
        let path = self.store.layout().manifest(&self.key);
        write_atomic(&path, &json_bytes(&path, manifest)?)?;
        self.sync_run_dir()
    }

    /// Writes the run's workspace tree.
    ///
    /// # Errors
    /// Returns [`StoreError::Io`] when the write fails.
    pub fn tree(&mut self, tree: &RunTree) -> Result<(), StoreError> {
        let path = self.store.layout().tree(&self.key);
        write_atomic(&path, &json_bytes(&path, tree)?)?;
        self.sync_run_dir()
    }

    /// Moves the finished `raw-frames/1` file at `gz` into `raw/<stream>.jsonl.gz` after checking
    /// its digest. The store frames nothing itself.
    ///
    /// # Errors
    /// Returns [`StoreError::DigestMismatch`] when the file is not `digest` (the file stays where
    /// it is), [`StoreError::StreamExists`] when the stream is stored, and [`StoreError::Io`] when
    /// a move fails.
    pub fn raw(
        &mut self,
        stream: &StreamName,
        gz: &Path,
        digest: &Digest,
    ) -> Result<(), StoreError> {
        let layout = self.store.layout();
        let target = layout.raw(&self.key, stream);
        if target.exists() {
            return Err(StoreError::StreamExists {
                run: self.key.to_string(),
                stream: stream.to_string(),
            });
        }
        let actual = digest_of_file(gz)?;
        if &actual != digest {
            return Err(StoreError::DigestMismatch {
                expected: digest.to_string(),
                actual: actual.to_string(),
            });
        }
        let dir = layout.raw_dir(&self.key);
        fs::create_dir_all(&dir).map_err(io_error("create the raw directory", &dir))?;
        File::open(gz)
            .and_then(|file| sync_file(&file))
            .map_err(io_error("sync a raw file", gz))?;
        match fs::rename(gz, &target) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::CrossesDevices => {
                let temp = temp_sibling(&target);
                if let Err(error) = copy_synced(gz, &temp) {
                    // EXPLICIT: the temporary file is removed best-effort; the copy error is the one to report
                    let _ = fs::remove_file(&temp);
                    return Err(error);
                }
                fs::rename(&temp, &target)
                    .map_err(io_error("rename a raw file into place", &target))?;
                fs::remove_file(gz).map_err(io_error("remove a moved raw file", gz))?;
            }
            Err(source) => return Err(io_error("move a raw file", &target)(source)),
        }
        sync_dir(&dir)
    }

    /// Writes the normalised trace.
    ///
    /// # Errors
    /// Returns [`StoreError::Io`] when the write fails.
    pub fn trace(&mut self, trace: &TraceFile) -> Result<(), StoreError> {
        let path = self.store.layout().trace(&self.key);
        write_atomic(&path, &trace.write())?;
        self.sync_run_dir()
    }

    /// Seals the run: writes `run.done` last and syncs the directory. Nothing in the run directory
    /// changes afterwards.
    ///
    /// # Errors
    /// Returns [`StoreError::Io`] when the seal cannot be written.
    pub fn done(self) -> Result<(), StoreError> {
        seal(self.store.layout(), &self.key)
    }

    fn sync_run_dir(&self) -> Result<(), StoreError> {
        sync_dir(&self.store.layout().run_dir(&self.key))
    }
}

/// Writes `run.done` and syncs the run directory.
fn seal(layout: &StoreLayout, key: &RunKey) -> Result<(), StoreError> {
    write_atomic(&layout.done(key), b"")?;
    sync_dir(&layout.run_dir(key))
}

/// Moves the run directory of `key` to the first free `runs/.replaced/<key>-<n>`.
fn move_aside(layout: &StoreLayout, key: &RunKey) -> Result<(), StoreError> {
    let mut n = 1_u32;
    let aside = loop {
        let candidate = layout.replaced_dir(key, n);
        if !candidate.exists() {
            break candidate;
        }
        n = n.saturating_add(1);
    };
    if let Some(parent) = aside.parent() {
        fs::create_dir_all(parent).map_err(io_error("create the replaced directory", parent))?;
    }
    fs::rename(layout.run_dir(key), &aside)
        .map_err(io_error("move an unfinished run aside", &aside))?;
    sync_dir(&layout.runs_dir())
}

impl Store {
    /// One past the highest attempt present for `cell` and `replicate`, sealed or not; 1 when none
    /// exists.
    ///
    /// # Errors
    /// Returns [`StoreError::Io`] when the runs directory cannot be listed.
    pub fn next_attempt(&self, cell: &Digest, replicate: u32) -> Result<u32, StoreError> {
        let highest = self
            .runs()?
            .iter()
            .filter(|entry| entry.key.cell() == cell && entry.key.replicate() == replicate)
            .map(|entry| entry.key.attempt())
            .max()
            .unwrap_or(0);
        Ok(highest.saturating_add(1))
    }

    /// Seals an unsealed run as `invalid {interrupted}`: its manifest is rewritten with that
    /// integrity and `run.done` is written. The attempt stays in the ledger.
    ///
    /// # Errors
    /// Returns [`StoreError::RunSealed`] when the run has `run.done`, [`StoreError::Parse`] when
    /// its manifest is unreadable, and [`StoreError::Io`] when a write fails.
    pub fn seal_interrupted(&self, key: &RunKey) -> Result<RunManifest, StoreError> {
        let layout = self.layout();
        if layout.done(key).exists() {
            return Err(StoreError::RunSealed {
                run: key.to_string(),
            });
        }
        let path = layout.manifest(key);
        let mut value: Value = read_json(&path)?;
        let integrity = serde_json::to_value(Integrity::invalid(InvalidReason::Interrupted))
            .map_err(|e| StoreError::Parse {
                path: path.clone(), // BORROW: the error owns the path
                detail: e.to_string(),
            })?;
        if let Some(object) = value.as_object_mut() {
            object.insert(String::from("integrity"), integrity);
        }
        let manifest: RunManifest =
            serde_json::from_value(value).map_err(|e| StoreError::Parse {
                path: path.clone(), // BORROW: the error owns the path
                detail: e.to_string(),
            })?;
        write_atomic(&path, &json_bytes(&path, &manifest)?)?;
        seal(layout, key)?;
        Ok(manifest)
    }

    /// Every run directory, sealed or not, in key order.
    ///
    /// # Errors
    /// Returns [`StoreError::Io`] when the runs directory cannot be listed.
    pub fn runs(&self) -> Result<Vec<RunEntry>, StoreError> {
        let mut entries: Vec<RunEntry> = visible_names(&self.layout().runs_dir())?
            .iter()
            // BORROW: explicit .as_str() instead of Deref coercion
            .filter_map(|name| name.as_str().parse::<RunKey>().ok()) // BORROW: explicit .as_str() instead of Deref coercion
            .map(|key| RunEntry {
                done: self.layout().done(&key).exists(),
                key,
            })
            .collect();
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(entries)
    }

    /// The manifest of a run.
    ///
    /// # Errors
    /// Returns [`StoreError::Io`] when the file is missing and [`StoreError::Parse`] when it is not
    /// a manifest.
    pub fn read_manifest(&self, key: &RunKey) -> Result<RunManifest, StoreError> {
        read_json(&self.layout().manifest(key))
    }

    /// The workspace tree of a run.
    ///
    /// # Errors
    /// Returns [`StoreError::Io`] when the file is missing and [`StoreError::Parse`] when it is not
    /// a tree.
    pub fn read_tree(&self, key: &RunKey) -> Result<RunTree, StoreError> {
        read_json(&self.layout().tree(key))
    }

    /// The normalised trace of a run.
    ///
    /// # Errors
    /// Returns [`StoreError::Io`] when the file is missing and [`StoreError::Parse`] when it is not
    /// a trace file.
    pub fn read_trace(&self, key: &RunKey) -> Result<TraceFile, StoreError> {
        let path = self.layout().trace(key);
        let bytes = fs::read(&path).map_err(io_error("read a stored file", &path))?;
        TraceFile::read(&bytes).map_err(|e| StoreError::Parse {
            path,
            detail: e.to_string(),
        })
    }

    /// The path of a run's finished raw file for `stream`, when it is stored.
    #[must_use]
    pub fn raw_path(&self, key: &RunKey, stream: &StreamName) -> Option<PathBuf> {
        let path = self.layout().raw(key, stream);
        path.is_file().then_some(path)
    }
}
