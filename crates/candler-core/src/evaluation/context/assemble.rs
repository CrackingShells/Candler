//! Step one of a context: choosing a run's artifacts and metadata from the needs of a suite.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use candler_schemas::evaluation::Needs;
use candler_schemas::execution::TreeEntry;
use candler_schemas::primitives::{Digest, FieldPath, RelPath, RunKey};
use candler_schemas::values::JsonValue;

use super::ContextError;
use super::rule::FileRule;
use crate::store::Store;

/// One artifact chosen for a context: its path below `outputs/`, the digest the store records
/// for its blob, where the blob lies and whether the file had an execute bit.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AssembledArtifact {
    /// The path relative to `outputs/`, with `/` separators.
    pub path: RelPath,
    /// The digest the store records for the file's bytes.
    pub sha256: Digest,
    /// The blob in the store. It is only read, never exposed to a plug-in.
    pub blob: PathBuf,
    /// Whether the stored file had an execute bit.
    pub executable: bool,
}

/// The artifacts and the metadata the needs chose for one run, before anything is copied.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Assembled {
    run: RunKey,
    store_root: PathBuf,
    artifacts: Vec<AssembledArtifact>,
    metadata: BTreeMap<FieldPath, JsonValue>,
}

impl Assembled {
    /// The key of the run.
    #[must_use]
    pub const fn run<'a>(&'a self) -> &'a RunKey {
        &self.run
    }

    /// The root directory of the store the run was read from.
    #[must_use]
    pub fn store_root<'a>(&'a self) -> &'a Path {
        self.store_root.as_path()
    }

    /// The chosen artifacts, sorted by path.
    #[must_use]
    pub const fn artifacts<'a>(&'a self) -> &'a [AssembledArtifact] {
        // BORROW: .as_slice() views the owned list
        self.artifacts.as_slice()
    }

    /// The metadata, one entry per metadata need.
    #[must_use]
    pub const fn metadata<'a>(&'a self) -> &'a BTreeMap<FieldPath, JsonValue> {
        &self.metadata
    }
}

/// Chooses the artifacts and metadata of the sealed run `run` that `needs` name.
///
/// The artifacts are the files of the run's stored `outputs/` that the file rule keeps for the
/// artifact globs, by path relative to `outputs/`. There is one metadata entry per metadata
/// need, keyed by the dotted path, holding the JSON leaf of the manifest's `comparable.values`.
/// The trace need is not served in this version, and the context carries none. An `outputs/`
/// with no files gives no artifacts and is not an error.
///
/// # Errors
/// Returns [`ContextError::Unsupported`] when the needs ask for snapshots or inputs,
/// [`ContextError::NotSealed`] when the run has no seal, [`ContextError::Glob`] for a bad glob,
/// [`ContextError::Store`] when the manifest or tree cannot be read, and
/// [`ContextError::MissingMetadata`] when a metadata path has no value in the manifest.
pub fn assemble(store: &Store, run: &RunKey, needs: &Needs) -> Result<Assembled, ContextError> {
    if !needs.snapshots.is_empty() {
        return Err(ContextError::Unsupported {
            need: "snapshots".into(),
        });
    }
    if !needs.inputs.is_empty() {
        return Err(ContextError::Unsupported {
            need: "inputs".into(),
        });
    }
    let layout = store.layout();
    if !layout.done(run).is_file() {
        return Err(ContextError::NotSealed {
            run: run.to_string(),
        });
    }
    let rule = FileRule::new(needs.artifacts.as_slice())?; // BORROW: explicit .as_slice()
    let tree = store.read_tree(run)?;
    let mut artifacts = Vec::new();
    for entry in tree.entries() {
        if let TreeEntry::File {
            path,
            sha256,
            executable,
            ..
        } = entry
            && rule.keeps(path)
        {
            artifacts.push(AssembledArtifact {
                path: path.clone(),     // BORROW: the result owns its paths
                sha256: sha256.clone(), // BORROW: the result owns its digests
                blob: layout.blob(sha256),
                executable: *executable,
            });
        }
    }
    artifacts.sort_by(|a, b| a.path.cmp(&b.path));
    let manifest = store.read_manifest(run)?;
    let mut metadata = BTreeMap::new();
    for path in &needs.metadata {
        let value = manifest
            .comparable()
            .values()
            .iter()
            .find(|value| value.path() == path)
            .ok_or_else(|| ContextError::MissingMetadata {
                path: path.to_string(),
                detail: "is not a value of the run manifest",
            })?;
        let json = value.value().ok_or_else(|| ContextError::MissingMetadata {
            path: path.to_string(),
            detail: "has no recorded value in the run manifest",
        })?;
        metadata.insert(path.clone(), json.clone()); // BORROW: the result owns its entries
    }
    Ok(Assembled {
        run: run.clone(), // BORROW: the result owns the key
        store_root: layout.root().to_path_buf(),
        artifacts,
        metadata,
    })
}
