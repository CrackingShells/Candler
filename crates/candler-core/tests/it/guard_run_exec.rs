#![cfg(test)]
//! Guards for executing a run: the table of how a run ends, and the order in which a run is
//! sealed. The expected values are literals or recorded files; the tests import neither the
//! store's layout nor the experiment paths.

use crate::support;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use candler_core::isolation::model::{ImageId, Inspect, InspectState, Labels};
use candler_core::jobs::model::JobCause;
use candler_core::runner::exec::collect::{RetryDecision, retry_decision};
use candler_core::runner::exec::ends::{EndInput, Retry, TraceFacts, classify_end};
use candler_core::runner::exec::seal::{RawStreamFile, SealProgress, SealStep, seal_run_observed};
use candler_core::store::Store;
use candler_core::store::run::{Existing, RunWriter};
use candler_schemas::execution::{RunManifest, RunTree};
use candler_schemas::primitives::{ContainerId, Digest, Timestamp};
use candler_schemas::raw::StreamName;
use candler_schemas::trace::{Cap, EndedBy, TraceFile};
use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::{Value, json};
use sha2::Digest as _;
use tempfile::TempDir;

/// The directory of this crate.
fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The recorded fixtures of the claude-code reader, by relative path from this crate.
fn streams() -> PathBuf {
    here().join("../candler-harness-claude-code/tests/fixtures/streams")
}

/// Reads a TOML file.
fn toml_of(path: &Path) -> toml::Table {
    let text = fs::read_to_string(path).expect("read a TOML file");
    text.parse::<toml::Table>().expect("parse a TOML file")
}

/// A text key of a table.
fn text<'a>(table: &'a toml::Table, key: &str) -> Option<&'a str> {
    table.get(key).and_then(toml::Value::as_str) // BORROW: the text of a table value
}

/// The recorded `docker inspect` named `name`, as an inspect. `exit_code` and `oom` replace the
/// recorded state when given, for a row that has no recording of its own.
fn inspect_of(name: &str, replace: Option<(i64, bool)>) -> Inspect {
    let path = here().join(format!("tests/fixtures/docker/inspect/{name}.json"));
    let value: Value = serde_json::from_slice(&fs::read(path).expect("read the recording"))
        .expect("parse the recording");
    let first = value.get(0).expect("one container");
    let state = first.get("State").expect("a state");
    let word = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).expect(key).to_owned(); // BORROW: owned text of a recorded field
    let (exit_code, oom) = replace.unwrap_or((
        state.get("ExitCode").and_then(Value::as_i64).expect("exit"),
        state
            .get("OOMKilled")
            .and_then(Value::as_bool)
            .expect("oom"),
    ));
    Inspect::new(
        ContainerId::from_str(&word(first, "Id")).expect("an id"),
        ImageId(word(first, "Image")),
        Timestamp::from_str(&word(first, "Created")).expect("a time"),
        Labels::new(),
        InspectState::new(word(state, "Status"), false, exit_code, oom, None, None),
    )
}

/// How the table's `stream` tag reads as what a reader made of it.
fn trace_of(tag: &str, tokens_zero: bool) -> TraceFacts {
    let read = |ended_by: EndedBy, tokens: u64| TraceFacts::Read {
        ended_by,
        tokens: if tokens_zero { 0 } else { tokens },
    };
    match tag {
        // The recorded mini-corpus stream: its hand-written facts say completed, and its
        // usage totals the output tokens.
        "result_ok" => recorded("mini_clean"),
        // The recorded `api_error` stream: agent_error with reason api_error, status null.
        "result_error_no_status" => recorded("api_error"),
        // A `result` line with `api_error_status: 529` reads as a gateway error
        // (candler-harness-claude-code guard_reader, the second row of `rows`).
        "result_error_provider_status" | "provider_error_during_teardown" => {
            read(EndedBy::GatewayError, 1)
        }
        // `subtype: error_max_turns` reads as the turn cap (the same table, the fifth row).
        "result_max_turns" => read(EndedBy::cap(Cap::Turns), 1),
        "zero_tokens" => read(EndedBy::Completed, 0),
        "none" => TraceFacts::Silent,
        "unparsable" => TraceFacts::ReaderError,
        "unknown_format_version" => TraceFacts::FormatUnknown,
        other => panic!("a stream tag the table has not defined: {other}"),
    }
}

/// What the recorded fixture `name` says: its root session's end and its output tokens.
fn recorded(name: &str) -> TraceFacts {
    let dir = streams().join(name);
    let expected = toml_of(&dir.join("expected.toml"));
    let totals = toml_of(&dir.join("usage_totals.toml"));
    let session = expected
        .get("sessions")
        .and_then(toml::Value::as_array)
        .and_then(|list| list.first())
        .and_then(toml::Value::as_table)
        .expect("one session");
    let ended_by = match text(session, "ended_by").expect("an end") {
        "completed" => EndedBy::Completed,
        "agent_error" => EndedBy::agent_error(Some("api_error".parse().expect("a reason"))),
        other => panic!("an end the table does not use: {other}"),
    };
    let tokens = totals
        .get("session_total")
        .and_then(toml::Value::as_table)
        .and_then(|totals| totals.values().next())
        .and_then(|total| total.get("output_tokens"))
        .and_then(toml::Value::as_integer)
        .expect("output tokens");
    TraceFacts::Read {
        ended_by,
        tokens: u64::try_from(tokens).expect("a count"),
    }
}

/// The text the table uses for an end.
fn ended_text(ended_by: &EndedBy) -> String {
    let value = serde_json::to_value(ended_by).expect("serialise an end");
    let kind = value.get("kind").and_then(Value::as_str).expect("a kind");
    match value.get("cap").and_then(Value::as_str) {
        Some(cap) => format!("{kind}:{cap}"),
        None => kind.to_owned(), // BORROW: owned kind text
    }
}

/// The row `row` with the fields of `variant` laid over it.
fn merged(row: &toml::Table, variant: Option<&toml::Table>) -> toml::Table {
    let mut all = row.clone(); // BORROW: the variant is laid over a copy of the row
    if let Some(variant) = variant {
        for (key, value) in variant {
            all.insert(key.clone(), value.clone()); // BORROW: owned field of the variant
        }
    }
    all
}

/// The input a row describes. A field the row omits takes a neutral value.
fn input_of(row: &toml::Table, collect_ok: bool) -> EndInput {
    let int = |key: &str| row.get(key).and_then(toml::Value::as_integer);
    let flag = |key: &str, otherwise: bool| {
        row.get(key)
            .and_then(toml::Value::as_bool)
            .unwrap_or(otherwise)
    };
    let inspect = match text(row, "inspect") {
        Some("missing") => None,
        Some("unrecorded") => {
            let exit = int("inspect_exit").expect("an exit");
            Some(inspect_of("clean", Some((exit, flag("oom_killed", false)))))
        }
        Some(name) => {
            let recorded = inspect_of(name, None);
            if let Some(exit) = int("inspect_exit") {
                assert_eq!(recorded.state.exit_code, exit, "{name}: the recorded exit");
            }
            assert_eq!(
                recorded.state.oom_killed,
                flag("oom_killed", false),
                "{name}: the recorded OOMKilled"
            );
            Some(recorded)
        }
        None => Some(inspect_of("clean", None)),
    };
    let stop = match text(row, "our_stop") {
        Some("budget_stop") => Some(JobCause::BudgetStop),
        Some("killed") => Some(JobCause::Killed),
        Some("none") | None => None,
        Some(other) => panic!("a stop the table has not defined: {other}"),
    };
    let tokens_zero = text(row, "tokens") == Some("zero");
    let trace = text(row, "stream").map_or_else(
        || trace_of("result_ok", tokens_zero),
        |tag| trace_of(tag, tokens_zero),
    );
    EndInput::new()
        .with_client_exit(Some(
            i32::try_from(int("client_exit").unwrap_or(0)).expect("an exit"),
        ))
        .with_inspect(inspect)
        .with_container_started(flag("container_started", true))
        .with_stop(stop)
        .with_deadline(flag("deadline", false))
        .with_gateway_failed(text(row, "gateway_signal") == Some("connection_refused"))
        .with_trace(trace)
        .with_collect_ok(collect_ok)
        .with_attempt(u32::try_from(int("attempt").unwrap_or(1)).expect("an attempt"))
}

/// Guards: B29, the ends table: every row E01 to E18 that `classify_end` owns gives the status,
/// reason, end and retry of the table, in the R2 order.
/// Oracle: `ends_table.toml`, hand-written
/// from R01 section Ends and 00-architecture section 13, with the recorded `docker inspect` of a
/// clean exit, a self-expiry and an OOM kill. Rows E01 and E02 read recorded claude-code streams
/// (`mini_clean`, `api_error`); every other row reads a hand-built `TraceFacts` with `guard_reader`
/// provenance (`trace_of`). E15 expects the literal `completed`.
/// Bite: swap
/// the `OOMKilled` and `ExitCode == 137` checks, and E10 stops being a never-retried failure.
#[test]
fn b29_ends_table() {
    let table = toml_of(&here().join("tests/oracle/vectors/ends_table.toml"));
    let rows = table
        .get("row")
        .and_then(toml::Value::as_array)
        .expect("rows");
    let mut seen = BTreeSet::new();
    let mut checked = 0_usize;
    for row in rows {
        let row = row.as_table().expect("a row table"); // BORROW: the row as a table
        let id = text(row, "id").expect("an id");
        seen.insert(id.to_owned()); // BORROW: owned id for the set
        if id == "E13" {
            // seal_interrupted after `lost`: never a live run, so not a classify_end row.
            continue;
        }
        let variants: Vec<Option<&toml::Table>> = match row.get("variant") {
            Some(list) => list
                .as_array()
                .expect("variants")
                .iter()
                .map(|v| v.as_table())
                .collect(),
            None => vec![None],
        };
        for variant in variants {
            let all = merged(row, variant);
            let label = format!("{id} {:?}", variant.and_then(|v| text(v, "name")));
            let end = classify_end(&input_of(
                &all,
                all.get("collect_ok") != Some(&false.into()),
            ));
            let status = text(&all, "status").expect("a status");
            assert_eq!(
                end.invalid.is_none(),
                status == "valid",
                "{label}: status {status}"
            );
            let reason = text(&all, "reason").unwrap_or("");
            let got = end.invalid.map_or(String::new(), |reason| {
                serde_json::to_value(reason)
                    .expect("serialise a reason")
                    .as_str() // BORROW: the reason as text
                    .expect("text")
                    .to_owned() // BORROW: owned reason text
            });
            assert_eq!(got, reason, "{label}: reason");
            let want = text(&all, "ended_by").expect("an end");
            assert_eq!(ended_text(&end.ended_by), want, "{label}: ended_by");
            let retry = text(&all, "retry").expect("a retry");
            let (want_retry, want_decision) = match retry {
                "once" => (Retry::Once, RetryDecision::Retry),
                "never" => (Retry::Never, RetryDecision::Final),
                "none" => (Retry::None, RetryDecision::Final),
                other => panic!("a retry the table has not defined: {other}"),
            };
            assert_eq!(end.retry, want_retry, "{label}: retry");
            assert_eq!(retry_decision(&end), want_decision, "{label}: decision");
            checked += 1;
        }
    }
    let ids: Vec<String> = (1..=18).map(|n| format!("E{n:02}")).collect();
    assert_eq!(seen, ids.into_iter().collect::<BTreeSet<_>>(), "E01 to E18");
    // 17 rows, E09 with 3 variants, E16 with 2 and E18 with 2: 17 - 3 + 3 + 2 + 2.
    assert_eq!(checked, 21, "the rows and variants that were checked");
}

/// The text of a digest, as a literal.
fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

/// A manifest of attempt 1 of a cell; the run key carries the cell's real digest.
fn manifest() -> RunManifest {
    let body = |cell: &str| {
        json!({
            "schema": "run-manifest/1",
            "run": {"cell": cell, "replicate": 1, "attempt": 1},
            "experiment": digest_text('0'),
            "factors": [],
            "comparable": {
                "values": [{"path": "plan.a", "value": 1}],
                "harness": {"name": "h", "version": "1.0.0"},
                "treatment": {"name": "t"},
                "snapshots": [],
                "inputs": [],
                "environments": []
            },
            "sources": [
                {"path": "plan.a", "source": {"kind": "experiment_file"}},
                {"path": "harness", "source": {"kind": "experiment_file"}},
                {"path": "treatment", "source": {"kind": "experiment_file"}}
            ],
            "secrets": [],
            "measured": [],
            "deviations": [],
            "integrity": {"kind": "valid"}
        })
    };
    let probe: RunManifest = serde_json::from_value(body(&digest_text('f'))).expect("a manifest");
    let cell = probe.cell_digest().to_string();
    serde_json::from_value(body(&cell)).expect("a manifest")
}

/// A run tree with one file.
fn tree() -> RunTree {
    serde_json::from_value(json!({
        "schema": "run-tree/1",
        "entries": [{"kind": "file", "path": "a.txt", "sha256": digest_text('a'), "executable": false}]
    }))
    .expect("a run tree")
}

/// A trace file of one session and one event.
fn trace() -> TraceFile {
    let lines = [
        r#"{"kind":"header","schema":"normalised-trace/1","reader":"claude-code-reader@0.1.0","raw":[{"stream":"stdout","digest":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}]}"#,
        r#"{"kind":"session","id":"root-1","source":{"harness":"claude-code","harness_version":"2.1.0","reader":"claude-code-reader","reader_version":"0.1.0"},"model":"claude-opus-4","started_at":"2026-11-05T10:00:00Z","ended_by":{"kind":"cap","cap":"turns"},"raw_ref":{"stream":"stdout","frame":0}}"#,
        r#"{"kind":"event","session":"root-1","event":{"seq":0,"kind":{"kind":"user_message","text":"hi"},"raw_ref":{"stream":"stdout","frame":0},"ext":{}}}"#,
    ];
    let mut text = lines.join("\n");
    text.push('\n');
    TraceFile::read(text.as_bytes()).expect("a trace file")
}

/// A finished `.gz` in `dir`, with the digest of its bytes taken by `sha2`.
fn gz_file(dir: &Path) -> (PathBuf, Digest) {
    let path = dir.join("stdout.gz");
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(b"{\"kind\":\"header\"}\n{\"kind\":\"frame\"}\n")
        .expect("gzip");
    let bytes = encoder.finish().expect("gzip");
    fs::write(&path, &bytes).expect("write the gz");
    let mut hex = String::new();
    for byte in sha2::Sha256::digest(&bytes) {
        write!(hex, "{byte:02x}").expect("write to a string");
    }
    (path, Digest::from_hex(&hex).expect("a digest"))
}

/// The regular files under `root`, by `/`-separated path relative to the run directory, which is
/// the one that holds `manifest.json`.
fn run_files(root: &Path) -> BTreeSet<String> {
    let mut all = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).expect("list a directory") {
            let entry = entry.expect("a directory entry");
            let kind = entry.file_type().expect("a file type");
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                let rel = entry
                    .path()
                    .strip_prefix(root)
                    .expect("below the root")
                    .to_string_lossy()
                    .into_owned(); // BORROW: owned relative path
                all.push(rel);
            }
        }
    }
    let Some(manifest) = all.iter().find(|name| name.ends_with("/manifest.json")) else {
        return BTreeSet::new();
    };
    let prefix = manifest.trim_end_matches("manifest.json").to_owned(); // BORROW: owned run directory prefix
    all.iter()
        .filter_map(|name| name.strip_prefix(prefix.as_str()).map(str::to_owned)) // BORROW: owned names below the run directory
        .collect()
}

/// Guards: B30, the sealed layout and `run.done` last: a seal stopped after any step leaves exactly
/// the files of the steps that ran and no `run.done`; a full seal writes the manifest, the tree,
/// the raw stream and the trace before `run.done`, and the run directory holds only names of the
/// layout.
/// Oracle: the step order and the layout, hand-written from R01 section Run store and
/// 00-architecture section 7 (manifest.json, run.done, raw/, trace.jsonl, tree.json, workspace/,
/// home/); a run that holds a canary is the positive control of the scan, and the sealed run is scanned
/// for the temporary root, the needle the leaf names.
/// Bite: write `run.done`
/// before `tree.json`.
#[test]
fn b30_sealed_layout_and_done_last() {
    // Files present once each step has run, in order (the layout names, relative to the run).
    let after = [
        (SealStep::Manifest, vec!["manifest.json"]),
        (SealStep::Tree, vec!["manifest.json", "tree.json"]),
        (
            SealStep::Raw,
            vec!["manifest.json", "raw/stdout.jsonl.gz", "tree.json"],
        ),
        (
            SealStep::Trace,
            vec![
                "manifest.json",
                "raw/stdout.jsonl.gz",
                "trace.jsonl",
                "tree.json",
            ],
        ),
    ];
    let stdout: StreamName = "stdout".parse().expect("a stream name");
    for (halt, files) in &after {
        let root = TempDir::new().expect("a temporary root");
        let scratch = TempDir::new().expect("a scratch directory");
        let store = Store::init(root.path()).expect("init");
        let manifest = manifest();
        let writer = RunWriter::create(&store, &manifest, Existing::Refuse).expect("create");
        let (gz, digest) = gz_file(scratch.path());
        let raw = [RawStreamFile::new(stdout.clone(), gz, digest)]; // BORROW: the file owns its stream name
        let progress = seal_run_observed(writer, &manifest, &tree(), &raw, &trace(), |step| {
            if step == *halt {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .expect("a seal halted by the observer");
        assert_eq!(progress, SealProgress::Halted(*halt));
        let want: BTreeSet<String> = files.iter().map(|name| (*name).to_owned()).collect(); // BORROW: owned names
        assert_eq!(run_files(root.path()), want, "files after {halt:?}");
    }
    let root = TempDir::new().expect("a temporary root");
    let scratch = TempDir::new().expect("a scratch directory");
    let store = Store::init(root.path()).expect("init");
    let manifest = manifest();
    let writer = RunWriter::create(&store, &manifest, Existing::Refuse).expect("create");
    let (gz, digest) = gz_file(scratch.path());
    let raw = [RawStreamFile::new(stdout, gz, digest)];
    let mut steps = Vec::new();
    let mut done_early = false;
    let progress = seal_run_observed(writer, &manifest, &tree(), &raw, &trace(), |step| {
        if step != SealStep::Done && run_files(root.path()).contains("run.done") {
            done_early = true;
        }
        steps.push(step);
        ControlFlow::Continue(())
    })
    .expect("a full seal");
    assert_eq!(progress, SealProgress::Sealed);
    assert_eq!(
        steps,
        [
            SealStep::Manifest,
            SealStep::Tree,
            SealStep::Raw,
            SealStep::Trace,
            SealStep::Done
        ]
    );
    assert!(!done_early, "run.done was written before the last step");
    let sealed = run_files(root.path());
    // R01's literal list: a full seal holds exactly these files (workspace and home hold none here).
    let literal: BTreeSet<String> = [
        "manifest.json",
        "raw/stdout.jsonl.gz",
        "run.done",
        "trace.jsonl",
        "tree.json",
    ]
    .iter()
    .map(|name| (*name).to_owned()) // BORROW: owned names
    .collect();
    assert_eq!(sealed, literal, "the full seal equals R01's list");
    // The scan: a canary written into a scratch directory is found (positive control), and the
    // sealed run holds none.
    let needle = "CANDLER-B30-CANARY";
    let control = TempDir::new().expect("a control directory");
    fs::write(control.path().join("probe.txt"), needle).expect("write the control");
    let found = support::scan(control.path(), needle).expect("scan the control");
    assert_eq!(found.hits.len(), 1, "the positive control is found");
    let root_text = root.path().to_str().expect("a UTF-8 temporary root");
    support::assert_clean(root.path(), root_text);
}
