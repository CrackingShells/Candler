//! The MCP servers of a cell, by the name the agent sees.
//!
//! A server key under `mcp.servers.<key>` is a field-path segment, so it cannot hold a production
//! name such as `movement-server`. A server may therefore carry an optional `name` leaf: the name
//! the server renders under (default: the key). Both drivers and the core read the servers through
//! [`servers`], the one reader of `mcp.servers`: each entry is read as an
//! [`McpServer`], launched from a `command` or reached at a
//! `url` and never both. `name` is removed from the
//! server, the server is keyed by its rendered name, and a name the agent could not address, or
//! that two servers share, is refused.

use std::collections::BTreeMap;
use std::str::FromStr as _;

use candler_schemas::field_path;
use candler_schemas::primitives::{FieldPath, Name};
use candler_schemas::servers::{McpServer, McpServerKind};
use serde_json::Value;

use crate::driver::DriverError;
use crate::spec::Cell;

/// The path whose table holds the servers, one table per server key.
pub const SERVERS: &str = "mcp.servers";

fn invalid(detail: String) -> DriverError {
    DriverError::InvalidValue {
        path: field_path!("mcp.servers"),
        detail,
    }
}

/// The field `path` leads to inside the server at `entry`: the entry's own path extended by every
/// key of `path` that is a field-path segment, up to the first step that is not one (an index).
fn field_at_fault(entry: &FieldPath, path: &serde_path_to_error::Path) -> FieldPath {
    use serde_path_to_error::Segment;
    let mut text = entry.to_string(); // BORROW: .to_string() gives owned text
    for segment in path {
        let Segment::Map { key } = segment else { break };
        let longer = format!("{text}.{key}");
        if FieldPath::from_str(&longer).is_err() {
            break;
        }
        text = longer;
    }
    FieldPath::from_str(&text).unwrap_or_else(|_| entry.clone()) // BORROW: the error owns the path
}

/// The entry `key` of `mcp.servers` read as a server.
pub(crate) fn read(key: &str, entry: Value) -> Result<McpServer, DriverError> {
    let entry_path = FieldPath::from_str(&format!("{SERVERS}.{key}"))
        .unwrap_or_else(|_| field_path!("mcp.servers"));
    let refused = |detail: String| DriverError::InvalidValue {
        path: entry_path.clone(), // BORROW: the error owns the path
        detail,
    };
    let server: McpServer = serde_path_to_error::deserialize(entry).map_err(|error| {
        DriverError::InvalidValue {
            path: field_at_fault(&entry_path, error.path()),
            detail: error.inner().to_string(), // BORROW: .to_string() gives owned text
        }
    })?;
    let remote = match server.kind {
        Some(McpServerKind::Http | McpServerKind::Sse | McpServerKind::Remote) => true,
        Some(_) => false,
        None => server.command.is_none() && server.url.is_some(),
    };
    if remote {
        if server.url.is_none() {
            return Err(refused("is a remote server and has no `url`".to_owned())); // BORROW: .to_owned() makes the owned text
        }
        if server.command.is_some() || server.args.is_some() || server.env.is_some() {
            return Err(refused(
                "is a remote server and sets `command`, `args` or `env`".to_owned(), // BORROW: .to_owned() makes the owned text
            ));
        }
    } else {
        if server.command.is_none() {
            return Err(refused(
                "has neither a `command` nor a `url`; a server is launched from a `command` or reached at a `url` with `type` http, sse or remote"
                    .to_owned(), // BORROW: .to_owned() makes the owned text
            ));
        }
        if server.url.is_some() || server.headers.is_some() {
            return Err(refused(
                "is a launched server and sets `url` or `headers`".to_owned(),
            )); // BORROW: .to_owned() makes the owned text
        }
    }
    Ok(server)
}

/// `servers` re-keyed by rendered name, with `name` removed from each server.
///
/// # Errors
/// [`DriverError::InvalidValue`] when a `name` is not a plain name (non-empty, with neither `@` nor
/// white space), contains `__` (which would split `mcp__<server>__<tool>`), or when two servers
/// render the same name, by name or by key.
pub fn by_name(
    servers: BTreeMap<String, McpServer>,
) -> Result<BTreeMap<String, McpServer>, DriverError> {
    let mut rendered: BTreeMap<String, McpServer> = BTreeMap::new();
    let mut keys: BTreeMap<String, String> = BTreeMap::new();
    for (key, mut server) in servers {
        let name = match server.name.take() {
            None => key.clone(), // BORROW: .clone() copies the key
            Some(name) => {
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
        };
        if let Some(first) = keys.get(&name) {
            return Err(invalid(format!(
                "servers `{first}` and `{key}` both render as `{name}`"
            )));
        }
        keys.insert(name.clone(), key); // BORROW: the map owns both
        rendered.insert(name, server);
    }
    Ok(rendered)
}

/// The servers of a cell by rendered name: the values at and below `mcp.servers`, each leaf placed
/// at its path under its key, each entry read as an [`McpServer`], then [`by_name`]. `None` when
/// the cell sets no server.
///
/// # Errors
/// [`DriverError`] when a value of the cell cannot be used, an entry is not a server (a field that
/// is not typed or not known, neither a `command` nor a `url`, or both families at once), or [`by_name`] refuses a name.
pub fn servers(cell: &Cell<'_>) -> Result<Option<BTreeMap<String, McpServer>>, DriverError> {
    let Some(table) = cell.table_at(&field_path!("mcp.servers"))? else {
        return Ok(None);
    };
    let read_all: Result<BTreeMap<String, McpServer>, DriverError> = table
        .into_iter()
        .map(|(key, entry)| read(&key, entry).map(|server| (key, server)))
        .collect();
    by_name(read_all?).map(Some)
}
