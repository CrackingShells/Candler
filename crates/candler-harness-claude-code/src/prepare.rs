//! The files the driver asks the core to write: the settings, the hook files, the MCP
//! configuration and the instruction texts.
//!
//! Nothing here touches the file system or runs a process.

use std::str::FromStr as _;

use candler_harness::agent_config::{hooks, instructions};
use candler_harness::driver::DriverError;
use candler_harness::spec::{CLAUDE_CODE, Cell, PreparedFile};
use candler_schemas::agent_config::ClaudeCodeHooks;
use candler_schemas::execution::paths::{TOOLS_ALLOWED, TOOLS_DISALLOWED};
use candler_schemas::field_path;
use candler_schemas::primitives::RelPath;
use candler_schemas::servers::{McpServer, McpServerKind};
use serde_json::{Map, Value, json};

use crate::driver::invalid;

/// The names a list of the cell holds, or none when the cell sets no list.
fn names(cell: &Cell<'_>, path: &str) -> Result<Vec<String>, DriverError> {
    match cell
        .value_str(path)
        .map_err(|error| invalid(path, error.to_string()))?
    {
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

/// The `.claude/settings.json` file of a cell: `permissions.allow` from `tools.allowed`,
/// `permissions.deny` from `tools.disallowed` and `hooks` from the `settings` of the cell's Claude
/// Code hooks, each only when the cell sets it. Nothing else of the hooks is taken, so they cannot
/// set another key.
///
/// # Errors
/// [`DriverError`] when a value of the cell cannot be used.
pub fn render_settings(cell: &Cell<'_>) -> Result<PreparedFile, DriverError> {
    let mut permissions = Map::new();
    if cell
        .value_str(TOOLS_ALLOWED)
        .map_err(|error| invalid(TOOLS_ALLOWED, error.to_string()))?
        .is_some()
    {
        // BORROW: .to_owned() makes the owned key
        permissions.insert("allow".to_owned(), json!(names(cell, TOOLS_ALLOWED)?));
    }
    if cell
        .value_str(TOOLS_DISALLOWED)
        .map_err(|error| invalid(TOOLS_DISALLOWED, error.to_string()))?
        .is_some()
    {
        // BORROW: .to_owned() makes the owned key
        permissions.insert("deny".to_owned(), json!(names(cell, TOOLS_DISALLOWED)?));
    }
    let mut root = Map::new();
    if !permissions.is_empty() {
        // BORROW: .to_owned() makes the owned key
        root.insert("permissions".to_owned(), Value::Object(permissions));
    }
    if let Some(settings) =
        hooks::<ClaudeCodeHooks>(cell, CLAUDE_CODE)?.and_then(|found| found.settings)
    {
        // BORROW: .to_owned() makes the owned key
        root.insert("hooks".to_owned(), Value::Object(settings));
    }
    json_file(".claude/settings.json", &Value::Object(root))
}

/// The directory the hook files of a cell go under, relative to the workspace.
const HOOK_DIR: &str = ".claude/hooks";

/// The hook files of a cell, each under `.claude/hooks/` and executable.
///
/// # Errors
/// [`DriverError`] when the cell's hooks do not fit their envelope or a path cannot be used.
pub fn render_hook_files(cell: &Cell<'_>) -> Result<Vec<PreparedFile>, DriverError> {
    let Some(found) = hooks::<ClaudeCodeHooks>(cell, CLAUDE_CODE)? else {
        return Ok(Vec::new());
    };
    found
        .files
        .iter()
        .map(|(path, text)| {
            let at = format!("{HOOK_DIR}/{path}");
            let rel =
                RelPath::from_str(&at).map_err(|error| invalid("hooks", error.to_string()))?; // BORROW: .to_string() gives owned text
            Ok(PreparedFile::new(rel, text.as_bytes().to_vec(), true)) // BORROW: .to_vec() copies the text into the file
        })
        .collect()
}

/// The instruction files of a cell: the global text as `.claude/CLAUDE.md` in the agent's home, and
/// the workspace text as `CLAUDE.md`, each only when the cell sets it. The workspace file is
/// mounted read-only by the core and written by no driver, so it is the plan's workspace file.
///
/// # Errors
/// [`DriverError`] when the cell's instructions cannot be used.
pub fn render_instructions(
    cell: &Cell<'_>,
) -> Result<(Option<PreparedFile>, Option<PreparedFile>), DriverError> {
    let Some(found) = instructions(cell, CLAUDE_CODE)? else {
        return Ok((None, None));
    };
    let file =
        |path: &str| RelPath::from_str(path).map_err(|error| invalid(path, error.to_string())); // BORROW: .to_string() gives owned text
    let global_path = file(GLOBAL_FILE)?;
    let workspace_path = file(WORKSPACE_FILE)?;
    let global = found.global.get(CLAUDE_CODE).map(|text| {
        // BORROW: .to_vec() copies the text into the file
        PreparedFile::in_home(global_path, text.as_bytes().to_vec(), false)
    });
    let workspace = found.workspace.as_ref().map(|text| {
        // BORROW: .to_vec() copies the text into the file
        PreparedFile::new(workspace_path, text.as_bytes().to_vec(), false)
    });
    Ok((global, workspace))
}

/// Whether the cell sets a global text for Claude Code, which only the `user` setting source reads.
///
/// # Errors
/// [`DriverError`] when the cell's instructions cannot be used.
pub fn has_global(cell: &Cell<'_>) -> Result<bool, DriverError> {
    Ok(
        instructions(cell, CLAUDE_CODE)?
            .is_some_and(|found| found.global.contains_key(CLAUDE_CODE)),
    )
}

/// The global instruction file, relative to the agent's home.
const GLOBAL_FILE: &str = ".claude/CLAUDE.md";
/// The workspace instruction file, at the workspace root.
pub(crate) const WORKSPACE_FILE: &str = "CLAUDE.md";

/// The file the MCP configuration is written to, and that the agent command names.
pub(crate) const MCP_FILE: &str = ".mcp.json";

/// One server as Claude Code's `.mcp.json` documents it: `stdio` with `command`, `args` and `env`,
/// `http` or `sse` with `url` and `headers`. A `local` server is written as `stdio` and a `remote`
/// one as `http`.
fn mcp_entry(name: &str, server: &McpServer) -> Result<Value, DriverError> {
    let refused = |what: &str| DriverError::InvalidValue {
        path: field_path!("mcp.servers"),
        detail: format!("the server `{name}` {what}"),
    };
    let mut out = Map::new();
    match candler_harness::mcp::kind(server) {
        McpServerKind::Stdio | McpServerKind::Local => {
            let command = server
                .command
                .as_ref()
                .ok_or_else(|| refused("has no `command`"))?;
            out.insert("type".to_owned(), json!("stdio")); // BORROW: .to_owned() makes the owned key
            out.insert("command".to_owned(), json!(command)); // BORROW: .to_owned() makes the owned key
            if let Some(args) = &server.args {
                out.insert("args".to_owned(), json!(args)); // BORROW: .to_owned() makes the owned key
            }
            if let Some(env) = &server.env {
                out.insert("env".to_owned(), json!(env)); // BORROW: .to_owned() makes the owned key
            }
        }
        kind @ (McpServerKind::Http | McpServerKind::Sse | McpServerKind::Remote) => {
            let url = server.url.as_ref().ok_or_else(|| refused("has no `url`"))?;
            let transport = if kind == McpServerKind::Sse {
                "sse"
            } else {
                "http"
            };
            out.insert("type".to_owned(), json!(transport)); // BORROW: .to_owned() makes the owned key
            out.insert("url".to_owned(), json!(url)); // BORROW: .to_owned() makes the owned key
            if let Some(headers) = &server.headers {
                out.insert("headers".to_owned(), json!(headers)); // BORROW: .to_owned() makes the owned key
            }
        }
    }
    Ok(Value::Object(out))
}

/// The `.mcp.json` file of a cell: the servers of `mcp.servers` as `mcpServers`, each under its
/// name and written in Claude Code's own shape. `None` when the cell sets no server.
///
/// # Errors
/// [`DriverError`] when a value of the cell cannot be used.
pub fn render_mcp(cell: &Cell<'_>) -> Result<Option<PreparedFile>, DriverError> {
    let Some(servers) = candler_harness::mcp::servers(cell)? else {
        return Ok(None);
    };
    let entries = servers
        .iter()
        .map(|(name, server)| Ok((name.clone(), mcp_entry(name, server)?))) // BORROW: .clone() copies the name into the key
        .collect::<Result<Map<String, Value>, DriverError>>()?;
    let mut root = Map::new();
    root.insert("mcpServers".to_owned(), Value::Object(entries)); // BORROW: .to_owned() makes the owned key
    json_file(MCP_FILE, &Value::Object(root)).map(Some)
}

#[cfg(test)]
mod tests {
    use candler_harness::driver::DriverError;
    use candler_harness::spec::Cell;
    use candler_schemas::primitives::FieldPath;
    use candler_schemas::values::ResolvedValue;

    use super::names;

    /// Guards: C2 — a path that is not a field path refuses as `DriverError::VersionOutput` with
    /// the parser's words, as the removed `value_at` copy did through `invalid`.
    /// Oracle: the variant and the parser's message, written by hand.
    #[test]
    fn names_refuses_a_bad_path_as_version_output() -> Result<(), Box<dyn std::error::Error>> {
        let values: Vec<ResolvedValue> = Vec::new();
        let parser = "Not.A.Path".parse::<FieldPath>().err().ok_or("parsed")?;
        assert_eq!(
            names(&Cell::new(&values), "Not.A.Path"),
            Err(DriverError::VersionOutput {
                detail: parser.to_string()
            })
        );
        Ok(())
    }
}
