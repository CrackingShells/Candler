#![cfg(test)]
//! Guards of the gateway census (amendment A1): the lines the core takes from the gateway's log,
//! the census a run records from them, and what a verdict makes of their source.
//!
//! The log is the real gateway's: `fixtures/gateway/census_log.txt` was written by the gateway
//! image on requests carrying tool descriptions and schemas, message text and a system prompt
//! that held canary words, and none of them is in it.

mod support;
#[path = "support/wiring.rs"]
mod wiring;

use candler_core::isolation::gateway::census_of;
use candler_core::manifest::compare::{ArmView, Verdict, stratum_verdict};
use candler_schemas::execution::RunManifest;
use candler_schemas::primitives::{FieldPath, RunKey};
use serde_json::{Value, json};
use support::run_plan::{Facts, Run, manifest};
use wiring::{DENIED_RUN, LOG_RUN, LOG_TAG, Scenario, census_log, census_log_deny_shell, run};

/// The run of the recorded log that carried no tools at all.
const NO_TOOLS_RUN: &str = "2222222222222222222222222222222222222222222222222222222222222222-r1-a1";

/// The twelve tool names of an `OpenCode` request, in the order its request lists them.
const TOOLS: [&str; 12] = [
    "edit",
    "glob",
    "grep",
    "question",
    "read",
    "shell",
    "skill",
    "subagent",
    "webfetch",
    "websearch",
    "write",
    "execute",
];

fn key(text: &str) -> RunKey {
    text.parse().expect("a run key")
}

/// The census lines of `log` for `run` under `tag`, as text.
fn lines(log: &str, run: &str, tag: &str) -> String {
    String::from_utf8(census_of(log, &key(run), tag)).expect("text")
}

/// Guards: amendment A1 — the core takes from the gateway's log, for the run and the tag it is
/// asked about and no other, one line of the sequence number and the tool names of each model
/// request, in order, with the tool-less requests kept; whatever else a line carries, a line that
/// is not exactly the gateway's shape, another run's line and a line with another tag are not
/// taken.
/// Oracle: the recorded gateway log, read by eye: run 1 sent a request with no tools, then two
/// with the twelve tools, and run 2 two with none; the hostile lines are written here.
/// Bite: the line matched on the run alone (the tag ignored), the array parsed as anything but
/// a list of strings, the sequence number dropped, or the access lines read as census lines.
#[test]
fn census_gateway_lines_are_the_runs_names_in_order() {
    let tools = json!(TOOLS).to_string();
    let log = census_log();
    let forged_tag = format!(
        "census run={LOG_RUN} tag={} seq=9 tools=[\"forged\"]\n",
        "c".repeat(32)
    );
    let other_run = format!("census run={NO_TOOLS_RUN} tag={LOG_TAG} seq=9 tools=[\"other\"]\n");
    let rows: Vec<(&str, String, &str, String)> = vec![
        (
            "a run of three requests",
            log.clone(), // BORROW: the row owns the log
            LOG_RUN,
            format!(
                "{{\"seq\":1,\"tools\":[]}}\n{{\"seq\":2,\"tools\":{tools}}}\n{{\"seq\":3,\"tools\":{tools}}}\n"
            ),
        ),
        (
            "a run whose requests carry no tools",
            log.clone(), // BORROW: the row owns the log
            NO_TOOLS_RUN,
            String::from("{\"seq\":1,\"tools\":[]}\n{\"seq\":2,\"tools\":[]}\n"),
        ),
        (
            "a line with another tag, and another run's line",
            format!("{log}{forged_tag}{other_run}"),
            LOG_RUN,
            format!(
                "{{\"seq\":1,\"tools\":[]}}\n{{\"seq\":2,\"tools\":{tools}}}\n{{\"seq\":3,\"tools\":{tools}}}\n"
            ),
        ),
        (
            "lines that are not the gateway's shape",
            [
                format!("census run={LOG_RUN} tag={LOG_TAG} seq=1 tools=[{{\"name\":\"x\"}}]"),
                format!("census run={LOG_RUN} tag={LOG_TAG} seq=one tools=[\"x\"]"),
                format!("census run={LOG_RUN} tag={LOG_TAG} seq=1 tools=[\"x\"] extra=\"CANARY\""),
                format!("census run={LOG_RUN} tag={LOG_TAG} tools=[\"x\"]"),
                format!("ts=now run={LOG_RUN} tag={LOG_TAG} census tools=[\"x\"]"),
            ]
            .join("\n"),
            LOG_RUN,
            String::new(),
        ),
    ];
    for (case, text, run_key, want) in rows {
        assert_eq!(lines(&text, run_key, LOG_TAG), want, "{case}");
    }
}

/// Guards: amendment A1, names only — the stream the core stores holds the sequence number and
/// the names, whatever the log carried: a canary in a tool's description, a schema, a message or
/// a credential never reaches it, and a name that tries to close its string is stored as one name.
/// Oracle: the canary words written into the hostile log here (the real gateway's own log holds
/// none, checked by eye with `grep CANARY`).
/// Bite: the stored line built from the log's whole line instead of the parsed names.
#[test]
fn census_gateway_names_only_stored_stream_carries_no_other_text() {
    let hostile = format!(
        "census run={LOG_RUN} tag={LOG_TAG} seq=1 tools=[\"a\",\"b\\\"; DESCRIPTION-CANARY\"]\n\
         ts=now run={LOG_RUN} tag={LOG_TAG} path=\"/MESSAGE-CANARY\" token=TOKEN-CANARY\n"
    );
    let stored = lines(&hostile, LOG_RUN, LOG_TAG);
    assert_eq!(
        stored, "{\"seq\":1,\"tools\":[\"a\",\"b\\\"; DESCRIPTION-CANARY\"]}\n",
        "the second name is one name"
    );
    for canary in ["MESSAGE", "TOKEN"] {
        assert!(!stored.contains(canary), "{canary} reached the stream");
    }
    let real = lines(&census_log(), LOG_RUN, LOG_TAG);
    for canary in ["DESCRIPTION", "SCHEMA", "MESSAGE", "SYSTEM"] {
        assert!(!real.contains(canary), "{canary} reached the stream");
    }
}

/// Guards: amendment A1 — a run records the census the gateway saw: `census.tools` as the first
/// request with tools listed them, `census.init_tools` sorted, and `census.source` `gateway`,
/// with the stream stored raw, tool-less requests included; a run none of whose requests carry
/// tools records no census value at all, never an empty set, and keeps its stream.
/// Oracle: the twelve names of `opencode_facts` (census section), sorted by hand; the recorded
/// log's two runs.
/// Bite: the census taken from the title request (the first line) or recorded empty for a run
/// with no tools, `census.source` left out, or the stream not stored; the denied row's log
/// replaced by `census_log.txt` (a request lists `shell`).
#[test]
fn census_gateway_run_records_the_first_request_with_tools_or_nothing() {
    let sorted = [
        "edit",
        "execute",
        "glob",
        "grep",
        "question",
        "read",
        "shell",
        "skill",
        "subagent",
        "webfetch",
        "websearch",
        "write",
    ];
    let with_tools = run(&Scenario::exited("completed_o1"), |_, _| {});
    assert_eq!(with_tools.measured("census.tools"), Some(json!(TOOLS)));
    assert_eq!(
        with_tools.measured("census.init_tools"),
        Some(json!(sorted))
    );
    assert_eq!(with_tools.measured("census.source"), Some(json!("gateway")));
    let stored = with_tools.raw("gateway_census").expect("the census stream");
    assert_eq!(
        stored.lines().count(),
        4,
        "a header and the three requests, the title request included: {stored}"
    );

    let mut bare = Scenario::exited("completed_o1");
    bare.log = census_log()
        .replace(LOG_RUN, "swap")
        .replace(NO_TOOLS_RUN, LOG_RUN)
        .replace("swap", NO_TOOLS_RUN);
    let none = run(&bare, |_, _| {});
    for path in [
        "census.tools",
        "census.init_tools",
        "census.source",
        "census.mcp_servers",
    ] {
        assert_eq!(none.measured(path), None, "{path} of a run with no tools");
    }
    assert!(none.raw("gateway_census").is_some(), "the stream is kept");
}

/// A row of the verdict guard: its name, two arms' census values and the verdict between them.
type Row<'a> = (
    &'a str,
    &'a [(&'a str, Value)],
    &'a [(&'a str, Value)],
    Verdict,
);

/// Guards: amendment A1 and `opencode_settings` — a run of `bypassPermissions` with
/// `tools.disallowed: ["Bash"]` offers the shell tool in no request: `census.tools` is the twelve
/// names less `shell`, and no stored line names `shell` or `bash`.
/// Oracle: a real run through the gateway, recorded against the fake model (`fixtures/gateway/
/// census_log_deny_shell.txt`, PROVENANCE of `candler-harness-opencode`), and the v2.2.7 source
/// that decides it: `packages/core/src/tool.ts:260` skips a tool for which `whollyDisabled` holds
/// (`tool.ts:386-389`: the last matching row is a deny on `*`).
/// Bite: the log replaced by `census_log.txt`, a run that lists `shell` in a request.
#[test]
fn census_gateway_denied_shell_is_offered_in_no_request() {
    // A run of `bypassPermissions` with `tools.disallowed: ["Bash"]`: OpenCode leaves a tool whose
    // last permission row is a deny on `*` out of the tools it sends (`packages/core/src/tool.ts:260`,
    // `whollyDisabled`, v2.2.7), so no request lists the shell tool; the log is a real run's
    // through the gateway (`fixtures/gateway/census_log_deny_shell.txt`, PROVENANCE).
    let mut denied = Scenario::exited("completed_o1");
    denied.log = census_log_deny_shell().replace(DENIED_RUN, LOG_RUN);
    let ran = run(&denied, |_, _| {});
    let kept: Vec<Value> = TOOLS
        .iter()
        .filter(|name| **name != "shell")
        .map(|name| json!(name))
        .collect();
    assert_eq!(ran.measured("census.tools"), Some(json!(kept)));
    let stream = ran.raw("gateway_census").expect("the census stream");
    assert_eq!(
        stream.lines().count(),
        3,
        "a header and the two requests: {stream}"
    );
    for tool in ["shell", "bash", "Bash"] {
        assert!(
            !stream.contains(&format!("\"{tool}\"")),
            "{tool} was offered to the model in a denied run: {stream}"
        );
    }
}

/// The manifest of an arm: a clean provenance block, and the census values `census` lists.
fn arm(census: &[(&str, Value)]) -> Result<ArmView, candler_core::manifest::compare::VerdictError> {
    let facts = Facts {
        secrets: Vec::new(),
        ..Facts::base()
    };
    let base = manifest(&Run {
        model: "m1",
        replicate: 1,
        attempt: 1,
        facts: &facts,
        integrity: json!({"kind": "valid"}),
        sealed: true,
        job: None,
    });
    let mut doc = serde_json::to_value(&base).expect("manifest JSON");
    let source = json!({"kind": "experiment_file"});
    let mut measured = vec![
        json!({"path": "provenance.eval_infra.commit", "value": "abc1234", "source": source}),
        json!({"path": "provenance.eval_infra.dirty", "value": false, "source": source}),
        json!({"path": "provenance.sandbox_contract", "value": "candler-container@1", "source": source}),
    ];
    for (path, value) in census {
        measured.push(json!({"path": path, "value": value, "source": source}));
    }
    doc["measured"] = Value::Array(measured); // INDEX: a JSON object member
    let manifest: RunManifest = serde_json::from_value(doc).expect("a manifest");
    ArmView::from_manifest(&manifest)
}

/// Guards: amendment A1 — a tool list the gateway saw never equals a harness's own report, a
/// collected command's, or none, however alike the names; two gateway lists of the same names
/// are comparable, and of other names are not.
/// Oracle: the A1 decision's condition 3, written as one row per pair of arms.
/// Bite: `census.source` ignored by the verdict (the first three rows pass as comparable), or
/// compared in place of the names (the control row refuses).
#[test]
fn census_gateway_verdict_never_takes_a_gateway_list_for_another_source() {
    let tools = json!(["Read", "Write"]);
    let gateway = [
        ("census.init_tools", tools.clone()),
        ("census.source", json!("gateway")),
    ]; // BORROW: the row owns its list
    let harness = [("census.init_tools", tools.clone())]; // BORROW: the row owns its list
    let collected = [
        ("census.init_tools", tools.clone()),
        ("census.source", json!("collected")),
    ]; // BORROW: the row owns its list
    let other = [
        ("census.init_tools", json!(["Read"])),
        ("census.source", json!("gateway")),
    ];
    let refused = Verdict::Refused(vec![
        "census.init_tools".parse::<FieldPath>().expect("a path"),
    ]);
    let rows: [Row<'_>; 5] = [
        (
            "against the harness's own",
            &gateway,
            &harness,
            refused.clone(),
        ), // BORROW: the row owns its verdict
        (
            "against a collected one",
            &gateway,
            &collected,
            refused.clone(),
        ), // BORROW: the row owns its verdict
        ("against none", &gateway, &[], refused.clone()), // BORROW: the row owns its verdict
        ("against other names", &gateway, &other, refused),
        (
            "against the same names",
            &gateway,
            &gateway,
            Verdict::Comparable,
        ),
    ];
    for (case, one, two, want) in rows {
        let verdict = stratum_verdict(&[arm(one), arm(two)], &[]);
        assert_eq!(verdict, want, "{case}");
    }
}
