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
use candler_schemas::primitives::Name;
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
    cell.table_at(&field_path!("mcp.servers"))?
        .map(|table| by_name(&table))
        .transpose()
}
