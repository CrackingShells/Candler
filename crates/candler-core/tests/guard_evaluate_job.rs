//! Guards for the evaluate job: its spec file keeps a run job's bytes, its log tells what it
//! committed, it refuses a suite that changed and a pool of zero, and (on Linux) it leaves no stray
//! of an evaluator behind and a lost one is recovered. The supervisor runs in a child process, as
//! the guards of the job runtime run theirs, because it makes itself a session leader and the
//! subreaper of its descendants. The plug-ins run for real, from `python/candler-evaluator` under
//! `uv run --frozen`.
#![cfg(test)]
#![cfg(unix)]

#[path = "support/job_evaluate.rs"]
mod job_evaluate;
#[path = "support/legacy_import.rs"]
mod legacy_import;
#[path = "support/legacy_rules.rs"]
mod legacy_rules;
#[path = "oracle/mod.rs"]
mod oracle;
mod support;

use std::collections::BTreeSet;
use std::fs;
use std::time::Duration;

use candler_core::evaluation::engine::RunSelection;
use candler_core::jobs::lock::{JobLock, LockState};
use candler_core::jobs::lost::{JobLiveness, classify};
use candler_core::jobs::model::{EvaluateSpec, JobSpecFile};
use candler_core::store::Store;
use candler_schemas::execution::{EventPayload, JobEnd};
use job_evaluate::{EvalRig, end_of, events_of, wait_exit};
use serde_json::Value;
use support::job_runtime::job_spec;

/// The re-exec entry of the supervisor child of the evaluate guards; it guards nothing itself. It
/// does nothing unless `CANDLER_TEST_EVALUATE_JOB` names a job.
///
/// Guards: none (re-exec entry of E1 and E3).
/// Oracle: none.
/// Bite: none.
#[test]
fn evaluate_child() {
    job_evaluate::evaluate_entry();
}

/// The re-exec entry of the recovery child of E3. It does nothing unless
/// `CANDLER_TEST_RECOVER_JOB` names a job.
///
/// Guards: none (re-exec entry of E3).
/// Oracle: none.
/// Bite: none.
#[test]
fn recover_child() {
    job_evaluate::recover_entry();
}

/// A spec of an evaluate job, written by hand, for the spec file guard.
fn evaluate_spec() -> EvaluateSpec {
    EvaluateSpec {
        suite: "/suites/ok.toml".into(),
        suite_digest: job_evaluate::digest_of(b"a suite"),
        plugin_dirs: vec!["/plugins/pool".into()],
        env_root: "/envs".into(),
        selection: RunSelection::Pending,
        request_deadline_secs: Some(30),
    }
}

/// Guards: E1 — a run job's spec file holds the bytes of its `JobSpec` and nothing around it, so
/// no digest a job record keeps moves; an evaluate spec round-trips and does not read as a run
/// spec. Oracle: the bytes of `serde_json::to_vec` of the bare `JobSpec`, the form `candler run`
/// wrote before the enum had a second arm.
/// Bite: tag the enum (`{"Run": …}`), which moves the run job's bytes.
#[test]
fn e1_a_run_spec_keeps_its_bytes_in_the_spec_file() {
    let bare = job_spec();
    let expected = serde_json::to_vec(&bare).expect("the bare spec serialises");
    let file = JobSpecFile::Run(bare);
    assert_eq!(
        serde_json::to_vec(&file).expect("the file serialises"),
        expected,
        "a run spec's file is the spec's bytes"
    );

    let spec = evaluate_spec();
    let written = serde_json::to_vec(&JobSpecFile::Evaluate(spec.clone())).expect("it serialises"); // BORROW: the test compares with the original
    assert_eq!(
        written,
        serde_json::to_vec(&spec).expect("the bare spec serialises"),
        "an evaluate spec's file is its bytes too"
    );
    let read: JobSpecFile = serde_json::from_slice(&written).expect("it reads back");
    assert_eq!(read, JobSpecFile::Evaluate(spec), "the round trip");
    assert!(
        !matches!(read, JobSpecFile::Run(_)),
        "an evaluate spec is not a run spec"
    );
    let reread: JobSpecFile = serde_json::from_slice(&expected).expect("a run spec reads back");
    assert!(
        matches!(reread, JobSpecFile::Run(_)),
        "a run spec is not an evaluate spec"
    );
}

/// The outcome files of the `pool` evaluator over every run of the store: their paths, and their
/// records as JSON.
fn pool_outcomes(store: &Store) -> (Vec<String>, Vec<Value>) {
    let mut paths = Vec::new();
    let mut records = Vec::new();
    for entry in store.runs().expect("the runs list") {
        for file in store.outcome_files(&entry.key).expect("the files list") {
            if file.evaluator.name() != "pool" {
                continue;
            }
            paths.push(
                store
                    .layout()
                    .outcome(&file.run, &file.evaluator, &file.request)
                    .display()
                    .to_string(),
            );
            for record in store.read_outcomes(&file).expect("the file reads") {
                records.push(serde_json::to_value(&record).expect("a record serialises"));
            }
        }
    }
    paths.sort();
    records.sort_by_key(ToString::to_string);
    (paths, records)
}

/// Guards: E1 — an evaluate job's log holds `job.started` first, then one `evaluation.planned`
/// whose `outcomes` is the number of records the job tries, then one `evaluation.outcome` for
/// each record of the store's outcome files, and `job.ended(completed)` last; the job's
/// `report.json` is the document the supervisor returned and names the files the store holds.
/// Oracle: the store's outcome files read back, and the job's `events.jsonl`.
/// Bite: commit outcomes and send no event; send no `evaluation.planned`, or send it after the
/// first outcome.
#[tokio::test]
async fn e1_the_evaluate_job_ends_with_one_outcome_event_per_committed_record() {
    let rig = EvalRig::new().await;
    let suite = rig.suite("suite_pool_ok.toml", &[]);
    let spec = rig.spec(&suite, RunSelection::Glob("*-r{6,12}-a1".to_owned()));
    let (job, dir) = rig.job(1);
    let mut child = rig.start(&job, &spec, "evaluate_child", None);
    let status = wait_exit(&mut child, Duration::from_secs(180)).expect("the job ends");
    assert!(status.success(), "{status:?}: {}", rig.child_err());

    let events = events_of(&dir);
    let kinds: Vec<&str> = events.iter().map(EventPayload::kind).collect();
    assert_eq!(kinds.first(), Some(&"job.started"));
    assert_eq!(kinds.last(), Some(&"job.ended"));
    assert_eq!(end_of(&events), Some(JobEnd::Completed));
    let planned: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, kind)| **kind == "evaluation.planned")
        .map(|(at, _)| at)
        .collect();
    assert_eq!(planned.len(), 1, "one `evaluation.planned`: {kinds:?}");
    let outcomes: Vec<&EventPayload> = events
        .iter()
        .filter(|event| event.kind() == "evaluation.outcome")
        .collect();
    let first_outcome = kinds
        .iter()
        .position(|kind| *kind == "evaluation.outcome")
        .expect("an outcome event");
    assert!(
        planned[0] < first_outcome,
        "the plan comes first: {kinds:?}"
    );

    let (files, mut records) = pool_outcomes(&rig.store);
    assert_eq!(files.len(), 2, "one file per run");
    assert_eq!(records.len(), 2, "one record per run");
    assert_eq!(outcomes.len(), records.len(), "one event per record");
    let EventPayload::EvaluationPlanned {
        outcomes: tried, ..
    } = &events[planned[0]]
    else {
        unreachable!("the position holds a planned event");
    };
    assert_eq!(usize::try_from(*tried).expect("a count"), records.len());
    let mut reported: Vec<Value> = outcomes
        .iter()
        .map(|event| {
            let EventPayload::EvaluationOutcome { record, .. } = event else {
                unreachable!("filtered on the kind");
            };
            serde_json::to_value(record.as_ref()).expect("a record serialises")
        })
        .collect();
    reported.sort_by_key(ToString::to_string);
    records.sort_by_key(ToString::to_string);
    assert_eq!(reported, records, "each event is a record of the store");

    let report: Value = serde_json::from_slice(
        &fs::read(dir.path().join("report.json")).expect("report.json is in the job directory"),
    )
    .expect("JSON");
    let returned = rig.returned();
    assert_eq!(returned["end"], "completed");
    assert_eq!(
        report, returned["document"],
        "the document the job returned"
    );
    let named: BTreeSet<String> = report["outcome_files"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|path| path.as_str().expect("a path").to_owned())
        .collect();
    assert_eq!(named, files.into_iter().collect::<BTreeSet<_>>());
    assert_eq!(JobLock::probe(&dir).expect("a probe"), LockState::Free);
    assert_eq!(
        classify(&dir).expect("a class"),
        JobLiveness::Ended(JobEnd::Completed)
    );
}

/// Guards: E1 — an evaluate job whose suite was edited after the spec was written ends `failed`
/// before any process starts, and so does one with `slots.evaluators = 0`, which says why and
/// never waits. Oracle: the job's log, the job's `children/` (no process record), the store (no
/// outcome file) and the error output of the supervisor.
/// Bite: leave the digest unchecked; read the zero as the default pool.
#[tokio::test]
async fn e1_the_evaluate_job_refuses_a_changed_suite_and_zero_slots() {
    let rig = EvalRig::new().await;
    let suite = rig.suite("suite_pool_ok.toml", &[]);

    // A suite edited after the spec was written.
    let spec = rig.spec(&suite, RunSelection::Glob("*-r{6,12}-a1".to_owned()));
    let mut text = fs::read_to_string(&suite).expect("the suite reads");
    text.push_str("# edited after the spec was written\n");
    fs::write(&suite, text).expect("the suite writes");
    let (job, dir) = rig.job(2);
    let mut child = rig.start(&job, &spec, "evaluate_child", None);
    let status = wait_exit(&mut child, Duration::from_secs(60)).expect("the job ends");
    assert!(status.success(), "{status:?}: {}", rig.child_err());
    assert_failed_before_any_process(&rig, &dir);
    assert!(
        rig.child_err().contains("changed"),
        "the reason is the suite: {}",
        rig.child_err()
    );

    // A pool of zero.
    fs::remove_file(rig.work.join("returned.json")).expect("the earlier result");
    let suite = rig.suite("suite_pool_ok.toml", &[]);
    let spec = rig.spec(&suite, RunSelection::Glob("*-r{6,12}-a1".to_owned()));
    let (job, dir) = rig.job(3);
    let mut child = rig.start(
        &job,
        &spec,
        "evaluate_child",
        Some("[slots]\nevaluators = 0\n"),
    );
    let status = wait_exit(&mut child, Duration::from_secs(60)).expect("the job ends, not hangs");
    assert!(status.success(), "{status:?}: {}", rig.child_err());
    assert_failed_before_any_process(&rig, &dir);
    assert!(
        rig.child_err().contains("no evaluator slot"),
        "{}",
        rig.child_err()
    );
}

/// Checks that the job of `dir` ended `failed`, planned nothing, started no process and committed
/// no outcome file.
fn assert_failed_before_any_process(rig: &EvalRig, dir: &candler_core::jobs::dir::JobDir) {
    let events = events_of(dir);
    assert_eq!(end_of(&events), Some(JobEnd::Failed));
    assert!(
        events
            .iter()
            .all(|event| event.kind() != "evaluation.planned"),
        "nothing was planned"
    );
    let records: Vec<_> = fs::read_dir(dir.children_dir())
        .map(|entries| entries.filter_map(Result::ok).collect())
        .unwrap_or_default();
    assert!(records.is_empty(), "a process record: {records:?}");
    assert_eq!(pool_outcomes(&rig.store).0, Vec::<String>::new(), "no file");
    assert_eq!(JobLock::probe(dir).expect("a probe"), LockState::Free);
    assert!(
        rig.returned()["error"].is_string(),
        "the supervisor returned an error: {}",
        rig.returned()
    );
}

// The guards below read `/proc`, so only Linux runs them: the subreaper, the orphan sweep and the
// recovery of a lost job.
#[cfg(target_os = "linux")]
use std::time::Instant;

#[cfg(target_os = "linux")]
use candler_core::supervise::platform::process_start;
#[cfg(target_os = "linux")]
use candler_schemas::process::ProcessStart;

#[cfg(target_os = "linux")]
use support::job_runtime::{all_gone, gone, wait_for};

/// Guards: E3 — a stray of an evaluator that left its group with `setsid` and ignores
/// SIGTERM is dead by the time the job has ended: the pid the kind wrote is gone, with the
/// start time read while it lived. Oracle: the OS (`kill(pid, 0)` and `/proc` start times),
/// the way the guard of the job runtime reads them.
/// Bite: no sweep at all (neither the marker sweep of each request nor the sweep of the
/// orphans), or no sweep and no subreaper. Either mechanism alone leaves the stray dead,
/// because the two overlap.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn e3_a_setsid_term_ignoring_stray_of_an_evaluator_is_dead_by_job_ended() {
    let rig = EvalRig::new().await;
    let pidfile = rig.work.join("stray.pids");
    let suite = rig.suite(
        "suite_pool_stray.toml",
        &[("@PIDFILE@", pidfile.display().to_string())],
    );
    let spec = rig.spec(&suite, RunSelection::Glob("*-r6-a1".to_owned()));
    let (job, dir) = rig.job(4);
    let mut child = rig.start(&job, &spec, "evaluate_child", None);

    // Read the start time of the stray while it lives.
    let mut strays: Vec<(u32, ProcessStart)> = Vec::new();
    let until = Instant::now() + Duration::from_secs(180);
    let status = loop {
        for line in fs::read_to_string(&pidfile).unwrap_or_default().lines() {
            if let Ok(pid) = line.trim().parse::<u32>()
                && !strays.iter().any(|(seen, _)| *seen == pid)
            {
                strays.push((pid, process_start(pid)));
            }
        }
        if let Some(status) = child.try_wait().expect("a wait") {
            break status;
        }
        assert!(Instant::now() < until, "the job did not end");
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(status.success(), "{status:?}: {}", rig.child_err());
    assert_eq!(
        end_of(&events_of(&dir)),
        Some(candler_schemas::execution::JobEnd::Completed)
    );

    let written: Vec<u32> = fs::read_to_string(&pidfile)
        .expect("the stray wrote its pid")
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect();
    assert_eq!(written.len(), 1, "one stray, for one run");
    assert_eq!(strays.len(), 1, "the guard saw the stray alive");
    assert!(
        all_gone(&strays, Duration::from_secs(5)),
        "the stray of the evaluator outlived the job"
    );
}

/// Guards: E3 — an evaluate job whose supervisor is killed while a request is in flight
/// leaves its lock free and no `job.ended`; the recovery of the job ends the evaluator
/// process from the process record the engine wrote in the job's `children/`, and the job
/// then reads as ended. The stray of a lost job is not asserted: the recovery has no marker
/// sweeper, as for a run job. Oracle: the OS for the pids, the job's lock and log.
/// Bite: the engine's records outside `children/`: the recovery finds no record and the
/// evaluator lives on.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn e3_a_lost_evaluate_job_is_recovered_and_its_evaluator_is_reaped() {
    let rig = EvalRig::new().await;
    let marks = rig.work.join("interval.log");
    let suite = rig.suite(
        "suite_pool_interval.toml",
        &[
            ("@FILE@", marks.display().to_string()),
            ("@SLEEP@", "120".to_owned()),
        ],
    );
    let spec = rig.spec(&suite, RunSelection::Glob("*-r6-a1".to_owned()));
    let (job, dir) = rig.job(5);
    let mut child = rig.start(&job, &spec, "evaluate_child", None);

    // A request is in flight once the plug-in has marked its start.
    let marked = wait_for(&marks, Duration::from_secs(180));
    let Some(marked) = marked else {
        panic!("no request started: {}", rig.child_err());
    };
    let python: u32 = marked
        .split_whitespace()
        .next()
        .and_then(|pid| pid.parse().ok())
        .expect("the plug-in wrote its pid");
    let mut tracked: Vec<(u32, ProcessStart)> = vec![(python, process_start(python))];
    for entry in fs::read_dir(dir.children_dir()).expect("the job's children") {
        let path = entry.expect("an entry").path();
        if path.extension().is_some_and(|ext| ext == "json")
            && let Some(pid) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.parse::<u32>().ok())
        {
            tracked.push((pid, process_start(pid)));
        }
    }
    assert!(
        tracked.len() >= 3,
        "the engine's records are in the job's children: {tracked:?}"
    );

    // The supervisor is lost: it is killed through the handle the test holds.
    child.kill().expect("the supervisor is ours to end");
    let _ = wait_exit(&mut child, Duration::from_secs(30)).expect("the supervisor ends");
    assert!(
        !gone(python, &tracked[0].1),
        "the evaluator outlives its supervisor"
    );
    assert_eq!(JobLock::probe(&dir).expect("a probe"), LockState::Free);
    assert_eq!(end_of(&events_of(&dir)), None, "no `job.ended`");
    assert_eq!(classify(&dir).expect("a class"), JobLiveness::Lost);

    let recovered = rig.recover(&job, "recover_child");
    assert!(recovered.contains("Recovered"), "{recovered}");
    assert!(
        all_gone(&tracked, Duration::from_secs(10)),
        "the recovery left an evaluator process: {recovered}"
    );
    assert!(
        matches!(classify(&dir).expect("a class"), JobLiveness::Ended(_)),
        "the recovered job reads as ended"
    );
}
