//! The MCP servers of a cell by rendered name: the rename and its two refusals.
#![cfg(test)]

use std::str::FromStr as _;

use candler_harness::driver::DriverError;
use candler_harness::mcp::servers;
use candler_harness::spec::Cell;
use candler_schemas::primitives::FieldPath;
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::{Value, json};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn cell(entries: &[(&str, Value)]) -> Res<Vec<ResolvedValue>> {
    entries
        .iter()
        .map(|(path, value)| {
            Ok(ResolvedValue::recorded(
                FieldPath::from_str(path)?,
                JsonValue::new(value.clone())?,
            ))
        })
        .collect()
}

fn detail(entries: &[(&str, Value)]) -> Res<String> {
    let values = cell(entries)?;
    match servers(&Cell::new(&values)) {
        Err(DriverError::InvalidValue { detail, .. }) => Ok(detail),
        other => Err(format!("expected an invalid value, got {other:?}").into()),
    }
}

/// Guards: G1 — a server with `name` is keyed by it with `name` removed, a server without keeps
/// its key, and a cell with no `name` reads as it did before the leaf existed.
/// Oracle: the literal tables written by hand.
#[test]
fn a_name_re_keys_the_server_and_leaves_no_name_leaf() -> Res<()> {
    let values = cell(&[
        ("mcp.servers.a.command", json!("uvx")),
        ("mcp.servers.a.name", json!("x-y")),
        ("mcp.servers.b.command", json!("node")),
    ])?;
    let got = servers(&Cell::new(&values))?;
    assert_eq!(
        Value::Object(got.ok_or("no servers")?),
        json!({"x-y": {"command": "uvx"}, "b": {"command": "node"}})
    );
    Ok(())
}

/// Guards: G1 — two servers that render one name are refused, name against name and name against
/// another server's key.
/// Oracle: the colliding pairs are written by hand.
#[test]
fn two_servers_rendering_one_name_are_refused() -> Res<()> {
    let by_name = detail(&[
        ("mcp.servers.a.name", json!("x-y")),
        ("mcp.servers.b.name", json!("x-y")),
    ])?;
    assert!(by_name.contains("both render as `x-y`"), "{by_name}");
    let by_key = detail(&[
        ("mcp.servers.a.name", json!("x_y")),
        ("mcp.servers.x_y.command", json!("uvx")),
    ])?;
    assert!(by_key.contains("both render as `x_y`"), "{by_key}");
    Ok(())
}

/// Guards: G1 — a `name` that is not a plain name, is not text, or holds `__` is refused.
/// Oracle: the names are written by hand against `NAME_PATTERN` (no `@`, no white space, non-empty).
#[test]
fn a_malformed_name_is_refused() -> Res<()> {
    for bad in [json!(""), json!("a b"), json!("a@b"), json!(3)] {
        detail(&[("mcp.servers.a.name", bad)])?;
    }
    let split = detail(&[("mcp.servers.a.name", json!("x__y"))])?;
    assert!(split.contains("contains `__`"), "{split}");
    Ok(())
}
