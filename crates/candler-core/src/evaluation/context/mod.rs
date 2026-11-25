//! The context of one run for one request: its assembly from a suite's needs in [`assemble()`],
//! the one file rule in `rule`, and the private workspace that copies the chosen stored files and
//! the digest of what was copied in [`materialise()`]. The artifacts are copied below `outputs/`,
//! the files of each input below `inputs/<kind>/`, the trace to `trace.jsonl`, the raw streams to
//! `raw/<stream>.jsonl.gz` and the manifest to `manifest.json` of the same workspace.

use std::fmt;
use std::io;
use std::path::PathBuf;

use candler_schemas::evaluation::EvaluationError;
use candler_schemas::primitives::RunKey;

use crate::store::StoreError;

mod assemble;
mod materialise;
mod rule;
mod union;

pub use assemble::{Assembled, AssembledFile, AssembledInput, Served, assemble};
pub use materialise::{Workspace, materialise};
pub use rule::{FileRule, file_rule_keeps};
pub use union::union_needs;

/// A part of a stored run that a kind may declare and a run may lack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunPart {
    /// The normalised trace, `trace.jsonl`.
    Trace,
    /// The raw streams, `raw/<stream>.jsonl.gz`.
    Raw,
}

impl fmt::Display for RunPart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Trace => "trace",
            Self::Raw => "raw",
        })
    }
}

/// Why a run's context could not be assembled or materialised.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ContextError {
    /// The store could not give the run's manifest or tree.
    #[error("the store: {0}")]
    Store(#[from] StoreError),
    /// The run has no `run.done` seal, so it can still change.
    #[error("run {run} is not sealed")]
    NotSealed {
        /// The key of the run.
        run: String,
    },
    /// An artifact glob is not a glob.
    #[error("artifact glob `{glob}` is invalid: {detail}")]
    Glob {
        /// The glob as the needs wrote it.
        glob: String,
        /// What the glob compiler said.
        detail: String,
    },
    /// The needs ask for something this version does not supply.
    #[error("unsupported in this version: {need}")]
    Unsupported {
        /// The need, such as `snapshots`.
        need: String,
    },
    /// An input need that the run cannot serve.
    #[error("input `{kind}` {reason}")]
    InputRefused {
        /// The kind the need named.
        kind: String,
        /// Why the run cannot serve it.
        reason: String,
    },
    /// A need names a part of the run that the run does not store: its trace or its raw streams.
    #[error("run {run} stores no `{part}`")]
    PartMissing {
        /// The key of the run.
        run: RunKey,
        /// The part the run lacks.
        part: RunPart,
    },
    /// A metadata need names a path the run manifest does not hold with a value.
    #[error("metadata path `{path}` {detail}")]
    MissingMetadata {
        /// The dotted path of the need.
        path: String,
        /// Whether the manifest lacks the path or records no value for it.
        detail: &'static str,
    },
    /// The workspace directory cannot hold a context.
    #[error("workspace {} {detail}", path.display())]
    Workspace {
        /// The directory that was named.
        path: PathBuf,
        /// What is wrong with it.
        detail: &'static str,
    },
    /// A file system call failed.
    #[error("{op} failed on {}: {source}", path.display())]
    Io {
        /// What was being done.
        op: &'static str,
        /// The path the call named.
        path: PathBuf,
        /// The error of the call.
        source: io::Error,
    },
    /// A copy of a stored file does not have the digest recorded for it: the store's record for
    /// a blob, the hash taken at assembly for the trace, a raw stream or the manifest.
    #[error(
        "the stored file {} no longer has the digest recorded for it: \
         expected {expected}, its copy is {actual}",
        stored.display()
    )]
    CorruptBlob {
        /// The stored file that was copied.
        stored: PathBuf,
        /// The digest recorded for it.
        expected: String,
        /// The digest of the copy.
        actual: String,
    },
    /// The context could not be built from what was materialised.
    #[error("the context: {0}")]
    Build(#[from] EvaluationError),
}
