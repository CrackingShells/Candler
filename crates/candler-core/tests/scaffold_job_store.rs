//! Class-A development tests of the job store; deleted by the last commit of the leaf.
#![cfg(test)]

mod support;

use std::fs;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::view::{ChildEntry, JobView};
use candler_core::store::layout::StoreLayout;
use candler_schemas::execution::JobState;
use support::job_store::{child_json, container_json, container_record, job_id, job_record};
use tempfile::TempDir;

#[test]
fn children_round_trip_and_unreadable() {
    let root = TempDir::new().expect("a store root");
    let layout = StoreLayout::new(root.path());
    let id = job_id(1);
    let dir =
        JobDir::create(&layout, id.clone(), &job_record(&id, JobState::Running)).expect("create");
    assert!(dir.control_dir().is_dir());
    dir.write_child(&container_record("abcdef012345abcd", &id, "r"))
        .expect("write child");
    let kids = dir.children_dir();
    fs::write(kids.join("4242.json"), child_json(4242, "m").to_string()).expect("write");
    fs::write(kids.join("bad.json"), "{\"schema\":\"child-record/1\"").expect("write");
    fs::write(kids.join("odd.json"), "{\"schema\":\"other/9\"}").expect("write");
    fs::write(kids.join("x.json.1.tmp"), "{}").expect("write");
    fs::write(
        kids.join("mode.json"),
        container_json("abcdef012345abcd", &id, "r").to_string(),
    )
    .expect("write");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(kids.join("mode.json"), fs::Permissions::from_mode(0)).expect("chmod");
    }
    fs::write(
        dir.control_dir().join("1.json"),
        child_json(1, "control").to_string(),
    )
    .expect("write");
    let entries = dir.children().expect("children");
    let kinds: Vec<&str> = entries
        .iter()
        .map(|e| match e {
            ChildEntry::Process(_) => "process",
            ChildEntry::Container(_) => "container",
            ChildEntry::Unreadable { .. } => "unreadable",
            _ => "?",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "process",
            "unreadable",
            "container",
            "unreadable",
            "unreadable",
            "unreadable"
        ]
    );
    dir.set_state(JobState::Ended).expect("set state");
    assert_eq!(dir.record().expect("record").state(), JobState::Ended);
}

use std::fs::OpenOptions;
use std::io::Write as _;
use std::process::Command;

use candler_core::jobs::events::{JobEvents, TailPos};
use candler_schemas::execution::EventPayload;

const WORKER_DIR: &str = "SCAFFOLD_JOB_STORE_DIR";
const WORKER_JOB: &str = "SCAFFOLD_JOB_STORE_JOB";

/// Run as a child of `two_writers_interleave`: appends 200 events.
#[test]
#[ignore = "a worker, run by two_writers_interleave"]
fn append_worker() {
    let (Ok(path), Ok(job)) = (std::env::var(WORKER_DIR), std::env::var(WORKER_JOB)) else {
        return;
    };
    let id = job.parse().expect("a job id");
    let dir = JobDir::new(id, path.into());
    let events = JobEvents::open(&dir).expect("open");
    for _ in 0..200 {
        events
            .append(None, EventPayload::job_started())
            .expect("append");
    }
}

#[test]
fn two_writers_interleave() {
    let root = TempDir::new().expect("a store root");
    let layout = StoreLayout::new(root.path());
    let id = job_id(2);
    let dir =
        JobDir::create(&layout, id.clone(), &job_record(&id, JobState::Running)).expect("create");
    let exe = std::env::current_exe().expect("exe");
    let spawn = || {
        Command::new(&exe)
            .args(["--ignored", "--exact", "append_worker"])
            .env(WORKER_DIR, dir.path())
            .env(WORKER_JOB, id.to_string())
            .spawn()
            .expect("spawn a worker")
    };
    let (mut a, mut b) = (spawn(), spawn());
    assert!(a.wait().expect("wait").success());
    assert!(b.wait().expect("wait").success());
    let events = JobEvents::open(&dir).expect("open");
    let tail = events.tail(TailPos(0)).expect("tail");
    let seqs: Vec<u64> = tail
        .events
        .iter()
        .map(candler_schemas::execution::Envelope::seq)
        .collect();
    assert_eq!(seqs, (0..400).collect::<Vec<u64>>());
}

#[test]
fn torn_line_is_cut_and_tail_resumes() {
    let root = TempDir::new().expect("a store root");
    let layout = StoreLayout::new(root.path());
    let id = job_id(3);
    let dir =
        JobDir::create(&layout, id.clone(), &job_record(&id, JobState::Running)).expect("create");
    let events = JobEvents::open(&dir).expect("open");
    events
        .append(None, EventPayload::job_started())
        .expect("append");
    let first = events.tail(TailPos(0)).expect("tail");
    assert_eq!(first.events.len(), 1);
    let path = dir.path().join("events.jsonl");
    OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open")
        .write_all(b"{\"v\":1,\"se")
        .expect("tear");
    let again = events.tail(first.next).expect("tail");
    assert!(again.events.is_empty());
    assert_eq!(again.next, first.next);
    let env = events
        .append(None, EventPayload::job_started())
        .expect("append");
    assert_eq!(env.seq(), 1);
    let fresh = events.tail(first.next).expect("tail");
    assert_eq!(fresh.events.len(), 1);
    assert!(events.health().is_ok());
}
