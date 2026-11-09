//! Guards for the job runtime: a supervisor that is killed ends its job `killed` and leaves no
//! process behind, and a process that left its group is found through the subreaper.
#![cfg(test)]

use std::fs;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::{JobEvents, TailPos};
use candler_core::jobs::lock::{JobLock, LockState};
use candler_core::jobs::lost::{JobLiveness, classify};
use candler_core::jobs::model::{JobCause, JobControl};
use candler_core::jobs::runtime::session::reap_lost_jobs;
use candler_core::jobs::runtime::{JobEntry, supervise_entry};
use candler_core::jobs::slots::Slots;
use candler_core::runner::Runtime;
use candler_core::runner::evidence::provenance::EvalInfra;
use candler_core::store::Store;
use candler_core::supervise::Supervisor;
use candler_core::supervise::platform::process_start;
use candler_schemas::execution::{EventPayload, JobEnd, JobState};
use candler_schemas::process::ProcessStart;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::watch;

mod support;

use support::job_orchestrate::{EmptyTree, QuietDriver, Rig, Silent};
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

/// The directory the re-exec entry of B42 works in; the entry does nothing unless it is set.
const REAP_WORK: &str = "CANDLER_TEST_REAP_WORK";

/// Appends `text` to the event log of `dir`, as written.
fn append_log(dir: &JobDir, text: &str) {
    fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(dir.path().join("events.jsonl"))
        .expect("the log")
        .write_all(text.as_bytes())
        .expect("a log line");
}

/// The line of a `job.started` event of `job`, written by hand from the event envelope.
fn started_line(job: &str) -> String {
    format!(
        "{{\"v\":1,\"seq\":0,\"at\":\"2026-11-06T09:00:00Z\",\"job_id\":\"{job}\",\"kind\":\"job.started\",\"payload\":{{}}}}\n"
    )
}

/// The line of a `job.ended` event of `job`, written by hand from the event envelope.
fn ended_line(job: &str) -> String {
    format!(
        "{{\"v\":1,\"seq\":1,\"at\":\"2026-11-06T10:00:00Z\",\"job_id\":\"{job}\",\"kind\":\"job.ended\",\"payload\":{{\"reason\":\"completed\"}}}}\n"
    )
}

/// The re-exec entry of B42: it runs `reap_lost_jobs` as the job `own` over the store of
/// `CANDLER_TEST_REAP_WORK`, once with a control cancelled before the call and once with a live
/// one, and writes what it saw to `reaped.txt`. It runs in a child whose `PATH` holds only the
/// fake client, because the sweep looks for `docker` on `PATH`.
///
/// Guards: none (re-exec entry of B42).
/// Oracle: none.
/// Bite: none.
#[test]
fn reap_child() {
    let Ok(work) = std::env::var(REAP_WORK) else {
        return;
    };
    let work = PathBuf::from(work);
    let store = Store::open(&work.join("store")).expect("a store");
    let own = job_id(34);
    let sibling = JobDir::open(store.layout(), &job_id(35)).expect("the sibling");
    let supervisor = Arc::new(Supervisor::new(Arc::new(Silent)));
    let records = work.join("records");
    fs::create_dir_all(&records).expect("a directory");
    let slots_dir = work.join("slots");
    fs::create_dir_all(&slots_dir).expect("a directory");
    let docker = DockerCli::new(Arc::clone(&supervisor), ClientEnv::from_process(), records)
        .expect("the fake client on PATH");
    let runtime = Runtime {
        supervisor,
        store,
        sink: Arc::new(Silent),
        slots: Slots::open(&slots_dir, 4).expect("the slots"),
        docker: Arc::new(docker),
        isolation: None,
        gateway: None,
        job: own.clone(), // BORROW: the runtime owns its job id
        driver: Arc::new(QuietDriver),
        skills: Arc::new(EmptyTree),
        cells: BTreeMap::new(),
        eval_infra: EvalInfra::default(),
        planner_package: None,
    };
    let tokio_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let mut out: Vec<String> = Vec::new();

    let cancelled = JobControl {
        cancel: Arc::new(watch::channel(None).0),
    };
    cancelled.cancel.send_replace(Some(JobCause::Killed));
    let none = tokio_runtime.block_on(reap_lost_jobs(&runtime, &own, &cancelled));
    out.push(format!("cancelled-reports {}", none.len()));
    out.push(format!(
        "cancelled-sibling {:?}",
        classify(&sibling).expect("a class")
    ));

    let live = JobControl {
        cancel: Arc::new(watch::channel(None).0),
    };
    for (id, result) in tokio_runtime.block_on(reap_lost_jobs(&runtime, &own, &live)) {
        match result {
            Ok(report) => out.push(format!("report {id} {:?}", report.outcome)),
            Err(reason) => out.push(format!("report {id} error {reason}")),
        }
    }
    fs::write(work.join("reaped.txt"), out.join("\n")).expect("the result file");
}

/// Guards: B42 — the supervisor's sweep of lost jobs recovers a lost sibling and never the job
/// that runs it, leaves an ended job alone, and recovers nothing once its job is told to stop.
/// Oracle: one hand-built store of three jobs (the running job, a lost sibling and an ended job,
/// all with a `running` record, a free lock and a `job.started`), classified by the log and the
/// lock; the fake client's call log holds no destroying verb.
/// Bite: remove the `own` filter in `reap_lost_jobs`, or the `cancelled` check in its loop.
#[test]
fn b42_reap_lost_jobs_spares_own_and_stops_on_cancel() {
    let rig = Rig::new(0);
    let work = rig.dir.path();
    let own = rig.job.clone(); // BORROW: the test keeps the id to read the job back
    let layout = rig.store.layout();
    let own_dir = JobDir::open(layout, &own).expect("the own job");
    append_log(&own_dir, &started_line(&own.to_string()));
    let (sibling, ended) = (job_id(35), job_id(36));
    let mut dirs = Vec::new();
    for id in [&sibling, &ended] {
        // BORROW: the directory owns a copy of the id
        let dir = JobDir::create(layout, id.clone(), &job_record(id, JobState::Running))
            .expect("a job directory");
        append_log(&dir, &started_line(&id.to_string()));
        dirs.push(dir);
    }
    // INDEX: `dirs` was pushed above, the sibling then the ended job: two entries
    let (sibling_dir, ended_dir) = (&dirs[0], &dirs[1]);
    append_log(ended_dir, &ended_line(&ended.to_string()));
    // The controls: before the sweep, the running job and the sibling read as lost; the third is ended.
    assert_eq!(classify(&own_dir).expect("a class"), JobLiveness::Lost);
    assert_eq!(classify(sibling_dir).expect("a class"), JobLiveness::Lost);
    assert_eq!(
        classify(ended_dir).expect("a class"),
        JobLiveness::Ended(JobEnd::Completed)
    );

    // The client of the sweep: it logs each call and reports every network and image absent, as a
    // daemon that holds none of them does; it prints no container.
    let bin = work.join("sweep-bin");
    fs::create_dir_all(&bin).expect("a directory");
    let log = work.join("sweep-docker.log");
    let client = bin.join("docker");
    fs::write(
        &client,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\ncase \"$1\" in\nnetwork | image)\n    echo 'Error: No such object' >&2\n    exit 1\n    ;;\nesac\n",
            log.display()
        ),
    )
    .expect("a client");
    fs::set_permissions(&client, fs::Permissions::from_mode(0o755)).expect("a mode");
    let status = Command::new(std::env::current_exe().expect("this binary"))
        .args(["--exact", "reap_child", "--nocapture"])
        .env_clear()
        .env(REAP_WORK, work)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", work.join("home"))
        .env("XDG_STATE_HOME", work.join("state"))
        .stdin(Stdio::null())
        .output()
        .expect("the sweeping child");
    assert!(
        status.status.success(),
        "the sweeping child failed: {}{}",
        String::from_utf8_lossy(&status.stdout),
        String::from_utf8_lossy(&status.stderr)
    );
    let seen = fs::read_to_string(work.join("reaped.txt")).expect("the child's result");
    let lines: Vec<&str> = seen.lines().collect();

    // (4) A control cancelled before the call recovers nothing: the sibling is still lost.
    assert!(lines.contains(&"cancelled-reports 0"), "{seen}");
    assert!(lines.contains(&"cancelled-sibling Lost"), "{seen}");
    // (2) With a live control the sibling is recovered, and only the sibling.
    let reports: Vec<&&str> = lines.iter().filter(|l| l.starts_with("report ")).collect();
    assert_eq!(
        reports,
        [&format!("report {sibling} Recovered").as_str()],
        "{seen}"
    );
    assert!(matches!(
        classify(sibling_dir).expect("a class"),
        JobLiveness::Ended(JobEnd::Failed)
    ));
    // (1) The running job is never recovered, though it reads as lost.
    assert_eq!(classify(&own_dir).expect("a class"), JobLiveness::Lost);
    assert!(!seen.contains(&own.to_string()), "{seen}");
    // (3) The ended job is left alone.
    assert_eq!(
        classify(ended_dir).expect("a class"),
        JobLiveness::Ended(JobEnd::Completed)
    );

    // The fake client was asked, and nothing was destroyed through it.
    let calls = fs::read_to_string(&log).unwrap_or_default();
    assert!(!calls.is_empty(), "the recovery never asked the client");
    for line in calls.lines() {
        for destroying in ["rm", "stop", "kill", "prune"] {
            assert!(
                !line.split(' ').take(2).any(|word| word == destroying),
                "the sweep destroyed: {line}"
            );
        }
    }
}
