//! Ingesting raw trace directories: every harness stream found under a path becomes one sealed run, the same run when the directory is ingested again, and nothing but relative paths is stored.
//!
//! A stream is recognised by its first line, through a [`Recogniser`] the caller supplies, so the
//! core names no harness. The run's cell holds only what the stream states, the harness and its
//! version, under a treatment named `unrecorded`; replicates are numbered in the order of the
//! streams' digests, so that any set of streams gives the same keys on a fresh store, and a
//! stream the store already holds is never written again.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use candler_schemas::execution::{
    Comparable, HarnessIdentity, ImportRecord, ImportSource, Integrity, InvalidReason, RunManifest,
    RunTree, TreatmentRef, experiment_digest,
};
use candler_schemas::primitives::{Digest, FieldPath, Name, NameToken, RelPath, RunKey};
use candler_schemas::raw::{Clock, StreamName};
use candler_schemas::refusal::Side;
use candler_schemas::refusal_table;
use candler_schemas::values::{SourceEntry, ValueSource};
use flate2::read::MultiGzDecoder;
use sha2::{Digest as _, Sha256};

use super::streams::ReaderFactory;
use crate::frames::{FrameMode, FrameWriter, RawFileInfo};
use crate::store::run::{Existing, RunWriter};
use crate::store::{Store, StoreError};
use crate::trace::{NormaliseError, normalise};

/// The hidden directory of the store where a stream is framed before it moves into its run.
const SCRATCH: &str = ".import-scratch";

/// How much of a file is read to recognise it: the start of its decompressed text.
const SAMPLE_BYTES: u64 = 64 * 1024;

/// The treatment of a raw run: the stream names none, and the manifest requires one.
const UNRECORDED: &str = "unrecorded";

/// The stream a raw trace is stored as.
const STORED_STREAM: &str = "stdout";

/// Why a raw ingest was refused or failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RawIngestError {
    /// The path holds no stream a harness reader recognises.
    #[error("`{path}` holds no raw trace stream (a results folder needs `--rules`)")]
    NoStream {
        /// The path that was ingested.
        path: String,
    },
    /// The path cannot be read.
    #[error("cannot read `{path}`: {detail}")]
    Unreadable {
        /// The path.
        path: String,
        /// The operating system's explanation.
        detail: String,
    },
    /// The store refused a step.
    #[error("the store refused a step of `{run}`: {detail}")]
    Store {
        /// The run, or `the store`.
        run: String,
        /// What the store said.
        detail: String,
    },
    /// A stream could not be framed.
    #[error("stream `{stream}` cannot be framed: {detail}")]
    Frames {
        /// The relative path of the stream.
        stream: String,
        /// What went wrong.
        detail: String,
    },
    /// A manifest that cannot be built from what the stream states.
    #[error("the manifest of stream `{stream}` cannot be built: {detail}")]
    Manifest {
        /// The relative path of the stream.
        stream: String,
        /// What went wrong.
        detail: String,
    },
    /// A sealed run whose content the same stream disagrees with.
    #[error("run `{run}` is sealed and disagrees with its stream: {detail}")]
    Conflict {
        /// The run.
        run: String,
        /// What differs.
        detail: String,
    },
}

/// The path of an error that names no place in a document.
fn no_path<E>(_error: &E) -> Option<String> {
    None
}

refusal_table! {
    RawIngestError, path = no_path;
    NoStream { .. } => row(
        "import.no_stream",
        Side::Environment,
        "The path given to `ingest` holds no raw trace stream.",
        "-",
    ),
    Unreadable { .. } => row(
        "import.unreadable_path",
        Side::Environment,
        "The path given to `ingest` cannot be read.",
        "-",
    ),
    Store { .. } => row(
        "candler.internal",
        Side::Candler,
        "The store refused a step of the ingest.",
        "-",
    ),
    Frames { .. } => row(
        "candler.internal",
        Side::Candler,
        "A stream could not be framed into a raw frame file.",
        "-",
    ),
    Manifest { .. } => row(
        "candler.internal",
        Side::Candler,
        "A run manifest could not be built from a stream.",
        "-",
    ),
    Conflict { .. } => row(
        "candler.internal",
        Side::Candler,
        "A sealed run disagrees with the stream that names it.",
        "-",
    ),
}

/// What a recogniser says of a stream: the harness, the version the stream names, and whether a reader reads that version.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Recognised {
    /// The harness that wrote the stream.
    pub harness: Name,
    /// The harness version the stream names.
    pub version: semver::Version,
    /// Whether a reader supports that version.
    pub supported: bool,
}

impl Recognised {
    /// A recognised stream.
    #[must_use]
    pub const fn new(harness: Name, version: semver::Version, supported: bool) -> Self {
        Self {
            harness,
            version,
            supported,
        }
    }
}

/// Recognises the start of a stream (at most 64 KiB of its decompressed text); the caller of the ingest supplies it.
// TRAIT_OBJECT: the caller of the ingest supplies a recogniser of unknown concrete type
pub type Recogniser = dyn Fn(&[u8]) -> Option<Recognised> + Send + Sync;

/// A stream found under the ingested path.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Found {
    /// The path of the stream relative to the ingested directory, or its file name for a file.
    pub rel: RelPath,
    /// Where the stream is on this machine; never stored.
    pub path: PathBuf,
    /// The sha256 of the file's bytes as they lie, compressed or not.
    pub digest: Digest,
    /// What the recogniser said.
    pub recognised: Recognised,
}

/// Why a file under the ingested path is not ingested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum SkipReason {
    /// A `.jsonl` file whose first line no harness recognises.
    NotAStream,
    /// A file or directory that cannot be read, or whose name is no relative path.
    Unreadable,
    /// A symbolic link, which is never followed.
    SymbolicLink,
}

impl SkipReason {
    /// The reason as the verb prints it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotAStream => "not_a_stream",
            Self::Unreadable => "unreadable",
            Self::SymbolicLink => "symbolic_link",
        }
    }
}

/// A file left out of the ingest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub struct Skipped {
    /// The path relative to the ingested directory.
    pub path: String,
    /// Why it was left out.
    pub reason: SkipReason,
}

/// The streams found under a path, and the files left out.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Streams {
    /// The recognised streams, in the order of their relative paths.
    pub found: Vec<Found>,
    /// The `.jsonl` files and links left out, in the order of their relative paths.
    pub skipped: Vec<Skipped>,
}

/// What one ingest did.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RawReport {
    /// The runs written by this call.
    pub created: u64,
    /// The streams the store already held.
    pub existing: u64,
    /// The files left out.
    pub skipped: Vec<Skipped>,
}

/// The error of a file system call on `path`.
fn unreadable(path: &Path, error: &io::Error) -> RawIngestError {
    RawIngestError::Unreadable {
        path: path.display().to_string(),
        detail: error.to_string(),
    }
}

/// The error of a store that refuses a step of `run`.
fn store_error(run: &str, error: &StoreError) -> RawIngestError {
    RawIngestError::Store {
        // BORROW: .to_owned() makes the owned label
        run: run.to_owned(),
        detail: error.to_string(),
    }
}

/// Whether `name` is the file name of a stream candidate.
fn is_candidate(name: &str) -> bool {
    let stem = Path::new(name)
        .file_stem()
        .and_then(std::ffi::OsStr::to_str);
    extension_is(name, "jsonl") || (is_gzip(name) && stem.is_some_and(|s| extension_is(s, "jsonl")))
}

/// Whether the extension of `name` is `extension`, in any case.
fn extension_is(name: &str, extension: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|found| found.eq_ignore_ascii_case(extension))
}

/// Whether `name` is a compressed stream.
fn is_gzip(name: &str) -> bool {
    extension_is(name, "gz")
}

/// The digest of a finished hash.
fn digest_of_hash(hasher: Sha256) -> Result<Digest, io::Error> {
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        write!(hex, "{byte:02x}").map_err(io::Error::other)?;
    }
    Digest::from_hex(hex.as_str()).map_err(io::Error::other) // BORROW: explicit .as_str()
}

/// The sha256 of the file at `path`, read in a stream.
fn digest_of_path(path: &Path) -> Result<Digest, io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(buffer.as_mut_slice())?; // BORROW: explicit .as_mut_slice()
        if read == 0 {
            return digest_of_hash(hasher);
        }
        hasher.update(buffer.get(..read).unwrap_or_default());
    }
}

/// The first [`SAMPLE_BYTES`] of the text of the file at `path`, decompressed when it is a `.gz`.
fn head_of(path: &Path, name: &str) -> Result<Vec<u8>, io::Error> {
    let file = File::open(path)?;
    let mut head = Vec::new();
    if is_gzip(name) {
        MultiGzDecoder::new(file)
            .take(SAMPLE_BYTES)
            .read_to_end(&mut head)?;
    } else {
        file.take(SAMPLE_BYTES).read_to_end(&mut head)?;
    }
    Ok(head)
}

/// The candidate files under `dir`, as `(path on disk, path relative to the ingested directory)`, in order; links and unreadable entries go to `skipped`.
fn walk(
    dir: &Path,
    relative: &str,
    files: &mut Vec<(PathBuf, String)>,
    skipped: &mut Vec<Skipped>,
) -> Result<(), io::Error> {
    let mut entries: Vec<fs::DirEntry> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            skipped.push(Skipped {
                path: format!("{relative}{}", name.to_string_lossy()),
                reason: SkipReason::Unreadable,
            });
            continue;
        };
        let shown = format!("{relative}{name}");
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            if is_candidate(name) {
                skipped.push(Skipped {
                    path: shown,
                    reason: SkipReason::SymbolicLink,
                });
            }
        } else if kind.is_dir() {
            if let Err(error) = walk(
                entry.path().as_path(),       // BORROW: explicit .as_path()
                format!("{shown}/").as_str(), // BORROW: explicit .as_str()
                files,
                skipped,
            ) {
                // EXPLICIT: a directory that cannot be read is reported and the walk goes on
                let _ = error;
                skipped.push(Skipped {
                    path: shown,
                    reason: SkipReason::Unreadable,
                });
            }
        } else if kind.is_file() && is_candidate(name) {
            files.push((entry.path(), shown));
        }
    }
    Ok(())
}

/// Finds the streams under `path`: a directory is walked recursively in name order and never through a link, a file is a directory of one.
///
/// A `.jsonl` or `.jsonl.gz` file is a stream when `recognise` names a harness from the first
/// 64 KiB of its text; any other such file goes to [`Streams::skipped`].
///
/// # Errors
/// Returns [`RawIngestError::Unreadable`] when `path` cannot be read and
/// [`RawIngestError::NoStream`] when it holds no stream.
pub fn find_streams(path: &Path, recognise: &Recogniser) -> Result<Streams, RawIngestError> {
    let meta = fs::metadata(path).map_err(|e| unreadable(path, &e))?;
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    let mut skipped: Vec<Skipped> = Vec::new();
    if meta.is_dir() {
        walk(path, "", &mut files, &mut skipped).map_err(|e| unreadable(path, &e))?;
    } else if let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str)
        && meta.is_file()
        && is_candidate(name)
    {
        files.push((path.to_path_buf(), name.to_owned())); // BORROW: the list owns its entries
    }
    let mut found = Vec::new();
    for (file, shown) in files {
        let Ok(rel) = RelPath::from_str(shown.as_str()) else {
            // BORROW: explicit .as_str()
            skipped.push(Skipped {
                path: shown,
                reason: SkipReason::Unreadable,
            });
            continue;
        };
        let name = shown.rsplit('/').next().unwrap_or_default();
        // BORROW: explicit .as_path()
        let Ok(head) = head_of(file.as_path(), name) else {
            skipped.push(Skipped {
                path: shown,
                reason: SkipReason::Unreadable,
            });
            continue;
        };
        let Some(recognised) = recognise(head.as_slice()) else {
            // BORROW: explicit .as_slice()
            skipped.push(Skipped {
                path: shown,
                reason: SkipReason::NotAStream,
            });
            continue;
        };
        let Ok(digest) = digest_of_path(file.as_path()) else {
            // BORROW: explicit .as_path()
            skipped.push(Skipped {
                path: shown,
                reason: SkipReason::Unreadable,
            });
            continue;
        };
        found.push(Found {
            rel,
            path: file,
            digest,
            recognised,
        });
    }
    skipped.sort();
    if found.is_empty() {
        return Err(RawIngestError::NoStream {
            path: path.display().to_string(),
        });
    }
    Ok(Streams { found, skipped })
}

/// The cell of a stream: the harness it names under the unrecorded treatment, and nothing else.
fn comparable_of(found: &Found) -> Result<Comparable, RawIngestError> {
    let manifest_error = |detail: String| RawIngestError::Manifest {
        stream: found.rel.to_string(),
        detail,
    };
    let treatment = Name::from_str(UNRECORDED).map_err(|e| manifest_error(e.to_string()))?;
    Comparable::builder()
        .harness(HarnessIdentity::new(
            // BORROW: .clone() copies the harness name and version into the cell
            found.recognised.harness.clone(),
            found.recognised.version.clone(),
        ))
        .treatment(TreatmentRef::new(treatment))
        .build()
        .map_err(|e| manifest_error(e.to_string()))
}

/// The manifest of the run `key` of `found`: no wall-clock time, an import record that names the stream's relative path and digest, and `integrity`.
fn manifest_of(
    found: &Found,
    key: &RunKey,
    comparable: &Comparable,
    format: &NameToken,
    integrity: Integrity,
) -> Result<RunManifest, RawIngestError> {
    let manifest_error = |detail: String| RawIngestError::Manifest {
        stream: found.rel.to_string(),
        detail,
    };
    let harness = FieldPath::from_str("harness").map_err(|e| manifest_error(e.to_string()))?;
    let treatment = FieldPath::from_str("treatment").map_err(|e| manifest_error(e.to_string()))?;
    // BORROW: .clone() copies the format label into the record
    let mut record = ImportRecord::raw(format.clone());
    record.sources = vec![ImportSource::new(
        // BORROW: .clone() copies the relative path and the digest into the record
        found.rel.clone(),
        found.digest.clone(),
    )];
    RunManifest::builder()
        // BORROW: .clone() copies the key into the manifest
        .run(key.clone())
        .experiment(experiment_digest(&[comparable.digest()]))
        // BORROW: .clone() copies the cell into the manifest
        .comparable(comparable.clone())
        .integrity(integrity)
        .source(SourceEntry::new(
            harness,
            // BORROW: .clone() copies the relative path into the source
            ValueSource::imported(found.rel.clone()),
        ))
        .source(SourceEntry::new(treatment, ValueSource::Unrecorded))
        .import(record)
        .build()
        .map_err(|e| manifest_error(e.to_string()))
}

/// Removes the directory at `path` with what it holds; a directory that is absent is already removed.
fn remove_tree(path: &Path) -> Result<(), RawIngestError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        // EXPLICIT: an absent directory is the wanted state
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(unreadable(path, &error)),
    }
}

/// Removes the copies of `key` that `Existing::ReplaceUnfinished` moved aside, so that the store holds only what a clean ingest holds.
///
/// Only an unfinished run is ever moved aside; a sealed run is never replaced or removed here.
fn discard_replaced(store: &Store, key: &RunKey) -> Result<(), RawIngestError> {
    let layout = store.layout();
    let mut n = 1_u32;
    loop {
        let aside = layout.replaced_dir(key, n);
        if !aside.exists() {
            break;
        }
        remove_tree(aside.as_path())?; // BORROW: explicit .as_path()
        n = n.saturating_add(1);
    }
    if let Some(parent) = layout.replaced_dir(key, 1).parent() {
        // EXPLICIT: the directory of moved-aside runs is removed when empty; one that still holds another run's copy stays
        let _ = fs::remove_dir(parent);
    }
    Ok(())
}

/// Frames the stream of `found` as `stdout` into `dir`, checking that the file is still the one that was found.
fn frame_stream(found: &Found, dir: &Path) -> Result<RawFileInfo, RawIngestError> {
    let frames_error = |detail: String| RawIngestError::Frames {
        stream: found.rel.to_string(),
        detail,
    };
    let bytes = fs::read(found.path.as_path()).map_err(|e| unreadable(&found.path, &e))?; // BORROW: explicit .as_path()
    let mut hasher = Sha256::new();
    hasher.update(bytes.as_slice()); // BORROW: explicit .as_slice()
    let digest = digest_of_hash(hasher).map_err(|e| frames_error(e.to_string()))?;
    if digest != found.digest {
        return Err(frames_error(String::from(
            "the file changed while it was ingested",
        )));
    }
    let plain = if is_gzip(found.rel.as_str()) {
        let mut plain = Vec::new();
        MultiGzDecoder::new(bytes.as_slice()) // BORROW: explicit .as_slice()
            .read_to_end(&mut plain)
            .map_err(|e| frames_error(e.to_string()))?;
        plain
    } else {
        bytes
    };
    let stream = StreamName::from_str(STORED_STREAM).map_err(|e| frames_error(e.to_string()))?;
    let mut writer = FrameWriter::create(
        dir,
        stream,
        FrameMode::Lines,
        Clock::Unrecorded,
        // BORROW: .clone() gives the header its own copy of the file's digest
        Some(found.digest.clone()),
    )
    .map_err(|e| frames_error(e.to_string()))?;
    writer
        .write(plain.as_slice(), None) // BORROW: explicit .as_slice()
        .map_err(|e| frames_error(e.to_string()))?;
    writer.finish().map_err(|e| frames_error(e.to_string()))
}

/// What the ingest needs to write one run.
struct Context<'a> {
    store: &'a Store,
    readers: &'a ReaderFactory,
    scratch: &'a Path,
    format: &'a NameToken,
}

/// Writes the run `key` of `found`, replacing an unsealed one, and seals it last.
///
/// A stream whose version no reader supports, or that a reader refuses, is sealed invalid with its
/// raw file kept, and only after its manifest, raw file and tree are written.
fn write_run(
    ctx: &Context<'_>,
    found: &Found,
    key: &RunKey,
    comparable: &Comparable,
) -> Result<(), RawIngestError> {
    let (store, label) = (ctx.store, key.to_string());
    let frames_dir = ctx.scratch.join(label.as_str()); // BORROW: explicit .as_str()
    remove_tree(frames_dir.as_path())?; // BORROW: explicit .as_path()
    let framed = frame_stream(found, frames_dir.as_path())?; // BORROW: explicit .as_path()
    let supported = found.recognised.supported;
    let first = if supported {
        Integrity::Valid
    } else {
        Integrity::invalid(InvalidReason::FormatUnknown)
    };
    let manifest = manifest_of(found, key, comparable, ctx.format, first)?;
    let mut writer = RunWriter::create(store, &manifest, Existing::ReplaceUnfinished)
        .map_err(|e| store_error(label.as_str(), &e))?; // BORROW: explicit .as_str()
    discard_replaced(store, key)?;
    let fail = |e: StoreError| store_error(label.as_str(), &e); // BORROW: explicit .as_str()
    writer
        .tree(
            &RunTree::builder()
                .build()
                .map_err(|e| RawIngestError::Manifest {
                    stream: found.rel.to_string(),
                    detail: e.to_string(),
                })?,
        )
        .map_err(fail)?;
    let stream = StreamName::from_str(STORED_STREAM).map_err(|e| RawIngestError::Frames {
        stream: found.rel.to_string(),
        detail: e.to_string(),
    })?;
    // BORROW: explicit .as_path()
    writer
        .raw(&stream, framed.path.as_path(), &framed.digest)
        .map_err(fail)?;
    if supported {
        let stored = store
            .raw_path(key, &stream)
            .ok_or_else(|| RawIngestError::Frames {
                stream: found.rel.to_string(),
                detail: String::from("the raw file is not in the run after it was stored"),
            })?;
        // BORROW: .clone() copies the digest
        let info = RawFileInfo::new(stored, framed.digest.clone(), framed.frames);
        match normalise((ctx.readers)(), &info) {
            Ok(trace) => writer.trace(&trace).map_err(fail)?,
            Err(error) => {
                let reason = match error {
                    NormaliseError::Undetected { .. } => InvalidReason::FormatUnknown,
                    NormaliseError::Frames(_)
                    | NormaliseError::Frame { .. }
                    | NormaliseError::Time { .. }
                    | NormaliseError::Finish { .. }
                    | NormaliseError::Trace { .. } => InvalidReason::ReaderError,
                };
                let invalid = manifest_of(
                    found,
                    key,
                    comparable,
                    ctx.format,
                    Integrity::invalid(reason),
                )?;
                writer.manifest(&invalid).map_err(fail)?;
            }
        }
    }
    writer.done().map_err(fail)?;
    remove_tree(frames_dir.as_path()) // BORROW: explicit .as_path()
}

/// A run the store holds that was read from a raw stream.
struct Known {
    key: RunKey,
    sealed: bool,
}

/// What the store holds already: the runs by the stream they were read from, and the highest replicate of each cell.
struct Holdings {
    /// `(sha256 text, relative path)` of a stream, to the run read from it.
    by_stream: BTreeMap<(String, String), Known>,
    /// The cell digest text, to the highest replicate any run of the cell has, sealed or not.
    highest: BTreeMap<String, u32>,
}

impl Holdings {
    /// Reads the manifests of every run directory of `store`.
    fn read(store: &Store) -> Result<Self, RawIngestError> {
        let mut by_stream: BTreeMap<(String, String), Known> = BTreeMap::new();
        let mut highest: BTreeMap<String, u32> = BTreeMap::new();
        for entry in store.runs().map_err(|e| store_error("the store", &e))? {
            let top = highest.entry(entry.key.cell().to_string()).or_insert(0);
            *top = (*top).max(entry.key.replicate());
            // EXPLICIT: a run whose manifest cannot be read holds its key but names no stream
            let Ok(manifest) = store.read_manifest(&entry.key) else {
                continue;
            };
            let Some(record) = manifest.import() else {
                continue;
            };
            if record.base_commit.is_some() || record.legacy_integrity.is_some() {
                continue;
            }
            for source in &record.sources {
                let name = (source.sha256.to_string(), source.path.to_string());
                let known = Known {
                    // BORROW: .clone() copies the key into the table
                    key: entry.key.clone(),
                    sealed: entry.done,
                };
                match by_stream.get(&name) {
                    Some(held) if held.sealed || !known.sealed => {}
                    Some(_) | None => {
                        by_stream.insert(name, known);
                    }
                }
            }
        }
        Ok(Self { by_stream, highest })
    }
}

/// The run key of `found`: the key of the run the store holds for it, or the next replicate of its cell.
fn key_for(
    holdings: &mut Holdings,
    found: &Found,
    comparable: &Comparable,
) -> Result<(RunKey, Option<bool>), RawIngestError> {
    let cell = comparable.digest();
    let name = (found.digest.to_string(), found.rel.to_string());
    if let Some(held) = holdings.by_stream.get(&name) {
        if held.key.cell() != &cell {
            return Err(RawIngestError::Conflict {
                run: held.key.to_string(),
                detail: String::from("the run's cell is not the cell its stream gives"),
            });
        }
        // BORROW: .clone() copies the key out of the table
        return Ok((held.key.clone(), Some(held.sealed)));
    }
    let top = holdings.highest.entry(cell.to_string()).or_insert(0);
    *top = top.saturating_add(1);
    let key = RunKey::new(cell, *top, 1).map_err(|e| RawIngestError::Manifest {
        stream: found.rel.to_string(),
        detail: e.to_string(),
    })?;
    Ok((key, None))
}

/// Writes a run for each stream of `found` that the store does not hold, in the order of the streams' digests; returns the counts of created and existing runs.
fn ingest_found(ctx: &Context<'_>, found: &[Found]) -> Result<(u64, u64), RawIngestError> {
    let mut order: Vec<&Found> = found.iter().collect();
    order.sort_by_key(|stream| (stream.digest.to_string(), stream.rel.to_string()));
    let mut holdings = Holdings::read(ctx.store)?;
    let (mut created, mut existing) = (0_u64, 0_u64);
    for stream in order {
        let comparable = comparable_of(stream)?;
        let (key, held) = key_for(&mut holdings, stream, &comparable)?;
        if held == Some(true) {
            existing = existing.saturating_add(1);
            continue;
        }
        write_run(ctx, stream, &key, &comparable)?;
        created = created.saturating_add(1);
    }
    Ok((created, existing))
}

/// Whether the index lists exactly the sealed runs of the store, in key order.
fn index_is_current(store: &Store) -> Result<bool, RawIngestError> {
    let sealed: Vec<RunKey> = store
        .runs()
        .map_err(|e| store_error("the store", &e))?
        .into_iter()
        .filter(|entry| entry.done)
        .map(|entry| entry.key)
        .collect();
    Ok(store
        .indexed_runs()
        .map_err(|e| store_error("the store", &e))?
        == sealed)
}

/// Ingests the raw trace streams under `path` into `store`: one sealed run per stream.
///
/// Streams are taken in the order of their sha256, so any set of streams gives the same keys on a
/// fresh store. A stream the store holds sealed, by digest and relative path, is counted as
/// existing and is not written again; an unsealed run of a stream is replaced under its own key;
/// any other stream takes the next replicate of its cell, attempt 1. A run is written as its
/// manifest, tree, raw frames and trace, and sealed last, so an ingest that stops anywhere is
/// finished by running it again, and the store then holds what one uninterrupted ingest holds. The
/// manifest holds no wall-clock time, so two ingests of one input are byte-equal.
///
/// # Errors
/// Returns the errors of [`find_streams`], [`RawIngestError::Store`] when the store refuses a
/// step, [`RawIngestError::Frames`] when a stream cannot be framed or changed while it was read,
/// and [`RawIngestError::Conflict`] when a sealed run disagrees with the cell its stream gives.
pub fn ingest_raw(
    store: &Store,
    path: &Path,
    recognise: &Recogniser,
    readers: &ReaderFactory,
) -> Result<RawReport, RawIngestError> {
    let streams = find_streams(path, recognise)?;
    let format = readers()
        .id()
        .to_string()
        .parse::<NameToken>()
        .map_err(|e| RawIngestError::Manifest {
            stream: path.display().to_string(),
            detail: e.to_string(),
        })?;
    let scratch = store.layout().root().join(SCRATCH);
    remove_tree(scratch.as_path())?; // BORROW: explicit .as_path()
    let ctx = Context {
        store,
        readers,
        scratch: scratch.as_path(), // BORROW: explicit .as_path()
        format: &format,
    };
    let counts = ingest_found(&ctx, streams.found.as_slice()); // BORROW: explicit .as_slice()
    let cleaned = remove_tree(scratch.as_path()); // BORROW: explicit .as_path()
    let (created, existing) = counts?;
    cleaned?;
    if !index_is_current(store)? {
        store
            .rebuild_index()
            .map_err(|e| store_error("the store", &e))?;
    }
    Ok(RawReport {
        created,
        existing,
        skipped: streams.skipped,
    })
}
