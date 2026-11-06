//! The mounts of a run's container.

use std::path::PathBuf;

/// Whether a mount may be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MountMode {
    /// The container may read only.
    ReadOnly,
    /// The container may read and write.
    ReadWrite,
}

/// One bind mount.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MountSpec {
    /// The host path.
    pub source: PathBuf,
    /// The path inside the container.
    pub target: String,
    /// Whether the container may write.
    pub mode: MountMode,
}
