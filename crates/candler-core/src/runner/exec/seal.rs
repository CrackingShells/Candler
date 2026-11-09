//! Sealing a run: its files go in one order and `run.done` goes last.

use std::ops::ControlFlow;
use std::path::PathBuf;

use candler_schemas::execution::{RunManifest, RunTree};
use candler_schemas::primitives::Digest;
use candler_schemas::raw::StreamName;
use candler_schemas::trace::TraceFile;

use crate::store::StoreError;
use crate::store::run::RunWriter;

/// A finished raw stream that waits to be moved into the run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RawStreamFile {
    /// The stream the file archives.
    pub stream: StreamName,
    /// Where the finished file is.
    pub gz: PathBuf,
    /// The digest of the file, which the store checks.
    pub digest: Digest,
}

impl RawStreamFile {
    /// A raw stream file.
    #[must_use]
    pub const fn new(stream: StreamName, gz: PathBuf, digest: Digest) -> Self {
        Self { stream, gz, digest }
    }
}

/// One step of the seal, in the order they happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SealStep {
    /// The manifest was written.
    Manifest,
    /// The tree was written.
    Tree,
    /// One raw stream was moved into the run.
    Raw,
    /// The trace was written.
    Trace,
    /// `run.done` was written.
    Done,
}

/// How far a seal got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SealProgress {
    /// Every step ran and the run is sealed.
    Sealed,
    /// The observer stopped the seal after this step; the run is left unsealed.
    Halted(SealStep),
}

/// Seals a run: manifest, tree, each raw stream, trace, and `run.done` last.
///
/// A failure before `run.done` leaves the run unsealed, and nothing is deleted.
///
/// # Errors
/// Returns the store's error of the first step that fails.
pub fn seal_run(
    writer: RunWriter,
    manifest: &RunManifest,
    tree: &RunTree,
    raw: &[RawStreamFile],
    trace: &TraceFile,
) -> Result<(), StoreError> {
    seal_run_observed(writer, manifest, tree, raw, trace, |_| {
        ControlFlow::Continue(())
    })
    .map(|_| ())
}

/// [`seal_run`] that tells `after` about each step once it is done and stops when it breaks.
///
/// # Errors
/// Returns the store's error of the first step that fails.
pub fn seal_run_observed(
    mut writer: RunWriter,
    manifest: &RunManifest,
    tree: &RunTree,
    raw: &[RawStreamFile],
    trace: &TraceFile,
    mut after: impl FnMut(SealStep) -> ControlFlow<()>,
) -> Result<SealProgress, StoreError> {
    writer.manifest(manifest)?;
    if after(SealStep::Manifest).is_break() {
        return Ok(SealProgress::Halted(SealStep::Manifest));
    }
    writer.tree(tree)?;
    if after(SealStep::Tree).is_break() {
        return Ok(SealProgress::Halted(SealStep::Tree));
    }
    for file in raw {
        writer.raw(&file.stream, &file.gz, &file.digest)?;
        if after(SealStep::Raw).is_break() {
            return Ok(SealProgress::Halted(SealStep::Raw));
        }
    }
    writer.trace(trace)?;
    if after(SealStep::Trace).is_break() {
        return Ok(SealProgress::Halted(SealStep::Trace));
    }
    writer.done()?;
    // EXPLICIT: nothing follows the seal, so the observer's answer to the last step is not read
    let _ = after(SealStep::Done);
    Ok(SealProgress::Sealed)
}
