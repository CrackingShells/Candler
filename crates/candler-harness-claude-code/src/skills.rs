//! The Claude Code skill layout: a treatment's skill tree lives at `.claude/skills/<name>`.

use std::path::Path;

use candler_harness::driver::DriverError;
use candler_harness::skills::SkillTree;
use candler_harness::spec::{CommandSpec, PreparedFile};
use candler_schemas::execution::TreatmentRef;

use crate::prepare::{skill_archive_command, skill_files};

/// The Claude Code skill layout.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeSkills;

impl SkillTree for ClaudeSkills {
    fn archive_command(
        &self,
        treatment: &TreatmentRef,
        source: &Path,
    ) -> Result<CommandSpec, DriverError> {
        skill_archive_command(treatment, source)
    }

    fn files(
        &self,
        treatment: &TreatmentRef,
        archive: &[u8],
    ) -> Result<Vec<PreparedFile>, DriverError> {
        skill_files(treatment, archive)
    }
}
