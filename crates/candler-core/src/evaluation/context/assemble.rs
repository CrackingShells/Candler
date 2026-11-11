//! Step one of a context: choosing a run's artifacts and metadata from the needs of a suite.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use candler_schemas::evaluation::Needs;
use candler_schemas::execution::{
    DigestRef, InputEntry, RunManifest, TreeEntry, input_list_digest,
};
use candler_schemas::primitives::{Digest, FieldPath, Name, RelPath, RunKey};
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

/// The kind of the input list the image step derives. It is the image's, never an evaluator's.
const RESERVED_INPUT_KIND: &str = "build_inputs";

/// One input an evaluator needs: its kind, its digest and the stored blob of each of its files.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AssembledInput {
    /// The kind the need named.
    pub kind: Name,
    /// The digest of the input, the key of its list blob.
    pub digest: Digest,
    /// The files of the input, sorted by path relative to the input's root.
    pub files: Vec<AssembledArtifact>,
}

/// The artifacts and the metadata the needs chose for one run, before anything is copied.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Assembled {
    run: RunKey,
    store_root: PathBuf,
    artifacts: Vec<AssembledArtifact>,
    inputs: Vec<AssembledInput>,
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

    /// The inputs, one per input need, in the order of the needs.
    #[must_use]
    pub const fn inputs<'a>(&'a self) -> &'a [AssembledInput] {
        // BORROW: .as_slice() views the owned list
        self.inputs.as_slice()
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
/// There is one input per input need: the kind is looked up in the manifest's `comparable.inputs`
/// and then in its `evaluation_inputs`, and the files are those of the input's list blob. The
/// trace need is not served in this version, and the context carries none. An `outputs/`
/// with no files gives no artifacts and is not an error.
///
/// # Errors
/// Returns [`ContextError::Unsupported`] when the needs ask for snapshots,
/// [`ContextError::InputRefused`] when an input need names the reserved kind `build_inputs`, a
/// kind the manifest holds in neither list, or a kind whose list or file blobs the store lacks,
/// [`ContextError::NotSealed`] when the run has no seal, [`ContextError::Glob`] for a bad glob,
/// [`ContextError::Store`] when the manifest or tree cannot be read, and
/// [`ContextError::MissingMetadata`] when a metadata path has no value in the manifest.
pub fn assemble(store: &Store, run: &RunKey, needs: &Needs) -> Result<Assembled, ContextError> {
    if !needs.snapshots.is_empty() {
        return Err(ContextError::Unsupported {
            need: "snapshots".into(),
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
    let mut inputs = Vec::with_capacity(needs.inputs.len());
    for kind in &needs.inputs {
        inputs.push(assemble_input(store, &manifest, kind)?);
    }
    Ok(Assembled {
        run: run.clone(), // BORROW: the result owns the key
        store_root: layout.root().to_path_buf(),
        artifacts,
        inputs,
        metadata,
    })
}

/// The entry of `kind` in a manifest list of inputs.
fn held<'a>(list: &'a [DigestRef], kind: &Name) -> Option<&'a DigestRef> {
    list.iter().find(|entry| &entry.kind == kind)
}

/// The input of `kind` that `manifest` holds, with the stored blob of each of its files.
fn assemble_input(
    store: &Store,
    manifest: &RunManifest,
    kind: &Name,
) -> Result<AssembledInput, ContextError> {
    let refuse = |reason: String| ContextError::InputRefused {
        kind: kind.to_string(),
        reason,
    };
    // BORROW: explicit .as_str() instead of Deref coercion
    if kind.as_str() == RESERVED_INPUT_KIND {
        return Err(refuse("is reserved for the image".into()));
    }
    let digest = held(manifest.comparable().inputs(), kind)
        .or_else(|| held(manifest.evaluation_inputs(), kind))
        .map(|held| held.digest.clone()) // BORROW: the result owns the digest
        .ok_or_else(|| refuse("is not an input of the run".into()))?;
    let layout = store.layout();
    let list_blob = layout.blob(&digest);
    if !list_blob.is_file() {
        return Err(refuse("has no list blob in the store".into()));
    }
    let bytes = std::fs::read(&list_blob).map_err(|source| ContextError::Io {
        op: "read an input list",
        path: list_blob.clone(), // BORROW: the error owns the path
        source,
    })?;
    let entries: Vec<InputEntry> = serde_json::from_slice(&bytes).map_err(|error| {
        refuse(format!(
            "has a list blob that is not a list of files: {error}"
        ))
    })?;
    let listed = input_list_digest(&entries).map_err(|error| {
        refuse(format!(
            "has a list blob that is not an input list: {error}"
        ))
    })?;
    if listed != digest {
        return Err(refuse(
            "has a list blob that is not the canonical list of its digest".into(),
        ));
    }
    let mut files = Vec::with_capacity(entries.len());
    for entry in &entries {
        let blob = layout.blob(&entry.sha256);
        if !blob.is_file() {
            return Err(refuse(format!("lacks the blob of `{}`", entry.path)));
        }
        files.push(AssembledArtifact {
            path: entry.path.clone(),     // BORROW: the result owns its paths
            sha256: entry.sha256.clone(), // BORROW: the result owns its digests
            blob,
            executable: false,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(AssembledInput {
        kind: kind.clone(), // BORROW: the result owns the kind
        digest,
        files,
    })
}
