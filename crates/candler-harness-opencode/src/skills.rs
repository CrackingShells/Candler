//! The `OpenCode` skill layout (a stand-in the guards fail on).

use std::path::Path;

use candler_harness::driver::DriverError;
use candler_harness::skills::SkillTree;
use candler_harness::spec::{CommandSpec, PreparedFile};
use candler_schemas::execution::TreatmentRef;

/// Where a treatment's skill tree lives for `OpenCode`.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenCodeSkills;

impl SkillTree for OpenCodeSkills {
    fn archive_command(
        &self,
        _treatment: &TreatmentRef,
        _source: &Path,
    ) -> Result<CommandSpec, DriverError> {
        Ok(CommandSpec::builder()
            .argv(vec![String::from("true")])
            .build()?)
    }

    fn files(
        &self,
        _treatment: &TreatmentRef,
        _archive: &[u8],
    ) -> Result<Vec<PreparedFile>, DriverError> {
        Ok(Vec::new())
    }
}
