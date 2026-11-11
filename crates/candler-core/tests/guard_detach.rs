#![cfg(test)]
#![cfg(unix)]
//! Guards for the launcher's wait for a detached supervisor.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::JobEvents;
use candler_core::jobs::runtime::RuntimeError;
use candler_core::jobs::runtime::detach::{DetachArgs, detach};
use candler_core::store::Store;
use candler_core::store::layout::StoreLayout;
use candler_schemas::execution::{EventPayload, JobEnd, JobState};

mod support;

use support::job_store::{job_id, job_record};

/// A supervisor that is a shell script: it appends `lines` (the log of a job that began and
/// ended at once) to the job's log, if there are any, and exits, never holding the lock a launcher
/// could see. Returns what `detach` made of it.
fn detach_script(n: u64, lines: Option<&[EventPayload]>) -> Result<(), RuntimeError> {
    let scratch = tempfile::tempdir().expect("a directory");
    let root = scratch.path().join("store");
    Store::init(&root).expect("a store");
    let layout = StoreLayout::new(&root);

    // The lines of a log, written by the log itself in a job directory of its own.
    let other = job_id(n + 1);
    let other_dir = JobDir::create(
        &layout,
        other.clone(),
        &job_record(&other, JobState::Running),
    )
    .expect("a dir");
    let log = JobEvents::open(&other_dir).expect("a log");
    for payload in lines.unwrap_or_default() {
        log.append(None, payload.clone()).expect("an event");
    }
    let written = other_dir.path().join("events.jsonl");

    let id = job_id(n);
    let dir =
        JobDir::create(&layout, id.clone(), &job_record(&id, JobState::Running)).expect("a dir");
    let exe = scratch.path().join("supervisor.sh");
    fs::write(
        &exe,
        "#!/bin/sh\n[ -f \"$LINES\" ] && cat \"$LINES\" >> \"$JOB_DIR/events.jsonl\"\nexit 0\n",
    )
    .expect("a script");
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).expect("a mode");
    detach(DetachArgs {
        exe,
        job_dir: dir.path().to_path_buf(),
        spec: Vec::new(),
        env: vec![
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
            ("JOB_DIR".to_owned(), dir.path().display().to_string()),
            ("LINES".to_owned(), written.display().to_string()),
        ],
    })
    .map(|_| ())
}

/// Guards: E1 (the launcher) — a job that begins and ends between two looks of the launcher at its
/// lock was started: `detach` reports it from the job's log, not as "the supervisor did not
/// start". A supervisor that exits having written nothing still did not start.
///
/// Oracle: the log of the job, written by hand as `job.started` then `job.ended` (`failed`); the
/// supervisor is a script that never holds the lock.
///
/// Bite: the old check, which reads the lock and the exit only (the first row turns red).
#[test]
fn e1_detach_reports_a_job_that_ends_at_once_as_started() {
    let began = [
        EventPayload::job_started(),
        EventPayload::job_ended(JobEnd::Failed),
    ];
    let first = detach_script(0x0d01, Some(&began));
    assert!(first.is_ok(), "a job that ended at once started: {first:?}");
    let second = detach_script(0x0d11, None);
    assert!(
        matches!(second, Err(RuntimeError::DidNotStart { .. })),
        "a supervisor that wrote nothing did not start: {second:?}"
    );
}
