//! Guards: the experiment schema cannot type the `mcp.servers` path, so it holds the entries as
//! generic JSON; the published mcp-server schema is the typed reader for the one entry, and it
//! refuses every shape `candler_harness::mcp::read` refuses.
//!
//! The experiment schema types every `fixed` value and every factor level as the same generic JSON
//! value, so it does not check the fields of an entry that sits under `mcp.servers`. The mcp-server
//! schema is the typed reader for the one entry: it requires exactly one of `command` and `url`
//! and keeps the two families apart, matching the reader.

use std::path::Path;
use std::sync::OnceLock;

use jsonschema::{Validator, validator_for};
use serde_json::Value;

fn mcp_server_schema<'a>() -> &'a Validator {
    static MCP_SERVER: OnceLock<Validator> = OnceLock::new();
    MCP_SERVER.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/mcp-server.schema.json");
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let value: Value = serde_json::from_str(&text).expect("committed schema JSON");
        validator_for(&value).expect("the committed schema compiles")
    })
}

/// The entry under `mcp.servers` that `mcp::read` accepts without launching anything.
fn valid_launched_entry() -> Value {
    serde_json::json!({
        "type": "stdio",
        "command": "docker",
        "args": ["run", "-v"],
        "env": { "KEY": "value" },
        "codemode": false,
        "name": "movement"
    })
}

/// The entry under `mcp.servers` that `mcp::read` accepts as reachable at a URL.
fn valid_remote_entry() -> Value {
    serde_json::json!({ "type": "sse", "url": "http://h" })
}

/// Every entry `candler_harness::mcp::read` refuses for its shape is refused by the published
/// mcp-server schema, and the two valid entries are accepted.
fn mcp_server_refusal_rows() -> Vec<(&'static str, Value)> {
    vec![
        ("neither `command` nor `url`", serde_json::json!({})),
        (
            "a launched entry with no `command`",
            serde_json::json!({ "type": "stdio", "name": "n" }),
        ),
        (
            "a remote entry with no `url`",
            serde_json::json!({ "type": "remote" }),
        ),
        (
            "a `command` under a URL type",
            serde_json::json!({ "command": "x", "type": "http" }),
        ),
        (
            "a `url` under a launched type",
            serde_json::json!({ "url": "http://h", "type": "stdio" }),
        ),
        (
            "both `command` and `url`",
            serde_json::json!({ "command": "x", "url": "http://h" }),
        ),
        (
            "both, under a stdio type",
            serde_json::json!({ "command": "x", "url": "http://h", "type": "stdio" }),
        ),
        (
            "both, under a remote type",
            serde_json::json!({ "command": "x", "url": "http://h", "type": "remote" }),
        ),
        (
            "a remote entry inferred from `url` alone, with `env`",
            serde_json::json!({ "url": "http://h", "env": { "K": "v" } }),
        ),
        (
            "a remote entry with `env`",
            serde_json::json!({ "url": "http://h", "env": { "K": "v" }, "type": "remote" }),
        ),
        (
            "a remote entry with `args`",
            serde_json::json!({ "url": "http://h", "args": ["run"], "type": "remote" }),
        ),
        (
            "a launched entry with `headers`",
            serde_json::json!({ "command": "x", "headers": { "K": "v" } }),
        ),
    ]
}

/// Every entry `candler_harness::mcp::read` refuses for its shape is refused by the published
/// mcp-server schema, and the two valid entries are accepted.
#[test]
fn the_mcp_server_schema_refuses_what_the_reader_refuses() {
    let mcp_server = mcp_server_schema();
    for (shape, entry) in mcp_server_refusal_rows() {
        assert!(
            !mcp_server.is_valid(&entry),
            "mcp-server schema must refuse a server `{shape}`: {entry}"
        );
    }
    assert!(
        mcp_server.is_valid(&valid_launched_entry()),
        "the launched entry the reader accepts must validate: {}",
        valid_launched_entry()
    );
    assert!(
        mcp_server.is_valid(&valid_remote_entry()),
        "the remote entry the reader accepts must validate: {}",
        valid_remote_entry()
    );
}

/// The well-formed entry a driver writes validates against the mcp-server schema.
#[test]
fn the_mcp_server_schema_accepts_what_a_driver_writes() {
    let mcp_server = mcp_server_schema();
    let wellformed = valid_launched_entry();
    assert!(
        mcp_server.is_valid(&wellformed),
        "a well-formed stdio server validates: {wellformed}"
    );
}

/// Guards: serde refuses a key a server does not have, and reads the keys it has.
/// Oracle: the two entries are written by hand.
/// Bite: drop `deny_unknown_fields` from the type; the entry with `timeout` then reads.
#[test]
fn serde_refuses_an_unknown_key_in_a_server_entry() {
    let unknown = serde_json::json!({ "command": "uvx", "timeout": 5 });
    assert!(serde_json::from_value::<candler_schemas::servers::McpServer>(unknown).is_err());
    let known = serde_json::json!({ "command": "uvx", "args": ["x"], "codemode": false });
    assert!(serde_json::from_value::<candler_schemas::servers::McpServer>(known).is_ok());
}
