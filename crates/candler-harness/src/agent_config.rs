//! The instruction texts and hooks of a cell, as the harness the cell runs reads them.
//!
//! [`instructions`] and [`hooks`] are the one reader of `instructions.*` and `hooks`: the core's
//! plan reads the values through them and each refuses what it does not understand. The cell
//! holds `instructions.global` and `hooks` as tables whose keys are harness ids, spelled as
//! registered, and every such key must be one of [`HARNESS_IDS`].

use candler_schemas::agent_config::{ClaudeCodeHooks, Instructions, OpenCodeHooks};
use candler_schemas::field_path;
use candler_schemas::primitives::FieldPath;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::driver::DriverError;
use crate::spec::{CLAUDE_CODE, Cell, HARNESS_IDS, OPENCODE};

/// The hooks the cell gives the harness it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Hooks {
    /// The hooks of a Claude Code cell.
    ClaudeCode(ClaudeCodeHooks),
    /// The hooks of an `OpenCode` cell.
    OpenCode(OpenCodeHooks),
}

fn invalid(path: &FieldPath, detail: impl Into<String>) -> DriverError {
    DriverError::InvalidValue {
        path: path.clone(), // BORROW: the error owns the path
        detail: detail.into(),
    }
}

/// `value` read as a `T`, a refusal at `path` naming the member at fault.
fn read<T: DeserializeOwned>(value: Value, path: &FieldPath) -> Result<T, DriverError> {
    serde_path_to_error::deserialize(value)
        .map_err(|error| invalid(path, format!("`{}`: {}", error.path(), error.inner())))
}

/// Refuses a key of the table at `path` that is not the id of a built-in harness.
fn listed<'k>(keys: impl Iterator<Item = &'k String>, path: &FieldPath) -> Result<(), DriverError> {
    for key in keys {
        if !HARNESS_IDS.contains(&key.as_str()) {
            // BORROW: explicit .as_str() instead of Deref coercion
            return Err(invalid(
                path,
                format!(
                    "`{key}` names no registered harness; the registered ones are {}",
                    HARNESS_IDS.join(", ")
                ),
            ));
        }
    }
    Ok(())
}

/// The instruction texts of a cell, with the global text of every harness it names. `None` when
/// the cell sets none.
///
/// `harness` is the id of the harness the cell runs.
///
/// # Errors
/// [`DriverError::InvalidValue`] when a value is not text or a key names no registered harness,
/// and [`DriverError::MissingValue`] when the cell sets a global text and none for `harness`.
pub fn instructions(cell: &Cell<'_>, harness: &str) -> Result<Option<Instructions>, DriverError> {
    let root = field_path!("instructions");
    let Some(table) = cell.table_at(&root)? else {
        return Ok(None);
    };
    let found: Instructions = read(Value::Object(table), &root)?;
    let global = field_path!("instructions.global");
    listed(found.global.keys(), &global)?;
    if !found.global.is_empty() && !found.global.contains_key(harness) {
        return Err(DriverError::MissingValue { path: global });
    }
    Ok(Some(found))
}

/// The hooks of a cell for `harness`. `None` when the cell sets none.
///
/// # Errors
/// [`DriverError::InvalidValue`] when a key names no registered harness or the entry of `harness`
/// does not fit its envelope (a hook file whose path is not a relative path included), and
/// [`DriverError::MissingValue`] when the cell sets hooks and none for `harness`.
pub fn hooks(cell: &Cell<'_>, harness: &str) -> Result<Option<Hooks>, DriverError> {
    let root = field_path!("hooks");
    let Some(table) = cell.table_at(&root)? else {
        return Ok(None);
    };
    listed(table.keys(), &root)?;
    let Some(entry) = table.get(harness) else {
        return Err(DriverError::MissingValue { path: root });
    };
    // BORROW: .clone() copies the entry out of the table
    let entry = entry.clone();
    match harness {
        CLAUDE_CODE => read(entry, &root).map(|found| Some(Hooks::ClaudeCode(found))),
        OPENCODE => read(entry, &root).map(|found| Some(Hooks::OpenCode(found))),
        other => Err(invalid(&root, format!("`{other}` has no hooks"))),
    }
}
