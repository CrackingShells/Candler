#![cfg(test)]
//! The Claude Code reader against eight streams: facts, citation, usage, version table, replay and the digest ratchet.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use candler_harness::reader::{FrameIn, TraceReader};
use candler_harness::spec::Confidence;
use candler_harness_claude_code::{ClaudeCodeReader, READER};
use candler_schemas::primitives::Digest;
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::{FrameData, RawFrame, RawRef, StreamName};
use candler_schemas::trace::{
    Cap, EndedBy, EventKind, RawFileRef, Session, SessionEvent, ToolOrigin, TraceFile, TraceHeader,
    UsageScope,
};
use sha2::{Digest as _, Sha256};

// TRAIT_OBJECT: a test returns any error its steps raise; the error is only printed
type Res<T> = Result<T, Box<dyn Error>>;

/// The stream name every fixture is read under.
const STREAM: &str = "agent_stream.jsonl";

/// One fixture stream with its hand-written facts.
struct Stream {
    name: String,
    expected: toml::Table,
    totals: toml::Table,
    bytes: Vec<u8>,
    lines: Vec<Vec<u8>>,
}

/// What a reader produced for one stream.
struct Output {
    events: Vec<SessionEvent>,
    sessions: Vec<Session>,
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn field<'a>(table: &'a toml::Table, key: &str) -> Res<&'a toml::Value> {
    Ok(table.get(key).ok_or(format!("missing key `{key}`"))?)
}

fn text<'a>(table: &'a toml::Table, key: &str) -> Res<&'a str> {
    Ok(field(table, key)?
        .as_str() // BORROW: .as_str() borrows the text
        .ok_or(format!("`{key}` is not text"))?)
}

fn list<'a>(table: &'a toml::Table, key: &str) -> Res<&'a [toml::Value]> {
    Ok(field(table, key)?
        .as_array()
        .ok_or(format!("`{key}` is not a list"))?)
}

fn table<'a>(value: &'a toml::Value, what: &str) -> Res<&'a toml::Table> {
    Ok(value.as_table().ok_or(format!("{what} is not a table"))?)
}

fn texts(table: &toml::Table, key: &str) -> Res<Vec<String>> {
    list(table, key)?
        .iter()
        .map(|value| {
            Ok(value.as_str().ok_or("not text")?.to_owned()) // BORROW: owned copy of the expected text
        })
        .collect()
}

fn frame_numbers(table: &toml::Table, key: &str) -> Res<BTreeSet<u64>> {
    list(table, key)?
        .iter()
        .map(|value| Ok(u64::try_from(value.as_integer().ok_or("not an integer")?)?))
        .collect()
}

fn read_table(path: &Path) -> Res<toml::Table> {
    Ok(fs::read_to_string(path)?.parse()?)
}

fn load_streams() -> Res<Vec<Stream>> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(fixtures().join("streams"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()?;
    dirs.sort();
    let mut streams = Vec::new();
    for dir in dirs {
        let expected = read_table(&dir.join("expected.toml"))?;
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(fs::File::open(dir.join(text(&expected, "stream")?))?)
            .read_to_end(&mut bytes)?;
        let mut lines: Vec<Vec<u8>> = bytes
            .split(|byte| *byte == b'\n')
            .map(<[u8]>::to_vec)
            .collect();
        if lines.last().is_some_and(Vec::is_empty) {
            lines.pop();
        }
        let name = dir
            .file_name()
            .ok_or("a stream directory has no name")?
            .to_string_lossy()
            .into_owned();
        streams.push(Stream {
            name,
            totals: read_table(&dir.join("usage_totals.toml"))?,
            expected,
            bytes,
            lines,
        });
    }
    Ok(streams)
}

/// Feeds `lines` to a fresh reader, asking it what it has observed after every frame when `watch` is set.
fn run(lines: &[Vec<u8>], watch: bool) -> Res<Output> {
    let stream: StreamName = STREAM.parse()?;
    let mut reader = ClaudeCodeReader::new();
    let mut events = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let frame = RawFrame::new(None, FrameData::from_bytes(line));
        let raw_ref = RawRef::new(stream.clone(), u64::try_from(index)?); // BORROW: one name per frame
        events.extend(reader.feed(FrameIn::new(raw_ref, None, &frame))?);
        if watch {
            let _seen = reader.observed();
        }
    }
    let sessions = Box::new(reader).finish()?;
    Ok(Output { events, sessions })
}

fn sha256_hex(bytes: &[u8]) -> Res<String> {
    let mut hex = String::new();
    for byte in &Sha256::digest(bytes) {
        write!(hex, "{byte:02x}")?;
    }
    Ok(hex)
}

fn trace_bytes(stream: &Stream, output: Output) -> Res<Vec<u8>> {
    let raw = RawFileRef::new(
        STREAM.parse()?,
        Digest::from_hex(&sha256_hex(&stream.bytes)?)?,
    );
    let header = TraceHeader::new(ClaudeCodeReader::new().id(), vec![raw]);
    Ok(TraceFile::new(header, output.sessions, output.events)?.write())
}

/// `completed`, `gateway_error`, `unknown`, `agent_error` or `cap:<cap>`, the way `expected.toml` writes an end.
fn label(ended_by: &EndedBy) -> String {
    match ended_by {
        EndedBy::Completed => "completed".to_owned(), // BORROW: .to_owned() gives owned copy of the text
        EndedBy::AgentError { reason, .. } => reason.as_ref().map_or_else(
            || "agent_error".to_owned(), // BORROW: .to_owned() gives owned copy of the text
            |reason| format!("agent_error/{reason}"),
        ),
        EndedBy::Cap { cap, .. } => match cap {
            Cap::Turns => "cap:turns".to_owned(), // BORROW: .to_owned() gives owned copy of the text
            Cap::Cost => "cap:cost".to_owned(), // BORROW: .to_owned() gives owned copy of the text
            Cap::Wall => "cap:wall".to_owned(), // BORROW: .to_owned() gives owned copy of the text
            _ => "cap:other".to_owned(), // BORROW: .to_owned() gives owned copy of the text // EXPLICIT: a cap this test does not know fails the comparison
        },
        EndedBy::GatewayError => "gateway_error".to_owned(), // BORROW: .to_owned() gives owned copy of the text
        EndedBy::BudgetStop => "budget_stop".to_owned(), // BORROW: .to_owned() gives owned copy of the text
        EndedBy::Killed => "killed".to_owned(), // BORROW: .to_owned() gives owned copy of the text
        EndedBy::Interrupted => "interrupted".to_owned(), // BORROW: .to_owned() gives owned copy of the text
        EndedBy::Unknown => "unknown".to_owned(), // BORROW: .to_owned() gives owned copy of the text
        _ => "other".to_owned(), // BORROW: .to_owned() gives owned copy of the text // EXPLICIT: an end this test does not know fails the comparison
    }
}

/// The sessions `expected.toml` lists: id, parent and end label.
fn expected_sessions(stream: &Stream) -> Res<Vec<(String, Option<String>, String)>> {
    list(&stream.expected, "sessions")?
        .iter()
        .map(|entry| {
            let entry = table(entry, "a session")?;
            let ended = text(entry, "ended_by")?;
            let end = match (entry.get("cap"), entry.get("reason")) {
                (Some(cap), _) => format!("{ended}:{}", cap.as_str().ok_or("cap")?), // BORROW: .as_str() borrows the text
                (None, Some(reason)) => format!("{ended}/{}", reason.as_str().ok_or("reason")?), // BORROW: .as_str() borrows the text
                (None, None) => ended.to_owned(), // BORROW: owned label
            };
            Ok((
                text(entry, "id")?.to_owned(), // BORROW: owned copy of the expected id
                entry
                    .get("parent")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned), // BORROW: owned copy
                end,
            ))
        })
        .collect()
}

/// The frames `expected.toml` pins to a session, as (frame, session).
fn expected_frame_sessions(stream: &Stream) -> Res<Vec<(u64, String)>> {
    let mut rows = Vec::new();
    for entry in list(&stream.expected, "frame_sessions")? {
        let entry = table(entry, "a frame row")?;
        let frame = u64::try_from(field(entry, "frame")?.as_integer().ok_or("frame")?)?;
        rows.push((frame, text(entry, "session")?.to_owned())); // BORROW: owned copy
    }
    Ok(rows)
}

/// A denial as `expected.toml` writes it: call id, reason, who refused it, and the session that holds the result.
type DenialRow = (String, Option<String>, String, String);

/// The denied results of a stream: call id, reason, who refused (as the wire names it) and session.
fn denied(output: &Output) -> Res<Vec<DenialRow>> {
    let mut rows = Vec::new();
    for record in &output.events {
        if let (
            EventKind::ToolResult {
                denied: Some(denial),
                ..
            },
            Some(span),
        ) = (&record.event.kind, &record.event.span)
        {
            let by = serde_json::to_value(denial.by)?
                .as_str() // BORROW: .as_str() borrows the text
                .ok_or("a denial's `by` is not text")?
                .to_owned(); // BORROW: owned wire name
            rows.push((
                span.to_string(),
                denial.reason.clone(), // BORROW: owned copy of the reason
                by,
                record.session.to_string(),
            ));
        }
    }
    rows.sort();
    Ok(rows)
}

fn expected_denials(stream: &Stream) -> Res<Vec<DenialRow>> {
    let mut rows = Vec::new();
    for entry in list(&stream.expected, "denials")? {
        let entry = table(entry, "a denial")?;
        rows.push((
            text(entry, "tool_use_id")?.to_owned(), // BORROW: owned copies of the expected text
            Some(text(entry, "reason")?.to_owned()), // BORROW: owned copy
            text(entry, "by")?.to_owned(),          // BORROW: owned copy
            text(entry, "session")?.to_owned(),     // BORROW: owned copy
        ));
    }
    rows.sort();
    Ok(rows)
}

/// The `result` line of a session `S` of a stream that starts with an init line of Claude Code 2.1.220.
fn stream_with(result: Option<&str>) -> Vec<Vec<u8>> {
    let init = r#"{"type":"system","subtype":"init","session_id":"S","model":"m","claude_code_version":"2.1.220"}"#;
    let mut lines = vec![init.as_bytes().to_vec()]; // BORROW: .to_vec() gives owned bytes
    lines.extend(result.map(|line| line.as_bytes().to_vec())); // BORROW: .to_vec() gives owned bytes
    lines
}

/// A result's exit as a row states it: session, call id, the name of the call, and the exit code when there is one.
type ExitRow<'a> = (&'a str, &'a str, &'a str, Option<i32>);

fn expected_exits(stream: &Stream) -> Res<Vec<ExitRow<'_>>> {
    let Some(rows) = stream.expected.get("exits") else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for entry in rows.as_array().ok_or("`exits` is not a list")? {
        let entry = table(entry, "an exit row")?;
        let code = entry
            .get("code")
            .map(|code| -> Res<i32> { Ok(i32::try_from(code.as_integer().ok_or("code")?)?) })
            .transpose()?;
        out.push((
            text(entry, "session")?,
            text(entry, "tool_use_id")?,
            text(entry, "tool")?,
            code,
        ));
    }
    Ok(out)
}

/// Checks each row against the reader's result of that call in that session: the call has the name the row gives, and the result has the exit code the row gives, or none.
fn check_exits(name: &str, output: &Output, rows: &[ExitRow<'_>]) -> Res<()> {
    for &(session, id, tool, want) in rows {
        let held: Vec<&SessionEvent> = output
            .events
            .iter()
            .filter(|record| {
                record.session.as_str() == session
                    && record
                        .event
                        .span
                        .as_ref()
                        .is_some_and(|span| span.as_str() == id)
            })
            .collect();
        let called: Vec<&str> = held
            .iter()
            .filter_map(|record| {
                if let EventKind::ToolCall { name, .. } = &record.event.kind {
                    Some(name.as_str())
                } else {
                    None
                }
            })
            .collect();
        if called != [tool] {
            return Err(
                format!("{name}: the call {id} in {session} is {called:?}, want {tool}").into(),
            );
        }
        let mut results = Vec::new();
        for record in &held {
            if let EventKind::ToolResult { exit, .. } = &record.event.kind {
                results.push(match exit {
                    None => None,
                    Some(ExitStatus::Code { code }) => Some(*code),
                    Some(other) => return Err(format!("{name}: {id}: unexpected {other:?}").into()),
                });
            }
        }
        if results != [want] {
            return Err(format!(
                "{name}: exit of the result of {tool} call {id} in {session} is {results:?}, want {want:?}"
            )
            .into());
        }
    }
    Ok(())
}

/// Checks that no result without a span carries an exit, and that the stream has one.
fn check_spanless_results(name: &str, output: &Output) -> Res<()> {
    let mut seen = 0_usize;
    for record in &output.events {
        if let (EventKind::ToolResult { exit, .. }, None) = (&record.event.kind, &record.event.span)
        {
            seen += 1;
            if exit.is_some() {
                return Err(format!("{name}: a result joined to no call has exit {exit:?}").into());
            }
        }
    }
    if seen == 0 {
        return Err(format!("{name}: the stream has no result without a span").into());
    }
    Ok(())
}

/// Lines in which a call id names a `Read` in the root session and a `Bash` in the subagent session `S/A`, so the result in each session gets the exit of its own call: `X` answers the root result first, `Y` the child result first; a `Bash` call whose result is not an error but reports `interrupted`; and a result whose id is no span (it holds white space), which joins no call.
fn exit_join_stream() -> Vec<Vec<u8>> {
    let bash = |id: &str| {
        format!(r#"{{"type":"tool_use","id":"{id}","name":"Bash","input":{{"command":"true"}}}}"#)
    };
    let read =
        |id: &str| format!(r#"{{"type":"tool_use","id":"{id}","name":"Read","input":{{}}}}"#);
    let agent = r#"{"type":"tool_use","id":"A","name":"Agent","input":{}}"#;
    let result = |id: &str, text: &str| {
        format!(r#"{{"type":"tool_result","tool_use_id":"{id}","content":"{text}"}}"#)
    };
    let line = |kind: &str, parent: &str, blocks: &[&str]| {
        format!(
            r#"{{"type":"{kind}","session_id":"S",{parent}"message":{{"role":"{kind}","content":[{}]}}}}"#,
            blocks.join(",")
        )
        .into_bytes()
    };
    let cut = r#"{"type":"tool_use","id":"I","name":"Bash","input":{"command":"sleep 9"}}"#;
    let child = r#""parent_tool_use_id":"A","#;
    let mut lines = stream_with(None);
    lines.push(line("assistant", "", &[agent, &read("X"), &read("Y")]));
    lines.push(line("assistant", child, &[&bash("X"), &bash("Y")]));
    lines.push(line("user", "", &[&result("X", "file text")]));
    lines.push(line("user", child, &[&result("X", "")]));
    lines.push(line("user", child, &[&result("Y", "")]));
    lines.push(line("user", "", &[&result("Y", "file text")]));
    lines.push(line("assistant", "", &[cut]));
    lines.push(line(
        "user",
        r#""tool_use_result":{"interrupted":true},"#,
        &[&result("I", "partial")],
    ));
    lines.push(line("user", "", &[&result("no span", "")]));
    lines
}

/// Guards: Reader correctness — sessions, subagent ids, denials by the harness, MCP origins, unreadable lines and `ended_by` of each stream, and `ended_by` for each result shape, are what the raw lines say. Oracle: the hand-written `expected.toml` of each stream, written from `jq` over its lines, and literal result lines with the ends R01 §Ends gives them.
#[test]
fn expected_facts() -> Res<()> {
    for stream in load_streams()? {
        let name = &stream.name;
        assert_eq!(
            u64::try_from(stream.lines.len())?,
            u64::try_from(
                field(&stream.expected, "frames")?
                    .as_integer()
                    .ok_or("frames")?
            )?,
            "{name}: frame count"
        );
        let output = run(&stream.lines, false)?;
        let sessions: Vec<(String, Option<String>, String)> = output
            .sessions
            .iter()
            .map(|session| {
                (
                    session.id().to_string(), // BORROW: .to_string() gives owned text
                    session.parent_session().map(ToString::to_string),
                    label(session.ended_by()),
                )
            })
            .collect();
        assert_eq!(sessions, expected_sessions(&stream)?, "{name}: sessions");
        let subagents: Vec<String> = output
            .sessions
            .iter()
            .filter_map(|session| {
                let parent = session.parent_session()?;
                let rest = session.id().as_str().strip_prefix(parent.as_str())?; // BORROW: .as_str() borrows the text
                rest.strip_prefix('/').map(str::to_owned) // BORROW: owned call id
            })
            .collect();
        assert_eq!(
            subagents,
            texts(&stream.expected, "subagent_ids")?,
            "{name}: subagents"
        );
        assert_eq!(
            denied(&output)?,
            expected_denials(&stream)?,
            "{name}: denials (call, reason, by, session)"
        );
        check_exits(name, &output, &expected_exits(&stream)?)?;
        for row in expected_frame_sessions(&stream)? {
            let (frame, session) = row;
            let holders: BTreeSet<String> = output
                .events
                .iter()
                .filter(|record| record.event.raw_ref.frame == frame)
                .map(|record| record.session.to_string()) // BORROW: .to_string() gives owned text
                .collect();
            assert_eq!(
                holders,
                BTreeSet::from([session]),
                "{name}: the session that holds frame {frame}"
            );
        }
        let mut mcp: Vec<(String, String)> = Vec::new();
        for record in &output.events {
            if let (
                EventKind::ToolCall {
                    origin: ToolOrigin::Mcp { server, .. },
                    ..
                },
                Some(span),
            ) = (&record.event.kind, &record.event.span)
            {
                mcp.push((span.to_string(), server.clone())); // BORROW: owned ids
            }
        }
        let mut want_mcp = Vec::new();
        for call in list(&stream.expected, "mcp_calls")? {
            let call = table(call, "an mcp call")?;
            want_mcp.push((
                text(call, "tool_use_id")?.to_owned(), // BORROW: .to_owned() gives owned copy of the text
                text(call, "server")?.to_owned(), // BORROW: .to_owned() gives owned copy of the text
            )); // BORROW: owned copies
        }
        assert_eq!(mcp, want_mcp, "{name}: MCP calls");
        let cited: BTreeSet<u64> = output
            .events
            .iter()
            .map(|record| record.event.raw_ref.frame)
            .collect();
        for frame in frame_numbers(&stream.expected, "blank_frames")? {
            assert!(
                !cited.contains(&frame),
                "{name}: blank frame {frame} holds no event"
            );
        }
        for frame in frame_numbers(&stream.expected, "non_json_frames")? {
            let kept = output.events.iter().any(|record| {
                record.event.raw_ref.frame == frame
                    && matches!(record.event.kind, EventKind::Notification { .. })
                    && record.event.ext.contains_key("text")
            });
            assert!(
                kept,
                "{name}: frame {frame} is kept as a notification with its text"
            );
        }
    }
    let rows: [(Option<&str>, EndedBy); 8] = [
        (
            Some(r#"{"type":"result","subtype":"success","is_error":false,"session_id":"S"}"#),
            EndedBy::Completed,
        ),
        (
            Some(
                r#"{"type":"result","subtype":"success","is_error":true,"api_error_status":529,"session_id":"S"}"#,
            ),
            EndedBy::GatewayError,
        ),
        (
            Some(
                r#"{"type":"result","subtype":"success","is_error":true,"api_error_status":null,"terminal_reason":"api_error","session_id":"S"}"#,
            ),
            EndedBy::agent_error(Some("api_error".parse()?)),
        ),
        (
            Some(
                r#"{"type":"result","subtype":"error_during_execution","is_error":true,"session_id":"S"}"#,
            ),
            EndedBy::agent_error(Some("error_during_execution".parse()?)),
        ),
        (
            Some(
                r#"{"type":"result","subtype":"error_max_turns","is_error":true,"session_id":"S"}"#,
            ),
            EndedBy::cap(Cap::Turns),
        ),
        (
            Some(
                r#"{"type":"result","subtype":"error_max_budget_usd","is_error":true,"session_id":"S"}"#,
            ),
            EndedBy::cap(Cap::Cost),
        ),
        (
            Some(
                r#"{"type":"result","subtype":"success","is_error":true,"api_error_status":null,"session_id":"S"}"#,
            ),
            EndedBy::agent_error(None),
        ),
        (None, EndedBy::Unknown),
    ];
    for (result, want) in rows {
        let output = run(&stream_with(result), false)?;
        let got = output
            .sessions
            .first()
            .ok_or("no session")?
            .ended_by()
            .clone(); // BORROW: owned end to compare
        assert_eq!(got, want, "result line {result:?}");
    }
    let shared = run(&exit_join_stream(), false)?;
    let rows: [ExitRow<'_>; 5] = [
        ("S", "X", "Read", None),
        ("S/A", "X", "Bash", Some(0)),
        ("S", "Y", "Read", None),
        ("S/A", "Y", "Bash", Some(0)),
        ("S", "I", "Bash", None),
    ];
    check_exits("shared call id", &shared, &rows)?;
    check_spanless_results("shared call id", &shared)?;
    Ok(())
}

/// Guards: I3, no silent loss — every frame that is not blank is cited by an event's `raw_ref`, and every citation names a frame of the stream. Oracle: the frame count and the blank test of the stream's own lines, counted by splitting the raw bytes at newlines.
#[test]
fn no_silent_loss() -> Res<()> {
    for stream in load_streams()? {
        let output = run(&stream.lines, false)?;
        let cited: BTreeSet<u64> = output
            .events
            .iter()
            .map(|record| record.event.raw_ref.frame)
            .collect();
        for (index, line) in stream.lines.iter().enumerate() {
            let blank = line.iter().all(u8::is_ascii_whitespace);
            let frame = u64::try_from(index)?;
            assert_eq!(
                cited.contains(&frame),
                !blank,
                "{}: frame {frame} (blank: {blank})",
                stream.name
            );
        }
        let last = u64::try_from(stream.lines.len())?;
        assert!(
            cited.iter().all(|frame| *frame < last),
            "{}: a citation past the end",
            stream.name
        );
    }
    Ok(())
}

type Counts = [u64; 4];

fn counts(table: &toml::Table) -> Res<Counts> {
    let count = |key: &str| -> Res<u64> {
        Ok(u64::try_from(
            field(table, key)?.as_integer().ok_or("not an integer")?,
        )?)
    };
    Ok([
        count("input_tokens")?,
        count("output_tokens")?,
        count("cache_read_tokens")?,
        count("cache_creation_tokens")?,
    ])
}

/// Guards: Cond 2 — each session's message usage is counted once per `message.id`, and a session's total is the sum of its turns with the last running cost. Oracle: `usage_totals.toml`, written by the committed `usage_totals.jq` over the first line of each `message.id`.
#[test]
fn usage_equals_the_committed_totals() -> Res<()> {
    for stream in load_streams()? {
        let name = &stream.name;
        let output = run(&stream.lines, false)?;
        let mut messages: BTreeMap<String, Counts> = output
            .sessions
            .iter()
            .map(|session| (session.id().to_string(), [0; 4])) // BORROW: .to_string() gives owned text
            .collect();
        let mut totals: BTreeMap<String, (Counts, Option<u64>)> = BTreeMap::new();
        for record in &output.events {
            if let EventKind::Usage {
                scope,
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_creation_tokens,
                cost_usd,
                ..
            } = &record.event.kind
            {
                let now = [
                    *input_tokens,
                    *output_tokens,
                    *cache_read_tokens,
                    *cache_creation_tokens,
                ];
                let session = record.session.to_string(); // BORROW: .to_string() gives owned text
                match scope {
                    UsageScope::Message => {
                        let sum = messages.entry(session).or_default();
                        for (slot, add) in sum.iter_mut().zip(now) {
                            *slot += add;
                        }
                    }
                    UsageScope::SessionTotal => {
                        totals.insert(session, (now, cost_usd.map(|cost| cost.get().to_bits())));
                    }
                    _ => {} // EXPLICIT: a scope this test does not know adds to neither table
                }
            }
        }
        let mut want_messages = BTreeMap::new();
        for (session, row) in table(field(&stream.totals, "message")?, "message")? {
            want_messages.insert(session.clone(), counts(table(row, "a message row")?)?); // BORROW: owned id
        }
        assert_eq!(messages, want_messages, "{name}: message usage per session");
        let mut want_totals = BTreeMap::new();
        for (session, row) in table(field(&stream.totals, "session_total")?, "session_total")? {
            let row = table(row, "a total row")?;
            let cost = row
                .get("cost_usd")
                .and_then(toml::Value::as_float)
                .map(f64::to_bits);
            want_totals.insert(session.clone(), (counts(row)?, cost)); // BORROW: owned id
        }
        assert_eq!(totals, want_totals, "{name}: session totals");
    }
    Ok(())
}

/// Guards: Cond 4 — only an init line of Claude Code `>=2.1.220, <2.2.0` is recognised, with certainty. Oracle: eight literal first lines, two of them pre-releases, and the range the architecture states.
#[test]
fn version_table() {
    let init = |version: &str| {
        format!(
            r#"{{"type":"system","subtype":"init","session_id":"S","claude_code_version":"{version}"}}"#
        )
    };
    let rows: [(String, Option<&str>); 8] = [
        (init("2.1.219"), None),
        (init("2.1.220"), Some("2.1.220")),
        (
            format!("{}\n{{\"type\":\"result\"}}\n", init("2.1.304")),
            Some("2.1.304"),
        ),
        (init("2.2.0"), None),
        (init("2.2.0-rc.1"), None),
        (init("2.1.221-rc.1"), None),
        (
            r#"{"type":"assistant","session_id":"S","message":{"content":[]}}"#.to_owned(), // BORROW: .to_owned() gives owned copy of the text
            None,
        ),
        ("this is not json".to_owned(), None), // BORROW: .to_owned() gives owned copy of the text
    ];
    let reader = ClaudeCodeReader::new();
    assert_eq!(
        reader.id().to_string(),
        READER,
        "the identity names the shipped reader"
    ); // BORROW: .to_string() gives owned text
    for (sample, want) in rows {
        let got = reader
            .detect(sample.as_bytes())
            .map(|(version, confidence)| (version.to_string(), confidence)); // BORROW: .to_string() gives owned text
        assert_eq!(
            got,
            want.map(|version| (version.to_owned(), Confidence::Certain)), // BORROW: .to_owned() gives owned copy of the text
            "sample {sample}"
        );
    }
}

/// Guards: I3 — a reader that is asked what it has observed after every frame writes the same trace file as one that is not. Oracle: byte equality of the two trace files, and the root ids that `observed()` lists against the sessions without a parent.
#[test]
fn replay_is_deterministic() -> Res<()> {
    for stream in load_streams()? {
        let plain = trace_bytes(&stream, run(&stream.lines, false)?)?;
        let watched = trace_bytes(&stream, run(&stream.lines, true)?)?;
        assert!(plain == watched, "{}: the trace files differ", stream.name);
        let mut reader = ClaudeCodeReader::new();
        let name: StreamName = STREAM.parse()?;
        for (index, line) in stream.lines.iter().enumerate() {
            let frame = RawFrame::new(None, FrameData::from_bytes(line));
            let raw_ref = RawRef::new(name.clone(), u64::try_from(index)?); // BORROW: one name per frame
            reader.feed(FrameIn::new(raw_ref, None, &frame))?;
        }
        let roots: Vec<String> = reader
            .observed()
            .session_ids
            .iter()
            .map(ToString::to_string)
            .collect();
        let want: Vec<String> = Box::new(reader)
            .finish()?
            .iter()
            .filter(|session| session.parent_session().is_none())
            .map(|session| session.id().to_string()) // BORROW: .to_string() gives owned text
            .collect();
        assert_eq!(roots, want, "{}: observed roots", stream.name);
    }
    Ok(())
}
