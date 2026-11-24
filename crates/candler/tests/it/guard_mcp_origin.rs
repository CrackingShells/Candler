#![cfg(test)]
//! Both readers record a call to one tool of one declared MCP server as the same origin.
//!
//! Guards: README (an MCP call reads the same in the trace of either harness) -- the recorded
//! output of one call to the tool `t` of the server `s`, read by Claude Code's live reader and by
//! `OpenCode`'s export reader, gives the same `origin`, and Claude Code's trace keeps the raw
//! `system/init` line that names the server's status.
//! Oracle: the `origin` object written by hand from the trace schema; the recorded streams in
//! `tests/fixtures/mcp_origin/`, whose tool is `mcp__s__t` for Claude Code and `s_t` for `OpenCode`.
//! Bite: a reader that loses the server (`other` for an unknown prefix), one that keeps the
//! harness's spelling of the tool, or a Claude Code reader that drops the init line.

use std::error::Error;
use std::path::{Path, PathBuf};

use candler_harness::reader::{FrameIn, TraceReader};
use candler_schemas::raw::{FrameData, RawFrame, RawRef, StreamName};
use candler_schemas::trace::SessionEvent;
use serde_json::{Value, json};

use crate::drivers::Registry;

// TRAIT_OBJECT: a test returns any error its steps raise; the error is only printed
type Res<T> = Result<T, Box<dyn Error>>;

fn fixture(file: &str) -> Res<Vec<u8>> {
    let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("mcp_origin");
    Ok(std::fs::read(dir.join(file))?)
}

/// Feeds each frame to `reader` and returns the events of the finished trace.
fn read_frames(
    mut reader: Box<dyn TraceReader>,
    stream: &str,
    frames: &[&[u8]],
) -> Res<Vec<SessionEvent>> {
    let name: StreamName = stream.parse()?;
    let mut events = Vec::new();
    for (index, bytes) in frames.iter().enumerate() {
        let frame = RawFrame::new(None, FrameData::from_bytes(bytes));
        let raw_ref = RawRef::new(name.clone(), u64::try_from(index)?); // BORROW: one name per frame
        events.extend(reader.feed(FrameIn::new(raw_ref, None, &frame))?);
    }
    reader.finish()?;
    Ok(events)
}

/// The `origin` of each tool call among `events`, as the trace file writes it.
fn origins(events: &[SessionEvent]) -> Res<Vec<Value>> {
    let mut found = Vec::new();
    for entry in events {
        let kind = serde_json::to_value(&entry.event.kind)?;
        if kind.get("kind") == Some(&json!("tool_call")) {
            found.push(kind.get("origin").cloned().unwrap_or(Value::Null)); // BORROW: .cloned() copies the origin
        }
    }
    Ok(found)
}

/// Claude Code's trace of the recorded stream.
fn claude_code() -> Res<Vec<SessionEvent>> {
    let bytes = fixture("claude_code.jsonl")?;
    let lines: Vec<&[u8]> = bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .collect();
    let driver = Registry::builtin()
        .driver("claude-code")
        .map_err(|_| "no driver")?;
    let reader = driver.live_reader().ok_or("no live reader")?;
    read_frames(reader, "stdout", &lines)
}

/// `OpenCode`'s trace of the recorded export of a cell that declares the server `s`.
fn opencode() -> Res<Vec<SessionEvent>> {
    let bytes = fixture("opencode_export.json")?;
    let driver = Registry::builtin()
        .driver("opencode")
        .map_err(|_| "no driver")?;
    let export: StreamName = "export".parse()?;
    let reader = driver.offline_reader(&export).ok_or("no export reader")?;
    read_frames(reader, "export", &[&bytes])
}

#[test]
fn one_call_has_one_origin_on_both_harnesses() -> Res<()> {
    let one = json!({"kind": "mcp", "server": "s", "tool": "t"});
    let want = json!({"claude-code": [one.clone()], "opencode": [one]}); // BORROW: .clone() gives each harness its own copy
    let got = json!({
        "claude-code": origins(&claude_code()?)?,
        "opencode": origins(&opencode()?)?,
    });
    assert_eq!(got, want, "the origin of the call on each harness");
    Ok(())
}

#[test]
fn claude_code_trace_keeps_the_raw_init_line() -> Res<()> {
    let events = serde_json::to_string(
        &claude_code()?
            .iter()
            .map(|entry| &entry.event)
            .collect::<Vec<_>>(),
    )?;
    assert!(
        events.contains("\"subtype\":\"init\"") && events.contains("\"status\":\"connected\""),
        "the trace holds the init line with the server status: {events}"
    );
    Ok(())
}
