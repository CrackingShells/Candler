#![cfg(test)]
//! The wire form of raw frames and of the normalised trace: a hand-authored trace file, its
//! refusals, the session ids it shares with evidence refs, and the JSON lines of frames.

use std::str::FromStr;

use candler_schemas::primitives::EvidenceRef;
use candler_schemas::raw::{
    Clock, ClockAnchor, FrameData, RawFrame, RawHeader, RawRecord, StreamName,
};
use candler_schemas::trace::{EventKind, Session, SessionId, TraceError, TraceFile};

/// The lines of a trace file written by hand from the architecture's §Normalised trace model, §Raw
/// traces and §Wire encodings, in the writer's field order: struct declaration order, keys of open
/// objects sorted. It holds a root session, a child, every event kind, an `ext` and a kind this
/// version does not know.
const TRACE_LINES: [&str; 10] = [
    r#"{"kind":"header","schema":"normalised-trace/1","reader":"claude-code-reader@0.1.0","raw":[{"stream":"stdout","digest":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}]}"#,
    r#"{"kind":"session","id":"root-1","source":{"harness":"claude-code","harness_version":"2.1.0","reader":"claude-code-reader","reader_version":"0.1.0"},"model":"claude-opus-4","started_at":"2026-11-05T10:00:00Z","ended_at":"2026-11-05T19:00:09.5+09:00","ended_by":{"kind":"cap","cap":"turns"},"raw_ref":{"stream":"stdout","frame":0}}"#,
    r#"{"kind":"session","id":"root-1/toolu_01","source":{"harness":"claude-code","harness_version":"2.1.0","reader":"claude-code-reader","reader_version":"0.1.0"},"ended_by":{"kind":"agent_error","reason":"tool_failed"},"parent_session":"root-1","raw_ref":{"stream":"stdout","frame":3}}"#,
    r#"{"kind":"event","session":"root-1","event":{"seq":0,"kind":{"kind":"user_message","text":"hi"},"raw_ref":{"stream":"stdout","frame":0},"ext":{}}}"#,
    r#"{"kind":"event","session":"root-1","event":{"seq":1,"at":"2026-11-05T10:00:01Z","kind":{"kind":"tool_call","name":"Bash","origin":{"kind":"builtin"},"argv":["ls","-l"],"wrapped_calls":[{"name":"search","origin":{"kind":"mcp","server":"docs"}}]},"span":"toolu_01","raw_ref":{"stream":"stdout","frame":1},"ext":{"cwd":"/work","extra":{"a":1,"b":[true,"x"]}}}}"#,
    r#"{"kind":"event","session":"root-1","event":{"seq":2,"kind":{"kind":"hologram","depth":3,"tags":["x","y"]},"raw_ref":{"stream":"stdout","frame":2},"ext":{}}}"#,
    r#"{"kind":"event","session":"root-1/toolu_01","event":{"seq":0,"kind":{"kind":"tool_result","is_error":true,"denied":{"by":"hook","reason":"policy"}},"span":"toolu_01","raw_ref":{"stream":"stdout","frame":3},"ext":{}}}"#,
    r#"{"kind":"event","session":"root-1","event":{"seq":3,"kind":{"kind":"usage","scope":"session_total","input_tokens":120,"output_tokens":45,"cache_read_tokens":0,"cache_creation_tokens":7,"cost_usd":0.0123},"raw_ref":{"stream":"stdout","frame":4},"ext":{}}}"#,
    r#"{"kind":"event","session":"root-1","event":{"seq":4,"kind":{"kind":"notification"},"raw_ref":{"stream":"stdout","frame":5},"ext":{"line":"warming up"}}}"#,
    r#"{"kind":"event","session":"root-1","event":{"seq":5,"kind":{"kind":"assistant_message"},"raw_ref":{"stream":"stdout","frame":6},"ext":{}}}"#,
];

/// Edits line `index` of `lines`, a line of the base trace file.
fn edit_line(lines: &mut [String], index: usize, from: &str, to: &str) {
    // INDEX: every caller names one of the ten lines of the base trace file
    let line = &mut lines[index];
    *line = edit(line, from, to);
}

/// An edit of the lines of the base trace file.
type Edit = fn(&mut Vec<String>);

fn joined(lines: &[String]) -> Vec<u8> {
    let mut text = lines.join("\n");
    text.push('\n');
    text.into_bytes()
}

fn base_lines() -> Vec<String> {
    // BORROW: .to_owned() makes the owned lines the edits change
    TRACE_LINES.iter().map(|line| (*line).to_owned()).collect()
}

/// Guards: I3, I18 — a trace file is reproduced byte for byte by a read and a write, with the
/// `ext` of an event and an event kind this version does not know kept as written. Oracle: the
/// trace file literal, written by hand from the architecture, not produced by the writer.
#[test]
fn a_hand_written_trace_file_survives_a_read_and_a_write_byte_for_byte() {
    let literal = joined(&base_lines());
    let file = TraceFile::read(&literal).expect("the hand-written file is valid");
    assert_eq!(file.write(), literal);

    let unknown = file
        .events()
        .iter()
        // BORROW: explicit .as_str() instead of Deref coercion
        .find(|record| record.event.seq == 2 && record.session.as_str() == "root-1")
        .expect("the event of the unknown kind is read");
    let EventKind::Unknown { kind, body, .. } = &unknown.event.kind else {
        panic!("expected the unknown kind, got {:?}", unknown.event.kind);
    };
    assert_eq!(kind, "hologram");
    assert_eq!(body.len(), 2);
    let with_ext = file
        .events()
        .iter()
        // BORROW: explicit .as_str() instead of Deref coercion
        .find(|record| record.event.seq == 1 && record.session.as_str() == "root-1")
        .expect("the event with an ext is read");
    assert_eq!(with_ext.event.ext.len(), 2);
}

/// Replaces the one occurrence of `from` in `line` by `to`.
fn edit(line: &str, from: &str, to: &str) -> String {
    assert_eq!(
        line.matches(from).count(),
        1,
        "{from:?} occurs once in {line}"
    );
    line.replacen(from, to, 1)
}

/// Guards: Cond 2 — a trace file that breaks one ordering or reference rule is refused, and the
/// refusal names the session or the `seq` at fault. Oracle: the trace file literal with one hand
/// edit each; the unedited file is the positive control.
#[test]
fn a_trace_file_with_one_broken_rule_is_refused_naming_its_session_or_seq() {
    // Positive control: the unedited base reads.
    TraceFile::read(&joined(&base_lines())).expect("the base is valid");

    // Each row: the fault, the edit of the base lines, and what the refusal must name.
    let rows: [(&str, Edit, &[&str]); 7] = [
        ("missing header", |l| drop(l.remove(0)), &["header"]),
        (
            "session after an event",
            |l| {
                let child = l.remove(2);
                l.insert(4, child);
            },
            &["root-1/toolu_01", "after an event"],
        ),
        (
            "child before its parent",
            |l| l.swap(1, 2),
            &["root-1/toolu_01", "before its parent"],
        ),
        (
            "child id is not parent/span",
            |l| {
                edit_line(
                    l,
                    2,
                    r#""id":"root-1/toolu_01""#,
                    r#""id":"other/toolu_01""#,
                );
            },
            &["other/toolu_01"],
        ),
        (
            "event of an undeclared session",
            |l| edit_line(l, 3, r#""session":"root-1""#, r#""session":"ghost""#),
            &["ghost", "seq 0"],
        ),
        (
            "seq gap",
            |l| edit_line(l, 4, r#""seq":1,"#, r#""seq":2,"#),
            &["root-1", "seq 2", "seq 1 is next"],
        ),
        (
            "raw_ref stream absent from the header",
            |l| {
                edit_line(
                    l,
                    5,
                    r#""raw_ref":{"stream":"stdout","frame":2}"#,
                    r#""raw_ref":{"stream":"stderr","frame":2}"#,
                );
            },
            &["root-1", "seq Some(2)", "stderr"],
        ),
    ];
    for (fault, apply, names) in rows {
        let mut lines = base_lines();
        apply(&mut lines);
        let error = TraceFile::read(&joined(&lines))
            .expect_err(fault)
            .to_string();
        for name in names {
            assert!(error.contains(name), "{fault}: {error:?} lacks {name:?}");
        }
    }

    // An empty file has no header either.
    assert_eq!(TraceFile::read(b"").err(), Some(TraceError::MissingHeader));

    // Two sibling faults of the rows above: a second header and a repeated seq.
    let mut second_header = base_lines();
    // INDEX: TRACE_LINES holds ten lines, the first is the header
    // BORROW: .to_owned() makes the owned line the file keeps
    second_header.insert(1, TRACE_LINES[0].to_owned());
    let error = TraceFile::read(&joined(&second_header))
        .expect_err("second header")
        .to_string();
    assert!(
        error.contains("line 2") && error.contains("second header"),
        "{error}"
    );
    let mut repeat = base_lines();
    edit_line(&mut repeat, 4, r#""seq":1,"#, r#""seq":0,"#);
    let error = TraceFile::read(&joined(&repeat))
        .expect_err("seq repeat")
        .to_string();
    assert!(
        error.contains("root-1") && error.contains("seq 0") && error.contains("seq 1 is next"),
        "{error}"
    );
}

/// Guards: D2 — a record is an object and nothing else: a header, an event, a `raw_ref`, a
/// tool origin, an end, a session or its source, or a raw frame written as a list is refused, and so is a `null` for an
/// optional field, each naming the line. Oracle: the hand-written lines, which the valid base
/// passes as the positive control.
#[test]
fn a_record_written_as_a_list_or_with_a_null_optional_is_refused_naming_its_line() {
    TraceFile::read(&joined(&base_lines())).expect("the base is valid");

    let list_header = r#"["header","normalised-trace/1","claude-code-reader@0.1.0",[{"stream":"stdout","digest":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}]]"#;
    let list_event = r#"["event","root-1",{"seq":0,"kind":{"kind":"user_message","text":"hi"},"raw_ref":{"stream":"stdout","frame":0},"ext":{}}]"#;
    // (the fault, the index of the line it is in, the text to replace, its replacement)
    // INDEX: TRACE_LINES holds ten lines, so lines 0 and 3 exist
    let rows: [(&str, usize, &str, &str); 8] = [
        ("header as a list", 0, TRACE_LINES[0], list_header),
        ("event as a list", 3, TRACE_LINES[3], list_event),
        (
            "raw_ref as a list",
            3,
            r#""raw_ref":{"stream":"stdout","frame":0}"#,
            r#""raw_ref":["stdout",0]"#,
        ),
        (
            "tool origin as a list",
            4,
            r#""origin":{"kind":"builtin"}"#,
            r#""origin":["builtin"]"#,
        ),
        (
            "end of a session as a list",
            1,
            r#""ended_by":{"kind":"cap","cap":"turns"}"#,
            r#""ended_by":["cap","turns"]"#,
        ),
        (
            "source of a session as a list",
            1,
            r#""source":{"harness":"claude-code","harness_version":"2.1.0","reader":"claude-code-reader","reader_version":"0.1.0"}"#,
            r#""source":["claude-code","2.1.0","claude-code-reader","0.1.0"]"#,
        ),
        (
            "null time on an event",
            3,
            r#""seq":0,"#,
            r#""seq":0,"at":null,"#,
        ),
        (
            "null model on a session",
            1,
            r#""model":"claude-opus-4","#,
            r#""model":null,"#,
        ),
    ];
    for (fault, index, from, to) in rows {
        let mut lines = base_lines();
        edit_line(&mut lines, index, from, to);
        let error = TraceFile::read(&joined(&lines))
            .expect_err(fault)
            .to_string();
        let named = format!("line {}", index + 1);
        assert!(
            // BORROW: explicit .as_str() instead of Deref coercion
            error.contains(named.as_str()),
            "{fault}: {error:?} lacks {named:?}"
        );
    }

    // A session on its own: the object reads, the same session as a list does not.
    serde_json::from_str::<Session>(TRACE_LINES[1]).expect("the valid session reads");
    assert!(
        serde_json::from_str::<Session>(r#"["root-1"]"#).is_err(),
        "a session written as a list is refused"
    );

    // The raw frame file: the valid lines read, the same records as lists or with a null do not.
    let valid = [
        r#"{"kind":"header","schema":"raw-frames/1","stream":"stdout","clock":{"kind":"unrecorded"}}"#,
        r#"{"kind":"frame","t_mono_ns":100,"data":{"kind":"text","text":"x"}}"#,
    ];
    for line in valid {
        serde_json::from_str::<RawRecord>(line).expect("the valid raw line reads");
    }
    let refused = [
        r#"["header","raw-frames/1","stdout",{"kind":"unrecorded"}]"#,
        r#"["frame",100,{"kind":"text","text":"x"}]"#,
        r#"{"kind":"frame","data":["text","x"]}"#,
        r#"{"kind":"header","schema":"raw-frames/1","stream":"stdout","clock":["unrecorded"]}"#,
        r#"{"kind":"frame","t_mono_ns":null,"data":{"kind":"text","text":"x"}}"#,
    ];
    for line in refused {
        assert!(
            serde_json::from_str::<RawRecord>(line).is_err(),
            "{line} is refused"
        );
    }
}

/// Guards: D2 — a session id is accepted exactly when `event:<id>#0` is an accepted evidence
/// ref, so every session id reads as the session part of an evidence ref. Oracle: the table of
/// hand-written ids, each with the verdict the architecture's §Wire encodings gives (non-empty,
/// no `#`, no Unicode `White_Space` or byte-order mark), checked through both parsers.
#[test]
fn a_session_id_is_accepted_exactly_when_event_evidence_refs_accept_it() {
    let table: [(&str, bool); 20] = [
        ("a", true),
        ("root-1", true),
        ("root-1/toolu_01", true),
        ("a/b/c", true),
        ("héllo", true),
        ("日本", true),
        ("a\u{200B}b", true),
        ("a\u{180E}b", true),
        ("a\u{001C}b", true),
        ("", false),
        ("#", false),
        ("a#b", false),
        ("a b", false),
        ("a\tb", false),
        ("a\nb", false),
        ("a\u{00A0}b", false),
        ("a\u{0085}b", false),
        ("a\u{2003}b", false),
        ("a\u{3000}b", false),
        ("a\u{FEFF}b", false),
    ];
    for (id, accepted) in table {
        let session = SessionId::from_str(id).is_ok();
        let evidence = EvidenceRef::from_str(&format!("event:{id}#0")).is_ok();
        assert_eq!(session, accepted, "SessionId on {id:?}");
        assert_eq!(evidence, accepted, "EvidenceRef on {id:?}");
    }
}

/// Guards: I3 — the frames of a stream carry its bytes: text when they are valid UTF-8, base64
/// when they are not, and concatenating the frames' bytes gives the stream back. Oracle: the
/// frame vectors and the JSON lines written by hand (the base64 of `ff fe 0a` is worked out from
/// the alphabet), not produced by the code.
#[test]
fn frames_of_awkward_streams_have_the_json_lines_the_test_states() {
    // (the frame's bytes, the JSON line a frame at t = 100 ns has)
    let vectors: [(&[u8], &str); 6] = [
        (
            b"line one\n",
            r#"{"kind":"frame","t_mono_ns":100,"data":{"kind":"text","text":"line one\n"}}"#,
        ),
        (
            b"cr\rhere\n",
            r#"{"kind":"frame","t_mono_ns":100,"data":{"kind":"text","text":"cr\rhere\n"}}"#,
        ),
        (
            b"\xff\xfe\n",
            r#"{"kind":"frame","t_mono_ns":100,"data":{"kind":"bytes","base64":"//4K"}}"#,
        ),
        (
            b"\n",
            r#"{"kind":"frame","t_mono_ns":100,"data":{"kind":"text","text":"\n"}}"#,
        ),
        (
            b"crlf\r\n",
            r#"{"kind":"frame","t_mono_ns":100,"data":{"kind":"text","text":"crlf\r\n"}}"#,
        ),
        (
            b"tail",
            r#"{"kind":"frame","t_mono_ns":100,"data":{"kind":"text","text":"tail"}}"#,
        ),
    ];
    let mut stream = Vec::new();
    let mut rebuilt = Vec::new();
    for (bytes, line) in vectors {
        let record = RawRecord::Frame(RawFrame::new(Some(100), FrameData::from_bytes(bytes)));
        assert_eq!(
            serde_json::to_string(&record).expect("a frame writes"),
            line
        );
        let read: RawRecord = serde_json::from_str(line).expect("the stated line reads");
        let RawRecord::Frame(frame) = read else {
            panic!("expected a frame, got {read:?}");
        };
        rebuilt.extend(frame.data.to_bytes());
        stream.extend_from_slice(bytes);
    }
    assert_eq!(rebuilt, stream);
    assert_eq!(stream, b"line one\ncr\rhere\n\xff\xfe\n\ncrlf\r\ntail");

    // The header lines, with and without a clock.
    let anchored = RawRecord::Header(RawHeader::new(
        StreamName::from_str("stdout").expect("a stream name"),
        Clock::monotonic(ClockAnchor::new(
            "2026-11-05T10:00:00Z".parse().expect("a timestamp"),
            123,
        )),
        None,
    ));
    assert_eq!(
        serde_json::to_string(&anchored).expect("a header writes"),
        r#"{"kind":"header","schema":"raw-frames/1","stream":"stdout","clock":{"kind":"monotonic","anchor":{"wall":"2026-11-05T10:00:00Z","mono_ns":123}}}"#
    );
    let legacy = RawRecord::Header(RawHeader::new(
        StreamName::from_str("legacy.log").expect("a stream name"),
        Clock::Unrecorded,
        Some(
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .parse()
                .expect("a digest"),
        ),
    ));
    assert_eq!(
        serde_json::to_string(&legacy).expect("a header writes"),
        r#"{"kind":"header","schema":"raw-frames/1","stream":"legacy.log","clock":{"kind":"unrecorded"},"source":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}"#
    );
}
