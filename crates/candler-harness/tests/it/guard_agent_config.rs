//! The instruction texts and hooks of a cell, read as the harness's own envelope.
#![cfg(test)]

use std::str::FromStr as _;

use candler_harness::agent_config::{hooks, instructions};
use candler_harness::driver::DriverError;
use candler_harness::spec::Cell;
use candler_schemas::agent_config::{ClaudeCodeHooks, OpenCodeHooks};
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

/// Guards: the hooks of a Claude Code cell are read as the settings table and the files it
/// lists, those of an `OpenCode` cell as its plugins, and the global and workspace texts as their
/// envelope.
/// Oracle: the values are written by hand.
/// Bite: read the cell's hooks as one harness's whatever the harness.
#[test]
fn a_cell_gives_each_harness_its_own_envelope() -> Res<()> {
    let values = cell(&[
        (
            "hooks",
            json!({
                "claude-code": {"settings": {"Stop": []}, "files": {"guard.sh": "#!/bin/sh"}},
                "opencode": {"plugins": {"p.js": "x"}}
            }),
        ),
        ("instructions.global", json!({"claude-code": "global"})),
        ("instructions.workspace", json!("workspace")),
    ])?;
    let view = Cell::new(&values);
    let Some(found) = hooks::<ClaudeCodeHooks>(&view, "claude-code")? else {
        return Err("claude code hooks".into());
    };
    assert_eq!(found.settings.map(Value::Object), Some(json!({"Stop": []})));
    assert_eq!(found.files.len(), 1);
    let Some(found) = hooks::<OpenCodeHooks>(&view, "opencode")? else {
        return Err("opencode hooks".into());
    };
    assert_eq!(found.plugins.len(), 1);
    let Some(text) = instructions(&view, "claude-code")? else {
        return Err("instructions".into());
    };
    assert_eq!(
        text.global.get("claude-code").map(String::as_str),
        Some("global")
    );
    assert_eq!(text.workspace.as_deref(), Some("workspace"));
    Ok(())
}

/// Guards: a member that is not part of the envelope, settings that are not a table, and a file
/// path that is not a relative path are each refused at `hooks`, naming the member at fault.
/// Oracle: the refused values are written by hand.
/// Bite: ignore unknown members, or accept any path.
#[test]
fn an_entry_that_does_not_fit_its_envelope_is_refused() -> Res<()> {
    let refused = |hooks_value: Value| -> Res<String> {
        let values = cell(&[("hooks", hooks_value)])?;
        match hooks::<ClaudeCodeHooks>(&Cell::new(&values), "claude-code") {
            Err(DriverError::InvalidValue { path, detail }) => {
                assert_eq!(path.to_string(), "hooks");
                Ok(detail)
            }
            other => Err(format!("expected an invalid value, got {other:?}").into()),
        }
    };
    let unknown = refused(json!({"claude-code": {"plugins": {}}}))?;
    assert!(unknown.contains("plugins"), "{unknown}");
    let settings = refused(json!({"claude-code": {"settings": "[1]"}}))?;
    assert!(settings.contains("settings"), "{settings}");
    let path = refused(json!({"claude-code": {"files": {"../a": "x"}}}))?;
    assert!(path.contains("../a"), "{path}");
    Ok(())
}
