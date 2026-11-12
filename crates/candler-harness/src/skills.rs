//! Where a treatment's skill tree comes from, per harness: each harness lays skills out in its own
//! directories, so the driver's crate supplies the implementation.

use std::path::Path;

use candler_schemas::execution::TreatmentRef;

use crate::driver::DriverError;
use crate::spec::{CommandSpec, PreparedFile};

/// Where a treatment's skill tree comes from: the command that archives it, and the files the
/// archive holds.
pub trait SkillTree: Send + Sync {
    /// The command that archives the tree `treatment` pins from `source`.
    ///
    /// # Errors
    /// [`DriverError`] when the treatment pins no tree or the command cannot be built.
    fn archive_command(
        &self,
        treatment: &TreatmentRef,
        source: &Path,
    ) -> Result<CommandSpec, DriverError>;

    /// The files of `archive`, the output of the archive command for `treatment`.
    ///
    /// # Errors
    /// [`DriverError`] when the archive cannot be read or holds an entry outside the treatment's
    /// skill tree.
    fn files(
        &self,
        treatment: &TreatmentRef,
        archive: &[u8],
    ) -> Result<Vec<PreparedFile>, DriverError>;
}
