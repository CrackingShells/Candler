#![cfg(test)]
//! The wire form of the execution records: objects only, never a list, and the structural rules of
//! each record refuse what R01 forbids.

use candler_schemas::execution::{
    Budget, Comparable, Deviation, DigestRef, Envelope, EnvironmentRef, ExperimentFile,
    ExperimentInput, Factor, HarnessIdentity, HarnessRequest, ImportRecord, ImportSource,
    ImportedRequest, InputAudience, InputEntry, Integrity, JobRecord, Level, PerRunCaps,
    RunManifest, RunTree, SecretPresence, StrayProcess, SupervisorIdentity, TreatmentRef,
    TreatmentSpec, TreeEntry, input_list_digest,
};
use candler_schemas::primitives::{Digest, RelPath};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::str::FromStr;

fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

fn message<T>(result: Result<T, serde_json::Error>) -> String {
    match result {
        Ok(_) => String::from("accepted"),
        Err(error) => error.to_string(),
    }
}

fn file(path: &str, executable: bool) -> Value {
    json!({"kind": "file", "path": path, "sha256": digest_text('a'), "executable": executable})
}

fn dir(path: &str) -> Value {
    json!({"kind": "dir", "path": path})
}

fn tree(entries: &[Value]) -> Value {
    json!({"schema": "run-tree/1", "entries": entries})
}

/// A sorted tree: an empty directory, a nested file and an executable file.
fn base_tree() -> Vec<Value> {
    vec![dir("a-empty"), file("src/main.rs", false), file("z", true)]
}

/// Guards: I17 — a run tree holds run-relative paths in sorted order, a path is listed once, no entry lies below a file or below a directory listed as empty, and the builder and the reader refuse the same. Oracle: R01 §Workspace tree, worked as a table of hand-written rows around the sorted base.
#[test]
fn run_tree_refuses_overlaps_and_paths_that_are_not_relative() {
    assert_eq!(
        message(serde_json::from_value::<RunTree>(tree(&base_tree()))),
        "accepted"
    );

    // Each row: what to insert at which index, and what the reader says.
    let rows: [(&str, usize, Value, &str); 6] = [
        ("repeat", 1, dir("a-empty"), "repeats an entry"),
        (
            "under a file",
            3,
            file("z/inner", false),
            "lies below the file",
        ),
        (
            "below an empty directory",
            1,
            file("a-empty/x", false),
            "listed as empty",
        ),
        ("absolute", 3, file("/etc/passwd", false), "/etc/passwd"),
        ("parent segment", 0, file("../x", false), "../x"),
        ("backslash", 3, dir("z\\y"), "backslash"),
    ];
    for (name, at, entry, needle) in rows {
        let mut entries = base_tree();
        entries.insert(at, entry);
        let text = message(serde_json::from_value::<RunTree>(tree(&entries)));
        assert!(text.contains(needle), "{name}: {text}");
    }

    // The file `a` and `a/c` are not neighbours in sorted order ("a-b" lies between them).
    let far = [file("a", false), file("a-b", false), file("a/c", false)];
    let text = message(serde_json::from_value::<RunTree>(tree(&far)));
    assert!(text.contains("lies below the file"), "{text}");

    let mut unsorted = base_tree();
    unsorted.swap(0, 2);
    let text = message(serde_json::from_value::<RunTree>(tree(&unsorted)));
    assert!(text.contains("not sorted"), "{text}");

    // The builder sorts what it is given and refuses the same overlaps.
    let digest = Digest::from_str(digest_text('a').as_str()).expect("digest"); // BORROW: explicit .as_str()
    let path = |text: &str| RelPath::from_str(text).expect("path");
    let built = RunTree::builder()
        .entry(TreeEntry::file(path("z"), digest.clone(), true)) // BORROW: .clone() reuses the digest
        .entry(TreeEntry::dir(path("a-empty")))
        .entry(TreeEntry::file(path("src/main.rs"), digest.clone(), false)) // BORROW: .clone() reuses the digest
        .build()
        .expect("builds");
    assert_eq!(
        built,
        serde_json::from_value::<RunTree>(tree(&base_tree())).expect("reads")
    );
    assert_eq!(
        serde_json::to_value(&built).expect("writes"),
        tree(&base_tree())
    );

    let under_file = RunTree::builder()
        .entry(TreeEntry::file(path("a"), digest.clone(), false)) // BORROW: .clone() reuses the digest
        .entry(TreeEntry::file(path("a-b"), digest.clone(), false)) // BORROW: .clone() reuses the digest
        .entry(TreeEntry::file(path("a/c"), digest.clone(), false)) // BORROW: .clone() reuses the digest
        .build();
    assert!(under_file.is_err());
    let under_dir = RunTree::builder()
        .entry(TreeEntry::dir(path("d")))
        .entry(TreeEntry::file(path("d/f"), digest.clone(), false)) // BORROW: .clone() reuses the digest
        .build();
    assert!(under_dir.is_err());
    let repeat = RunTree::builder()
        .entry(TreeEntry::dir(path("d")))
        .entry(TreeEntry::dir(path("d")))
        .build();
    assert!(repeat.is_err());
}

/// The rows of the list-form table that failed, so that one run names every row that does.
static FAILURES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Reads `object`, which must be accepted, and `list`, which must be refused; a row that fails is recorded under `name`.
fn object_not_list<T: DeserializeOwned>(name: &str, object: Value, list: Value) {
    let mut failures = FAILURES.lock().expect("failure list");
    if message(serde_json::from_value::<T>(object)) != "accepted" {
        failures.push(format!("{name}: the object form is refused"));
    }
    if message(serde_json::from_value::<T>(list)) == "accepted" {
        failures.push(format!("{name}: the list form is accepted"));
    }
}

const JOB: &str = "018f2b3c-4d5e-7a6b-8c7d-9e0f1a2b3c4d";

fn supervisor() -> Value {
    json!({"pid": 4242, "start": {"kind": "darwin", "sec": 1_700_000_000, "usec": 5}})
}

/// The start of a process as the supervisor records it.
fn start() -> Value {
    json!({"kind": "darwin", "sec": 1_700_000_000, "usec": 5})
}

/// A hand-written cell: one value, the harness and the treatment, and no list entries.
fn comparable() -> Value {
    json!({
        "values": [{"path": "plan.a", "value": 1}],
        "harness": {"name": "h", "version": "1.0.0"},
        "treatment": {"name": "t"},
        "snapshots": [],
        "inputs": [],
        "environments": []
    })
}

/// A hand-written manifest of that cell.
fn manifest() -> Value {
    json!({
        "schema": "run-manifest/1",
        "run": {"cell": digest_text('f'), "replicate": 1, "attempt": 1},
        "experiment": digest_text('0'),
        "factors": [],
        "comparable": comparable(),
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
}

/// An envelope line of `kind` whose payload is `payload`.
fn envelope(kind: &str, payload: &Value) -> Value {
    json!({"v": 1, "seq": 0, "at": "2026-01-02T03:04:05Z", "job_id": JOB,
           "kind": kind, "payload": payload})
}

/// Guards: Wire rule — every struct and tagged enum of the execution records is read from an object only; a list with the right values in field order is refused, and the object form is the positive control. Oracle: the serde derive accepts a list for a struct by default (README §Gotchas wire rules), so each row is a hand-written list.
#[test]
fn execution_records_refuse_the_list_form() {
    object_not_list::<RunTree>(
        "RunTree",
        tree(&base_tree()),
        json!(["run-tree/1", base_tree()]),
    );
    object_not_list::<TreeEntry>(
        "TreeEntry file",
        file("z", true),
        json!(["file", "z", digest_text('a'), true]),
    );
    object_not_list::<TreeEntry>("TreeEntry dir", dir("d"), json!(["dir", "d"]));
    object_not_list::<SupervisorIdentity>(
        "SupervisorIdentity",
        supervisor(),
        json!([4242, start()]),
    );
    object_not_list::<JobRecord>(
        "JobRecord",
        json!({"schema": "job-record/1", "id": JOB, "supervisor": supervisor(),
               "spec_digest": digest_text('b'), "state": "running"}),
        json!([
            "job-record/1",
            JOB,
            supervisor(),
            digest_text('b'),
            "running"
        ]),
    );
    // The Envelope and the payload rows are regression guards: they bite today on serde's own
    // refusal (the flattened event, the map read of the payload), not on `object_only!` or
    // `MapOnly`, so a change to those two does not move them.
    object_not_list::<Envelope>(
        "Envelope",
        envelope("job.started", &json!({})),
        json!([1, 0, "2026-01-02T03:04:05Z", JOB, "job.started", {}]),
    );

    // The experiment file and its parts.
    let caps = json!({"wall_s": 5});
    let budget = json!({"per_run": caps});
    object_not_list::<PerRunCaps>("PerRunCaps", caps.clone(), json!([5])); // BORROW: .clone() reuses the caps
    object_not_list::<Budget>("Budget", budget.clone(), json!([caps])); // BORROW: .clone() reuses the budget
    object_not_list::<HarnessRequest>("HarnessRequest", json!({"name": "h"}), json!(["h"]));
    object_not_list::<TreatmentRef>("TreatmentRef", json!({"name": "t"}), json!(["t"]));
    object_not_list::<TreatmentSpec>(
        "TreatmentSpec",
        json!({"name": "t", "source": "skills"}),
        json!(["t", null, null, "skills"]),
    );
    object_not_list::<ExperimentInput>(
        "ExperimentInput",
        json!({"kind": "reference", "path": "data", "audience": "evaluators"}),
        json!(["reference", "data", null, "evaluators"]),
    );
    object_not_list::<InputEntry>(
        "InputEntry",
        json!({"path": "a.txt", "sha256": digest_text('a')}),
        json!(["a.txt", digest_text('a')]),
    );
    object_not_list::<Factor>(
        "Factor",
        json!({"path": "plan.a", "levels": [1, 2]}),
        json!(["plan.a", [1, 2]]),
    );
    object_not_list::<ExperimentFile>(
        "ExperimentFile",
        json!({"schema": "experiment/1", "replicates": 1, "budget": budget,
               "harness": {"name": "h"}, "treatment": {"name": "t"}}),
        json!([
            "experiment/1",
            {},
            [],
            1,
            budget,
            {"name": "h"},
            {"name": "t"},
            []
        ]),
    );
    // A level is a value or a fill table: a list is one value, never the table's fields in order,
    // and the table is the positive control.
    assert!(matches!(
        serde_json::from_value::<Level>(json!({"model": "m", "skill": {"v": 2}})),
        Ok(Level::Fill(_))
    ));
    assert!(matches!(
        serde_json::from_value::<Level>(json!([["model", "m"]])),
        Ok(Level::Value(_))
    ));
    assert!(serde_json::from_value::<Level>(json!({})).is_err());

    // The cell and the parts of a manifest.
    object_not_list::<Comparable>(
        "Comparable",
        comparable(),
        json!([
            [{"path": "plan.a", "value": 1}],
            {"name": "h", "version": "1.0.0"},
            {"name": "t"},
            [],
            [],
            []
        ]),
    );
    object_not_list::<HarnessIdentity>(
        "HarnessIdentity",
        json!({"name": "h", "version": "1.0.0"}),
        json!(["h", "1.0.0"]),
    );
    object_not_list::<DigestRef>(
        "DigestRef",
        json!({"kind": "catalog", "digest": digest_text('a')}),
        json!(["catalog", digest_text('a')]),
    );
    object_not_list::<EnvironmentRef>(
        "EnvironmentRef",
        json!({"plugin": "alpha-eval@1.0.0", "lock_digest": digest_text('d')}),
        json!(["alpha-eval@1.0.0", digest_text('d')]),
    );
    object_not_list::<SecretPresence>(
        "SecretPresence",
        json!({"name": "API_KEY", "present": true}),
        json!(["API_KEY", true]),
    );
    object_not_list::<Deviation>(
        "Deviation",
        json!({"path": "plan.a", "stored": 1, "observed": 2}),
        json!(["plan.a", 1, 2]),
    );
    object_not_list::<Integrity>(
        "Integrity valid",
        json!({"kind": "valid"}),
        json!(["valid"]),
    );
    object_not_list::<Integrity>(
        "Integrity invalid",
        json!({"kind": "invalid", "reason": "killed"}),
        json!(["invalid", "killed"]),
    );
    let import_source = json!({"path": "run/out.json", "sha256": digest_text('a')});
    let imported = json!({"evaluator": "judge@1.0.0", "request": JOB});
    object_not_list::<ImportSource>(
        "ImportSource",
        import_source.clone(), // BORROW: .clone() reuses the source
        json!(["run/out.json", digest_text('a')]),
    );
    object_not_list::<ImportedRequest>(
        "ImportedRequest",
        imported.clone(), // BORROW: .clone() reuses the request
        json!(["judge@1.0.0", JOB]),
    );
    object_not_list::<ImportRecord>(
        "ImportRecord",
        json!({"format": "campaign-import@v1",
               "base_commit": "0123456789abcdef0123456789abcdef01234567",
               "legacy_integrity": {"valid": true},
               "sources": [import_source], "requests": [imported]}),
        json!([
            "campaign-import@v1",
            "0123456789abcdef0123456789abcdef01234567",
            {"valid": true},
            [import_source],
            [imported]
        ]),
    );
    object_not_list::<RunManifest>(
        "RunManifest",
        manifest(),
        json!([
            "run-manifest/1",
            {"cell": digest_text('f'), "replicate": 1, "attempt": 1},
            digest_text('0'),
            [],
            comparable(),
            manifest()["sources"],
            [],
            [],
            [],
            "2026-01-02T03:04:05Z",
            "2026-01-02T03:04:06Z",
            "host",
            JOB,
            {"kind": "valid"},
            {"format": "campaign-import@v1",
             "base_commit": "0123456789abcdef0123456789abcdef01234567",
             "legacy_integrity": 1, "sources": [], "requests": []}
        ]),
    );

    // The payload of every event kind that has members, and the empty one.
    let child = json!({"schema": "child-record/1", "pid": 4242, "pgid": 4242,
                       "start": start(), "marker": "run-1",
                       "spawned_at": "2026-01-02T03:04:05Z"});
    let outcome: Value =
        serde_json::from_str(include_str!("../fixtures/outcomes/golden/01-pass-A1.json"))
            .expect("fixture is json");
    let stray = json!({"pid": 10, "start": start()});
    object_not_list::<StrayProcess>(
        "StrayProcess",
        stray.clone(), // BORROW: .clone() reuses the entry
        json!([10, start()]),
    );
    let payloads = [
        ("job.started", json!({}), json!([])),
        (
            "job.ended",
            json!({"reason": "completed"}),
            json!(["completed"]),
        ),
        ("run.started", json!({}), json!([])),
        (
            "run.ended",
            json!({"ended_by": {"kind": "completed"}, "exit": {"kind": "code", "code": 0}}),
            json!([{"kind": "completed"}, {"kind": "code", "code": 0}]),
        ),
        (
            "run.failed",
            json!({"reason": "spawn_failed"}),
            json!(["spawn_failed"]),
        ),
        ("run.stray", json!({"processes": [stray]}), json!([[stray]])),
        (
            "evaluation.outcome",
            json!({"record": outcome}),
            json!([outcome]),
        ),
        ("process.spawned", json!({"record": child}), json!([child])),
        (
            "process.stop_step",
            json!({"pid": 10, "start": start(), "step": "sweep",
                   "at": "2026-01-02T03:04:06Z", "outcome": "no sweeper"}),
            json!([10, start(), "sweep", "2026-01-02T03:04:06Z", "no sweeper"]),
        ),
        (
            "process.exited",
            json!({"pid": 10, "start": start(), "status": {"kind": "code", "code": 0},
                   "group_empty": true}),
            json!([10, start(), {"kind": "code", "code": 0}, true]),
        ),
    ];
    // Regression guards, as the Envelope row above: serde refuses a list payload itself.
    for (kind, object, list) in payloads {
        object_not_list::<Envelope>(kind, envelope(kind, &object), envelope(kind, &list));
    }
    let failures = FAILURES.lock().expect("failure list");
    assert!(failures.is_empty(), "rows that failed: {failures:#?}");
}

/// Guards: D3 — the events of a child process and of the strays a run left carry the process id and the start time, the pair a liveness check compares, and a process id of 0 or a missing start is refused. Oracle: architecture D3 (a recorded process is the pair of its id and its start), worked as hand-written payloads around an accepted base.
#[test]
fn process_events_carry_the_pid_and_start_pair() {
    let stop = json!({"pid": 10, "start": start(), "step": "signal",
                      "at": "2026-01-02T03:04:06Z"});
    let exited = json!({"pid": 10, "start": start(), "status": {"kind": "code", "code": 0},
                        "group_empty": true});
    let stray = json!({"processes": [{"pid": 10, "start": start()}]});
    let bases = [
        ("process.stop_step", stop),
        ("process.exited", exited),
        ("run.stray", stray),
    ];
    for (kind, base) in bases {
        let read =
            |payload: Value| message(serde_json::from_value::<Envelope>(envelope(kind, &payload)));
        assert_eq!(read(base.clone()), "accepted", "{kind} base"); // BORROW: .clone() reuses the base

        let mut zero = base.clone(); // BORROW: .clone() edits a copy
        let holder = if kind == "run.stray" {
            zero.pointer_mut("/processes/0").expect("entry")
        } else {
            &mut zero
        };
        // INDEX: the holder is a payload object
        holder["pid"] = json!(0);
        assert!(read(zero).contains("pid"), "{kind} pid 0");

        let mut bare = base.clone(); // BORROW: .clone() edits a copy
        let holder = if kind == "run.stray" {
            bare.pointer_mut("/processes/0").expect("entry")
        } else {
            &mut bare
        };
        holder.as_object_mut().expect("object").remove("start");
        assert!(read(bare).contains("start"), "{kind} without start");
    }
    // The old shape, bare process ids, is refused.
    let old = envelope("run.stray", &json!({"pids": [10, 11]}));
    assert_ne!(message(serde_json::from_value::<Envelope>(old)), "accepted");
}

/// Guards: I18 — an envelope line of every first-set kind reads as that kind, whatever the other keys, and writes back as the same bytes, so the names of the kinds are the names on the wire. Oracle: hand-written lines for the kinds that need no other record; the child record and the outcome record come from hand-written text and from `tests/fixtures/outcomes/golden`.
#[test]
fn every_first_set_event_kind_round_trips_its_wire_form() {
    let head = format!(r#"{{"v":1,"seq":3,"at":"2026-01-02T03:04:05Z","job_id":"{JOB}","#);
    let child = r#"{"schema":"child-record/1","pid":4242,"pgid":4242,"start":{"kind":"darwin","sec":1700000000,"usec":5},"marker":"run-1","spawned_at":"2026-01-02T03:04:05Z"}"#;
    let cases: [(&str, &str); 12] = [
        ("job.started", r#""kind":"job.started","payload":{}"#),
        (
            "job.ended",
            r#""kind":"job.ended","payload":{"reason":"budget_stop"}"#,
        ),
        ("run.started", r#""kind":"run.started","payload":{}"#),
        (
            "run.ended",
            r#""kind":"run.ended","payload":{"ended_by":{"kind":"completed"}}"#,
        ),
        (
            "run.ended",
            r#""kind":"run.ended","payload":{"ended_by":{"kind":"killed"},"exit":{"kind":"signal","signal":9}}"#,
        ),
        (
            "run.failed",
            r#""kind":"run.failed","payload":{"reason":"spawn_failed"}"#,
        ),
        (
            "run.stray",
            r#""kind":"run.stray","payload":{"processes":[{"pid":10,"start":{"kind":"darwin","sec":1700000000,"usec":5}},{"pid":11,"start":{"kind":"darwin","sec":1700000001,"usec":6}}]}"#,
        ),
        (
            "process.stop_step",
            r#""kind":"process.stop_step","payload":{"pid":10,"start":{"kind":"darwin","sec":1700000000,"usec":5},"step":"kill_group","at":"2026-01-02T03:04:06Z"}"#,
        ),
        (
            "process.stop_step",
            r#""kind":"process.stop_step","payload":{"pid":10,"start":{"kind":"darwin","sec":1700000000,"usec":5},"step":"sweep","at":"2026-01-02T03:04:06Z","outcome":"2 killed"}"#,
        ),
        (
            "process.exited",
            r#""kind":"process.exited","payload":{"pid":10,"start":{"kind":"darwin","sec":1700000000,"usec":5},"status":{"kind":"code","code":0},"group_empty":true}"#,
        ),
        ("process.spawned", ""),
        ("evaluation.outcome", ""),
    ];
    let outcome = include_str!("../fixtures/outcomes/golden/01-pass-A1.json");
    for (kind, tail) in cases {
        let tail = match kind {
            "process.spawned" => {
                format!(r#""kind":"process.spawned","payload":{{"record":{child}}}"#)
            }
            "evaluation.outcome" => {
                format!(r#""kind":"evaluation.outcome","payload":{{"record":{outcome}}}"#)
            }
            _ => String::from(tail),
        };
        let line = format!("{head}{tail}}}");
        let envelope: Envelope = serde_json::from_str(line.as_str()).expect(kind); // BORROW: explicit .as_str()
        assert_eq!(envelope.event().kind(), kind);
        let written = serde_json::to_value(&envelope).expect("writes");
        let expected: Value = serde_json::from_str(line.as_str()).expect("literal is json"); // BORROW: explicit .as_str()
        assert_eq!(written, expected, "{kind}");
        if kind != "evaluation.outcome" {
            assert_eq!(
                serde_json::to_string(&envelope).expect("writes"),
                line,
                "{kind}"
            );
        }
    }
}

/// The cases of the input-list vector file: the entries as written, and the digest or the refusal expected.
fn input_list_cases() -> Vec<toml::Table> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/vectors/input_list_digest.toml");
    let text = std::fs::read_to_string(path).expect("input_list_digest.toml is committed");
    // BORROW: .as_str() lends the vector text to the parser
    let doc: toml::Table = toml::from_str(text.as_str()).expect("a TOML vector file");
    doc.get("case")
        .and_then(toml::Value::as_array)
        .expect("cases")
        .iter()
        .map(|case| case.as_table().expect("a case table").clone()) // BORROW: the case outlives the document
        .collect()
}

/// The entries of a case, in the order the vector file writes them.
fn entries_of(case: &toml::Table) -> Vec<InputEntry> {
    case.get("entry")
        .and_then(toml::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .map(|entry| {
                    let text = |key: &str| {
                        entry
                            .get(key)
                            .and_then(toml::Value::as_str)
                            .expect("an entry text")
                    };
                    InputEntry::new(
                        RelPath::from_str(text("path")).expect("a relative path"),
                        Digest::from_str(text("sha256")).expect("a digest"),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Guards: Evaluator protocol, `inputs[] {kind, path, digest}` — the digest of an input is the sha256 of the RFC 8785 form of its list sorted by path, so any language recomputes it from the files alone, whatever order the files were listed in. Oracle: `gen_input_list.py`, a second engine (the Python `rfc8785` package) that runs no Candler code.
#[test]
fn input_list_digest_matches_the_second_engine() {
    let mut checked = 0;
    for case in input_list_cases() {
        let Some(want) = case.get("digest").and_then(toml::Value::as_str) else {
            continue;
        };
        let name = case
            .get("name")
            .and_then(toml::Value::as_str)
            .expect("a name");
        let entries = entries_of(&case);
        let got = input_list_digest(&entries).expect(name);
        assert_eq!(got.as_str(), want, "{name}");
        let mut reversed = entries;
        reversed.reverse();
        assert_eq!(
            input_list_digest(&reversed).expect(name).as_str(),
            want,
            "{name}: the digest follows the listing order"
        );
        checked += 1;
    }
    assert!(checked >= 3, "one file, several files and non-ASCII paths");
}

/// Guards: Evaluator protocol, `inputs[] {kind, path, digest}` — an input with no file and an input that lists one path twice have no digest. Oracle: the `refused` cases of `gen_input_list.py`, an empty list and a repeated path, written by the generator.
#[test]
fn input_list_digest_refuses_empty_and_repeated_lists() {
    let mut refused = 0;
    for case in input_list_cases() {
        if case.get("refused").and_then(toml::Value::as_bool) != Some(true) {
            continue;
        }
        let name = case
            .get("name")
            .and_then(toml::Value::as_str)
            .expect("a name");
        assert!(input_list_digest(&entries_of(&case)).is_err(), "{name}");
        refused += 1;
    }
    assert_eq!(refused, 2);
}

/// An experiment document with `head` between the replicates and the required tables.
fn experiment_text(head: &str) -> String {
    format!(
        "schema = \"experiment/1\"\nreplicates = 1\n{head}\n[budget.per_run]\nwall_s = 5\n[harness]\nname = \"h\"\n[treatment]\nname = \"t\"\n"
    )
}

/// Guards: Experiment file — an input is mounted by default for the agent, a file may name the evaluators, and a mount directory that is absolute or leaves the workspace, a repeated kind and the kind `build_inputs` are refused. Oracle: the D2 rules of the approved plan, applied by hand to each document.
#[test]
fn declared_inputs_are_read_and_the_forbidden_ones_refused() {
    // BORROW: .as_str() lends the built document to the parser
    let read = |head: &str| ExperimentFile::from_toml_str(experiment_text(head).as_str());
    let agent = "[[inputs]]\nkind = \"data\"\npath = \"in/data\"\n";
    let file = read(agent).expect("an agent input");
    let [input] = file.inputs() else {
        panic!("one input expected");
    };
    assert_eq!(input.kind().as_str(), "data");
    assert_eq!(input.path(), "in/data");
    assert_eq!(input.at(), None);
    assert_eq!(input.audience(), InputAudience::Agent);

    let both = format!(
        "{agent}[[inputs]]\nkind = \"gold\"\npath = \"gold.json\"\nat = \"ref/x\"\naudience = \"evaluators\"\n"
    );
    let file = read(both.as_str()).expect("two inputs");
    assert_eq!(file.inputs().len(), 2);
    assert_eq!(
        file.inputs().get(1).map(ExperimentInput::at),
        Some(Some("ref/x"))
    );
    assert_eq!(
        file.inputs().get(1).map(ExperimentInput::audience),
        Some(InputAudience::Evaluators)
    );

    for refused in [
        "[[inputs]]\nkind = \"d\"\npath = \"p\"\nat = \"/abs\"\n",
        "[[inputs]]\nkind = \"d\"\npath = \"p\"\nat = \"a/../../b\"\n",
        "[[inputs]]\nkind = \"d\"\npath = \"p\"\nat = \"..\"\n",
        "[[inputs]]\nkind = \"d\"\npath = \"p\"\n[[inputs]]\nkind = \"d\"\npath = \"q\"\n",
        "[[inputs]]\nkind = \"build_inputs\"\npath = \"p\"\n",
        "[[inputs]]\nkind = \"d\"\npath = \"p\"\naudience = \"everyone\"\n",
    ] {
        assert!(read(refused).is_err(), "{refused}");
    }
}

/// Guards: Experiment file — the treatment's source is read, and the reference a cell records drops it. Oracle: the TOML written by hand: the reference holds the name, tag and commit only.
#[test]
fn treatment_source_is_read_and_left_out_of_the_reference() {
    let text = "schema = \"experiment/1\"\nreplicates = 1\n[budget.per_run]\nwall_s = 5\n[harness]\nname = \"h\"\n[treatment]\nname = \"t\"\ntag = \"v1\"\nsource = \"../skills\"\n";
    let file = ExperimentFile::from_toml_str(text).expect("a treatment with a source");
    assert_eq!(file.treatment().source.as_deref(), Some("../skills"));
    let reference = serde_json::to_value(file.treatment().reference()).expect("JSON");
    assert_eq!(reference, json!({"name": "t", "tag": "v1"}));
    let plain = text.replace("source = \"../skills\"\n", "");
    let file = ExperimentFile::from_toml_str(plain.as_str()).expect("no source");
    assert_eq!(
        serde_json::to_value(file.treatment()).expect("JSON"),
        json!({"name": "t", "tag": "v1"})
    );
}

/// Guards: Run manifest — the evaluator-only inputs are absent when there are none, and a list that is unsorted or repeats an entry is refused. Oracle: the manifest written by hand; the lists follow the order rule of `comparable.inputs`.
#[test]
fn evaluation_inputs_are_optional_and_sorted() {
    let plain = manifest();
    let read: RunManifest = serde_json::from_value(plain.clone()).expect("no evaluation inputs");
    assert_eq!(read.evaluation_inputs(), []);
    assert_eq!(serde_json::to_value(&read).expect("JSON"), plain);

    let gold = json!({"kind": "gold", "digest": digest_text('a')});
    let other = json!({"kind": "other", "digest": digest_text('b')});
    let with = |list: Value| {
        let mut document = manifest();
        document["evaluation_inputs"] = list;
        serde_json::from_value::<RunManifest>(document)
    };
    let sorted = with(json!([gold, other])).expect("sorted");
    assert_eq!(sorted.evaluation_inputs().len(), 2);
    assert!(with(json!([other, gold])).is_err());
    assert!(with(json!([gold, gold])).is_err());
}
