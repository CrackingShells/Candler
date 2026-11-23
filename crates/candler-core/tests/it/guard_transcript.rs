//! The transcript of a stored trace: one rendered line per stored event, in the stored order.

use candler_core::trace::{TranscriptForm, render_transcript};
use candler_schemas::trace::TraceFile;

/// A stored trace written by hand: a root session, a child, and events of every known kind and of
/// a kind this version does not know, interleaved across the two sessions.
const STORED: [&str; 10] = [
    r#"{"kind":"header","schema":"normalised-trace/1","reader":"claude-code-reader@0.1.0","raw":[{"stream":"stdout","digest":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}]}"#,
    r#"{"kind":"session","id":"root-1","source":{"harness":"claude-code","harness_version":"2.1.0","reader":"claude-code-reader","reader_version":"0.1.0"},"raw_ref":{"stream":"stdout","frame":0}}"#,
    r#"{"kind":"session","id":"root-1/toolu_01","source":{"harness":"claude-code","harness_version":"2.1.0","reader":"claude-code-reader","reader_version":"0.1.0"},"parent_session":"root-1","raw_ref":{"stream":"stdout","frame":3}}"#,
    r#"{"kind":"event","session":"root-1","event":{"seq":0,"kind":{"kind":"user_message","text":"hi"},"raw_ref":{"stream":"stdout","frame":0},"ext":{}}}"#,
    r#"{"kind":"event","session":"root-1","event":{"seq":1,"at":"2026-11-05T10:00:01Z","kind":{"kind":"tool_call","name":"Bash","origin":{"kind":"builtin"},"argv":["ls","-l"]},"span":"toolu_01","raw_ref":{"stream":"stdout","frame":1},"ext":{}}}"#,
    r#"{"kind":"event","session":"root-1","event":{"seq":2,"kind":{"kind":"hologram","depth":3},"raw_ref":{"stream":"stdout","frame":2},"ext":{}}}"#,
    r#"{"kind":"event","session":"root-1/toolu_01","event":{"seq":0,"kind":{"kind":"tool_result","is_error":true},"span":"toolu_01","raw_ref":{"stream":"stdout","frame":3},"ext":{}}}"#,
    r#"{"kind":"event","session":"root-1","event":{"seq":3,"kind":{"kind":"usage","scope":"session_total","input_tokens":120,"output_tokens":45,"cache_read_tokens":0,"cache_creation_tokens":7,"cost_usd":0.0123},"raw_ref":{"stream":"stdout","frame":4},"ext":{}}}"#,
    r#"{"kind":"event","session":"root-1","event":{"seq":4,"kind":{"kind":"notification"},"raw_ref":{"stream":"stdout","frame":5},"ext":{"line":"warming up"}}}"#,
    r#"{"kind":"event","session":"root-1","event":{"seq":5,"kind":{"kind":"assistant_message"},"raw_ref":{"stream":"stdout","frame":6},"ext":{}}}"#,
];

/// The stored lines that hold an event, in file order.
fn stored_events() -> Vec<&'static str> {
    STORED
        .iter()
        .copied()
        .filter(|line| line.starts_with(r#"{"kind":"event""#))
        .collect()
}

/// Guards: a run's transcript lists every event of its stored trace, in the stored order, so no
/// event kind is dropped, grouped or reordered. It asserts the relation between the stored lines
/// and the rendered ones, so a new event kind needs no edit here.
/// Oracle: the stored trace's own lines, written by hand; the text form is read for each event's
/// session and `seq`, which the stored line also holds.
/// Bite: a renderer that skips a kind, or sorts events by kind, fails the JSON or the text row.
#[test]
fn transcript_lists_every_event_in_order() {
    let stored = stored_events();
    let mut text = STORED.join("\n");
    text.push('\n');
    let trace = TraceFile::read(text.as_bytes()).expect("the stored trace is valid");

    let json = render_transcript(&trace, TranscriptForm::Json);
    assert_eq!(json, stored, "JSON: each event as stored, in stored order");

    let lines = render_transcript(&trace, TranscriptForm::Text);
    assert_eq!(lines.len(), stored.len(), "text: one line per event");
    for (line, stored_line) in lines.iter().zip(&stored) {
        let event: serde_json::Value =
            serde_json::from_str(stored_line).expect("a stored line is JSON");
        let session = event["session"].as_str().expect("a session id");
        let seq = event["event"]["seq"].as_u64().expect("a seq");
        let kind = event["event"]["kind"]["kind"].as_str().expect("a kind");
        assert!(
            line.starts_with(&format!("{session} {seq} ")),
            "text line `{line}` opens with the event's session and seq"
        );
        assert!(
            line.contains(kind),
            "text line `{line}` names kind `{kind}`"
        );
    }
}
