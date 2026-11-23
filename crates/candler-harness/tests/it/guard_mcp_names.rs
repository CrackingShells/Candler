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
