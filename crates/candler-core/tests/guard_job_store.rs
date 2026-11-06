//! Guards for the job store: that a job's state is read from the OS lock and the event log, never
//! from a field, and that machine-wide slots are never over-granted. B01 re-executes this test
//! binary as two worker processes; each worker exits by itself after a fixed number of holds.
#![cfg(test)]

mod support;

use std::fs::{self, File, OpenOptions};
use std::io::Write as _;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::lost::{JobLiveness, classify};
use candler_schemas::execution::{JobEnd, JobState};
use rustix::fs::{FlockOperation, flock};
use support::job_store::{job_id, job_record};
use tempfile::TempDir;

/// A fresh job directory whose `job.json` says `running`, whatever the lock and the log say.
fn fresh_job(n: u64) -> (TempDir, JobDir, String) {
    let root = TempDir::new().expect("a store root");
    let layout = candler_core::store::layout::StoreLayout::new(root.path());
    let id = job_id(n);
    let dir = JobDir::create(&layout, id.clone(), &job_record(&id, JobState::Running))
        .expect("create the job directory");
    (root, dir, id.to_string())
}

/// A second descriptor on the job's `lock` file, locked exclusively by the test.
fn hold(dir: &JobDir) -> File {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.path().join("lock"))
        .expect("open the lock file");
    flock(&file, FlockOperation::NonBlockingLockExclusive).expect("the test takes the lock");
    file
}

/// Appends `text` to the job's event log as written, with no newline added.
fn write_log(dir: &JobDir, text: &str) {
    OpenOptions::new()
        .append(true)
        .create(true)
        .open(dir.path().join("events.jsonl"))
        .expect("open the log")
        .write_all(text.as_bytes())
        .expect("write the log");
}

/// The line of a `job.ended` event, written by hand from the R01 event envelope.
fn ended_line(job: &str, seq: u64) -> String {
    format!(
        "{{\"v\":1,\"seq\":{seq},\"at\":\"2026-11-06T10:00:00Z\",\"job_id\":\"{job}\",\"kind\":\"job.ended\",\"payload\":{{\"reason\":\"completed\"}}}}\n"
    )
}

/// The line of a `job.started` event, written by hand from the R01 event envelope.
fn started_line(job: &str) -> String {
    format!(
        "{{\"v\":1,\"seq\":0,\"at\":\"2026-11-06T09:00:00Z\",\"job_id\":\"{job}\",\"kind\":\"job.started\",\"payload\":{{}}}}\n"
    )
}

/// Guards: B02 — a job is derived as running (lock held, no `job.ended`), lost (lock free, no
/// `job.ended`, a torn last line included) or ended (any `job.ended`), and its `state` field is
/// never consulted: every row here keeps `state` at `running`.
/// Oracle: the R01 sentence "lost = lock free and no `job.ended` event", with a second real file
/// descriptor that holds or releases the `flock` and event lines written by hand.
/// Bite: in `JobLock::probe`, treat `EWOULDBLOCK` as free.
#[test]
fn lost_is_lock_free_and_no_job_ended() {
    // Row 1: held, no job.ended: running.
    let (_root, dir, job) = fresh_job(1);
    write_log(&dir, &started_line(&job));
    let held = hold(&dir);
    assert_eq!(classify(&dir).expect("classify"), JobLiveness::Running);

    // Row 1b: held, with a job.ended: the supervisor is still exiting, and ended is the safe read.
    write_log(&dir, &ended_line(&job, 1));
    assert_eq!(
        classify(&dir).expect("classify"),
        JobLiveness::Ended(JobEnd::Completed)
    );
    drop(held);

    // Row 2: free, no job.ended: lost.
    let (_root2, dir2, job2) = fresh_job(2);
    write_log(&dir2, &started_line(&job2));
    assert_eq!(classify(&dir2).expect("classify"), JobLiveness::Lost);

    // Row 3: free, with a job.ended: ended.
    let (_root3, dir3, job3) = fresh_job(3);
    write_log(&dir3, &started_line(&job3));
    write_log(&dir3, &ended_line(&job3, 1));
    assert_eq!(
        classify(&dir3).expect("classify"),
        JobLiveness::Ended(JobEnd::Completed)
    );

    // Row 4: free, the last line torn in the middle of a job.ended: lost.
    let (_root4, dir4, job4) = fresh_job(4);
    write_log(&dir4, &started_line(&job4));
    let torn = ended_line(&job4, 1);
    let (head, _) = torn.split_at(torn.len() / 2);
    write_log(&dir4, head);
    assert_eq!(classify(&dir4).expect("classify"), JobLiveness::Lost);

    // The hint is not consulted: a record that says `ended` over a lost job is still lost.
    let record = dir4.path().join("job.json");
    let text = fs::read_to_string(&record).expect("read job.json");
    fs::write(&record, text.replace("running", "ended")).expect("rewrite job.json");
    assert_eq!(classify(&dir4).expect("classify"), JobLiveness::Lost);
}
