//! Guards for the job store: that a job's state is read from the OS lock and the event log, never
//! from a field, and that machine-wide slots are never over-granted. B01 re-executes this test
//! binary as two worker processes; each worker exits by itself after a fixed number of holds.
#![cfg(test)]

mod support;

use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::process::Command;
use std::time::Duration;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::lost::{JobLiveness, classify};
use candler_core::jobs::slots::{Holder, Slots};
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

/// The slots directory a B01 worker shares with its parent.
const SLOTS_DIR_VAR: &str = "GUARD_JOB_STORE_SLOTS_DIR";
/// The append-only log a B01 worker writes to.
const LOG_VAR: &str = "GUARD_JOB_STORE_LOG";
/// Which worker this is.
const WORKER_VAR: &str = "GUARD_JOB_STORE_WORKER";
/// The slot count the parent and the workers agree on.
const SLOT_COUNT: u32 = 2;
/// How many contenders each worker runs at once; with two workers that is twice the slots.
const TASKS_PER_WORKER: u32 = 2;
/// How many holds each contender makes before the worker exits by itself.
const HOLDS: u32 = 4;
/// How long a hold lasts: at least 100 ms.
const HOLD_FOR: Duration = Duration::from_millis(120);

/// Appends one whole line to the shared log with a single `write` on an `O_APPEND` descriptor.
fn log_line(log: &std::path::Path, line: &str) {
    OpenOptions::new()
        .append(true)
        .create(true)
        .open(log)
        .expect("open the log")
        .write_all(format!("{line}\n").as_bytes())
        .expect("append to the log");
}

/// The body of one contender: acquire, log `A`, hold, log `R` before releasing, and again.
async fn contend(slots: &Slots, log: &std::path::Path, worker: &str, task: u32) {
    for hold in 0..HOLDS {
        let id = format!("{worker}-{task}-{hold}");
        let holder = Holder {
            job: worker.to_owned(),
            run: Some(id.clone()),
        };
        let slot = slots.acquire(&holder).await.expect("acquire a slot");
        log_line(log, &format!("A {id}"));
        tokio::time::sleep(HOLD_FOR).await;
        log_line(log, &format!("R {id}"));
        drop(slot);
    }
}

/// A worker of `slots_are_never_over_granted`, run by the test as a separate process. It does
/// nothing when the variables are not set, and it exits by itself after its last hold.
#[test]
#[ignore = "a worker process of slots_are_never_over_granted"]
fn slot_worker() {
    let (Ok(dir), Ok(log), Ok(worker)) = (
        std::env::var(SLOTS_DIR_VAR),
        std::env::var(LOG_VAR),
        std::env::var(WORKER_VAR),
    ) else {
        return;
    };
    let slots = Slots::open(std::path::Path::new(&dir), SLOT_COUNT).expect("open the slots");
    let log = std::path::PathBuf::from(log);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("a runtime");
    runtime.block_on(async {
        let tasks = (0..TASKS_PER_WORKER).map(|task| contend(&slots, &log, &worker, task));
        futures::future::join_all(tasks).await;
    });
}

/// Guards: B01 — two real processes sharing one slots directory with `slots.count = 2` and four
/// contenders never hold more than 2 slots at once, and do hold 2 at once (a lock that never
/// grants proves nothing).
/// Oracle: the peak overlap computed here from an `O_APPEND` log in which each holder writes `A`
/// after it acquires and `R` before it releases; the code's own counters are not read.
/// Bite: in `Slots::try_acquire`, the loop `0..self.count` becomes `0..self.count + 1`.
#[test]
fn slots_are_never_over_granted() {
    let state = TempDir::new().expect("a state directory");
    let slots_dir = state.path().join("slots");
    let log = state.path().join("slots.log");
    let exe = std::env::current_exe().expect("this test binary");
    let spawn = |worker: &str| {
        Command::new(&exe)
            .args(["--ignored", "--exact", "slot_worker"])
            .env(SLOTS_DIR_VAR, &slots_dir)
            .env(LOG_VAR, &log)
            .env(WORKER_VAR, worker)
            .spawn()
            .expect("spawn a worker")
    };
    let mut first = spawn("w1");
    let mut second = spawn("w2");
    assert!(first.wait().expect("wait for w1").success(), "w1 failed");
    assert!(second.wait().expect("wait for w2").success(), "w2 failed");

    let text = fs::read_to_string(&log).expect("read the log");
    let (mut now, mut peak, mut acquired, mut released) = (0_i32, 0_i32, 0_u32, 0_u32);
    for line in text.lines() {
        match line.split_once(' ') {
            Some(("A", _)) => {
                now += 1;
                acquired += 1;
            }
            Some(("R", _)) => {
                now -= 1;
                released += 1;
            }
            _ => panic!("a line that is not `A <id>` or `R <id>`: {line:?}"),
        }
        peak = peak.max(now);
    }
    let expected = 2 * TASKS_PER_WORKER * HOLDS;
    assert_eq!(
        (acquired, released),
        (expected, expected),
        "every hold is logged"
    );
    assert!(
        peak >= 2,
        "liveness: the peak overlap was {peak}, so two slots were never held at once"
    );
    assert!(peak <= 2, "the peak overlap was {peak}, above the 2 slots");
}
