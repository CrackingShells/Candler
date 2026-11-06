//! The context of one run for one request: its assembly from a suite's needs in [`assemble()`]
//! and the one file rule in `rule`.

use std::io;
use std::path::PathBuf;

use candler_schemas::evaluation::EvaluationError;

use crate::store::StoreError;

mod assemble;
mod rule;

pub use assemble::{Assembled, AssembledArtifact, assemble};
pub use rule::{FileRule, file_rule_keeps};

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
    /// A copied file does not have the digest the store records for its blob.
    #[error(
        "the blob of `{path}` is corrupt: the store records {expected} and the copy is {actual}"
    )]
    CorruptBlob {
        /// The artifact path relative to `outputs/`.
        path: String,
        /// The digest the store records.
        expected: String,
        /// The digest of the copy.
        actual: String,
    },
    /// The context could not be built from what was materialised.
    #[error("the context: {0}")]
    Build(#[from] EvaluationError),
}
