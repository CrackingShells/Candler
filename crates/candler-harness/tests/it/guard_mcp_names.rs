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
        ("mcp.servers.a.command", json!("uvx")),
        ("mcp.servers.b.name", json!("x-y")),
        ("mcp.servers.b.command", json!("uvx")),
    ])?;
    assert!(by_name.contains("both render as `x-y`"), "{by_name}");
    let by_key = detail(&[
        ("mcp.servers.a.name", json!("x_y")),
        ("mcp.servers.a.command", json!("uvx")),
        ("mcp.servers.x_y.command", json!("uvx")),
    ])?;
    assert!(by_key.contains("both render as `x_y`"), "{by_key}");
    Ok(())
}

/// Guards: R01 §Agent environment — a server name, given by `name` or by the key, is ASCII letters,
/// digits and `-` joined by single `_`: `s_`, `_s`, `a__b`, `a.b` and `my srv` are refused, and
/// `a-b` and `a_b` are accepted. A key goes through the same check as a `name`.
/// Oracle: the names are written by hand from `^[A-Za-z0-9-]+(_[A-Za-z0-9-]+)*$`.
/// Bite: a check that applies to `name` only, or one that lets a trailing `_`, a `.` or a space by.
#[test]
fn a_server_name_is_letters_digits_and_dashes_joined_by_single_underscores() -> Res<()> {
    for refused in ["s_", "_s", "a__b", "a.b", "my srv", "a@b", ""] {
        let by_name = detail(&[
            ("mcp.servers.k.name", json!(refused)),
            ("mcp.servers.k.command", json!("uvx")),
        ])?;
        assert!(
            by_name.contains("not made of ASCII letters"),
            "{refused}: {by_name}"
        );
    }
    for key in ["s_", "a__b"] {
        let by_key = detail(&[(&format!("mcp.servers.{key}.command"), json!("uvx"))])?;
        assert!(
            by_key.contains("not made of ASCII letters"),
            "{key}: {by_key}"
        );
    }
    for accepted in ["a-b", "a_b", "A1", "x-y_z"] {
        let values = cell(&[
            ("mcp.servers.k.name", json!(accepted)),
            ("mcp.servers.k.command", json!("uvx")),
        ])?;
        let found = servers(&Cell::new(&values))?.ok_or("no servers")?;
        assert_eq!(
            found.keys().map(String::as_str).collect::<Vec<_>>(),
            [accepted]
        );
    }
    let keyed = cell(&[("mcp.servers.a_b.command", json!("uvx"))])?;
    assert!(servers(&Cell::new(&keyed))?.is_some_and(|found| found.contains_key("a_b")));
    Ok(())
}
