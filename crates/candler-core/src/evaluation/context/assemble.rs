//! Step one of a context: choosing a run's artifacts and metadata from the needs of a suite.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use candler_schemas::evaluation::Needs;
use candler_schemas::execution::{
    DigestRef, InputEntry, RunManifest, TreeEntry, input_list_digest,
};
use candler_schemas::primitives::{Digest, FieldPath, Name, RelPath, RunKey};
use candler_schemas::raw::StreamName;
use candler_schemas::values::JsonValue;

use super::rule::FileRule;
use super::{ContextError, RunPart};
use crate::store::Store;
use crate::store::blob::digest_of_file;
use crate::store::layout::{MANIFEST_FILE, RAW_DIR, TRACE_FILE, raw_file_name};

/// What a stored file is in the context it is served in.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Served {
    /// An artifact, named by its path relative to `outputs/`.
    Artifact(RelPath),
    /// A file of an input; the kind is the directory it lies in.
    Input,
    /// The run's normalised trace, `trace.jsonl`.
    Trace,
    /// One raw stream of the run, `raw/<stream>.jsonl.gz`.
    Raw(StreamName),
    /// The run's manifest, `manifest.json`.
    Manifest,
}

/// One stored file chosen for a context: where it lies in the store, where its copy goes below
/// the workspace, the digest its copy must have and what it is served as.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AssembledFile {
    /// The stored file. It is only read, never exposed to a plug-in.
    pub source: PathBuf,
    /// The path of the copy relative to the workspace, with `/` separators.
    pub workspace: RelPath,
    /// The digest the bytes of the file have: the store's record for a blob, the hash taken when
    /// the file was chosen for any other file.
    pub sha256: Digest,
    /// Whether the stored file had an execute bit.
    pub executable: bool,
    /// What the file is served as.
    pub served: Served,
}

/// The kind of the input list the image step derives. It is the image's, never an evaluator's.
const RESERVED_INPUT_KIND: &str = "build_inputs";

/// One input an evaluator needs: its kind and its digest. Its files are among
/// [`Assembled::files`], served as [`Served::Input`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AssembledInput {
    /// The kind the need named.
    pub kind: Name,
    /// The digest of the input, the key of its list blob.
    pub digest: Digest,
}

/// The files and the metadata the needs chose for one run, before anything is copied.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Assembled {
    run: RunKey,
    store_root: PathBuf,
    files: Vec<AssembledFile>,
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

    /// Every stored file the context serves, in the order they are copied: the artifacts sorted
    /// by path, the files of each input in the order of the needs, then the trace, the raw
    /// streams sorted by name and the manifest.
    #[must_use]
    pub const fn files<'a>(&'a self) -> &'a [AssembledFile] {
        // BORROW: .as_slice() views the owned list
        self.files.as_slice()
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

/// Chooses the stored files and the metadata of the sealed run `run` that `needs` name.
///
/// A context serves exactly what the needs name. The engine builds one context per request, from
/// the union of the needs of the kinds of the entries the request sends, so a part is served
/// when a kind of the request needs it. The artifacts are the files of the run's
/// stored `outputs/` that the file rule keeps for the artifact globs, by path relative to
/// `outputs/`. There is one metadata entry per metadata need, keyed by the dotted path, holding
/// the JSON leaf of the manifest's `comparable.values`. There is one input per input need: the
/// kind is looked up in the manifest's `comparable.inputs` and then in its `evaluation_inputs`,
/// and the files are those of the input's list blob. The trace need serves the stored
/// `trace.jsonl`, the raw need serves every stored `raw/<stream>.jsonl.gz`, and the manifest need
/// serves the stored `manifest.json`; each is hashed here, at assembly, and its copy is checked
/// against that hash. A need the run cannot meet is refused: a trace or raw need on a run that
/// does not store that part, an input the run lacks, a metadata path with no value, and
/// snapshots. A stored trace can hold its header alone, as when the reader failed or the session
/// was silent, and it is served as stored. A run holds its manifest from the moment its directory exists, so that need has no
/// refusal of its own. An `outputs/` with no files gives no artifacts and is not an error.
///
/// # Errors
/// Returns [`ContextError::Unsupported`] when the needs ask for snapshots,
/// [`ContextError::InputRefused`] when an input need names the reserved kind `build_inputs`, a
/// kind the manifest holds in neither list, or a kind whose list or file blobs the store lacks,
/// [`ContextError::PartMissing`] when the trace or the raw streams are needed and the run stores
/// none, [`ContextError::NotSealed`] when the run has no seal, [`ContextError::Glob`] for a bad
/// glob, [`ContextError::Store`] when the manifest or tree cannot be read or `raw/` holds an
/// entry that is not a raw stream file,
/// [`ContextError::Io`] when a stored file cannot be read, and
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
            artifacts.push(AssembledFile {
                source: layout.blob(sha256),
                workspace: below("outputs", path.as_str())?, // BORROW: explicit .as_str()
                sha256: sha256.clone(),                      // BORROW: the result owns its digests
                executable: *executable,
                served: Served::Artifact(path.clone()), // BORROW: the result owns its paths
            });
        }
    }
    artifacts.sort_by(|a, b| a.workspace.cmp(&b.workspace));
    let mut files = artifacts;
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
        let (input, input_files) = assemble_input(store, &manifest, kind)?;
        inputs.push(input);
        files.extend(input_files);
    }
    if needs.trace {
        let source = layout.trace(run);
        match std::fs::metadata(&source) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(ContextError::PartMissing {
                    run: run.clone(), // BORROW: the error owns the key
                    part: RunPart::Trace,
                });
            }
            Err(error) => {
                return Err(ContextError::Io {
                    op: "stat the trace",
                    path: source,
                    source: error,
                });
            }
        }
        files.push(stored_file(source, TRACE_FILE, Served::Trace)?);
    }
    if needs.raw {
        let streams = store.raw_streams(run)?;
        if streams.is_empty() {
            return Err(ContextError::PartMissing {
                run: run.clone(), // BORROW: the error owns the key
                part: RunPart::Raw,
            });
        }
        for stream in streams {
            let source = layout.raw(run, &stream);
            let workspace = format!("{RAW_DIR}/{}", raw_file_name(&stream));
            files.push(stored_file(source, &workspace, Served::Raw(stream))?);
        }
    }
    if needs.manifest {
        files.push(stored_file(
            layout.manifest(run),
            MANIFEST_FILE,
            Served::Manifest,
        )?);
    }
    Ok(Assembled {
        run: run.clone(), // BORROW: the result owns the key
        store_root: layout.root().to_path_buf(),
        files,
        inputs,
        metadata,
    })
}

/// The path `name` below the workspace directory `dir`.
fn below(dir: &str, name: &str) -> Result<RelPath, ContextError> {
    stored_path(&format!("{dir}/{name}"))
}

/// `text` as a path relative to the workspace.
fn stored_path(text: &str) -> Result<RelPath, ContextError> {
    RelPath::from_str(text).map_err(|_| ContextError::Workspace {
        path: PathBuf::from(text),
        detail: "is not a relative path",
    })
}

/// A stored file that has no digest on record: its hash is taken here, at assembly, and its copy
/// is checked against it. The workspace name is the file's name in the store, so an evaluator
/// finds `trace.jsonl`, `raw/<stream>.jsonl.gz` and `manifest.json` where a run directory holds
/// them; both take their spelling from `store::layout`.
fn stored_file(
    source: PathBuf,
    workspace: &str,
    served: Served,
) -> Result<AssembledFile, ContextError> {
    let sha256 = digest_of_file(&source)?;
    Ok(AssembledFile {
        source,
        workspace: stored_path(workspace)?,
        sha256,
        executable: false,
        served,
    })
}

/// The entry of `kind` in a manifest list of inputs.
fn held<'a>(list: &'a [DigestRef], kind: &Name) -> Option<&'a DigestRef> {
    list.iter().find(|entry| &entry.kind == kind)
}

/// The input of `kind` that `manifest` holds, and the stored blob of each of its files.
fn assemble_input(
    store: &Store,
    manifest: &RunManifest,
    kind: &Name,
) -> Result<(AssembledInput, Vec<AssembledFile>), ContextError> {
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
        files.push(AssembledFile {
            source: blob,
            // BORROW: explicit .as_str()
            workspace: below(&format!("inputs/{}", kind.as_str()), entry.path.as_str())?,
            sha256: entry.sha256.clone(), // BORROW: the result owns its digests
            executable: false,
            served: Served::Input,
        });
    }
    files.sort_by(|a, b| a.workspace.cmp(&b.workspace));
    let input = AssembledInput {
        kind: kind.clone(), // BORROW: the result owns the kind
        digest,
    };
    Ok((input, files))
}
