//! Guards for the process supervisor: records, liveness, handles, events, stops and reaping.
#![cfg(test)]

use std::fmt::Write as _;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use candler_core::frames::FrameMode;
use candler_core::supervise::http::is_loopback;
use candler_core::supervise::platform::{Liveness, liveness, process_start};
use candler_core::supervise::reap::ReapRefusal;
use candler_core::supervise::signal::Signaller;
use candler_core::supervise::spawn::{OutputSpec, SpawnRequest, SpawnRequestBuilder, StdinSpec};
use candler_core::supervise::stop::{Ended, StraySweeper};
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::spec::{Action, CommandSpec, HttpMethod, StopSignal, StopSpec};
use candler_schemas::execution::{Envelope, EventPayload, StrayProcess};
use candler_schemas::primitives::{Digest, JobId, Marker, RunKey, Timestamp};
use candler_schemas::process::{ChildRecord, ExitStatus, ProcessStart};
use candler_schemas::raw::StreamName;
use rustix::io::Errno;
use rustix::process::{Pid, Signal, kill_process, test_kill_process};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

mod support;

/// The marker every test child receives.
const MARKER: &str = "m1-supervisor-guard";

/// Keeps every event the supervisor emits, in order.
#[derive(Default)]
struct Recorder {
    events: Mutex<Vec<(Option<RunKey>, EventPayload)>>,
}

impl EventSink for Recorder {
    fn emit(&self, run: Option<&RunKey>, payload: EventPayload) {
        // BORROW: .cloned() keeps the run key for the test to read
        let run = run.cloned();
        self.events.lock().expect("a lock").push((run, payload));
    }
}

fn supervisor() -> (Supervisor, Arc<Recorder>) {
    let recorder = Arc::new(Recorder::default());
    // TRAIT_OBJECT: the supervisor takes its sink as a trait object
    // BORROW: .clone() shares the recorder between the supervisor and the test
    let sink: Arc<dyn EventSink> = recorder.clone();
    (Supervisor::new(sink), recorder)
}

fn marker() -> Marker {
    MARKER.parse().expect("a marker")
}

/// A request for `argv` with the one variable a child needs to find `sleep` and `sh`.
fn request(argv: &[&str], records: &Path) -> SpawnRequestBuilder {
    SpawnRequest::builder()
        .argv(argv.iter().map(|part| (*part).to_owned()).collect())
        .env(vec![("PATH".to_owned(), "/usr/bin:/bin".to_owned())])
        .marker(marker())
        .record_dir(records.to_path_buf())
}

/// Kills every child its supervisor still holds when it goes out of scope, so a failed assertion
/// leaves no process. The kill goes through the supervisor's guard, which signals only groups the
/// supervisor spawned and re-verified.
struct KillAll<'a>(&'a Supervisor);

impl Drop for KillAll<'_> {
    fn drop(&mut self) {
        self.0.kill_all();
    }
}

/// The value at `path` in `value`, or `Null`.
fn at<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter()
        .fold(value, |here, key| here.get(*key).unwrap_or(&Value::Null))
}

/// What `ps` reports about a process: its process group and its start as seconds since the epoch.
struct PsRow {
    pid: u32,
    pgid: u32,
    started: u64,
}

/// Reads the process through `ps -o pid=,pgid=,lstart=` with `TZ=UTC`, parsing the start time
/// from its `Thu Nov  5 14:03:22 2026` form.
fn ps_row(pid: u32) -> PsRow {
    let out = Command::new("ps")
        .args(["-o", "pid=,pgid=,lstart=", "-p", &pid.to_string()])
        .env("TZ", "UTC")
        .env("LC_ALL", "C")
        .output()
        .expect("ps runs");
    let text = String::from_utf8(out.stdout).expect("ps prints text");
    let fields: Vec<&str> = text.split_whitespace().collect();
    assert_eq!(fields.len(), 7, "ps printed {text:?}");
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let get = |n: usize| fields.get(n).copied().expect("a ps field");
    let month = months
        .iter()
        .position(|name| *name == get(3))
        .expect("a month name");
    let clock: Vec<i8> = get(5)
        .split(':')
        .map(|part| part.parse().expect("a clock part"))
        .collect();
    let at = |n: usize| clock.get(n).copied().expect("hh:mm:ss");
    let started = jiff::civil::date(
        get(6).parse().expect("a year"),
        i8::try_from(month + 1).expect("a month"),
        get(4).parse().expect("a day"),
    )
    .at(at(0), at(1), at(2), 0)
    .to_zoned(jiff::tz::TimeZone::UTC)
    .expect("a valid time")
    .timestamp()
    .as_second();
    PsRow {
        pid: get(0).parse().expect("a pid"),
        pgid: get(1).parse().expect("a pgid"),
        started: u64::try_from(started).expect("a start after the epoch"),
    }
}

/// The ticks the kernel reports for field 22 of `/proc/<pid>/stat`, read through the shell.
#[cfg(target_os = "linux")]
fn shell_ticks(pid: u32) -> u64 {
    let script = format!("sed 's/^.*) //' /proc/{pid}/stat | cut -d' ' -f20");
    let out = Command::new("sh")
        .args(["-c", &script])
        .output()
        .expect("sh runs");
    String::from_utf8(out.stdout)
        .expect("text")
        .trim()
        .parse()
        .expect("a number")
}

/// The boot identifier the kernel reports, read through `cat`.
#[cfg(target_os = "linux")]
fn shell_boot_id() -> String {
    let out = Command::new("cat")
        .arg("/proc/sys/kernel/random/boot_id")
        .output()
        .expect("cat runs");
    String::from_utf8(out.stdout)
        .expect("text")
        .trim()
        .to_owned()
}

/// The one record file under `dir`, read and parsed, with the JSON it was parsed from.
fn only_record(dir: &Path) -> (PathBuf, Value, ChildRecord) {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("a record dir")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(names.len(), 1, "record dir holds {names:?}");
    let path = dir.join(names.first().expect("a name"));
    let bytes = fs::read(&path).expect("a readable record");
    let json: Value = serde_json::from_slice(&bytes).expect("a JSON record");
    let record = serde_json::from_slice(&bytes).expect("a child record");
    (path, json, record)
}

/// Guards: G4 — the record is on disk when `spawn` returns, and its pid, pgid and start equal what the operating system reports. Oracle: `ps -o pid=,pgid=,lstart=` with `TZ=UTC` (and, on Linux, `sed`/`cut` over `/proc/<pid>/stat` and `cat` of the boot id).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn record_on_disk_equals_what_the_os_reports() {
    let records = tempfile::tempdir().expect("a temp dir");
    let (supervisor, _events) = supervisor();
    let child = supervisor
        .spawn(
            request(&["sleep", "30"], records.path())
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    // Read at once: nothing between `spawn` returning and this read may be needed.
    let (path, json, record) = only_record(records.path());
    let pid = child.record().pid();
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(format!("{pid}.json").as_str())
    );
    assert_eq!(&record, child.record());
    assert_eq!(at(&json, &["schema"]), &json!("child-record/1"));
    assert_eq!(at(&json, &["marker"]), &json!(MARKER));

    let ps = ps_row(pid);
    assert_eq!(record.pid(), ps.pid);
    assert_eq!(record.pgid(), ps.pgid);
    assert_eq!(record.pgid(), record.pid(), "the child leads its group");
    match at(&json, &["start", "kind"]).as_str() {
        Some("darwin") => {
            assert_eq!(at(&json, &["start", "sec"]).as_u64(), Some(ps.started));
            let usec = at(&json, &["start", "usec"]).as_u64().expect("usec");
            assert!(usec < 1_000_000, "usec is {usec}");
        }
        #[cfg(target_os = "linux")]
        Some("linux") => {
            assert_eq!(
                at(&json, &["start", "ticks"]).as_u64(),
                Some(shell_ticks(pid))
            );
            assert_eq!(
                at(&json, &["start", "boot_id"]).as_str(),
                Some(shell_boot_id().as_str())
            );
        }
        other => panic!("the record's start is {other:?}"),
    }
    assert_eq!(liveness(&record), Liveness::Alive);
    supervisor.kill_all();
}

/// Guards: I15 and PID reuse — a record whose start differs by one tick (or microsecond), whose start is unavailable, or whose pid names another live process reads `Gone` while the child runs; the true record reads `Alive` and then `Gone` once the child is dead. Oracle: hand-forged records built from the true one, and the OS for the child's life.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_forged_record_does_not_read_alive() {
    let records = tempfile::tempdir().expect("a temp dir");
    let (supervisor, _events) = supervisor();
    let child = supervisor
        .spawn(
            request(&["sleep", "30"], records.path())
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    let (_, json, record) = only_record(records.path());
    assert_eq!(liveness(&record), Liveness::Alive);

    let with_start = |start: Value, pid: Option<u32>| -> ChildRecord {
        let mut forged = json.clone(); // BORROW: .clone() forges a copy and leaves the true record alone
        if let Some(map) = forged.as_object_mut() {
            map.insert("start".to_owned(), start);
            if let Some(pid) = pid {
                map.insert("pid".to_owned(), json!(pid));
                map.insert("pgid".to_owned(), json!(pid));
            }
        }
        serde_json::from_value(forged).expect("a forged record")
    };
    let true_start = at(&json, &["start"]).clone(); // BORROW: .clone() copies the true start
    let off_by_one = {
        let mut start = true_start.clone(); // BORROW: .clone() copies the true start
        let field = if at(&true_start, &["kind"]).as_str() == Some("darwin") {
            "usec"
        } else {
            "ticks"
        };
        let value = at(&true_start, &[field]).as_u64().expect("a start number");
        let moved = if field == "usec" && value >= 999_999 {
            value - 1
        } else {
            value + 1
        };
        if let Some(map) = start.as_object_mut() {
            map.insert(field.to_owned(), json!(moved));
        }
        start
    };
    let table: [(&str, ChildRecord); 3] = [
        ("one off", with_start(off_by_one, None)),
        (
            "unavailable",
            with_start(json!({"kind": "unavailable"}), None),
        ),
        (
            "another live pid",
            with_start(true_start.clone(), Some(std::process::id())), // BORROW: .clone() copies the true start
        ),
    ];
    for (label, forged) in &table {
        assert_eq!(liveness(forged), Liveness::Gone, "{label}");
    }
    assert_eq!(liveness(&record), Liveness::Alive);

    supervisor.kill_all();
    drop(child);
    let mut waited = Duration::ZERO;
    while liveness(&record) == Liveness::Alive && waited < Duration::from_secs(10) {
        tokio::time::sleep(Duration::from_millis(50)).await;
        waited += Duration::from_millis(50);
    }
    assert_eq!(
        liveness(&record),
        Liveness::Gone,
        "the dead child still reads alive"
    );
}

/// A job id for the envelopes the tests build.
const JOB: &str = "018f2b3c-4d5e-7a6b-8c7d-9e0f1a2b3c4d";

/// A shell that ignores `TERM`, creates the file named by its first argument and then becomes
/// `sleep`, which keeps the ignored signal: only `SIGKILL` ends it. The file says the signal is
/// ignored, so that a test stops the child only after that.
fn stubborn(ready: &str) -> [&str; 5] {
    [
        "sh",
        "-c",
        "trap '' TERM; : > \"$1\"; exec sleep 30",
        "sh",
        ready,
    ]
}

/// Waits until `path` exists, for at most 10 s.
async fn until_ready(path: &Path) {
    let until = Instant::now() + Duration::from_secs(10);
    while !path.exists() {
        assert!(Instant::now() < until, "the child never became ready");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn stop_spec(pre_stop: Option<Action>, grace_ms: u64) -> StopSpec {
    StopSpec::new(pre_stop, StopSignal::Term, Duration::from_millis(grace_ms))
}

fn run_key() -> RunKey {
    let cell: Digest = format!("sha256:{}", "a".repeat(64))
        .parse()
        .expect("a digest");
    RunKey::new(cell, 1, 1).expect("a run key")
}

/// Every event the recorder holds, wrapped in an envelope and written to JSON.
fn envelopes(recorder: &Recorder) -> Vec<Value> {
    let events = recorder.events.lock().expect("a lock");
    events
        .iter()
        .enumerate()
        .map(|(seq, (run, payload))| {
            let mut builder = Envelope::builder()
                .seq(u64::try_from(seq).expect("a small index"))
                .at("2026-01-02T03:04:05Z".parse::<Timestamp>().expect("a time"))
                .job_id(JOB.parse::<JobId>().expect("a job id"))
                .event(payload.clone()); // BORROW: .clone() keeps the recorded event
            if let Some(run) = run {
                builder = builder.run(run.clone()); // BORROW: .clone() keeps the recorded run key
            }
            serde_json::to_value(builder.build().expect("an envelope")).expect("JSON")
        })
        .collect()
}

/// The validator for the published `event-envelope` schema.
fn envelope_validator() -> jsonschema::Validator {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("candler-schemas")
        .join("schema")
        .join("event-envelope.schema.json");
    let text = fs::read_to_string(path).expect("the committed schema exists");
    let schema: Value = serde_json::from_str(&text).expect("the schema is JSON");
    jsonschema::validator_for(&schema).expect("the schema compiles")
}

/// `kind`, and `kind:step` for a stop step, of each envelope.
fn shape(envelopes: &[Value]) -> Vec<String> {
    envelopes
        .iter()
        .map(|envelope| {
            let kind = at(envelope, &["kind"]).as_str().expect("a kind");
            match at(envelope, &["payload", "step"]).as_str() {
                Some(step) => format!("{kind}:{step}"),
                None => kind.to_owned(), // BORROW: .to_owned() makes the owned label
            }
        })
        .collect()
}

/// Guards: G3 and R01 §escalation — a stopped child's sink receives the ordered events `process.spawned`, the stop steps `signal`, `terminate_group`, `kill_group`, `sweep`, then `process.exited`, each stop step and the exit carrying the child's `{pid, start}` and the run key, and every envelope validates against the published `event-envelope` schema. Oracle: the literal ordered list written here from R01's ladder, and the schema file.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stopped_child_reports_each_step_in_order() {
    let records = tempfile::tempdir().expect("a temp dir");
    let ready = records.path().join("ready");
    let ready_arg = ready.to_str().expect("a UTF-8 path");
    let (supervisor, recorder) = supervisor();
    let child = supervisor
        .spawn(
            request(&stubborn(ready_arg), records.path())
                .run(run_key())
                .stop(stop_spec(None, 300))
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    until_ready(&ready).await;
    let record_json = serde_json::to_value(child.record()).expect("JSON");
    let pid = child.record().pid();
    let exit = child.stop().await.expect("an exit");

    assert_eq!(exit.ended, Ended::Stopped);
    assert!(exit.group_empty);
    assert_eq!(exit.status, ExitStatus::Signal { signal: 9 });
    let taken: Vec<String> = exit
        .steps
        .iter()
        .map(|step| serde_json::to_value(step.step).expect("JSON").to_string())
        .collect();
    assert_eq!(
        taken,
        [
            "\"signal\"",
            "\"terminate_group\"",
            "\"kill_group\"",
            "\"sweep\""
        ]
    );

    let all = envelopes(&recorder);
    assert_eq!(
        shape(&all),
        [
            "process.spawned",
            "process.stop_step:signal",
            "process.stop_step:terminate_group",
            "process.stop_step:kill_group",
            "process.stop_step:sweep",
            "process.exited",
        ]
    );
    let validator = envelope_validator();
    let run = serde_json::to_value(run_key()).expect("JSON");
    for envelope in &all {
        let problems: Vec<String> = validator
            .iter_errors(envelope)
            .map(|problem| problem.to_string())
            .collect();
        assert!(problems.is_empty(), "{envelope}: {problems:?}");
        assert_eq!(at(envelope, &["run"]), &run, "{envelope}");
    }
    for envelope in all.iter().skip(1) {
        assert_eq!(at(envelope, &["payload", "pid"]), &json!(pid), "{envelope}");
        assert_eq!(
            at(envelope, &["payload", "start"]),
            at(&record_json, &["start"]),
            "{envelope}"
        );
    }
    let last = all.last().expect("an exit event");
    assert_eq!(
        at(last, &["payload", "status"]),
        &json!({"kind": "signal", "signal": 9})
    );
    assert_eq!(at(last, &["payload", "group_empty"]), &json!(true));
}

/// Guards: Cond 2 (no secret reaches disk, an event or a `Debug` print) — a canary placed in the child's environment appears in no file under `record_dir`, no event and no `Debug` output of the request, the supervisor, the child or the exit, and the child did receive it. Oracle: `tests/support::assert_clean` scanning the written files, and a `printf` child that echoes the variable as the positive control.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_secret_in_the_environment_is_written_nowhere() {
    const CANARY: &str = "canary-7d3f9a51c2e84b06";
    let records = tempfile::tempdir().expect("a temp dir");
    let with_canary = |argv: &[&str]| {
        request(argv, records.path())
            .env(vec![
                ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
                ("API_KEY".to_owned(), CANARY.to_owned()),
            ])
            .run(run_key())
    };
    let (supervisor, recorder) = supervisor();

    // The positive control: the variable reaches the child.
    let echoed = supervisor
        .run_captured(
            with_canary(&["sh", "-c", "printf %s \"$API_KEY\""])
                .record_dir(tempfile::tempdir().expect("a temp dir").keep())
                .build()
                .expect("a request"),
            1024,
        )
        .await
        .expect("a probe");
    assert_eq!(echoed.stdout, CANARY.as_bytes());

    let built = with_canary(&["sleep", "30"]).build().expect("a request");
    let mut debug = format!("{built:?}\n{supervisor:?}\n");
    let child = supervisor.spawn(built).expect("a child");
    let _cleanup = KillAll(&supervisor);
    writeln!(debug, "{child:?}\n{:?}", child.record()).expect("a write");
    let exit = child.stop().await.expect("an exit");
    writeln!(debug, "{exit:?}\n{echoed:?}").expect("a write");

    let scratch = tempfile::tempdir().expect("a temp dir");
    let events = envelopes(&recorder)
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(scratch.path().join("events.jsonl"), events).expect("written");
    fs::write(scratch.path().join("debug.txt"), debug).expect("written");
    support::assert_clean(records.path(), CANARY);
    support::assert_clean(scratch.path(), CANARY);
}

/// Guards: R01 §escalation (wait ends by exit plus the grace) — a grandchild in another session that holds the child's standard output for 8 s does not hold `wait` past deadline + grace + 2 s, and the frames written before the cut-off are kept. Oracle: the wall clock around `wait`, with the held pipe proved by the grandchild's own `ready` line in the raw file.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_grandchild_holding_the_pipe_does_not_hold_wait() {
    let records = tempfile::tempdir().expect("a temp dir");
    let raw = tempfile::tempdir().expect("a temp dir");
    let script = "perl -MPOSIX -e 'POSIX::setsid(); print qq(ready\\n); $|=1; exec q(sleep), q(8)' & exec sleep 30";
    let (supervisor, _events) = supervisor();
    let child = supervisor
        .spawn(
            request(&["sh", "-c", script], records.path())
                .stdout(OutputSpec::framed(
                    raw.path().to_path_buf(),
                    "stdout".parse::<StreamName>().expect("a stream name"),
                    FrameMode::Lines,
                ))
                .deadline(Duration::from_millis(1500))
                .stop(stop_spec(None, 500))
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    let began = Instant::now();
    let exit = child.wait().await.expect("an exit");
    let took = began.elapsed();
    assert_eq!(exit.ended, Ended::Deadline);
    assert!(took >= Duration::from_millis(1500), "waited only {took:?}");
    assert!(took < Duration::from_secs(4), "wait was held for {took:?}");
    let file = exit.raw.first().expect("the stdout raw file");
    assert!(file.frames >= 1, "the grandchild never wrote: {file:?}");
}

/// A request that fails or ignores its pre-stop and a child that ignores `TERM`.
async fn stop_after_failed_pre_stop(action: Action) -> candler_core::supervise::stop::Exit {
    let records = tempfile::tempdir().expect("a temp dir");
    let ready = records.path().join("ready");
    let ready_arg = ready.to_str().expect("a UTF-8 path");
    let (supervisor, _events) = supervisor();
    let child = supervisor
        .spawn(
            request(&stubborn(ready_arg), records.path())
                .stop(stop_spec(Some(action), 400))
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    until_ready(&ready).await;
    child.stop().await.expect("an exit")
}

/// Guards: R01 §escalation (a failed or timed-out pre-stop continues the ladder) — an HTTP pre-stop to a closed loopback port and a command pre-stop that sleeps past the grace each report their failure, then the child, which ignores `TERM`, is taken through `signal`, `terminate_group` and `kill_group` and dies of `SIGKILL`. Oracle: the literal step list and outcomes written here, and the OS exit status of the child.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_pre_stop_still_reaches_term_then_kill() {
    let closed_port = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
        listener.local_addr().expect("an address").port()
    };
    let sleeper = CommandSpec::builder()
        .argv(vec!["sleep".to_owned(), "30".to_owned()])
        .build()
        .expect("a command");
    let table: [(&str, Action, &str); 2] = [
        (
            "closed port",
            Action::http(
                HttpMethod::Get,
                format!("http://127.0.0.1:{closed_port}/stop"),
                None,
            ),
            "http failed: connect",
        ),
        (
            "sleeping command",
            Action::Command(sleeper),
            "command timed out and was killed",
        ),
    ];
    for (label, action, outcome) in table {
        let exit = stop_after_failed_pre_stop(action).await;
        let steps: Vec<(String, String)> = exit
            .steps
            .iter()
            .map(|step| {
                (
                    serde_json::to_value(step.step)
                        .expect("JSON")
                        .as_str()
                        .expect("a step name")
                        .to_owned(), // BORROW: .to_owned() makes the owned name
                    step.outcome.clone(), // BORROW: .clone() keeps the outcome
                )
            })
            .collect();
        let names: Vec<&str> = steps.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "pre_stop",
                "signal",
                "terminate_group",
                "kill_group",
                "sweep"
            ],
            "{label}"
        );
        let said = |n: usize| steps.get(n).map(|(_, text)| text.as_str()).expect("a step");
        assert_eq!(said(0), outcome, "{label}");
        assert_eq!(
            said(1),
            "signal sent; leader still running after grace",
            "{label}"
        );
        assert_eq!(said(2), "group still has members after SIGTERM", "{label}");
        assert_eq!(said(3), "group empty after SIGKILL", "{label}");
        assert_eq!(exit.status, ExitStatus::Signal { signal: 9 }, "{label}");
        assert!(exit.group_empty, "{label}");
    }
}

/// Reads one request (headers and `body_len` body bytes) from the first connection to `listener`
/// within 10 s, answers `204`, and returns the request text; `None` if nobody connects.
fn serve_once(listener: &TcpListener, body_len: usize) -> Option<String> {
    listener.set_nonblocking(true).expect("nonblocking");
    let until = Instant::now() + Duration::from_secs(10);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(_) if Instant::now() < until => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => return None,
        }
    };
    stream.set_nonblocking(false).expect("blocking");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("a timeout");
    let mut seen = Vec::new();
    let mut chunk = [0_u8; 256];
    loop {
        let count = stream.read(&mut chunk).expect("a read");
        seen.extend_from_slice(chunk.get(..count).expect("a chunk"));
        let text = String::from_utf8_lossy(&seen);
        if let Some(split) = text.find("\r\n\r\n")
            && seen.len() >= split + 4 + body_len
        {
            break;
        }
        assert!(count > 0, "the client closed early: {text:?}");
    }
    stream
        .write_all(b"HTTP/1.1 204 No Content\r\n\r\n")
        .expect("a reply");
    Some(String::from_utf8_lossy(&seen).into_owned())
}

/// Guards: R01 §escalation and the loopback rule — only `http` URLs to `localhost`, `127.0.0.0/8` and `[::1]` without userinfo are loopback; an accepted URL is called with the literal request bytes below and a refused one opens no connection. Oracle: the hand-written URL table, and a listener that records the bytes it receives.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn only_loopback_urls_are_called() {
    let accepted = [
        "http://127.0.0.1:8080/x",
        "http://127.1.2.3/",
        "http://[::1]:9/",
        "http://localhost/",
        "http://localhost:3000/a?b=c",
        "http://127.0.0.1#@evil.com/",
        "http://2130706433/",
        "http://0x7f.0.0.1/",
    ];
    let refused = [
        "http://127.0.0.1.evil.com/",
        "http://127.0.0.1@evil.com/",
        "http://evil.com@127.0.0.1/",
        "http://user:pw@127.0.0.1/",
        "http://evil.com\\@127.0.0.1/",
        "https://127.0.0.1/",
        "http://[::ffff:127.0.0.1]/",
        "http://0.0.0.0/",
        "http://127.0.0.1:0/",
        "http://localhost.evil.com/",
        "http://10.0.0.1/",
        "ftp://127.0.0.1/",
        "127.0.0.1",
        "",
    ];
    for url in accepted {
        assert!(is_loopback(url), "{url:?} should be loopback");
    }
    for url in refused {
        assert!(!is_loopback(url), "{url:?} should not be loopback");
    }

    // An accepted URL is called, with these exact bytes.
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = listener.local_addr().expect("an address").port();
    let served = std::thread::spawn(move || serve_once(&listener, 2));
    let records = tempfile::tempdir().expect("a temp dir");
    let (supervisor, _events) = supervisor();
    let call = Action::http(
        HttpMethod::Post,
        format!("http://127.0.0.1:{port}/stop?now=1"),
        Some("ok".to_owned()),
    );
    let child = supervisor
        .spawn(
            request(&["sleep", "30"], records.path())
                .stop(stop_spec(Some(call), 400))
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    let exit = child.stop().await.expect("an exit");
    let said = exit.steps.first().map(|step| step.outcome.as_str());
    assert_eq!(said, Some("http 204"));
    assert_eq!(
        served.join().expect("the listener thread"),
        Some(format!(
            "POST /stop?now=1 HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Length: 2\r\n\r\nok"
        ))
    );

    // A refused URL is not called, even though its text names the listener's address.
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = listener.local_addr().expect("an address").port();
    let watcher = std::thread::spawn(move || {
        listener.set_nonblocking(true).expect("nonblocking");
        std::thread::sleep(Duration::from_millis(1500));
        listener.accept().is_ok()
    });
    let call = Action::http(
        HttpMethod::Get,
        format!("http://evil.com@127.0.0.1:{port}/"),
        None,
    );
    let child = supervisor
        .spawn(
            request(&["sleep", "30"], records.path())
                .stop(stop_spec(Some(call), 400))
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    let exit = child.stop().await.expect("an exit");
    let said = exit.steps.first().map(|step| step.outcome.as_str());
    assert_eq!(said, Some("refused: not loopback"));
    assert!(
        !watcher.join().expect("the watcher thread"),
        "a connection arrived"
    );
}

/// One row of the capture table: label, argv, limit, expected stdout, expected stderr.
type Probe<'a> = (&'a str, &'a [&'a str], usize, &'a [u8], &'a [u8]);

/// Guards: R01 §handles — bytes written through `take_stdin` of a `cat` child come back through `take_stdout`; a `StopHandle` request from another task ends a waiting child `Stopped`; `run_captured` returns the literal bytes, cut at the limit. Oracle: the bytes and the limits written here, and the OS exit status of `cat`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handles_carry_bytes_requests_and_captures() {
    let records = tempfile::tempdir().expect("a temp dir");
    let (supervisor, _events) = supervisor();

    let mut cat = supervisor
        .spawn(
            request(&["cat"], records.path())
                .stdin(StdinSpec::Pipe)
                .stdout(OutputSpec::Pipe)
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cat_cleanup = KillAll(&supervisor);
    let mut stdin = cat.take_stdin().expect("the stdin pipe");
    let mut stdout = cat.take_stdout().expect("the stdout pipe");
    assert!(cat.take_stdin().is_none(), "the pipe is given once");
    stdin.write_all(b"hello\nworld").await.expect("a write");
    drop(stdin);
    let mut echoed = Vec::new();
    stdout.read_to_end(&mut echoed).await.expect("a read");
    assert_eq!(echoed, b"hello\nworld");
    let exit = cat.wait().await.expect("an exit");
    assert_eq!(exit.ended, Ended::Exited);
    assert_eq!(exit.status, ExitStatus::Code { code: 0 });

    let records = tempfile::tempdir().expect("a temp dir");
    let sleeper = supervisor
        .spawn(
            request(&["sleep", "20"], records.path())
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _sleeper_cleanup = KillAll(&supervisor);
    let handle = sleeper.stop_handle();
    let asker = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        handle.request();
    });
    let began = Instant::now();
    let exit = sleeper.wait().await.expect("an exit");
    asker.await.expect("the asking task");
    assert_eq!(exit.ended, Ended::Stopped);
    assert!(
        began.elapsed() < Duration::from_secs(15),
        "the request was ignored"
    );

    let table: [Probe<'_>; 3] = [
        (
            "stdout",
            &["printf", "%s", "lit\nbytes"],
            1024,
            b"lit\nbytes",
            b"",
        ),
        ("cut", &["printf", "%s", "abcdef"], 3, b"abc", b""),
        (
            "both",
            &["sh", "-c", "printf out; printf err >&2"],
            1024,
            b"out",
            b"err",
        ),
    ];
    for (label, argv, limit, out, err) in table {
        let records = tempfile::tempdir().expect("a temp dir");
        let captured = supervisor
            .run_captured(
                request(argv, records.path()).build().expect("a request"),
                limit,
            )
            .await
            .expect("a probe");
        assert_eq!(captured.stdout, out, "{label}");
        assert_eq!(captured.stderr, err, "{label}");
        assert_eq!(
            captured.exit.status,
            ExitStatus::Code { code: 0 },
            "{label}"
        );
    }
}

/// Records each group it is asked to signal and signals nothing.
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

/// Guards: I22 — the core never signals a process group or pid it did not spawn and re-verify. Oracle: the fake's call log against the pid of the child the test spawned; the ladder of R01 §escalation (`TERM`, `TERM`, `KILL`). The fake signals nothing, so the children are `sleep` commands that end on their own.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_ladder_signals_exactly_the_group_it_spawned() {
    let recording = Arc::new(Recording::default());
    // TRAIT_OBJECT: the supervisor takes its signaller as a trait object
    // BORROW: .clone() shares the log between the supervisor and the test
    let signaller: Arc<dyn Signaller> = recording.clone();
    let recorder = Arc::new(Recorder::default());
    // TRAIT_OBJECT: the supervisor takes its sink as a trait object
    // BORROW: .clone() shares the recorder between the supervisor and the test
    let sink: Arc<dyn EventSink> = recorder.clone();
    let supervisor = Supervisor::new(sink).with_signaller(signaller);
    let records = tempfile::tempdir().expect("a temp dir");
    let spawn = |argv: &[&str], deadline: Option<Duration>| {
        let mut builder = request(argv, records.path()).stop(stop_spec(None, 200));
        if let Some(limit) = deadline {
            builder = builder.deadline(limit);
        }
        supervisor
            .spawn(builder.build().expect("a request"))
            .expect("a child")
    };
    let asked = spawn(&["sleep", "8"], None);
    let late = spawn(&["sleep", "8"], Some(Duration::from_millis(300)));
    let natural = spawn(&["sh", "-c", "exit 0"], None);
    let (asked_pid, late_pid, natural_pid) = (
        i32::try_from(asked.record().pid()).expect("a pid"),
        i32::try_from(late.record().pid()).expect("a pid"),
        i32::try_from(natural.record().pid()).expect("a pid"),
    );
    // The fake delivers nothing, so the two long children outlive the ladder and the waits end
    // in an error; what is checked is what the guard let through.
    let (_asked, _late, natural_exit) = tokio::join!(asked.stop(), late.wait(), natural.wait());
    assert_eq!(
        natural_exit.expect("an exit").status,
        ExitStatus::Code { code: 0 }
    );

    let calls = recording.calls.lock().expect("a lock").clone(); // BORROW: .clone() copies the log out of the lock
    let (term, kill) = (Signal::TERM.as_raw(), Signal::KILL.as_raw());
    for (label, pid) in [("stopped", asked_pid), ("deadline", late_pid)] {
        let own: Vec<(i32, i32)> = calls.iter().copied().filter(|(g, _)| *g == pid).collect();
        assert_eq!(
            own,
            [(pid, term), (pid, term), (pid, kill)],
            "{label}: {calls:?}"
        );
    }
    assert!(
        calls.iter().all(|(g, _)| [asked_pid, late_pid].contains(g)),
        "a group the test did not spawn, or a child that exited by itself, was signalled: {calls:?} (natural child {natural_pid})"
    );
    assert_eq!(calls.len(), 6, "{calls:?}");
}

/// A sweeper that finds nothing and notes the marker of every sweep.
#[derive(Default)]
struct Counting {
    markers: Mutex<Vec<String>>,
}

impl StraySweeper for Counting {
    fn sweep(&self, marker: &Marker) -> Vec<StrayProcess> {
        self.markers
            .lock()
            .expect("a lock")
            .push(marker.to_string());
        Vec::new()
    }
}

/// Guards: I22 — `reap` reads the record's leader from the operating system again and sends no signal for a record it cannot verify. Oracle: the fake's call log, empty for a forged start, a group the record does not lead, the test process's own id and process 1, and exactly `TERM`, `TERM`, `KILL` to the child's group for a true record (the positive control); POSIX kill(2) for pid <= 1.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reap_signals_nothing_for_a_record_it_cannot_verify() {
    let records = tempfile::tempdir().expect("a temp dir");
    let (spawner, _events) = supervisor();
    let child = spawner
        .spawn(
            request(&["sleep", "30"], records.path())
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&spawner);
    let (_, json, record) = only_record(records.path());
    let pid = record.pid();
    let me = std::process::id();
    let with = |edits: &[(&str, Value)]| -> ChildRecord {
        let mut forged = json.clone(); // BORROW: .clone() forges a copy and leaves the true record alone
        if let Some(map) = forged.as_object_mut() {
            for (field, value) in edits {
                map.insert((*field).to_owned(), value.clone()); // BORROW: .clone() copies the edit into the record
            }
        }
        serde_json::from_value(forged).expect("a forged record")
    };
    let start_of = |pid: u32| serde_json::to_value(process_start(pid)).expect("JSON");
    let wrong_start = {
        let mut start = at(&json, &["start"]).clone(); // BORROW: .clone() copies the true start
        let field = if at(&start, &["kind"]).as_str() == Some("darwin") {
            "usec"
        } else {
            "ticks"
        };
        let value = at(&start, &[field]).as_u64().expect("a start number");
        let moved = if field == "usec" && value >= 999_999 {
            value - 1
        } else {
            value + 1
        };
        if let Some(map) = start.as_object_mut() {
            map.insert(field.to_owned(), json!(moved));
        }
        start
    };
    // Each row: a record the supervisor must not signal for, and whether the leader reads alive.
    let table: [(&str, ChildRecord, Option<bool>); 4] = [
        ("forged start", with(&[("start", wrong_start)]), Some(false)),
        (
            "a group the process does not lead",
            with(&[("pgid", json!(1))]),
            Some(true),
        ),
        (
            "the test process itself",
            with(&[
                ("pid", json!(me)),
                ("pgid", json!(me)),
                ("start", start_of(me)),
            ]),
            Some(true),
        ),
        (
            "process 1",
            with(&[
                ("pid", json!(1)),
                ("pgid", json!(1)),
                ("start", start_of(1)),
            ]),
            None,
        ),
    ];
    for (label, forged, alive) in &table {
        let recording = Arc::new(Recording::default());
        let sweeper = Arc::new(Counting::default());
        // TRAIT_OBJECT: the supervisor takes its signaller and sweeper as trait objects
        // BORROW: .clone() shares the log and the sweeper with the test
        let (signaller, sweeping): (Arc<dyn Signaller>, Arc<dyn StraySweeper>) =
            (recording.clone(), sweeper.clone());
        let recorder = Arc::new(Recorder::default());
        // TRAIT_OBJECT: the supervisor takes its sink as a trait object
        // BORROW: .clone() shares the recorder with the supervisor
        let sink: Arc<dyn EventSink> = recorder.clone();
        let reaper = Supervisor::new(sink)
            .with_signaller(signaller)
            .with_sweeper(sweeping);
        let report = reaper
            .reap(Some(&run_key()), forged, &stop_spec(None, 200))
            .await
            .expect("a report");
        let calls = recording.calls.lock().expect("a lock").clone(); // BORROW: .clone() copies the log out of the lock
        assert_eq!(calls, Vec::<(i32, i32)>::new(), "{label} was signalled");
        assert!(report.refusal.is_some(), "{label}: {report:?}");
        if let Some(expected) = alive {
            assert_eq!(report.was_alive, *expected, "{label}");
        }
        let names: Vec<String> = report
            .steps
            .iter()
            .map(|step| serde_json::to_value(step.step).expect("JSON").to_string())
            .collect();
        assert_eq!(names, ["\"sweep\""], "{label}");
        assert_eq!(
            sweeper.markers.lock().expect("a lock").len(),
            1,
            "{label}: the sweep did not run once"
        );
    }

    // The positive control: the true record is signalled, exactly in the group of the child.
    let recording = Arc::new(Recording::default());
    // TRAIT_OBJECT: the supervisor takes its signaller as a trait object
    // BORROW: .clone() shares the log with the supervisor
    let signaller: Arc<dyn Signaller> = recording.clone();
    let recorder = Arc::new(Recorder::default());
    // TRAIT_OBJECT: the supervisor takes its sink as a trait object
    // BORROW: .clone() shares the recorder with the supervisor
    let sink: Arc<dyn EventSink> = recorder.clone();
    let reaper = Supervisor::new(sink).with_signaller(signaller);
    let report = reaper
        .reap(None, &record, &stop_spec(None, 200))
        .await
        .expect("a report");
    let group = i32::try_from(pid).expect("a pid");
    let calls = recording.calls.lock().expect("a lock").clone(); // BORROW: .clone() copies the log out of the lock
    assert_eq!(
        calls,
        [
            (group, Signal::TERM.as_raw()),
            (group, Signal::TERM.as_raw()),
            (group, Signal::KILL.as_raw())
        ]
    );
    assert!(report.was_alive);
    assert_eq!(report.refusal, None);
    drop(child);
}

/// Guards: I22 — `reap` refuses a record whose process the operating system puts in another group than the record names, though the pid and the start time verify. Oracle: the fake's call log is empty, the report names `GroupMismatch`, and the child, which is in the test's own group, is alive after the reap; getpgid(2) for the group of a process that never called setpgid.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reap_refuses_a_record_whose_group_the_process_does_not_lead() {
    let records = tempfile::tempdir().expect("a temp dir");
    let (spawner, _events) = supervisor();
    let template = spawner
        .spawn(
            request(&["sleep", "30"], records.path())
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&spawner);
    let (_, json, _) = only_record(records.path());
    // A child that stays in the test's own group, with a record that says it leads its own.
    let mut bystander = Command::new("sleep").arg("30").spawn().expect("a child");
    let pid = bystander.id();
    let mut forged = json.clone(); // BORROW: .clone() forges a copy and leaves the true record alone
    if let Some(map) = forged.as_object_mut() {
        map.insert("pid".to_owned(), json!(pid));
        map.insert("pgid".to_owned(), json!(pid));
        map.insert(
            "start".to_owned(),
            serde_json::to_value(process_start(pid)).expect("JSON"),
        );
    }
    let record: ChildRecord = serde_json::from_value(forged).expect("a forged record");

    let recording = Arc::new(Recording::default());
    // TRAIT_OBJECT: the supervisor takes its signaller as a trait object
    // BORROW: .clone() shares the log with the supervisor
    let signaller: Arc<dyn Signaller> = recording.clone();
    let recorder = Arc::new(Recorder::default());
    // TRAIT_OBJECT: the supervisor takes its sink as a trait object
    // BORROW: .clone() shares the recorder with the supervisor
    let sink: Arc<dyn EventSink> = recorder.clone();
    let reaper = Supervisor::new(sink).with_signaller(signaller);
    let report = reaper
        .reap(None, &record, &stop_spec(None, 200))
        .await
        .expect("a report");
    let calls = recording.calls.lock().expect("a lock").clone(); // BORROW: .clone() copies the log out of the lock
    let alive_after = bystander.try_wait().expect("a status").is_none();
    // EXPLICIT: handle kill of this test's own unreaped child; no pid or group number is passed
    let _ = bystander.kill();
    let _ = bystander.wait();
    assert_eq!(calls, Vec::<(i32, i32)>::new(), "a signal was sent");
    assert_eq!(report.refusal, Some(ReapRefusal::GroupMismatch));
    assert!(alive_after, "the bystander was ended");
    drop(template);
}

/// Guards: I22 — reaping the record of a group this supervisor already holds does not drop it from the table: the child's own handle may still stop it afterwards. Oracle: `holds_group` before and after, under a fake signaller that delivers nothing, so the child is ended by the test through its pid and start time. It needs a real record that verifies, so it cannot sit in the binary that never spawns.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reaping_a_group_the_supervisor_holds_leaves_it_held() {
    let records = tempfile::tempdir().expect("a temp dir");
    let recording = Arc::new(Recording::default());
    // TRAIT_OBJECT: the supervisor takes its signaller as a trait object
    // BORROW: .clone() shares the log with the supervisor
    let signaller: Arc<dyn Signaller> = recording.clone();
    let recorder = Arc::new(Recorder::default());
    // TRAIT_OBJECT: the supervisor takes its sink as a trait object
    // BORROW: .clone() shares the recorder with the supervisor
    let sink: Arc<dyn EventSink> = recorder.clone();
    let supervisor = Supervisor::new(sink).with_signaller(signaller);
    let child = supervisor
        .spawn(
            request(&["sleep", "30"], records.path())
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let (_, _, record) = only_record(records.path());
    let pid = record.pid();
    let _child = KillPids(vec![(pid, process_start(pid))]);
    assert!(supervisor.holds_group(pid), "the spawned group is not held");
    let report = supervisor
        .reap(None, &record, &stop_spec(None, 100))
        .await
        .expect("a report");
    assert!(report.was_alive);
    assert!(
        supervisor.holds_group(pid),
        "reap dropped a group the supervisor held"
    );
    drop(child);
}

/// The processes of `tests/fixtures/supervise/tree.sh`, as it wrote them.
struct Tree {
    leader: u32,
    group: u32,
    detached: u32,
}

impl Tree {
    /// Every process of the tree with its start time as the system reports it now.
    fn starts(&self) -> Vec<(u32, ProcessStart)> {
        [self.leader, self.group, self.detached]
            .into_iter()
            .map(|pid| (pid, process_start(pid)))
            .collect()
    }
}

/// The argv that runs `tree.sh`, writing its process ids to `pidfile`.
fn tree_argv(pidfile: &Path) -> Vec<String> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("supervise")
        .join("tree.sh");
    vec![
        "sh".to_owned(),
        script.to_str().expect("a UTF-8 path").to_owned(), // BORROW: .to_owned() makes the owned argument
        pidfile.to_str().expect("a UTF-8 path").to_owned(), // BORROW: .to_owned() makes the owned argument
    ]
}

/// A request for the tree, writing its process ids to `pidfile`.
fn tree_request(pidfile: &Path, records: &Path) -> SpawnRequestBuilder {
    let argv = tree_argv(pidfile);
    let parts: Vec<&str> = argv.iter().map(String::as_str).collect();
    request(&parts, records)
}

/// Waits for the `ready` line of the tree and reads its process ids, for at most 10 s.
async fn read_tree(pidfile: &Path) -> Tree {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let text = fs::read_to_string(pidfile).unwrap_or_default();
        if text.lines().any(|line| line == "ready") {
            let pid_of = |label: &str| -> u32 {
                text.lines()
                    .find_map(|line| line.strip_prefix(label)?.trim().parse().ok())
                    .unwrap_or_else(|| panic!("no {label} line in {text:?}"))
            };
            return Tree {
                leader: pid_of("leader "),
                group: pid_of("group "),
                detached: pid_of("detached "),
            };
        }
        assert!(Instant::now() < until, "the tree never became ready");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Whether the process `pid` is gone, as the system answers a signal-0 probe, or is another
/// process than the one that started at `before`.
fn gone(pid: u32, before: &ProcessStart) -> bool {
    let Some(target) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return true;
    };
    match test_kill_process(target) {
        Ok(()) => process_start(pid) != *before,
        Err(error) => error == Errno::SRCH,
    }
}

/// Waits up to 5 s for every process of `starts` to be gone.
async fn all_gone(starts: &[(u32, ProcessStart)]) -> bool {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if starts.iter().all(|(pid, before)| gone(*pid, before)) {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Kills, when it goes out of scope, each process that still has the start time it was noted
/// with, so a failed assertion leaves no process. It signals one process id at a time, never a
/// group, and never 0, 1 or the test process.
struct KillPids(Vec<(u32, ProcessStart)>);

impl Drop for KillPids {
    fn drop(&mut self) {
        for (pid, start) in &self.0 {
            let own = std::process::id();
            let target = i32::try_from(*pid).ok().and_then(Pid::from_raw);
            if let Some(target) = target
                && *pid > 1
                && *pid != own
                // An unreadable start equals any other unreadable start, so it identifies nothing.
                && *start != ProcessStart::Unavailable
                && process_start(*pid) == *start
            {
                // EXPLICIT: single-pid kill of a process this test spawned, re-verified by its start time
                let _ = kill_process(target, Signal::KILL);
            }
        }
    }
}

/// Guards: R01 §escalation (no survivor in the group) — after `Child::stop` of a tree that ignores `TERM`, and after `reap` of a second such tree from its record alone, every process that was in the group at spawn is gone, and the group is empty. Oracle: `kill(pid, 0)` answering `ESRCH` or the OS start time of the pid differing from the one read at spawn, polled for 5 s; the process ids are those `tree.sh` wrote.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stopped_and_a_reaped_tree_leave_no_survivor() {
    let work = tempfile::tempdir().expect("a temp dir");

    // Stopped through its handle.
    let records = tempfile::tempdir().expect("a temp dir");
    let pidfile = work.path().join("stopped.pids");
    let (stopper, _events) = supervisor();
    let child = stopper
        .spawn(
            tree_request(&pidfile, records.path())
                .stop(stop_spec(None, 300))
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&stopper);
    let tree = read_tree(&pidfile).await;
    let starts = tree.starts();
    let _detached = KillPids(vec![(tree.detached, process_start(tree.detached))]);
    let in_group: Vec<(u32, ProcessStart)> = starts
        .iter()
        .filter(|(pid, _)| *pid != tree.detached)
        .cloned() // BORROW: .cloned() copies the starts for the poll
        .collect();
    assert_eq!(in_group.len(), 2);
    let exit = child.stop().await.expect("an exit");
    assert_eq!(exit.ended, Ended::Stopped);
    assert!(exit.group_empty, "the group still has members");
    assert!(
        all_gone(&in_group).await,
        "a process of the group survived the stop"
    );

    // Reaped from its record alone: the handle is dropped, as it would be after a crash.
    let records = tempfile::tempdir().expect("a temp dir");
    let pidfile = work.path().join("reaped.pids");
    let (spawner, _events) = supervisor();
    let child = spawner
        .spawn(
            tree_request(&pidfile, records.path())
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _spawner_cleanup = KillAll(&spawner);
    let tree = read_tree(&pidfile).await;
    let _detached = KillPids(vec![(tree.detached, process_start(tree.detached))]);
    let in_group: Vec<(u32, ProcessStart)> = tree
        .starts()
        .into_iter()
        .filter(|(pid, _)| *pid != tree.detached)
        .collect();
    let (_, _, record) = only_record(records.path());
    drop(child);
    let (reaper, _events) = supervisor();
    let report = reaper
        .reap(Some(&run_key()), &record, &stop_spec(None, 300))
        .await
        .expect("a report");
    assert!(report.was_alive);
    assert_eq!(report.refusal, None);
    assert!(report.group_empty, "the group still has members");
    assert!(
        all_gone(&in_group).await,
        "a process of the group survived the reap"
    );
}

/// A sweeper that kills the process `tree.sh` wrote as `detached`, if it still has the start time
/// it was noted with, and notes the marker of every sweep. It signals one process id, never a
/// group.
struct Killer {
    pidfile: PathBuf,
    noted: Mutex<Option<(u32, ProcessStart)>>,
    markers: Mutex<Vec<String>>,
}

impl StraySweeper for Killer {
    fn sweep(&self, marker: &Marker) -> Vec<StrayProcess> {
        self.markers
            .lock()
            .expect("a lock")
            .push(marker.to_string());
        let Some((pid, start)) = self.noted.lock().expect("a lock").clone() else {
            // BORROW: .clone() copies the noted process out of the lock
            return Vec::new();
        };
        let text = fs::read_to_string(&self.pidfile).unwrap_or_default();
        let written = text
            .lines()
            .any(|line| line.strip_prefix("detached ") == Some(pid.to_string().as_str()));
        let target = i32::try_from(pid).ok().and_then(Pid::from_raw);
        match target {
            Some(target)
                if written
                    && pid > 1
                    && pid != std::process::id()
                    && start != ProcessStart::Unavailable
                    && process_start(pid) == start
                    // EXPLICIT: single-pid kill of a process this test spawned, re-verified by its start time
                    && kill_process(target, Signal::KILL).is_ok() =>
            {
                StrayProcess::new(pid, start).into_iter().collect()
            }
            Some(_) | None => Vec::new(),
        }
    }
}

/// Guards: R01 §escalation (the sweep) and G3 — the injected sweeper is called once per stop, after `kill_group`, with the run's marker; the process that left the group is then gone and a `run.stray` event names it, and every envelope validates against the published schema. Oracle: the call log of the test's sweeper, the literal event order written here, the schema file, and `kill(pid, 0)`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_sweeper_is_called_once_after_kill_group_and_run_stray_names_its_kill() {
    let work = tempfile::tempdir().expect("a temp dir");
    let records = tempfile::tempdir().expect("a temp dir");
    let pidfile = work.path().join("tree.pids");
    let killer = Arc::new(Killer {
        pidfile: pidfile.clone(), // BORROW: .clone() gives the sweeper its own path
        noted: Mutex::new(None),
        markers: Mutex::new(Vec::new()),
    });
    let recorder = Arc::new(Recorder::default());
    // TRAIT_OBJECT: the supervisor takes its sink and sweeper as trait objects
    // BORROW: .clone() shares the recorder and the sweeper with the supervisor
    let (sink, sweeping): (Arc<dyn EventSink>, Arc<dyn StraySweeper>) =
        (recorder.clone(), killer.clone());
    let supervisor = Supervisor::new(sink).with_sweeper(sweeping);
    let child = supervisor
        .spawn(
            tree_request(&pidfile, records.path())
                .run(run_key())
                .stop(stop_spec(None, 300))
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    let tree = read_tree(&pidfile).await;
    let detached = (tree.detached, process_start(tree.detached));
    *killer.noted.lock().expect("a lock") = Some(detached.clone()); // BORROW: .clone() gives the sweeper its own copy
    let _detached = KillPids(vec![detached.clone()]); // BORROW: .clone() gives the cleanup its own copy
    let exit = child.stop().await.expect("an exit");
    assert!(exit.group_empty);

    assert_eq!(
        *killer.markers.lock().expect("a lock"),
        [MARKER.to_owned()],
        "the sweeper was not called once with the marker"
    );
    assert!(all_gone(&[detached]).await, "the detached process survived");
    let all = envelopes(&recorder);
    assert_eq!(
        shape(&all),
        [
            "process.spawned",
            "process.stop_step:signal",
            "process.stop_step:terminate_group",
            "process.stop_step:kill_group",
            "process.stop_step:sweep",
            "run.stray",
            "process.exited",
        ]
    );
    let validator = envelope_validator();
    for envelope in &all {
        let problems: Vec<String> = validator
            .iter_errors(envelope)
            .map(|problem| problem.to_string())
            .collect();
        assert!(problems.is_empty(), "{envelope}: {problems:?}");
    }
    let stray = all.get(5).expect("the run.stray event");
    let named: Vec<&Value> = stray
        .get("payload")
        .and_then(|payload| payload.get("processes"))
        .and_then(Value::as_array)
        .expect("a process list")
        .iter()
        .map(|process| at(process, &["pid"]))
        .collect();
    assert_eq!(named, [&json!(tree.detached)]);
}

/// Guards: R01 §escalation (a spawn that fails leaves nothing behind) — a program that cannot start leaves no `.jsonl.tmp` in the raw directory, and a program that starts leaves its `.jsonl.gz`. Oracle: the directory listing, with the successful spawn as the positive control.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_spawn_leaves_no_frame_file() {
    let records = tempfile::tempdir().expect("a temp dir");
    let raw = tempfile::tempdir().expect("a temp dir");
    let framed = || {
        OutputSpec::framed(
            raw.path().to_path_buf(),
            "stdout".parse::<StreamName>().expect("a stream name"),
            FrameMode::Lines,
        )
    };
    let names = || -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(raw.path())
            .expect("a directory")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    };
    let (supervisor, _events) = supervisor();
    let refused = supervisor.spawn(
        request(&["/nonexistent/candler-program"], records.path())
            .stdout(framed())
            .build()
            .expect("a request"),
    );
    assert!(refused.is_err(), "the program started");
    assert_eq!(names(), Vec::<String>::new());

    let child = supervisor
        .spawn(
            request(&["sh", "-c", "echo hi"], records.path())
                .stdout(framed())
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    child.wait().await.expect("an exit");
    assert_eq!(names(), ["stdout.jsonl.gz"]);
}

/// Guards: I22 — a group is forgotten when its leader is reaped and the group is empty, and kept while it has members. Oracle: the fake's call log across `kill_all`: nothing is sent to a forgotten group and exactly one `KILL` to a kept one. The fake signals nothing, so the member of the kept group is ended by the test through its pid and start time.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_group_is_forgotten_when_it_is_empty_and_kept_while_it_has_members() {
    let work = tempfile::tempdir().expect("a temp dir");
    let table: [(&str, &str, bool); 2] = [
        ("an empty group", "exit 0", false),
        (
            "a group with a member",
            "sleep 30 & echo $! > \"$1\"; exit 0",
            true,
        ),
    ];
    for (label, script, kept) in table {
        let records = tempfile::tempdir().expect("a temp dir");
        let member_file = work
            .path()
            .join(format!("{}.member", label.replace(' ', "_")));
        let recording = Arc::new(Recording::default());
        // TRAIT_OBJECT: the supervisor takes its signaller as a trait object
        // BORROW: .clone() shares the log with the supervisor
        let signaller: Arc<dyn Signaller> = recording.clone();
        let recorder = Arc::new(Recorder::default());
        // TRAIT_OBJECT: the supervisor takes its sink as a trait object
        // BORROW: .clone() shares the recorder with the supervisor
        let sink: Arc<dyn EventSink> = recorder.clone();
        let supervisor = Supervisor::new(sink).with_signaller(signaller);
        let member_arg = member_file.to_str().expect("a UTF-8 path");
        let child = supervisor
            .spawn(
                request(&["sh", "-c", script, "sh", member_arg], records.path())
                    .stop(stop_spec(None, 100))
                    .build()
                    .expect("a request"),
            )
            .expect("a child");
        let pid = child.record().pid();
        let exit = child.wait().await.expect("an exit");
        let member = fs::read_to_string(&member_file)
            .ok()
            .and_then(|text| text.trim().parse::<u32>().ok());
        let _member = KillPids(
            member
                .map(|pid| (pid, process_start(pid)))
                .into_iter()
                .collect(),
        );
        assert_eq!(exit.group_empty, !kept, "{label}");
        assert_eq!(supervisor.holds_group(pid), kept, "{label}");
        let before = recording.calls.lock().expect("a lock").len();
        supervisor.kill_all();
        let after = recording.calls.lock().expect("a lock").clone(); // BORROW: .clone() copies the log out of the lock
        let group = i32::try_from(pid).expect("a pid");
        let sent: Vec<&(i32, i32)> = after.iter().skip(before).collect();
        if kept {
            assert_eq!(sent, [&(group, Signal::KILL.as_raw())], "{label}");
        } else {
            assert_eq!(
                sent,
                Vec::<&(i32, i32)>::new(),
                "{label}: a forgotten group was signalled"
            );
        }
    }
}

/// Guards: R01 §escalation and the leaf (the deadline counts from the spawn) — a child spawned with a 2 s deadline whose `wait` is called 1.2 s later ends by the deadline within about 0.8 s of that call. Oracle: the wall clock around `wait`, with 0.9 s of slack below the 2 s a deadline counted from `wait` would take.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_deadline_counts_from_the_spawn() {
    let records = tempfile::tempdir().expect("a temp dir");
    let (supervisor, _events) = supervisor();
    // Taken before the spawn, so that the deadline cannot fall before it.
    let spawned = Instant::now();
    let child = supervisor
        .spawn(
            request(&["sleep", "30"], records.path())
                .deadline(Duration::from_millis(2000))
                .stop(stop_spec(None, 300))
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    tokio::time::sleep(Duration::from_millis(1200)).await;
    let called = Instant::now();
    let exit = child.wait().await.expect("an exit");
    let after_call = called.elapsed();
    assert_eq!(exit.ended, Ended::Deadline);
    assert!(
        spawned.elapsed() >= Duration::from_millis(2000),
        "ended early"
    );
    assert!(
        after_call < Duration::from_millis(1700),
        "the deadline was counted from wait: {after_call:?}"
    );
}

/// A sweeper that blocks for `block` and finds nothing.
struct Slow {
    block: Duration,
}

impl StraySweeper for Slow {
    fn sweep(&self, _marker: &Marker) -> Vec<StrayProcess> {
        std::thread::sleep(self.block);
        Vec::new()
    }
}

/// Guards: R01 §escalation (the sweep is bounded) — a sweeper that blocks for 2 s with a grace of 0.3 s does not hold `stop` for more than 1.5 s, and the `sweep` step says it timed out. Oracle: the wall clock around `stop` and the literal outcome written here.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sweeper_that_overruns_the_grace_is_not_waited_for() {
    let records = tempfile::tempdir().expect("a temp dir");
    let recorder = Arc::new(Recorder::default());
    // TRAIT_OBJECT: the supervisor takes its sink and sweeper as trait objects
    // BORROW: .clone() shares the recorder with the supervisor
    let (sink, sweeping): (Arc<dyn EventSink>, Arc<dyn StraySweeper>) = (
        recorder.clone(),
        Arc::new(Slow {
            block: Duration::from_secs(2),
        }),
    );
    let supervisor = Supervisor::new(sink).with_sweeper(sweeping);
    let child = supervisor
        .spawn(
            request(&["sleep", "30"], records.path())
                .stop(stop_spec(None, 300))
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    let began = Instant::now();
    let exit = child.stop().await.expect("an exit");
    let took = began.elapsed();
    assert!(
        took < Duration::from_millis(1500),
        "stop was held for {took:?}"
    );
    let sweep = exit.steps.last().expect("a sweep step");
    assert_eq!(sweep.outcome, "sweeper timed out; what it found is unknown");
}

/// Guards: R01 §escalation (the events of a child that exits by itself) — a child that exits with no stop reports `process.spawned`, the one `sweep` step, then `process.exited`, each validating against the published schema. Oracle: the literal ordered list written here.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_child_that_exits_by_itself_reports_spawn_sweep_exit() {
    let records = tempfile::tempdir().expect("a temp dir");
    let (supervisor, recorder) = supervisor();
    let child = supervisor
        .spawn(
            request(&["sh", "-c", "exit 0"], records.path())
                .run(run_key())
                .stop(stop_spec(None, 200))
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    let exit = child.wait().await.expect("an exit");
    assert_eq!(exit.ended, Ended::Exited);
    let all = envelopes(&recorder);
    assert_eq!(
        shape(&all),
        [
            "process.spawned",
            "process.stop_step:sweep",
            "process.exited"
        ]
    );
    let validator = envelope_validator();
    for envelope in &all {
        let problems: Vec<String> = validator
            .iter_errors(envelope)
            .map(|problem| problem.to_string())
            .collect();
        assert!(problems.is_empty(), "{envelope}: {problems:?}");
    }
}

/// Guards: R01 §escalation (the pre-stop command's group) — what a pre-stop command leaves in its own group after it exits is ended by the stop. Oracle: the pid the command wrote for its background `sleep 37`, polled for 5 s against `kill(pid, 0)`; the command's own exit is the only thing that would otherwise end it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pre_stop_command_leaves_nothing_in_its_group() {
    let records = tempfile::tempdir().expect("a temp dir");
    let ready = records.path().join("ready");
    let ready_arg = ready.to_str().expect("a UTF-8 path");
    let left = records.path().join("left");
    let left_arg = left.to_str().expect("a UTF-8 path");
    let command = CommandSpec::builder()
        .argv(
            ["sh", "-c", "sleep 37 & echo $! > \"$1\"", "sh", left_arg]
                .map(str::to_owned)
                .to_vec(),
        )
        .build()
        .expect("a command");
    let (supervisor, _events) = supervisor();
    let child = supervisor
        .spawn(
            request(&stubborn(ready_arg), records.path())
                .stop(stop_spec(Some(Action::Command(command)), 400))
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    until_ready(&ready).await;
    child.stop().await.expect("an exit");
    let pid: u32 = fs::read_to_string(&left)
        .expect("the command wrote a pid")
        .trim()
        .parse()
        .expect("a pid");
    let start = process_start(pid);
    let _left = KillPids(vec![(pid, start.clone())]); // BORROW: .clone() gives the cleanup its own start
    assert!(
        all_gone(&[(pid, start)]).await,
        "what the pre-stop command left survived the stop"
    );
}

/// Guards: Cond 2 and I22 (the child's environment is exactly the request's) — a child started with only `PATH` and a canary does not see the parent's `HOME`, sees the canary and the marker, and a pre-stop command sees the child's environment and not the parent's. Oracle: what the children print themselves; the test process's own `HOME`, asserted present so that the absence means something.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_parents_environment_reaches_neither_the_child_nor_its_pre_stop() {
    assert!(
        std::env::var_os("HOME").is_some(),
        "the test needs a HOME in its own environment"
    );
    let records = tempfile::tempdir().expect("a temp dir");
    let (supervisor, _events) = supervisor();
    let seen = supervisor
        .run_captured(
            request(
                &[
                    "sh",
                    "-c",
                    "printf '%s|%s|%s' \"${HOME-unset}\" \"${CANARY-unset}\" \"${CANDLER_RUN-unset}\"",
                ],
                records.path(),
            )
            .env(vec![
                ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
                ("CANARY".to_owned(), "from-request".to_owned()),
            ])
            .build()
            .expect("a request"),
            1024,
        )
        .await
        .expect("a probe");
    assert_eq!(
        String::from_utf8_lossy(&seen.stdout),
        format!("unset|from-request|{MARKER}")
    );

    let ready = records.path().join("ready");
    let ready_arg = ready.to_str().expect("a UTF-8 path");
    let printed = records.path().join("printed");
    let printed_arg = printed.to_str().expect("a UTF-8 path");
    let command = CommandSpec::builder()
        .argv(
            [
                "sh",
                "-c",
                "printf '%s|%s' \"${HOME-unset}\" \"${CANARY-unset}\" > \"$1\"",
                "sh",
                printed_arg,
            ]
            .map(str::to_owned)
            .to_vec(),
        )
        .build()
        .expect("a command");
    let child = supervisor
        .spawn(
            request(&stubborn(ready_arg), records.path())
                .env(vec![
                    ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
                    ("CANARY".to_owned(), "from-request".to_owned()),
                ])
                .stop(stop_spec(Some(Action::Command(command)), 300))
                .build()
                .expect("a request"),
        )
        .expect("a child");
    let _cleanup = KillAll(&supervisor);
    until_ready(&ready).await;
    child.stop().await.expect("an exit");
    assert_eq!(
        fs::read_to_string(&printed).expect("the command printed"),
        "unset|from-request"
    );
}
