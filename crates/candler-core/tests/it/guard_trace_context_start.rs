#![cfg(test)]
//! Guards of the trace's starting context: the first event of a run's trace is the first model
//! request's system text and first message as the gateway saw them, on Claude Code and on
//! `OpenCode`, and a trace stored without the event still reads.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use candler_core::frames::FrameReader;
use candler_core::isolation::gateway::{census_context, census_of};
use candler_core::jobs::orchestrate::controls::{ControlRunner as _, Scripted};
use candler_harness::driver::HarnessDriver;
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_harness_opencode::driver::OpenCodeDriver;
use candler_schemas::primitives::RunKey;
use candler_schemas::raw::FrameData;
use candler_schemas::trace::{EventKind, TraceFile};
use serde_json::{Value, json};

use crate::guard_container_e2e::controls_e2e_runs::ControlWorld;
use crate::support::wiring::{Scenario, run};

/// The files named `name` below `dir`.
fn files_below(dir: &Path, name: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir)
        .expect("a readable directory")
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_below(&path, name));
        } else if path.file_name().is_some_and(|found| found == name) {
            found.push(path);
        }
    }
    found
}

/// A stored trace and the lines of the gateway stream its first event points into.
struct Stored {
    trace: TraceFile,
    lines: Vec<Value>,
}

/// The trace the production run loop stores for the fake model's script `c2` through `driver`, run
/// behind the real gateway against the scripted fake model, with the gateway's stored lines.
async fn stored_trace(
    driver: Arc<dyn HarnessDriver>, // TRAIT_OBJECT: the driver is chosen at run time
    base_dir: &str,
    settings: Vec<(&'static str, Value)>,
) -> Stored {
    let world = ControlWorld::new(driver, base_dir).await;
    let identity = world
        .runner
        .identify(&world.rt)
        .await
        .expect("the base image is identified");
    let work = world.rig.dir.path().join("trace-work");
    let (_cancel, watch) = tokio::sync::watch::channel(None);
    let ran = world
        .runner
        .run_in(
            &world.rt,
            &identity,
            &world.harness(),
            &Scripted {
                script: "c2",
                settings,
            },
            watch,
            work.clone(),
        )
        .await;
    let traces = files_below(&work, "trace.jsonl");
    let bytes = traces
        .first()
        .map(|path| std::fs::read(path).expect("the trace"));
    let trace = bytes
        .as_deref()
        .map(|bytes| TraceFile::read(bytes).expect("the stored trace reads"));
    let lines = trace
        .as_ref()
        .and_then(|trace| trace.events().first())
        .map(|first| {
            let stream = format!("{}.jsonl.gz", first.event.raw_ref.stream);
            let file = files_below(&work, &stream);
            FrameReader::open(file.first().expect("the gateway stream is stored"))
                .expect("the stream opens")
                .map(|frame| {
                    let FrameData::Text { text } = frame.expect("a frame").data else {
                        panic!("a frame that is not text");
                    };
                    serde_json::from_str(&text).expect("a JSON line")
                })
                .collect::<Vec<Value>>()
        })
        .unwrap_or_default();
    world.finish();
    ran.expect("the control ran");
    Stored {
        trace: trace.expect("the run stored a trace"),
        lines,
    }
}

/// The problems with the stored trace's first event: it is not `context_start`, not seq 0, not the
/// gateway's line of the lowest sequence number among the requests with tools, different from that
/// line, or empty or not ending in the prompt.
fn check(harness: &str, stored: &Stored) -> Vec<String> {
    let Some(first) = stored.trace.events().first() else {
        return vec![format!("{harness}: the trace has no events")];
    };
    let EventKind::ContextStart {
        system,
        first_message,
        ..
    } = &first.event.kind
    else {
        return vec![format!(
            "{harness}: the first event is not context_start: {:?}",
            first.event.kind
        )];
    };
    let mut problems = Vec::new();
    let frame = usize::try_from(first.event.raw_ref.frame).expect("a small frame index");
    let line = stored.lines.get(frame);
    let tooled = stored
        .lines
        .iter()
        .filter(|line| {
            line["tools"]
                .as_array()
                .is_some_and(|tools| !tools.is_empty())
        })
        .filter_map(|line| line["seq"].as_u64())
        .min();
    if first.event.seq != 0 {
        problems.push(format!(
            "{harness}: the first event has seq {}",
            first.event.seq
        ));
    }
    if line.and_then(|line| line["seq"].as_u64()) != tooled || tooled.is_none() {
        problems.push(format!(
            "{harness}: the event is the gateway's line {line:?}, not the first request with tools ({tooled:?})"
        ));
    }
    let context = line.map(|line| &line["context"]);
    if context.and_then(|context| context["system"].as_str()) != Some(system.as_str())
        || context.and_then(|context| context["first_message"].as_str())
            != Some(first_message.as_str())
    {
        problems.push(format!(
            "{harness}: the event differs from the gateway's line"
        ));
    }
    if system.is_empty() || !first_message.ends_with("go") {
        problems.push(format!(
            "{harness}: system {} chars, first message {first_message:.100}",
            system.len()
        ));
    }
    problems
}

/// Guards: the live trace proof — a run of the real Claude Code and a run of the real `OpenCode`,
/// each behind the real gateway against the scripted fake model, store a trace whose first event is
/// `context_start`, equal to the line the gateway recorded for the run's first request that
/// carries tools (on `OpenCode` that is not its title request) and ending in the prompt. The two
/// harnesses run one after the other, because both use the one fake model image.
/// Oracle: the gateway's stored census lines of the same run, read from the run's raw file, and the
/// prompt `go` the cell sent.
/// Bite: record the first request instead of the first request with tools (`OpenCode`'s title
/// request has no tools), or leave the event out of the trace.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
async fn trace_context_first_event_is_the_first_request_with_tools_on_both_harnesses() {
    let claude = stored_trace(
        Arc::new(ClaudeCodeDriver::new()),
        "base",
        vec![
            ("runner.idle_s", json!(2)),
            ("runner.mode", json!("session")),
        ],
    )
    .await;
    let opencode = stored_trace(Arc::new(OpenCodeDriver::new()), "base-opencode", Vec::new()).await;
    let mut problems = check("claude-code", &claude);
    problems.extend(check("opencode", &opencode));
    assert!(problems.is_empty(), "{problems:#?}");
}

/// Guards: a trace stored without a `context_start` still reads — the run of the wiring (the
/// `OpenCode` driver over the fake docker client and the recorded gateway log, which holds no
/// context line) makes its trace through the production run loop, the trace file reads as it is
/// stored, holds no `context_start`, and counts its events from 0 without a gap.
/// Oracle: the recorded log, which has census lines only.
/// Bite: make `context_start` a kind a reader must find, or place the event with no log line.
#[test]
fn trace_context_a_trace_stored_without_the_event_still_reads() {
    let ran = run(&Scenario::exited("completed_o1"), |_, _| {});
    let bytes = std::fs::read(ran.dir.join("trace.jsonl")).expect("the stored trace");
    let trace = TraceFile::read(&bytes).expect("a trace without the event reads");
    assert!(!trace.events().is_empty(), "the run has events");
    assert!(
        trace
            .events()
            .iter()
            .all(|entry| !matches!(entry.event.kind, EventKind::ContextStart { .. })),
        "no event is context_start"
    );
    assert_eq!(
        trace.write(),
        bytes,
        "reading and writing gives the file back"
    );
}

/// Guards: the reader of the gateway log takes the context line of the run and tag it is asked
/// about, the lowest sequence number of them, with the line's index in the stream; it drops a line
/// of another run or tag, a body with a member beside the two, a body that is not JSON and a
/// context line with no sequence number, and leaves the census lines as they were.
/// Oracle: the log written here, by the shape the gateway's script writes.
/// Bite: take the last context line instead of the lowest, or ignore the run.
#[test]
fn trace_context_reader_takes_the_lowest_line_of_the_run() {
    let key = |text: &str| -> RunKey { text.parse().expect("a run key") };
    let cell = "1".repeat(64);
    let (mine, theirs) = (format!("{cell}-r1-a1"), format!("{cell}-r2-a1"));
    let tag = "b".repeat(32);
    let body = |system: &str| format!("{{\"system\":\"{system}\",\"first_message\":\"m\"}}");
    let log = [
        format!("census run={mine} tag={tag} seq=1 tools=[]"),
        format!("context run={mine} tag={tag} seq=4 body={}", body("late")),
        format!("context run={theirs} tag={tag} seq=1 body={}", body("other run")),
        format!("context run={mine} tag={} seq=1 body={}", "c".repeat(32), body("other tag")),
        format!("context run={mine} tag={tag} seq=0 body={{\"system\":\"x\",\"first_message\":\"m\",\"tools\":[]}}"),
        format!("context run={mine} tag={tag} seq=2 body=not json"),
        format!("context run={mine} tag={tag} seq=x body={}", body("no seq")),
        format!("census run={mine} tag={tag} seq=2 tools=[\"read\"]"),
        format!("context run={mine} tag={tag} seq=2 body={}", body("first")),
    ]
    .join("\n");
    let stream = census_of(&log, &key(&mine), &tag);
    let text = String::from_utf8(stream.clone()).expect("text");
    assert_eq!(text.lines().count(), 4, "{text}");
    let (frame, kind) = census_context(&stream).expect("a context");
    assert_eq!(frame, 3, "the line of seq 2 is the fourth of the stream");
    assert_eq!(
        kind,
        EventKind::context_start(String::from("first"), String::from("m"))
    );
}
