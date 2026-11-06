//! The evidence a run leaves: its catalog, its census, the leak check and its tree.

pub mod catalog;
pub mod census;
pub mod leak;
pub mod tree;

use candler_schemas::execution::RunTree;
use candler_schemas::primitives::Digest;
use candler_schemas::trace::Session;

/// The tree of a run as the manifest records it.
pub type TreeJson = RunTree;

/// A file the run produced and its digest.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct OutputDigest {
    /// The path relative to the workspace.
    pub path: String,
    /// The digest of the content.
    pub digest: Digest,
}

/// A file the run was given and its digest.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InputDigest {
    /// The path relative to the workspace.
    pub path: String,
    /// The digest of the content.
    pub digest: Digest,
}

/// What the run was given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Inputs {
    /// The digest of every input file.
    pub digests: Vec<InputDigest>,
}

/// What a run left behind.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Evidence {
    /// The sessions the trace holds.
    pub sessions: Vec<Session>,
    /// The files the run produced.
    pub outputs: Vec<OutputDigest>,
    /// The tree of the run.
    pub tree: RunTree,
}
