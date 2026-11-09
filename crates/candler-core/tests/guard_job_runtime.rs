//! Guards for the job runtime: a supervisor that is killed ends its job `killed` and leaves no
//! process behind, and a process that left its group is found through the subreaper.
#![cfg(test)]

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::{JobEvents, TailPos};
use candler_core::jobs::lock::{JobLock, LockState};
use candler_core::jobs::lost::{JobLiveness, classify};
use candler_core::jobs::runtime::{JobEntry, supervise_entry};
use candler_core::store::Store;
use candler_core::supervise::platform::process_start;
use candler_schemas::execution::{EventPayload, JobEnd, JobState};
use candler_schemas::process::ProcessStart;

mod support;

use support::job_runtime::{
    ENV_JOB, ENV_STORE, ENV_WORK, HeldChild, all_gone, job_spec, killed_body, read_log, services,
    stub_docker, term_child, wait_for,
};
use support::job_store::{job_id, job_record};

/// The re-exec entry of the supervisor child that B32 and B33 start; it guards nothing itself. It
/// does nothing unless `CANDLER_TEST_SUPERVISOR_JOB` names a job: run on its own it is an empty
/// test.
///
/// Guards: none (re-exec entry of B32 and B33).
/// Oracle: none.
/// Bite: none.
#[test]
fn supervisor_child() {
    let Ok(job) = std::env::var(ENV_JOB) else {
        return;
    };
    let root = std::env::var(ENV_STORE).expect("a store");
    let store = Store::open(Path::new(&root)).expect("a store");
    let entry = JobEntry {
        store,
        job: job.parse().expect("a job id"),
        spec: job_spec(),
        services: services(),
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let outcome = runtime
        .block_on(supervise_entry(entry, killed_body))
        .expect("the job ends in order");
    assert_eq!(outcome.end, JobEnd::Killed);
}

/// The events of the job in `dir`, as kinds, and the reason of its last `job.ended`.
fn kinds_of(dir: &JobDir) -> (Vec<String>, Option<JobEnd>) {
    let tail = JobEvents::open(dir)
        .expect("the log")
        .tail(TailPos(0))
        .expect("the events");
    let kinds = tail
        .events
        .iter()
        // BORROW: the list owns the kinds
        .map(|envelope| envelope.event().kind().to_owned())
        .collect();
    let end = tail.events.iter().rev().find_map(|envelope| {
        if let EventPayload::JobEnded { reason, .. } = envelope.event() {
            Some(*reason)
        } else {
            None
        }
    });
    (kinds, end)
}

/// Guards: I15, I20 — SIGTERM delivers Killed to the body, its children die, job.ended(killed) is last whatever end the body returns, the lock is free.
/// Oracle: the OS and the literal event kinds.
/// Bite: the SIGTERM handler does not send Killed, or the runtime ends the job with the body's end.
#[test]
fn b32_sigterm_seals_killed() {
    let work = tempfile::tempdir().expect("a temp dir");
    let root = work.path().join("store");
    let store = Store::init(&root).expect("a store");
    let job = job_id(32);
    let dir = JobDir::create(
        store.layout(),
        job.clone(),
        &job_record(&job, JobState::Running),
    )
    .expect("a job directory");
    let bin = stub_docker(&work.path().join("bin"));
    let path = format!("{}:/usr/bin:/bin", bin.display());

    let mut child = HeldChild::new(
        Command::new(std::env::current_exe().expect("this binary"))
            .args(["--exact", "supervisor_child", "--nocapture"])
            .env_clear()
            .env(ENV_JOB, job.to_string())
            .env(ENV_STORE, &root)
            .env(ENV_WORK, work.path())
            .env("PATH", path)
            .env("HOME", work.path().join("home"))
            .env("XDG_STATE_HOME", work.path().join("state"))
            .stdin(Stdio::null())
            .stdout(fs::File::create(work.path().join("child.out")).expect("a log"))
            .stderr(fs::File::create(work.path().join("child.err")).expect("a log"))
            .spawn()
            .expect("the supervisor"),
    );
    let supervisor = child.id();

    let pids = wait_for(&work.path().join("children.pids"), Duration::from_secs(30));
    let Some(pids) = pids else {
        child.kill().expect("the supervisor is ours to end");
        child.wait().expect("a status");
        panic!(
            "the body never started its children: {}",
            fs::read_to_string(work.path().join("child.err")).unwrap_or_default()
        );
    };
    let children: Vec<(u32, ProcessStart)> = pids
        .lines()
        .filter_map(|line| line.strip_prefix("child ")?.trim().parse().ok())
        .map(|pid: u32| (pid, process_start(pid)))
        .collect();
    assert_eq!(children.len(), 2);

    // The control: while the job runs, its lock is held and its log has no `job.ended`.
    assert_eq!(JobLock::probe(&dir).expect("a probe"), LockState::Held);
    assert_eq!(classify(&dir).expect("a class"), JobLiveness::Running);
    assert_eq!(kinds_of(&dir).1, None);
    assert!(!work.path().join("killed-seen").exists());

    term_child(&child);
    let until = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().expect("a wait") {
            break status;
        }
        if Instant::now() >= until {
            child.kill().expect("the supervisor is ours to end");
            child.wait().expect("a status");
            panic!("the supervisor did not end after SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        status.success(),
        "the supervisor ended {status:?}: {}",
        fs::read_to_string(work.path().join("child.err")).unwrap_or_default()
    );

    assert!(
        work.path().join("killed-seen").exists(),
        "the body never saw Killed"
    );
    assert!(
        all_gone(&children, Duration::from_secs(5)),
        "a child of the body survived"
    );
    let (kinds, end) = kinds_of(&dir);
    assert_eq!(kinds.first().map(String::as_str), Some("job.started"));
    assert_eq!(kinds.last().map(String::as_str), Some("job.ended"));
    assert_eq!(end, Some(JobEnd::Killed));
    assert_eq!(JobLock::probe(&dir).expect("a probe"), LockState::Free);
    assert_eq!(
        classify(&dir).expect("a class"),
        JobLiveness::Ended(JobEnd::Killed)
    );

    // Every signal the log names went to a group the body spawned.
    let log = read_log(&work.path().join("signals.log"));
    assert!(
        !log.is_empty(),
        "the children were not stopped through the guard"
    );
    let spawned: Vec<i32> = children
        .iter()
        .map(|(pid, _)| i32::try_from(*pid).expect("a pid"))
        .collect();
    let own = i32::try_from(supervisor).expect("a pid");
    let test_group = i32::try_from(std::process::id()).expect("a pid");
    for (kind, target, _) in &log {
        assert_eq!(kind, "group", "a signal to one process: {target}");
        assert!(
            spawned.contains(target),
            "a signal to {target}, which the body did not spawn"
        );
        assert_ne!(*target, own);
        assert_ne!(*target, test_group);
    }
}

/// Guards: I15 — a setsid'd, TERM-ignoring descendant dies through the subreaper.
/// Oracle: the OS (kill(pid,0), /proc start time), M1's tree.sh fixture.
/// Bite: remove the `set_child_subreaper` call.
#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b33_setsid_descendant_dies() {
    use std::sync::Arc;

    use candler_core::jobs::runtime::session::{ZombieTick, become_subreaper};
    use candler_core::jobs::runtime::sweep::sweep_orphans;
    use candler_core::supervise::spawn::SpawnRequest;
    use candler_core::supervise::stop::Ended;
    use candler_core::supervise::{EventSink, Supervisor};
    use candler_harness::spec::{StopSignal, StopSpec};
    use support::job_runtime::{Recorder, marked_tree};

    /// The marker the orphan of the tree carries.
    const MARKER: &str = "b33-orphan";
    become_subreaper().expect("the subreaper role");
    let work = tempfile::tempdir().expect("a temp dir");
    let records = work.path().join("records");
    fs::create_dir_all(&records).expect("a record directory");
    let script = marked_tree(work.path(), MARKER);
    let pidfile = work.path().join("tree.pids");
    let recorder = Arc::new(Recorder::default());
    // TRAIT_OBJECT: the supervisor takes its sink as a trait object
    let sink: Arc<dyn EventSink> = recorder.clone(); // BORROW: the test reads the recorder too
    let supervisor = Supervisor::new(Arc::clone(&sink));
    let request = SpawnRequest::builder()
        .argv(vec![
            "sh".to_owned(),
            script.to_str().expect("a UTF-8 path").to_owned(), // BORROW: .to_owned() makes the owned argument
            pidfile.to_str().expect("a UTF-8 path").to_owned(), // BORROW: .to_owned() makes the owned argument
        ])
        // BORROW: .to_owned() makes the owned name and value
        .env(vec![("PATH".to_owned(), "/usr/bin:/bin".to_owned())])
        .marker(MARKER.parse().expect("a marker"))
        .record_dir(records)
        .stop(StopSpec::new(
            None,
            StopSignal::Term,
            Duration::from_millis(300),
        ))
        .build()
        .expect("a request");
    let child = supervisor.spawn(request).expect("a child");

    let until = Instant::now() + Duration::from_secs(10);
    let text = loop {
        let text = fs::read_to_string(&pidfile).unwrap_or_default();
        if text.lines().any(|line| line == "ready") {
            break text;
        }
        assert!(Instant::now() < until, "the tree never became ready");
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    let pid_of = |label: &str| -> u32 {
        text.lines()
            .find_map(|line| line.strip_prefix(label)?.trim().parse().ok())
            .unwrap_or_else(|| panic!("no {label} line in {text:?}"))
    };
    let (leader, group, detached) = (pid_of("leader "), pid_of("group "), pid_of("detached "));
    let tree: Vec<(u32, ProcessStart)> = [leader, group, detached]
        .into_iter()
        .map(|pid| (pid, process_start(pid)))
        .collect();

    // The tick the supervisor runs for the life of its job: it waits for the exited processes it
    // adopted, so that a group whose members were reparented to it can read as empty.
    let _tick = ZombieTick::start(&supervisor);
    let exit = child.stop().await.expect("an exit");
    assert_eq!(exit.ended, Ended::Stopped);
    // INDEX: `tree` is built above as leader, group, detached; three entries
    assert!(
        all_gone(&tree[..2], Duration::from_secs(5)),
        "a process of the group survived the stop"
    );
    // The control: the process that left the group is still there, and only the sweep can find it.
    // INDEX: `tree` is built above as leader, group, detached; three entries
    assert!(
        !all_gone(&tree[2..], Duration::from_millis(200)),
        "the detached process ended with its group"
    );

    let swept = tokio::task::spawn_blocking(move || {
        let swept = sweep_orphans(&supervisor, recorder.as_ref());
        (swept, recorder)
    })
    .await
    .expect("the sweep");
    assert!(
        swept.0.iter().any(|stray| stray.pid() == detached),
        "the sweep did not find the detached process"
    );
    assert!(
        all_gone(&tree, Duration::from_secs(5)),
        "a process of the tree survived the sweep"
    );
}
