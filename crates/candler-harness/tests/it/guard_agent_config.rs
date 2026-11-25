//! The instruction texts and hooks of a cell, read as the harness's own envelope.
#![cfg(test)]

use std::str::FromStr as _;

use candler_harness::agent_config::{Hooks, hooks, instructions};
use candler_harness::driver::DriverError;
use candler_harness::spec::Cell;
use candler_schemas::primitives::FieldPath;
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::{Value, json};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn registered() -> Vec<String> {
    vec!["claude-code".to_owned(), "opencode".to_owned()]
}

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

/// Guards: the hooks of a Claude Code cell are read as the settings object its text holds and the
/// files it lists, and the global and workspace texts as their envelope.
/// Oracle: the values are written by hand.
/// Bite: read `settings` as text, or the file list as a list (the typed envelopes differ).
#[test]
fn a_cell_gives_each_harness_its_own_envelope() -> Res<()> {
    let values = cell(&[
        ("hooks.claude_code.settings", json!("{\"Stop\": []}")),
        (
            "hooks.claude_code.files",
            json!([{"path": "guard.sh", "text": "#!/bin/sh"}]),
        ),
        (
            "hooks.opencode.plugins",
            json!([{"path": "p.js", "text": "x"}]),
        ),
        ("instructions.global.claude_code", json!("global")),
        ("instructions.workspace", json!("workspace")),
    ])?;
    let view = Cell::new(&values);
    let Some(Hooks::ClaudeCode(found)) = hooks(&view, "claude-code", &registered())? else {
        return Err("claude code hooks".into());
    };
    assert_eq!(found.settings.map(Value::Object), Some(json!({"Stop": []})));
    assert_eq!(found.files.len(), 1);
    let Some(Hooks::OpenCode(found)) = hooks(&view, "opencode", &registered())? else {
        return Err("opencode hooks".into());
    };
    assert_eq!(found.plugins.len(), 1);
    let Some(text) = instructions(&view, "claude-code", &registered())? else {
        return Err("instructions".into());
    };
    assert_eq!(
        text.global.get("claude-code").map(String::as_str),
        Some("global")
    );
    assert_eq!(text.workspace.as_deref(), Some("workspace"));
    Ok(())
}

/// Guards: an entry that is not an envelope member, settings that are not a JSON object, and a
/// file path that appears twice are each refused at the value.
/// Oracle: the refused values are written by hand.
/// Bite: ignore unknown members, or parse the settings leniently.
#[test]
fn an_entry_that_does_not_fit_its_envelope_is_refused() -> Res<()> {
    let refused = |entries: &[(&str, Value)]| -> Res<String> {
        let values = cell(entries)?;
        match hooks(&Cell::new(&values), "claude-code", &registered()) {
            Err(DriverError::InvalidValue { path, .. }) => Ok(path.to_string()),
            other => Err(format!("expected an invalid value, got {other:?}").into()),
        }
    };
    assert_eq!(
        refused(&[("hooks.claude_code.plugins", json!([]))])?,
        "hooks.claude_code.plugins"
    );
    assert_eq!(
        refused(&[("hooks.claude_code.settings", json!("[1]"))])?,
        "hooks.claude_code.settings"
    );
    assert_eq!(
        refused(&[("hooks.claude_code.settings", json!("not json"))])?,
        "hooks.claude_code.settings"
    );
    assert_eq!(
        refused(&[(
            "hooks.claude_code.files",
            json!([{"path": "a", "text": "x"}, {"path": "a", "text": "y"}])
        )])?,
        "hooks.claude_code.files"
    );
    Ok(())
}
