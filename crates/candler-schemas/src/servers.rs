//! The MCP servers an experiment declares, in the shape the drivers read them.
//!
//! One entry under `mcp.servers.<key>` is a table that names a server the cell's agent may call.
//! The drivers read the entry as a map (`candler_harness::mcp::servers`) and opencode's renderer
//! (`candler_harness_opencode::prepare::server`) takes the same fields; this module types them so a
//! malformed entry fails validation instead of being read by hand.
//!
//! The entry is typed here as its own generated schema, because the experiment schema cannot type
//! the one dotted path `mcp.servers`: under `fixed` every field-path key maps to the same generic
//! JSON value, and a factor level is likewise any JSON value. A server declared in either place
//! therefore sits under this type, and a single reader applies it in both.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::generate::GeneratedSchema;

/// The wire root schema this module publishes.
#[must_use]
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    vec![GeneratedSchema::of::<McpServer>("mcp-server")]
}

/// The transport family a server speaks: a locally-started command or a remote URL.
///
/// A `stdio` (or `local`) server starts a command and is the default when an entry sets no `type`;
/// a remote one is reached at a URL. The renderer reads `http`, `sse` and `remote` alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[expect(
    clippy::exhaustive_enums,
    reason = "EXHAUSTIVE: the drivers live in other crates and answer each transport; a new one must break each of them."
)]
pub enum McpServerKind {
    /// A server that prints the MCP protocol over a child process's standard streams.
    Stdio,
    /// A local server the drivers treat as a stdio one.
    Local,
    /// A server reached over HTTP.
    Http,
    /// A server reached over a Server-Sent Events stream.
    Sse,
    /// A server reached at a URL.
    Remote,
}

/// The names a server may render under, by `name` or by its key: ASCII letters, digits and `-`
/// joined by single `_`, so that no harness rewrites or splits the name in a tool id.
pub const SERVER_NAME_PATTERN: &str = r"^[A-Za-z0-9-]+(?:_[A-Za-z0-9-]+)*(?![\s\S])";

/// One entry of an experiment's `mcp.servers`, as the drivers read it.
///
/// A server is launched from a `command` (with optional `args` and `env`) or reached at a `url`
/// (with optional `headers`); exactly one of `command` and `url` is required, and a `type` marker
/// agrees with it: `stdio` or `local` with `command`, `http`, `sse` or `remote` with `url`.
/// `codemode` selects the prompts the agent sees, and is `false` when absent. A `name`
/// leaf names the server under a key; without it the key names it. Every field the drivers read
/// is typed here, so a non-text `command`, a non-list `args` or a non-text environment value fails
/// validation rather than being read leniently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
#[schemars(extend(
    "oneOf" = [
        {
            "required": ["command"],
            "properties": {
                "url": false,
                "headers": false,
                "type": { "enum": ["stdio", "local"] }
            }
        },
        {
            "required": ["url"],
            "properties": {
                "command": false,
                "args": false,
                "env": false,
                "type": { "enum": ["http", "sse", "remote"] }
            }
        }
    ]
))]
pub struct McpServer {
    /// The transport marker: `stdio` or `local` for a launched command, `http`, `sse` or `remote`
    /// for a URL; absent when the driver would launch a command.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<McpServerKind>,
    /// The launched command, as a program name or path; required of an entry with no URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The arguments the launched command receives, one per list item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    /// The environment of the launched command, by variable name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    /// The URL a remote server is reached at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The headers a remote server is called with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    /// Whether the server's tools show in the agent's codemode prompts; false when absent.
    #[serde(default)]
    pub codemode: bool,
    /// The name the server renders under, when its key would not do; a key
    /// matches the same pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(regex(pattern = SERVER_NAME_PATTERN))]
    pub name: Option<String>,
}
