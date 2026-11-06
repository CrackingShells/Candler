//! The files the driver asks the core to write into the workspace: the settings, the MCP
//! configuration and the skill tree.

use std::path::Path;
use std::str::FromStr as _;

use candler_harness::driver::DriverError;
use candler_harness::spec::{Cell, CommandSpec, PreparedFile};
use candler_schemas::execution::TreatmentRef;
use candler_schemas::primitives::FieldPath;

/// The error every stub of this module returns until the driver fills it in.
fn unavailable(what: &str) -> DriverError {
    match FieldPath::from_str("harness") {
        Ok(path) => DriverError::InvalidValue {
            path,
            detail: format!("{what} is not available"),
        },
        Err(error) => DriverError::VersionOutput {
            detail: error.to_string(),
        },
    }
}

/// The `.claude/settings.json` file of a cell.
///
/// # Errors
/// [`DriverError`] when a value of the cell cannot be used.
pub fn render_settings(cell: &Cell<'_>) -> Result<PreparedFile, DriverError> {
    let _ = cell;
    Err(unavailable("render_settings"))
}

/// The `.mcp.json` file of a cell.
///
/// # Errors
/// [`DriverError`] when a value of the cell cannot be used.
pub fn render_mcp(cell: &Cell<'_>) -> Result<PreparedFile, DriverError> {
    let _ = cell;
    Err(unavailable("render_mcp"))
}

/// The command that archives the skill tree at the commit `treatment` pins.
///
/// # Errors
/// [`DriverError::Command`] when the command cannot be built.
pub fn skill_archive_command(
    treatment: &TreatmentRef,
    source: &Path,
) -> Result<CommandSpec, DriverError> {
    let _ = (treatment, source);
    Err(unavailable("skill_archive_command"))
}

/// The files of the skill archive `tar`.
///
/// # Errors
/// [`DriverError::InvalidValue`] when an entry is a link, a device or leaves the root.
pub fn skill_files(tar: &[u8]) -> Result<Vec<PreparedFile>, DriverError> {
    let _ = tar;
    Err(unavailable("skill_files"))
}
