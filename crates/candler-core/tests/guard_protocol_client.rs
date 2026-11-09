//! Guards for the evaluator protocol client: the failure paths against scripted stubs, and the
//! rule that no evaluation module signals a process.
#![cfg(test)]
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use candler_core::evaluation::client::{Client, ClientError, EvaluatorCommand, ItemOutcome};
use candler_core::supervise::signal::Signaller;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::evaluation::EvaluateParams;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::RunKey;
use rustix::io::Errno;
use rustix::process::{Pid, Signal};
use serde_json::json;
use tokio::sync::watch;

mod support;

/// Takes the supervisor's events and drops them.
struct Quiet;

impl EventSink for Quiet {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// Records each (group, signal) it is asked to send and sends none.
#[derive(Default)]
struct Recording {
    calls: Mutex<Vec<(i32, i32)>>,
}

impl Signaller for Recording {
    fn signal_group(&self, group: Pid, signal: Signal) -> Result<(), Errno> {
        self.calls
            .lock()
            .expect("a lock")
            .push((group.as_raw_nonzero().get(), signal.as_raw()));
        Ok(())
    }
}

/// What an item must come to, as a hand literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expect {
    Reported,
    NotMeasured(&'static str),
}

/// The three items every stub knows, `A1`, `A2` and `A3`.
fn params() -> EvaluateParams {
    let item = |id: &str| json!({"expectation_id": id, "kind": "file_exists", "params": {"path": "plan.md"}, "deadline_ms": 5000});
    serde_json::from_value(json!({
        "context": {
            "run": {"cell": format!("sha256:{}", "ab".repeat(32)), "replicate": 1, "attempt": 1},
            "root": "/runs/r1", "raw": [], "artifacts": [], "snapshots": [], "inputs": [], "metadata": {}
        },
        "items": [item("A1"), item("A2"), item("A3")],
        "prior": [],
        "scratch": "/scratch/r1"
    }))
    .expect("evaluate params")
}

/// The command that runs the stub `name` with `sh`, in the directory `cwd`.
fn stub(name: &str, records: &Path, cwd: &Path) -> EvaluatorCommand {
    let script: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "fixtures",
        "client",
        name,
    ]
    .iter()
    .collect();
    EvaluatorCommand::new(
        vec!["/bin/sh".to_owned(), script.display().to_string()],
        "m2a-client-guard".parse().expect("a marker"),
        records.to_path_buf(),
    )
    .env(vec![("PATH".to_owned(), "/usr/bin:/bin".to_owned())])
    .cwd(cwd.to_path_buf())
}

/// A supervisor whose stops go to a recording fake, and that fake.
fn recording_supervisor() -> (Supervisor, Arc<Recording>) {
    let recording = Arc::new(Recording::default());
    // TRAIT_OBJECT: the supervisor takes its signaller as a trait object
    // BORROW: .clone() shares the log between the supervisor and the test
    let signaller: Arc<dyn Signaller> = recording.clone();
    let sink: Arc<dyn EventSink> = Arc::new(Quiet);
    (Supervisor::new(sink).with_signaller(signaller), recording)
}

/// The (item, outcome) pairs of a finished request.
fn outcomes(items: &[ItemOutcome]) -> Vec<(&str, Expect)> {
    items
        .iter()
        .map(|item| match item {
            ItemOutcome::Reported(record) => (record.expectation_id().as_str(), Expect::Reported),
            ItemOutcome::Unreported(missing) => (
                missing.expectation_id.as_str(),
                Expect::NotMeasured(missing.reason.code()),
            ),
        })
        .collect()
}

/// One row of the failure table.
struct Row {
    stub: &'static str,
    /// The request deadline in ms.
    deadline_ms: u64,
    /// Flip the caller's cancel after this many ms, and expect `ClientError::Cancelled`.
    cancel_after_ms: Option<u64>,
    /// The outcome of A1, A2 and A3 (ignored when the caller cancels).
    expected: [Expect; 3],
    /// Whether the supervisor must be asked to stop the process.
    stops: bool,
    /// Whether the stub must have been sent a `cancel` line (`hang.sh` keeps what it read).
    cancel_line_seen: bool,
}

/// Runs one row and checks it.
async fn check(row: Row) {
    let name = row.stub;
    let (supervisor, recording) = recording_supervisor();
    let client = Client::new(&supervisor)
        .with_graces(Duration::from_millis(1500), Duration::from_millis(100));
    let records = tempfile::tempdir().expect("a temp dir");
    let cwd = tempfile::tempdir().expect("a temp dir");
    let (flip, cancel) = watch::channel(false);
    let command = stub(name, records.path(), cwd.path());
    // Keeps the sender alive when it is not used, so that the watch is not "dropped".
    let _keep = if let Some(after) = row.cancel_after_ms {
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(after)).await;
            let _ = flip.send(true);
        });
        None
    } else {
        Some(flip)
    };
    let run = client.evaluate(
        &command,
        params(),
        Duration::from_millis(row.deadline_ms),
        cancel,
    );
    let done = tokio::time::timeout(Duration::from_secs(60), run)
        .await
        .unwrap_or_else(|_| panic!("{name}: the request did not end"));
    if row.cancel_after_ms.is_some() {
        assert!(
            matches!(done, Err(ClientError::Cancelled)),
            "{name}: a cancel of the caller must end in Cancelled, got {:?}",
            done.map(|ok| ok.notes)
        );
    } else {
        let done = done.unwrap_or_else(|error| panic!("{name}: {error}"));
        let got = outcomes(&done.items);
        let want: Vec<(&str, Expect)> = ["A1", "A2", "A3"].into_iter().zip(row.expected).collect();
        assert_eq!(got, want, "{name}: {:?}", done.notes);
        match name {
            "partial_frame.sh" => assert!(
                done.notes.iter().any(|note| note.contains("partial frame")),
                "{name}: {:?}",
                done.notes
            ),
            "flood.sh" => assert!(
                !done.stderr.is_empty() && done.stderr.len() <= 16 * 1024,
                "{name}: kept {} bytes of stderr",
                done.stderr.len()
            ),
            _ => {}
        }
    }
    let signals = recording.calls.lock().expect("a lock").clone(); // BORROW: .clone() copies the log out of the lock
    assert_eq!(
        !signals.is_empty(),
        row.stops,
        "{name}: a stop was {} but the supervisor signalled {signals:?}",
        if row.stops {
            "expected"
        } else {
            "not expected"
        }
    );
    if row.cancel_line_seen {
        let seen = std::fs::read_to_string(cwd.path().join("seen")).unwrap_or_default();
        assert!(
            seen.contains("\"method\":\"cancel\""),
            "{name}: the stub was sent {seen:?} instead of a cancel line"
        );
        // Only the spawned process group is ever signalled: the stub wrote its pid, which is
        // the leader of its group.
        let pid: i32 = std::fs::read_to_string(cwd.path().join("pid"))
            .expect("the stub's pid")
            .trim()
            .parse()
            .expect("a pid");
        assert!(
            signals.iter().all(|(group, _)| *group == pid),
            "{name}: signalled {signals:?}, but the only group is {pid}"
        );
    }
}

/// Guards: R6 — a request ends within its deadline with exactly one outcome per item, and every failure path of an evaluator is told apart: an item it never reports is `not_measured {instrument_error}`, an item the request deadline cuts off is `not_measured {deadline}`, a cancel answered with `-32800` needs no stop, a result that arrives after the deadline but inside the cancel grace is discarded, a partial frame is `instrument_error`, 4 MB of stderr does not block the child, and a caller's cancel ends the request in `Cancelled` with a `cancel` line sent and the stop requested only for the spawned group. Oracle: hand-written expected statuses per stub; JSON-RPC 2.0 §5.1 for the `-32800` answer and R01's deadline text.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_failure_path_of_an_evaluator_is_accounted_for() {
    use Expect::{NotMeasured as Not, Reported as Rep};
    let row = |stub, deadline_ms, expected, stops, cancel_line_seen| Row {
        stub,
        deadline_ms,
        cancel_after_ms: None,
        expected,
        stops,
        cancel_line_seen,
    };
    let rows = vec![
        row(
            "unreported.sh",
            20_000,
            [Rep, Not("instrument_error"), Not("instrument_error")],
            false,
            false,
        ),
        row(
            "hang.sh",
            2000,
            [Rep, Not("deadline"), Not("deadline")],
            true,
            true,
        ),
        row(
            "cancel.sh",
            2000,
            [Rep, Not("deadline"), Not("deadline")],
            false,
            false,
        ),
        row(
            "late.sh",
            400,
            [Not("deadline"), Not("deadline"), Not("deadline")],
            false,
            false,
        ),
        row(
            "partial_frame.sh",
            20_000,
            [Rep, Not("instrument_error"), Not("instrument_error")],
            false,
            false,
        ),
        row("flood.sh", 20_000, [Rep, Rep, Rep], false, false),
        Row {
            cancel_after_ms: Some(300),
            ..row("hang.sh", 20_000, [Rep; 3], true, true)
        },
    ];
    futures::future::join_all(rows.into_iter().map(check)).await;
}

/// Guards: R6 — when the request deadline has passed and the evaluator's whole answer is already waiting in the pipe, the deadline wins and every item is `not_measured {deadline}`: the runtime is blocked past the deadline while the stub answers, so that both are ready at the next poll. Oracle: R01's deadline text, as a hand literal.
#[tokio::test(flavor = "current_thread")]
async fn a_deadline_beats_an_answer_that_is_already_waiting() {
    use Expect::NotMeasured as Not;
    let (supervisor, recording) = recording_supervisor();
    let client = Client::new(&supervisor)
        .with_graces(Duration::from_millis(1500), Duration::from_millis(100));
    let records = tempfile::tempdir().expect("a temp dir");
    let cwd = tempfile::tempdir().expect("a temp dir");
    let (_keep, cancel) = watch::channel(false);
    let command = stub("ready.sh", records.path(), cwd.path());
    // Blocks the only thread from 0.1 s to 1.3 s: the stub answers at 0.3 s, the deadline is 0.4 s.
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        std::thread::sleep(Duration::from_millis(1200));
    });
    let done = client
        .evaluate(&command, params(), Duration::from_millis(400), cancel)
        .await
        .expect("a finished request");
    let want = vec![
        ("A1", Not("deadline")),
        ("A2", Not("deadline")),
        ("A3", Not("deadline")),
    ];
    assert_eq!(outcomes(&done.items), want, "{:?}", done.notes);
    assert!(recording.calls.lock().expect("a lock").is_empty());
}

/// Guards: R6, I22 — no module under `src/evaluation/` signals a process: the text `kill(`, `killpg`, `Signal::`, `kill_process` and `Command::new("kill")` appear nowhere in it, so stopping goes only through the supervisor; each needle is first planted in a scratch file to show that the scan catches it. Oracle: the planted-violation self-test of `assert_clean` in `guard_assert_clean`, repeated per needle here.
#[test]
fn no_evaluation_module_signals_a_process() {
    let dir: PathBuf = [env!("CARGO_MANIFEST_DIR"), "src", "evaluation"]
        .iter()
        .collect();
    for needle in [
        "kill(",
        "killpg",
        "Signal::",
        "kill_process",
        "Command::new(\"kill\")",
    ] {
        let planted = tempfile::tempdir().expect("a temp dir");
        std::fs::write(
            planted.path().join("planted.rs"),
            format!("fn f() {{ {needle} }}\n"),
        )
        .expect("a planted file");
        let path = planted.path().to_path_buf();
        assert!(
            std::panic::catch_unwind(|| support::assert_clean(&path, needle)).is_err(),
            "the scan missed a planted {needle}"
        );
        support::assert_clean(&dir, needle);
    }
}
