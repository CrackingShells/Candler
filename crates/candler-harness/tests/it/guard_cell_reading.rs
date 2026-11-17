//! Reading a cell by a dotted path: `Cell::value_str` and `Cell::table_at`.
#![cfg(test)]

use std::str::FromStr as _;

use candler_harness::driver::DriverError;
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

/// Guards: C2 — a path written as text finds the value the typed path finds, `None` where the cell
/// has none, and refuses text that is not a field path with the parser's own words.
/// Oracle: the values and the parser's message, written by hand.
#[test]
fn value_str_reads_by_text() -> Res<()> {
    let values = cell(&[
        ("budget.per_run.turns", json!(7)),
        ("tools.allowed", json!("x")),
    ])?;
    let cell = Cell::new(&values);
    assert_eq!(cell.value_str("budget.per_run.turns")?, Some(&json!(7)));
    assert_eq!(cell.value_str("budget.per_run.wall_s")?, None);
    assert_eq!(cell.value_str("budget.per_run")?, None);
    let refused = cell.value_str("Not.A.Path").err().ok_or("accepted")?;
    assert_eq!(
        DriverError::from(refused.clone()),
        DriverError::VersionOutput {
            detail: refused.to_string()
        }
    );
    Ok(())
}

/// Guards: C2 — a table is rebuilt from its leaves, an object at the root merges in whole, a cell
/// with nothing there reads as `None`, and the two refusals name the leaf and say why.
/// Oracle: the literal tables and messages written by hand.
#[test]
fn table_at_rebuilds_the_table_from_leaves() -> Res<()> {
    let root = FieldPath::from_str("mcp.servers")?;
    let values = cell(&[
        ("mcp.servers.a.command", json!("uvx")),
        ("mcp.servers.a.args", json!(["-q"])),
        ("mcp.servers.b.env.k", json!("v")),
        ("mcp.serversx.c", json!(1)),
        ("model.id", json!("m")),
    ])?;
    assert_eq!(
        Value::Object(Cell::new(&values).table_at(&root)?.ok_or("no table")?),
        json!({"a": {"command": "uvx", "args": ["-q"]}, "b": {"env": {"k": "v"}}})
    );

    let values = cell(&[("mcp.servers", json!({"s": {"command": "x"}}))])?;
    assert_eq!(
        Value::Object(Cell::new(&values).table_at(&root)?.ok_or("no table")?),
        json!({"s": {"command": "x"}})
    );

    let values = cell(&[("model.id", json!("m"))])?;
    assert_eq!(Cell::new(&values).table_at(&root)?, None);

    let values = cell(&[("mcp.servers", json!(1))])?;
    assert_eq!(
        Cell::new(&values).table_at(&root),
        Err(DriverError::InvalidValue {
            path: root.clone(),
            detail: "is not an object".to_owned()
        })
    );

    let values = cell(&[("mcp.servers.a", json!(1)), ("mcp.servers.a.b", json!(2))])?;
    assert_eq!(
        Cell::new(&values).table_at(&root),
        Err(DriverError::InvalidValue {
            path: FieldPath::from_str("mcp.servers.a.b")?,
            detail: "lies below a value that is not a table".to_owned()
        })
    );
    Ok(())
}
