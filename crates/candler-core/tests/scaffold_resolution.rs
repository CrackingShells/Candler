#![cfg(test)]
//! Development tests of the experiment resolution leaf.

use std::time::Duration;

use candler_core::experiment::budget::to_caps;
use candler_core::experiment::surface::{Surface, SurfaceError};
use candler_harness::spec::Cell;
use candler_schemas::execution::Budget;
use candler_schemas::primitives::FieldPath;
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::{Value, json};

fn rv(path: &str, value: Value) -> ResolvedValue {
    ResolvedValue::recorded(
        path.parse::<FieldPath>().expect("a field path"),
        JsonValue::new(value).expect("a JSON value"),
    )
}

fn base() -> Vec<ResolvedValue> {
    vec![
        rv("model.id", json!("m1")),
        rv("model.endpoint", json!("https://up.example/v1")),
        rv("permission_mode", json!("default")),
        rv("prompt.template", json!("do {task}")),
    ]
}

fn budget(per_run: &Value) -> Budget {
    serde_json::from_value(json!({ "per_run": per_run, "max_cost_usd": 99.0 })).expect("a budget")
}

#[test]
fn a_minimal_cell_reads() {
    let values = base();
    let surface = Surface::from_cell(&Cell::new(&values)).expect("a surface");
    assert_eq!(surface.model_id, "m1");
    assert_eq!(surface.tools_allowed.len(), 0);
    assert!(surface.mcp.is_none());
}

#[test]
fn every_required_path_is_required() {
    for drop in [
        "model.id",
        "model.endpoint",
        "permission_mode",
        "prompt.template",
    ] {
        let values: Vec<_> = base()
            .into_iter()
            .filter(|v| v.path().as_str() != drop)
            .collect();
        let error = Surface::from_cell(&Cell::new(&values)).expect_err(drop);
        assert!(
            matches!(error, SurfaceError::Missing(ref p) if p.as_str() == drop),
            "{drop}"
        );
    }
}

#[test]
fn wrong_types_and_absolute_paths_are_refused() {
    let mut values = base();
    values.push(rv("tools.allowed", json!("Bash")));
    assert!(matches!(
        Surface::from_cell(&Cell::new(&values)),
        Err(SurfaceError::Type { .. })
    ));
    let mut values = base();
    values.push(rv("mcp", json!({"fs": {"command": "/usr/bin/x"}})));
    assert!(matches!(
        Surface::from_cell(&Cell::new(&values)),
        Err(SurfaceError::AbsolutePath { .. })
    ));
}

#[test]
fn a_value_below_a_reserved_root_is_refused() {
    let mut values = base();
    values.push(rv("harness.extra", json!("x")));
    assert!(matches!(
        Surface::from_values(&values),
        Err(SurfaceError::ReservedRoot { .. })
    ));
}

#[test]
fn caps_become_flags_and_expiry() {
    let caps = to_caps(&budget(
        &json!({"wall_s": 3600, "turns": 7, "max_cost_usd": 0.5}),
    ));
    assert_eq!(
        caps.flags(),
        ["--max-turns", "7", "--max-budget-usd", "0.5"]
    );
    assert_eq!(caps.expiry(), Some(Duration::from_mins(61)));
    let bare = to_caps(&budget(&json!({"wall_s": 10})));
    assert_eq!(bare.flags().len(), 0);
}
