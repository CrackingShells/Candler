#![cfg(test)]
//! Guards of the `OpenCode` wiring in the run loop: the collect command runs before the container
//! goes and its export is the trace, the pre-stop runs inside the agent's container with the
//! observed session id, and the run keeps what the driver resolved from its cell.
//!
//! The agent is a fake that prints a recorded live stream, the docker client is the PATH-fake
//! that logs every call and answers a collect with a recorded export; no model, no docker.

use crate::support::wiring;

use std::fs;

use serde_json::{Value, json};
use wiring::{
    CONFIG_PATH, CONFIG_TEXT, LITERAL_CANARY, Scenario, agent_cid, docker_calls, planned_key,
    recorded, run, session_of, wait_for,
};

/// The index of the first logged call that `wanted` accepts.
fn first_call(calls: &[Vec<String>], wanted: impl Fn(&[String]) -> bool) -> Option<usize> {
    calls.iter().position(|call| wanted(call))
}

/// The one frame of a stored document stream, as text.
fn document_frame(raw: &str) -> String {
    let records: Vec<Value> = raw
        .lines()
        .map(|line| serde_json::from_str(line).expect("a record"))
        .collect();
    assert_eq!(records.len(), 2, "a header and one frame: {raw}");
    let frame = records.last().expect("a frame");
    let data = &frame["data"];
    data["text"]
        .as_str()
        .unwrap_or_else(|| panic!("a text frame: {frame}"))
        .to_owned()
}

/// `stdout` with one more event after it that names another session, as a child's: the root is
/// the session the stream named first.
fn with_child(stdout: &str, root: &str) -> String {
    let last = stdout.lines().last().expect("a line");
    format!(
        "{stdout}{}\n",
        last.replace(root, "ses_childchildchildchild")
    )
}

/// Guards: R01 §Harness drivers (the export goes to `offline_reader`), I3 — a run that exits has
/// its collect command run in a container over the run's mounts *before* the agent's container
/// is destroyed; the command carries the session id the live stream named, the run's state
/// variables and neither the gateway's address nor its token; its output is framed as one
/// `Document`, stored as the raw stream `export`, and read by the offline reader into the trace,
/// whose header names that reader; the end is the export's.
/// Oracle: the recorded `completed_o1`, `interrupt_api` and `sigterm_mid_tool` runs of
/// `opencode_facts` (stdout, export, and the session id and outcome of each hand-written
/// `expected.toml`); the PATH-fake docker client's own log of its calls, in order; the end of each
/// from R01 §Ends by hand: succeeded is completed, interrupted, by the API or by
/// a signal, is unknown.
/// Bite: `collect_commands` called after `finish_attempt` (the destroy goes first), the export
/// output discarded, or the export's sessions and reader dropped for the live reader's.
#[test]
fn wiring_collect_exports_before_the_destroy_and_the_export_is_the_trace() {
    let rows = [
        ("completed_o1", "completed"),
        ("interrupt_api", "unknown"),
        ("sigterm_mid_tool", "unknown"),
    ];
    for (case, ended) in rows {
        let ran = run(&Scenario::exited(case), |_, _| {});
        let id = session_of(case);

        let export = first_call(&ran.docker, |call| {
            call.first().is_some_and(|verb| verb == "run") && call.contains(&String::from("export"))
        })
        .unwrap_or_else(|| panic!("{case}: no export call in {:?}", ran.docker));
        let destroy = first_call(&ran.docker, |call| {
            call.starts_with(&[String::from("container"), String::from("rm")])
        })
        .unwrap_or_else(|| panic!("{case}: no destroy in {:?}", ran.docker));
        assert!(export < destroy, "{case}: the destroy came first");

        let call = &ran.docker[export]; // INDEX: found above
        assert_eq!(call.get(1).map(String::as_str), Some("--rm"), "{case}");
        assert!(
            call.windows(2).any(|pair| pair == ["--network", "none"]),
            "{case}: {call:?}"
        );
        assert!(
            call.ends_with(&[
                String::from("opencode"),
                String::from("session"),
                String::from("export"),
                String::from("--standalone"),
                id.clone(), // BORROW: the row compares with the id
            ]),
            "{case}: {call:?}"
        );
        let text = call.join(" ");
        for kept in ["XDG_DATA_HOME=", "OPENCODE_DB="] {
            assert!(text.contains(kept), "{case}: {kept} missing: {text}");
        }
        for gone in [
            "CANDLER_GATEWAY_URL",
            "CANDLER_GATEWAY_TOKEN",
            "dummy-token",
        ] {
            assert!(
                !text.contains(gone),
                "{case}: {gone} reached the collect: {text}"
            );
        }

        let raw = ran
            .raw("export")
            .unwrap_or_else(|| panic!("{case}: no export stream"));
        assert_eq!(
            document_frame(&raw),
            recorded(case, "export.json"),
            "{case}"
        );

        let trace = fs::read_to_string(ran.dir.join("trace.jsonl")).expect("a trace");
        let first: Value =
            serde_json::from_str(trace.lines().next().expect("a header")).expect("header JSON");
        assert_eq!(
            first["reader"], "opencode-export@0.1.0",
            "{case}: the trace names the export reader: {first}"
        );
        assert!(
            trace.contains(&id),
            "{case}: the trace holds the session {id}"
        );
        assert_eq!(
            ran.measured("run.ended_by"),
            Some(json!({"kind": ended})),
            "{case}"
        );
    }
    // A stream that names a child session after the root: the collect command gets the root's id.
    let root = session_of("completed_o1");
    let mut scenario = Scenario::exited("completed_o1");
    scenario.stdout = with_child(&scenario.stdout, &root);
    let ran = run(&scenario, |_, _| {});
    let export = first_call(&ran.docker, |call| {
        call.first().is_some_and(|verb| verb == "run") && call.contains(&String::from("export"))
    })
    .expect("an export call");
    assert_eq!(
        ran.docker[export].last(), // INDEX: found above
        Some(&root),
        "the collect command carries the root session, not the last one seen"
    );
}

/// Guards: R01 §Process supervision (stop escalation, containers) — a stopped run's pre-stop is
/// a `docker exec` in the agent's container, which the guard re-verified, carrying the session id
/// the live reader observed; it is never an HTTP call or a command on the host; a pre-stop with no
/// id observed yet is skipped and a failed one is reported, and the escalation goes on to the
/// signal in each case.
/// Oracle: the PATH-fake docker client's log of calls and the ordered outcomes of the stop steps
/// the run's events carry; the session id from the hand-written `expected.toml` of
/// `completed_o1`; the expected outcome text of each row written here.
/// Bite: a pre-stop with no observed id run anyway (the skipped row runs an `exec`), or an `exec`
/// that fails reported as done.
#[test]
fn wiring_stop_execs_in_the_container_with_the_observed_id() {
    let id = session_of("completed_o1");
    // (case, what the agent prints, the fake client's exit for `exec`, the pre-stop outcome, exec)
    let rows: [(&str, String, &str, &str, bool); 4] = [
        (
            "an id observed",
            recorded("completed_o1", "stdout.jsonl"),
            "0",
            "exec done",
            true,
        ),
        (
            "no id yet",
            String::new(),
            "0",
            "skipped: no session id observed yet",
            false,
        ),
        (
            "the exec fails",
            recorded("completed_o1", "stdout.jsonl"),
            "1",
            "exec failed",
            true,
        ),
        (
            "a child session seen after the root",
            with_child(&recorded("completed_o1", "stdout.jsonl"), &id),
            "0",
            "exec done",
            true,
        ),
    ];
    for (case, stdout, exec_code, outcome, execs) in rows {
        let mut scenario = Scenario::exited("completed_o1");
        scenario.stdout = stdout.clone(); // BORROW: the scenario owns the text
        scenario.sleep_s = 30;
        let observed = !stdout.is_empty();
        let ran = run(&scenario, |rig, ctl| {
            fs::write(rig.canned().join("exec.code"), exec_code).expect("an exit code");
            let key = planned_key();
            let record = rig
                .store
                .layout()
                .job_dir(&rig.job)
                .join("children")
                .join(format!("container-{}.json", &agent_cid().as_str()[..12])); // INDEX: the id is 64 hex digits
            wait_for(&record);
            if observed {
                // The live frames are flushed: the stream file holds the id before the stop.
                let frames = rig
                    .store
                    .layout()
                    .job_dir(&rig.job)
                    .join("scratch")
                    .join(key.to_string())
                    .join("frames")
                    .join("stdout.jsonl.tmp");
                let began = std::time::Instant::now();
                while !fs::read_to_string(&frames).is_ok_and(|text| text.contains("ses_")) {
                    assert!(began.elapsed().as_secs() < 10, "no frame with the id");
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
            let _ = ctl
                .cancel
                .send(Some(candler_core::jobs::model::JobCause::Killed));
        });
        let calls = docker_calls(&ran.rig);
        let exec = first_call(&calls, |call| {
            call.first().is_some_and(|verb| verb == "exec")
        });
        assert_eq!(
            exec.is_some(),
            execs,
            "{case}: {:?} {calls:?}",
            ran.pre_stops()
        );
        if let Some(at) = exec {
            let call = &calls[at]; // INDEX: found above
            assert!(
                call.contains(&agent_cid().as_str().to_owned()),
                "{case}: {call:?}"
            ); // BORROW: .to_owned() makes the owned id
            assert_eq!(
                call.last(),
                Some(&id),
                "{case}: the observed id is the last argument"
            );
            assert!(
                !call.iter().any(|arg| arg.contains("session_id")),
                "{case}: the placeholder was not filled: {call:?}"
            );
        }
        let stops = ran.pre_stops();
        assert!(
            stops.first().is_some_and(|text| text.starts_with(outcome)),
            "{case}: {stops:?}"
        );
        assert!(
            ran.steps().iter().any(|step| step.contains("Signal")),
            "{case}: the escalation went on: {:?}",
            ran.steps()
        );
    }
}

/// Guards: amendment A2 — a run keeps what the driver resolved from its cell: the harness argv,
/// the names of the variables its overlay sets (a literal's value never), and each prepared file
/// with its contents, whose secrets are `{env:VAR}` names; a file rooted at the agent's home is
/// written there, not in the workspace.
/// Oracle: the literals the fake driver prepares, written in `support/wiring.rs`, and a canary
/// as the overlay's literal and the gateway's dummy token, neither of which may appear.
/// Bite: the record keeping the overlay's values, the prepared file's text dropped, or a home
/// file written into the workspace.
#[test]
fn wiring_resolved_the_run_keeps_its_argv_and_prepared_files_without_secrets() {
    let ran = run(&Scenario::exited("completed_o1"), |_, _| {});
    let raw = ran.raw("resolved").expect("the resolved stream");
    let record: Value = serde_json::from_str(&document_frame(&raw)).expect("the record JSON");
    assert_eq!(
        record["argv"],
        json!(["opencode", "run", "--", "the prompt"])
    );
    assert_eq!(
        record["env"],
        json!([{"name": "OC_LITERAL", "kind": "literal"}])
    );
    let file = &record["files"][0]; // INDEX: a JSON member
    assert_eq!(file["path"], CONFIG_PATH);
    assert_eq!(file["root"], "home");
    assert_eq!(file["text"], CONFIG_TEXT);
    assert!(
        file["sha256"]
            .as_str()
            .is_some_and(|text| text.starts_with("sha256:"))
    );
    for gone in [LITERAL_CANARY, "dummy-token"] {
        assert!(!raw.contains(gone), "{gone} reached the record");
    }
    let in_home = ran.dir.join("home").join(CONFIG_PATH);
    assert!(in_home.is_file(), "the home file is written to the home");
    assert!(
        !ran.dir.join("workspace").join(CONFIG_PATH).exists(),
        "and not to the workspace"
    );
}
