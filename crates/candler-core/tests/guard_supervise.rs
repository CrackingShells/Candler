//! Guards for the process supervisor: records, liveness, handles, events, stops and reaping.
#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use candler_core::supervise::platform::{Liveness, liveness};
use candler_core::supervise::spawn::{SpawnRequest, SpawnRequestBuilder};
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{Marker, RunKey};
use candler_schemas::process::ChildRecord;
use serde_json::{Value, json};

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
