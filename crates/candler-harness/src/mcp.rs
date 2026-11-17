//! The MCP servers of a cell, by the name the agent sees.
//!
//! A server key under `mcp.servers.<key>` is a field-path segment, so it cannot hold a production
//! name such as `movement-server`. A server may therefore carry an optional `name` leaf: the name
//! the server renders under (default: the key). Both drivers read the servers through here, so the
//! rule lives once: `name` is removed from the server object, the server is keyed by its rendered
//! name, and a name the agent could not address, or that two servers share, is refused.

use std::collections::BTreeMap;
use std::str::FromStr as _;

use candler_schemas::field_path;
use candler_schemas::primitives::{FieldPath, Name};
use serde_json::{Map, Value};

use crate::driver::DriverError;
use crate::spec::Cell;

/// The path whose table holds the servers, one table per server key.
pub const SERVERS: &str = "mcp.servers";

/// The leaf of a server that names it.
const NAME: &str = "name";

fn invalid(detail: String) -> DriverError {
    DriverError::InvalidValue {
        path: field_path!("mcp.servers"),
        detail,
    }
}

/// Puts `value` at the dotted path `relative` of `table`, one object per segment.
fn put_nested(
    table: &mut Map<String, Value>,
    relative: &str,
    value: &Value,
    path: &FieldPath,
) -> Result<(), DriverError> {
    match relative.split_once('.') {
        None => {
            // BORROW: .to_owned() makes the owned key; .clone() copies the value out of the cell
            table.insert(relative.to_owned(), value.clone());
            Ok(())
        }
        Some((head, rest)) => match table
            .entry(head.to_owned()) // BORROW: .to_owned() makes the owned key
            .or_insert_with(|| Value::Object(Map::new()))
        {
            Value::Object(inner) => put_nested(inner, rest, value, path),
            Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Array(_) => {
                Err(DriverError::InvalidValue {
                    path: path.clone(), // BORROW: the error owns the path
                    detail: "lies below a value that is not a table".to_owned(), // BORROW: .to_owned() makes the owned text
                })
            }
        },
    }
}

/// `servers`, a table of server objects by key, re-keyed by rendered name with `name` removed from
/// each object.
///
/// # Errors
/// [`DriverError::InvalidValue`] when a `name` is not text, is not a plain name (non-empty, with
/// neither `@` nor white space), contains `__` (which would split `mcp__<server>__<tool>`), or when
/// two servers render the same name, by name or by key.
pub fn by_name(servers: &Map<String, Value>) -> Result<Map<String, Value>, DriverError> {
    let mut rendered: Map<String, Value> = Map::new();
    let mut keys: BTreeMap<String, String> = BTreeMap::new();
    for (key, entry) in servers {
        let mut entry = entry.clone(); // BORROW: .clone() copies the server out of the cell
        let name = match entry.as_object_mut().and_then(|object| object.remove(NAME)) {
            None => key.clone(), // BORROW: .clone() copies the key
            Some(Value::String(name)) => {
                if let Err(error) = Name::from_str(&name) {
                    return Err(invalid(format!("the name of server `{key}` {error}")));
                }
                if name.contains("__") {
                    return Err(invalid(format!(
                        "the name `{name}` of server `{key}` contains `__`, which splits `mcp__<server>__<tool>`"
                    )));
                }
                name
            }
            Some(_) => {
                return Err(invalid(format!("the name of server `{key}` is not text")));
            }
        };
        if let Some(first) = keys.get(&name) {
            return Err(invalid(format!(
                "servers `{first}` and `{key}` both render as `{name}`"
            )));
        }
        keys.insert(name.clone(), key.clone()); // BORROW: the map owns both
        rendered.insert(name, entry);
    }
    Ok(rendered)
}

/// The servers of a cell by rendered name: the values at and below `mcp.servers`, each leaf placed
/// at its path under its key, then [`by_name`]. `None` when the cell sets no server.
///
/// # Errors
/// [`DriverError`] when a value of the cell cannot be used, or [`by_name`] refuses a name.
pub fn servers(cell: &Cell<'_>) -> Result<Option<Map<String, Value>>, DriverError> {
    let root = field_path!("mcp.servers");
    let mut table = Map::new();
    for (path, value) in cell.below(&root) {
        let text = path.as_str(); // BORROW: explicit .as_str() instead of Deref coercion
        match text
            .strip_prefix(SERVERS)
            .and_then(|rest| rest.strip_prefix('.'))
        {
            Some(relative) => put_nested(&mut table, relative, value, path)?,
            None => match value {
                Value::Object(inner) => table.extend(inner.clone()), // BORROW: .clone() copies the table out of the cell
                Value::Null
                | Value::Bool(_)
                | Value::Number(_)
                | Value::String(_)
                | Value::Array(_) => {
                    return Err(DriverError::InvalidValue {
                        path: path.clone(),                    // BORROW: the error owns the path
                        detail: "is not an object".to_owned(), // BORROW: .to_owned() makes the owned text
                    });
                }
            },
        }
    }
    if table.is_empty() {
        return Ok(None);
    }
    by_name(&table).map(Some)
}
