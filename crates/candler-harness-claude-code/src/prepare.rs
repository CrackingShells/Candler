//! The files the driver asks the core to write into the workspace: the settings and the MCP
//! configuration.
//!
//! Nothing here touches the file system or runs a process.

use std::str::FromStr as _;

use candler_harness::driver::DriverError;
use candler_harness::spec::{Cell, PreparedFile};
use candler_schemas::primitives::RelPath;
use serde_json::{Map, Value, json};

use crate::driver::{invalid, value_at};

/// The cell path of the allowed tools.
const TOOLS_ALLOWED: &str = "tools.allowed";
/// The cell path of the disallowed tools.
const TOOLS_DISALLOWED: &str = "tools.disallowed";
/// The cell path of the MCP servers.
const MCP: &str = "mcp";

/// The names a list of the cell holds, or none when the cell sets no list.
fn names(cell: &Cell<'_>, path: &str) -> Result<Vec<String>, DriverError> {
    match value_at(cell, path)? {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| match item {
                Value::String(text) => Ok(text.clone()), // BORROW: .clone() copies the text out of the cell
                Value::Null
                | Value::Bool(_)
                | Value::Number(_)
                | Value::Array(_)
                | Value::Object(_) => {
                    Err(invalid(path, "holds an entry that is not text".to_owned())) // BORROW: .to_owned() makes the owned text
                }
            })
            .collect(),
        Some(
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Object(_),
        ) => {
            Err(invalid(path, "is not a list".to_owned())) // BORROW: .to_owned() makes the owned text
        }
    }
}

/// A file at `path` holding `value` as JSON.
fn json_file(path: &str, value: &Value) -> Result<PreparedFile, DriverError> {
    let mut text = serde_json::to_string_pretty(value).map_err(|error| {
        invalid(path, error.to_string()) // BORROW: .to_string() gives owned text
    })?;
    text.push('\n');
    let rel = RelPath::from_str(path).map_err(|error| invalid(path, error.to_string()))?; // BORROW: .to_string() gives owned text
    Ok(PreparedFile::new(rel, text.into_bytes(), false))
}

/// The `.claude/settings.json` file of a cell: `permissions.allow` from `tools.allowed` and
/// `permissions.deny` from `tools.disallowed`, each only when the cell sets it.
///
/// # Errors
/// [`DriverError`] when a value of the cell cannot be used.
pub fn render_settings(cell: &Cell<'_>) -> Result<PreparedFile, DriverError> {
    let mut permissions = Map::new();
    if value_at(cell, TOOLS_ALLOWED)?.is_some() {
        // BORROW: .to_owned() makes the owned key
        permissions.insert("allow".to_owned(), json!(names(cell, TOOLS_ALLOWED)?));
    }
    if value_at(cell, TOOLS_DISALLOWED)?.is_some() {
        // BORROW: .to_owned() makes the owned key
        permissions.insert("deny".to_owned(), json!(names(cell, TOOLS_DISALLOWED)?));
    }
    let mut root = Map::new();
    if !permissions.is_empty() {
        // BORROW: .to_owned() makes the owned key
        root.insert("permissions".to_owned(), Value::Object(permissions));
    }
    json_file(".claude/settings.json", &Value::Object(root))
}

/// The `.mcp.json` file of a cell: the object of `mcp` as `mcpServers`, each server as given.
/// `None` when the cell sets no server.
///
/// # Errors
/// [`DriverError`] when a value of the cell cannot be used.
pub fn render_mcp(cell: &Cell<'_>) -> Result<Option<PreparedFile>, DriverError> {
    match value_at(cell, MCP)? {
        None => Ok(None),
        Some(Value::Object(servers)) if servers.is_empty() => Ok(None),
        Some(Value::Object(servers)) => {
            let mut root = Map::new();
            // BORROW: .to_owned() makes the owned key; .clone() copies the servers out of the cell
            root.insert("mcpServers".to_owned(), Value::Object(servers.clone()));
            json_file(".mcp.json", &Value::Object(root)).map(Some)
        }
        Some(
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_),
        ) => {
            Err(invalid(MCP, "is not an object".to_owned())) // BORROW: .to_owned() makes the owned text
        }
    }
}
