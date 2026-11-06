//! The files the driver asks the core to write into the workspace: the settings, the MCP
//! configuration and the skill tree.
//!
//! Nothing here touches the file system or runs a process. The skill tree arrives as the bytes of
//! a `git archive` the core ran with [`skill_archive_command`], and [`skill_files`] reads them.

use std::path::Path;
use std::str::FromStr as _;

use candler_harness::driver::DriverError;
use candler_harness::spec::{Cell, CommandSpec, PreparedFile};
use candler_schemas::execution::TreatmentRef;
use candler_schemas::primitives::RelPath;
use serde_json::{Map, Value, json};

use crate::driver::{invalid, missing, value_at};

/// The cell path of the allowed tools.
const TOOLS_ALLOWED: &str = "tools.allowed";
/// The cell path of the disallowed tools.
const TOOLS_DISALLOWED: &str = "tools.disallowed";
/// The cell path of the MCP servers.
const MCP: &str = "mcp";
/// The cell path of the treatment the skill tree comes from.
const TREATMENT: &str = "treatment";
/// The size of a tar block.
const BLOCK: usize = 512;

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

/// The command that archives the skill tree at the commit `treatment` pins:
/// `git -C <source> archive --format=tar <commit>`.
///
/// # Errors
/// [`DriverError::MissingValue`] when the treatment pins no commit.
/// [`DriverError::InvalidValue`] when `source` is not text.
/// [`DriverError::Command`] when the command cannot be built.
pub fn skill_archive_command(
    treatment: &TreatmentRef,
    source: &Path,
) -> Result<CommandSpec, DriverError> {
    let commit = treatment
        .commit
        .as_ref()
        .ok_or_else(|| missing("treatment.commit"))?;
    let source = source
        .to_str()
        .ok_or_else(|| invalid(TREATMENT, "the treatment source is not text".to_owned()))?; // BORROW: .to_owned() makes the owned text
    // BORROW: .to_owned() makes the owned arguments the command keeps
    let argv = vec![
        "git".to_owned(),
        "-C".to_owned(),
        source.to_owned(),
        "archive".to_owned(),
        "--format=tar".to_owned(),
        commit.to_string(), // BORROW: .to_string() gives owned text
    ];
    Ok(CommandSpec::builder().argv(argv).build()?)
}

/// The text of a header field up to its first NUL.
fn text_field(bytes: &[u8]) -> &[u8] {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    bytes.get(..end).unwrap_or_default()
}

/// The octal number a header field holds.
fn octal(bytes: &[u8]) -> Result<usize, DriverError> {
    let text = std::str::from_utf8(text_field(bytes))
        .map_err(|_| invalid(TREATMENT, "a tar number is not text".to_owned()))?; // BORROW: .to_owned() makes the owned text
    usize::from_str_radix(text.trim(), 8)
        .map_err(|_| invalid(TREATMENT, format!("a tar number is not octal: `{text}`")))
}

/// The bytes `range` of `header`.
fn span(header: &[u8], range: std::ops::Range<usize>) -> &[u8] {
    header.get(range).unwrap_or_default()
}

/// The `path` and `linkpath` records of a pax extended header.
fn pax_records(data: &[u8]) -> Result<Vec<(String, String)>, DriverError> {
    let bad = || invalid(TREATMENT, "a pax header is malformed".to_owned()); // BORROW: .to_owned() makes the owned text
    let mut records = Vec::new();
    let mut rest = data;
    while !rest.is_empty() {
        let space = rest.iter().position(|b| *b == b' ').ok_or_else(bad)?;
        let length: usize = std::str::from_utf8(rest.get(..space).unwrap_or_default())
            .ok()
            .and_then(|text| text.parse().ok())
            .ok_or_else(bad)?;
        let record = rest.get(..length).ok_or_else(bad)?;
        let body = record.get(space.saturating_add(1)..).ok_or_else(bad)?;
        let body = std::str::from_utf8(body).map_err(|_| bad())?;
        let (key, value) = body
            .trim_end_matches('\n')
            .split_once('=')
            .ok_or_else(bad)?;
        // BORROW: .to_owned() makes the owned strings the list keeps
        records.push((key.to_owned(), value.to_owned()));
        rest = rest.get(length..).unwrap_or_default();
    }
    Ok(records)
}

/// The files of the skill archive `tar`: regular files with their executable bit.
///
/// Directories and the pax global header are skipped. The reader takes ustar and pax headers and
/// GNU long names.
///
/// # Errors
/// [`DriverError::InvalidValue`] when an entry is a link, a device or a FIFO, when a path is
/// absolute, empty or leaves the root, or when the archive is cut short.
pub fn skill_files(tar: &[u8]) -> Result<Vec<PreparedFile>, DriverError> {
    let cut = || invalid(TREATMENT, "the archive is cut short".to_owned()); // BORROW: .to_owned() makes the owned text
    let mut files = Vec::new();
    let mut at = 0_usize;
    let mut long_path: Option<String> = None;
    loop {
        let rest = tar.get(at..).unwrap_or_default();
        if rest.is_empty() {
            break;
        }
        let header = rest.get(..BLOCK).ok_or_else(cut)?;
        if header.iter().all(|b| *b == 0) {
            break;
        }
        let size = octal(span(header, 124..136))?;
        let kind = header.get(156).copied().unwrap_or(0);
        let data_end = BLOCK.checked_add(size).ok_or_else(cut)?;
        let data = rest.get(BLOCK..data_end).ok_or_else(cut)?;
        let padded = data_end
            .div_ceil(BLOCK)
            .checked_mul(BLOCK)
            .ok_or_else(cut)?;
        at = at.checked_add(padded).ok_or_else(cut)?;
        match kind {
            b'g' | b'5' => {}
            b'x' => {
                for (key, value) in pax_records(data)? {
                    match key.as_str() {
                        // BORROW: explicit .as_str()
                        "path" => long_path = Some(value),
                        "linkpath" => {
                            return Err(invalid(TREATMENT, "an entry is a link".to_owned())); // BORROW: .to_owned() makes the owned text
                        }
                        _ => {}
                    }
                }
            }
            b'L' => {
                let name = std::str::from_utf8(text_field(data))
                    .map_err(|_| invalid(TREATMENT, "a long name is not text".to_owned()))?; // BORROW: .to_owned() makes the owned text
                long_path = Some(name.to_owned()); // BORROW: .to_owned() makes the owned text
            }
            b'0' | 0 => {
                let path = match long_path.take() {
                    Some(path) => path,
                    None => entry_path(header)?,
                };
                let rel = RelPath::from_str(path.as_str()) // BORROW: explicit .as_str()
                    .map_err(|error| invalid(TREATMENT, error.to_string()))?; // BORROW: .to_string() gives owned text
                let mode = octal(span(header, 100..108))?;
                // BORROW: .to_vec() copies the file out of the archive
                files.push(PreparedFile::new(rel, data.to_vec(), mode & 0o111 != 0));
            }
            other => {
                return Err(invalid(
                    TREATMENT,
                    format!("an entry has the type flag {other} (a link, device or FIFO)"),
                ));
            }
        }
    }
    Ok(files)
}

/// The path of the entry a ustar `header` describes: the prefix, a `/` and the name.
fn entry_path(header: &[u8]) -> Result<String, DriverError> {
    let text = |bytes| {
        std::str::from_utf8(text_field(bytes))
            .map_err(|_| invalid(TREATMENT, "a path is not text".to_owned())) // BORROW: .to_owned() makes the owned text
    };
    let name = text(span(header, 0..100))?;
    let ustar = span(header, 257..262) == b"ustar";
    let prefix = if ustar {
        text(span(header, 345..500))?
    } else {
        ""
    };
    Ok(if prefix.is_empty() {
        name.to_owned() // BORROW: .to_owned() makes the owned text
    } else {
        format!("{prefix}/{name}")
    })
}
