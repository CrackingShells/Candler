//! Guards: the experiment schema cannot type the `mcp.servers` path, so a malformed server entry
//! still validates against it; the published mcp-server schema is the one reader that refuses it.
//!
//! The experiment schema types every `fixed` value and every factor level as the same generic JSON
//! value, so a server whose `command` is not text (here `123`) passes where `mcp.servers` sits.
//! The mcp-server schema is the typed reader for the one path, and it refuses the same entry.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use jsonschema::{Validator, validator_for};
use serde_json::Value;

fn schema_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("schema")
}

fn schema<'a>(name: &'static str) -> &'a Validator {
    static EXPERIMENT: OnceLock<Validator> = OnceLock::new();
    static MCP_SERVER: OnceLock<Validator> = OnceLock::new();
    let cell = match name {
        "experiment.schema.json" => &EXPERIMENT,
        "mcp-server.schema.json" => &MCP_SERVER,
        _ => panic!("unknown schema {name}"),
    };
    cell.get_or_init(|| {
        let path = schema_dir().join(name);
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let value: Value = serde_json::from_str(&text).expect("committed schema JSON");
        validator_for(&value).expect("the committed schema compiles")
    })
}

/// A server entry with a non-text `command`, the malformed shape this guard refuses.
fn malformed_server() -> Value {
    serde_json::json!({ "type": "stdio", "command": 123 })
}

/// The malformed server as a driver reads it: the value of the dotted path `mcp.servers` under
/// `fixed`, holding one server by key, within an otherwise valid experiment file.
fn malformed_experiment() -> Value {
    serde_json::json!({
        "schema": "experiment/1",
        "replicates": 1,
        "budget": { "max_runs": 1, "per_run": { "wall_s": 60 } },
        "harness": { "name": "opencode" },
        "fixed": { "mcp.servers": { "docker": malformed_server() } }
    })
}

/// The committed experiment schema still accepts a server whose `command` is not text, because it
/// reads every `fixed` path (including `mcp.servers`) as the same generic JSON value.
#[test]
fn the_experiment_schema_cannot_type_the_mcp_servers_path() {
    let experiment = schema("experiment.schema.json");
    let malformed = malformed_experiment();
    assert!(
        experiment.is_valid(&malformed),
        "experiment schema must hold `mcp.servers` as generic JSON; it validated: {malformed}"
    );
}

/// The published mcp-server schema refuses the same malformed entry.
#[test]
fn the_mcp_server_schema_refuses_a_non_text_command() {
    let mcp_server = schema("mcp-server.schema.json");
    let malformed = malformed_server();
    assert!(
        !mcp_server.is_valid(&malformed),
        "mcp-server schema must refuse a non-text command: {malformed}"
    );
}

/// The well-formed entry a driver writes validates against the mcp-server schema.
#[test]
fn the_mcp_server_schema_accepts_what_a_driver_writes() {
    let mcp_server = schema("mcp-server.schema.json");
    let wellformed = serde_json::json!({
        "type": "stdio",
        "command": "docker",
        "args": ["run", "-v"],
        "env": { "KEY": "value" },
        "codemode": false,
        "name": "movement"
    });
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
