//! The instruction texts and hooks of a cell, as the harness the cell runs reads them.
//!
//! [`instructions`] and [`hooks`] are the one reader of `instructions.*` and `hooks`: both
//! drivers and the core read the values through them, and each refuses what it does not
//! understand. The cell holds the values as leaves (see [`candler_schemas::agent_config`]); a
//! harness id is written with `_` where it has `-`, and every id below `instructions.global` and
//! `hooks` is checked against the harnesses the caller says are registered.

use std::str::FromStr as _;

use candler_schemas::agent_config::{ClaudeCodeHooks, Instructions, OpenCodeHooks};
use candler_schemas::execution::paths::{HOOKS, INSTRUCTIONS_GLOBAL};
use candler_schemas::field_path;
use candler_schemas::primitives::{FieldPath, RelPath};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::driver::DriverError;
use crate::spec::Cell;

/// The id of the Claude Code harness.
const CLAUDE_CODE: &str = "claude-code";
/// The id of the `OpenCode` harness.
const OPENCODE: &str = "opencode";

/// The hooks the cell gives the harness it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Hooks {
    /// The hooks of a Claude Code cell.
    ClaudeCode(ClaudeCodeHooks),
    /// The hooks of an `OpenCode` cell.
    OpenCode(OpenCodeHooks),
}

/// One file of a hook list: where it goes and what it holds.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HookFile {
    path: String,
    text: String,
}

/// How a harness id is written as a key of a cell value.
fn spelled(id: &str) -> String {
    id.replace('-', "_")
}

/// The field path `text`, or `root` when `text` is not one.
fn at(text: &str, root: &FieldPath) -> FieldPath {
    FieldPath::from_str(text).unwrap_or_else(|_| root.clone()) // BORROW: the error owns the path
}

fn invalid(path: &FieldPath, detail: impl Into<String>) -> DriverError {
    DriverError::InvalidValue {
        path: path.clone(), // BORROW: the error owns the path
        detail: detail.into(),
    }
}

/// The registered harness whose key is `key`.
fn registered_id<'a>(
    key: &str,
    registered: &'a [String],
    root: &FieldPath,
) -> Result<&'a str, DriverError> {
    registered
        .iter()
        .find(|id| spelled(id) == key)
        .map(String::as_str) // BORROW: explicit .as_str() instead of Deref coercion
        .ok_or_else(|| {
            let names: Vec<String> = registered.iter().map(|id| spelled(id)).collect();
            invalid(
                root,
                format!(
                    "`{key}` names no registered harness; the registered ones are {}",
                    names.join(", ")
                ),
            )
        })
}

fn object<'v>(value: &'v Value, path: &FieldPath) -> Result<&'v Map<String, Value>, DriverError> {
    value
        .as_object()
        .ok_or_else(|| invalid(path, "is not a table"))
}

fn text<'v>(value: &'v Value, path: &FieldPath) -> Result<&'v str, DriverError> {
    value.as_str().ok_or_else(|| invalid(path, "is not text"))
}

/// A list of `{path, text}` entries as the object from path to text.
///
/// # Errors
/// [`DriverError::InvalidValue`] for an entry that is not `{path, text}`, a path that is not a
/// relative path, or a path that repeats.
fn files(value: &Value, at_path: &FieldPath) -> Result<Value, DriverError> {
    let Value::Array(items) = value else {
        return Err(invalid(at_path, "is not a list of `{path, text}` entries"));
    };
    let mut out = Map::new();
    for item in items {
        let entry: HookFile = serde_json::from_value(item.clone()) // BORROW: .clone() copies the entry out of the cell
            .map_err(|error| invalid(at_path, error.to_string()))?; // BORROW: .to_string() gives owned text
        RelPath::from_str(&entry.path).map_err(|error| invalid(at_path, error.to_string()))?; // BORROW: .to_string() gives owned text
        if out
            .insert(entry.path.clone(), Value::String(entry.text))
            .is_some()
        {
            // BORROW: .clone() keeps the path for the message
            return Err(invalid(
                at_path,
                format!("holds the path `{}` twice", entry.path),
            ));
        }
    }
    Ok(Value::Object(out))
}

/// The entry of one harness shaped as the object its envelope reads: `settings` read as the JSON
/// object its text holds, the list named `list` as an object from path to text.
fn shaped(
    entry: &Value,
    entry_path: &str,
    root: &FieldPath,
    list: &str,
    settings: bool,
) -> Result<Value, DriverError> {
    let entry_at = at(entry_path, root);
    let mut out = Map::new();
    for (member, value) in object(entry, &entry_at)? {
        let member_at = at(&format!("{entry_path}.{member}"), root);
        if settings && member == "settings" {
            let parsed: Value = serde_json::from_str(text(value, &member_at)?)
                .map_err(|error| invalid(&member_at, format!("is not JSON: {error}")))?;
            if !parsed.is_object() {
                return Err(invalid(&member_at, "is not the text of a JSON object"));
            }
            out.insert(member.clone(), parsed); // BORROW: the object owns its key
        } else if member == list {
            out.insert(member.clone(), files(value, &member_at)?); // BORROW: the object owns its key
        } else {
            return Err(invalid(
                &member_at,
                format!(
                    "is not a member of the hooks of this harness; they are `{list}`{}",
                    if settings { " and `settings`" } else { "" }
                ),
            ));
        }
    }
    Ok(Value::Object(out))
}

/// The instruction texts of a cell, with the global text of every harness it names. `None` when
/// the cell sets none.
///
/// `harness` is the id of the harness the cell runs and `registered` the ids of every harness
/// that is registered.
///
/// # Errors
/// [`DriverError::InvalidValue`] when a value is not text or a key names no registered harness,
/// and [`DriverError::MissingValue`] when the cell sets a global text and none for `harness`.
pub fn instructions(
    cell: &Cell<'_>,
    harness: &str,
    registered: &[String],
) -> Result<Option<Instructions>, DriverError> {
    let Some(table) = cell.table_at(&field_path!("instructions"))? else {
        return Ok(None);
    };
    let global_path = field_path!("instructions.global");
    let mut shaped = Map::new();
    for (member, value) in &table {
        match member.as_str() {
            "global" => {
                let mut by_id = Map::new();
                for (key, text_value) in object(value, &global_path)? {
                    let id = registered_id(key, registered, &global_path)?;
                    let own = at(&format!("{INSTRUCTIONS_GLOBAL}.{key}"), &global_path);
                    text(text_value, &own)?;
                    by_id.insert(id.to_owned(), text_value.clone()); // BORROW: the object owns its key and text
                }
                if !by_id.contains_key(harness) {
                    return Err(DriverError::MissingValue { path: global_path });
                }
                shaped.insert(member.clone(), Value::Object(by_id)); // BORROW: the object owns its key
            }
            "workspace" => {
                text(value, &field_path!("instructions.workspace"))?;
                shaped.insert(member.clone(), value.clone()); // BORROW: the object owns its key and value
            }
            _ => {
                return Err(invalid(
                    &at(
                        &format!("instructions.{member}"),
                        &field_path!("instructions"),
                    ),
                    "is not an instruction text; they are `global` and `workspace`",
                ));
            }
        }
    }
    serde_json::from_value(Value::Object(shaped))
        .map(Some)
        .map_err(|error| invalid(&field_path!("instructions"), error.to_string())) // BORROW: .to_string() gives owned text
}

/// The hooks of a cell for `harness`. `None` when the cell sets none.
///
/// # Errors
/// [`DriverError::InvalidValue`] when a key names no registered harness or the entry of `harness`
/// does not fit its envelope (a hook file whose path is not a relative path included), and
/// [`DriverError::MissingValue`] when the cell sets hooks and none for `harness`.
pub fn hooks(
    cell: &Cell<'_>,
    harness: &str,
    registered: &[String],
) -> Result<Option<Hooks>, DriverError> {
    let root = field_path!("hooks");
    let Some(table) = cell.table_at(&root)? else {
        return Ok(None);
    };
    for key in table.keys() {
        registered_id(key, registered, &root)?;
    }
    let key = spelled(harness);
    let Some(entry) = table.get(&key) else {
        return Err(DriverError::MissingValue { path: root });
    };
    let entry_path = format!("{HOOKS}.{key}");
    match harness {
        CLAUDE_CODE => serde_json::from_value(shaped(entry, &entry_path, &root, "files", true)?)
            .map(|found| Some(Hooks::ClaudeCode(found)))
            .map_err(|error| invalid(&at(&entry_path, &root), error.to_string())), // BORROW: .to_string() gives owned text
        OPENCODE => serde_json::from_value(shaped(entry, &entry_path, &root, "plugins", false)?)
            .map(|found| Some(Hooks::OpenCode(found)))
            .map_err(|error| invalid(&at(&entry_path, &root), error.to_string())), // BORROW: .to_string() gives owned text
        other => Err(invalid(
            &at(&entry_path, &root),
            format!("`{other}` has no hooks"),
        )),
    }
}
