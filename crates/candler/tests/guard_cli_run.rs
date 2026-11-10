//! `candler run` starts a detached supervisor and returns; nothing marked outlives the job.
#![cfg(test)]

#[path = "support/cli_run.rs"]
mod cli_run;

use std::fs;
use std::time::Duration;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::lock::{JobLock, LockState};
use candler_core::store::layout::StoreLayout;
use candler_core::supervise::platform::process_start;
use candler_schemas::primitives::JobId;

use cli_run::{wait_for, world};

/// The word of a docker call that removes a container, built from parts.
fn remove_verb() -> String {
    ["r", "m"].concat()
}

/// Guards: I15, I16 — run returns at once, the supervisor outlives it holding the job lock, and
/// after job.ended nothing marked or unmatched remains.
///
/// Oracle: the OS; the PATH-fake docker log.
///
/// Bite: wait on the supervisor in the launcher (no detach).
#[test]
fn b35_run_returns_supervisor_outlives() {
    let world = world();
    let out = world.launch();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "the launcher exits 0: {:?}\nstdout: {stdout}\nstderr: {stderr}\ncalls:\n{}",
        out.status,
        world.calls()
    );
    let id: JobId = stdout.trim().parse().unwrap();
    let layout = StoreLayout::new(&world.store());
    let dir = JobDir::open(&layout, &id).unwrap();

    // The launcher has exited, and the supervisor named in job.json is alive, with its start.
    let record = dir.record().unwrap();
    let supervisor = record.supervisor();
    assert_ne!(supervisor.pid(), std::process::id());
    assert_eq!(
        process_start(supervisor.pid()),
        *supervisor.start(),
        "the supervisor named in job.json is alive with the recorded start time"
    );
    // A test-side try of the job lock reads held: the supervisor holds it.
    assert_eq!(
        JobLock::probe(&dir).unwrap(),
        LockState::Held,
        "the supervisor holds the job lock after the launcher has gone"
    );

    // The job ends, and the lock is released last.
    let events = dir.path().join("events.jsonl");
    let ended = wait_for(Duration::from_secs(90), || {
        fs::read_to_string(&events).is_ok_and(|text| text.contains("job.ended"))
            && JobLock::probe(&dir).unwrap() == LockState::Free
    });
    assert!(
        ended,
        "job.ended appears and the lock is released\ncalls:\n{}\nsupervisor.log:\n{}",
        world.calls(),
        fs::read_to_string(dir.path().join("supervisor.log")).unwrap_or_default()
    );

    // No process carries a marker of this job's: the gateway, the skill archive, a control
    // container of the supervisor, or a run key the fake saw.
    let calls = world.calls();
    let mut markers = vec![
        "CANDLER_RUN=gateway ".to_owned(),
        "CANDLER_RUN=skill-archive ".to_owned(),
        format!("CANDLER_RUN=control-{}-", supervisor.pid()),
    ];
    for key in cli_run::run_keys(&calls) {
        markers.push(format!("CANDLER_RUN={key} "));
    }
    let listing = cli_run::ps_with_environment();
    for line in listing.lines() {
        for marker in &markers {
            assert!(
                !line.contains(marker.as_str()),
                "a process marked by the job remains: {line}"
            );
        }
    }

    // Every container the fake handed out has a removal in its log.
    let handed = world.handed_out();
    assert!(!handed.is_empty(), "the job created containers\n{calls}");
    let verb = remove_verb();
    for id in &handed {
        assert!(
            calls
                .lines()
                .any(|line| line.contains(&format!(" {verb} ")) && line.ends_with(id.as_str())),
            "container {id} has no removal in the log\n{calls}"
        );
    }
    world.remove();
}
