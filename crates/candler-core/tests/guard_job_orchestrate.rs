//! Guards for job orchestration: what a job cap stops, and what it leaves alone.
//!
//! The job runs end to end over a store in a temporary directory, a PATH-fake docker client whose
//! `run` verb sleeps, a gateway that serves nothing and a harness that reads no output. The real
//! supervisor spawns and stops the fake runs; no container exists.
#![cfg(test)]

mod support;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use candler_core::jobs::model::{JobCause, JobControl, JobOutcome};
use candler_core::jobs::orchestrate::orchestrate_with;
use candler_core::runner::RunStatus;
use candler_schemas::execution::{InvalidReason, JobEnd};
use candler_schemas::primitives::RunKey;
use candler_schemas::trace::EndedBy;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use support::job_orchestrate::{Rig, spec};
use support::run_plan::{Facts, Run, manifest, snapshot, write_run};
use tokio::sync::watch;

/// The facts of every cell of the table: no secret is declared, so none is present.
fn facts() -> Facts {
    Facts {
        secrets: Vec::new(),
        ..Facts::base()
    }
}

/// A run of `model`, replicate 1, attempt 1, sealed valid when `sealed`.
fn row<'a>(model: &'a str, facts: &'a Facts) -> Run<'a> {
    Run {
        model,
        replicate: 1,
        attempt: 1,
        facts,
        integrity: json!({"kind": "valid"}),
        sealed: true,
        job: None,
    }
}

/// The key of the run of `model` that a job would plan.
fn key_of(model: &str, facts: &Facts) -> RunKey {
    manifest(&row(model, facts)).run().clone()
}

/// The lowercase hex of `bytes`.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// The sha256 of every file of the run directory of `key`, by path relative to the store.
fn hashes(root: &Path, key: &RunKey) -> BTreeMap<PathBuf, String> {
    let prefix = PathBuf::from("runs").join(key.to_string());
    snapshot(root)
        .into_iter()
        .filter(|(path, _)| path.starts_with(&prefix))
        .map(|(path, bytes)| (path, hex(&Sha256::digest(&bytes))))
        .collect()
}

/// The integrity of the manifest of `key`, read from disk.
fn integrity_on_disk(root: &Path, key: &RunKey) -> Value {
    let path = root
        .join("runs")
        .join(key.to_string())
        .join("manifest.json");
    let manifest: Value =
        serde_json::from_slice(&fs::read(path).expect("a manifest file")).expect("manifest JSON");
    manifest["integrity"].clone()
}

/// Runs the job of `rig` over `models` to its end.
fn run_job(rig: &Rig, models: &[&str], wall_s: u32) -> JobOutcome {
    let job = spec(models, &facts(), wall_s, 2);
    let (cancel, _keep) = watch::channel::<Option<JobCause>>(None);
    let ctl = JobControl {
        cancel: std::sync::Arc::new(cancel),
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    runtime.block_on(orchestrate_with(rig.runtime(), job, ctl, rig.host()))
}

/// The table: six cells of one replicate each, in the order they are planned.
const TABLE: [&str; 6] = ["m1", "m2", "m3", "m4", "m5", "m6"];

/// Guards: I20 — crossing a job cap gives invalid(budget_stop) on live runs only; sealed runs keep
/// their status; no new run starts.
/// Oracle: R01 §Budget; a literal run table.
/// Bite: apply the cap to all_runs instead of live_runs.
#[test]
#[expect(
    clippy::doc_markdown,
    reason = "the guard header names identifiers in the form the guard index reads"
)]
fn b34_cap_stops_live_runs_only() {
    // The literal table: m1 and m2 are sealed valid before the job; m3 and m4 are the two runs
    // in flight, because `max_parallel` is 2 and the fake `run` sleeps; m5 and m6 are pending.
    let facts = facts();
    let rig = Rig::new(30);
    let root = rig.store.layout().root().to_path_buf();
    for model in ["m1", "m2"] {
        write_run(&rig.store, &row(model, &facts));
    }
    let sealed = ["m1", "m2"].map(|model| key_of(model, &facts));
    let live = ["m3", "m4"].map(|model| key_of(model, &facts));
    let pending = ["m5", "m6"].map(|model| key_of(model, &facts));
    let before: Vec<_> = sealed.iter().map(|key| hashes(&root, key)).collect();
    assert!(
        before.iter().all(|files| !files.is_empty()),
        "the sealed runs hold files before the job"
    );

    // wall_s crossed: the job's cap is one second, the fake `run` sleeps thirty.
    let outcome = run_job(&rig, &TABLE, 1);

    assert_eq!(
        outcome.end,
        JobEnd::BudgetStop,
        "the job ends by its budget"
    );
    let mut ended: Vec<RunKey> = outcome.runs.iter().map(|run| run.key.clone()).collect();
    ended.sort();
    let mut want = live.to_vec();
    want.sort();
    assert_eq!(ended, want, "only the runs in flight ended");
    for run in &outcome.runs {
        assert_eq!(run.status, RunStatus::Invalid, "{} is invalid", run.key);
        assert_eq!(
            run.invalid,
            Some(InvalidReason::BudgetStop),
            "{} is invalid(budget_stop)",
            run.key
        );
        assert_eq!(
            run.ended_by,
            EndedBy::BudgetStop,
            "{} ended by the budget",
            run.key
        );
    }
    for key in &live {
        assert_eq!(
            integrity_on_disk(&root, key),
            json!({"kind": "invalid", "reason": "budget_stop"}),
            "the manifest of {key} is sealed invalid(budget_stop)"
        );
        assert!(
            root.join("runs")
                .join(key.to_string())
                .join("run.done")
                .is_file(),
            "{key} is sealed"
        );
    }
    let after: Vec<_> = sealed.iter().map(|key| hashes(&root, key)).collect();
    assert_eq!(
        before, after,
        "the sealed runs keep every file, byte for byte"
    );
    for key in &sealed {
        assert_eq!(
            integrity_on_disk(&root, key),
            json!({"kind": "valid"}),
            "{key} keeps its status"
        );
    }
    for key in &pending {
        assert!(
            !root.join("runs").join(key.to_string()).exists(),
            "no run directory exists for {key}: no new run started"
        );
    }
    assert_eq!(
        rig.runs_logged(),
        live.len(),
        "the fake `run` was called once per run in flight and never again"
    );

    // A control job under its cap changes nothing: every planned run starts and ends by itself.
    let control = Rig::new(0);
    let control_root = control.store.layout().root().to_path_buf();
    for model in ["m1", "m2"] {
        write_run(&control.store, &row(model, &facts));
    }
    let control_sealed = ["m1", "m2"].map(|model| key_of(model, &facts));
    let control_before: Vec<_> = control_sealed
        .iter()
        .map(|key| hashes(&control_root, key))
        .collect();
    let calm = run_job(&control, &TABLE, 600);
    assert_eq!(calm.end, JobEnd::Completed, "the control job completes");
    assert_eq!(
        calm.runs.len(),
        4,
        "the control job ran its four planned runs"
    );
    assert!(
        calm.runs
            .iter()
            .all(|run| run.invalid != Some(InvalidReason::BudgetStop)),
        "no control run is stopped by the budget"
    );
    assert_eq!(
        control.runs_logged(),
        4,
        "the fake `run` count is the table's"
    );
    let control_after: Vec<_> = control_sealed
        .iter()
        .map(|key| hashes(&control_root, key))
        .collect();
    assert_eq!(
        control_before, control_after,
        "the control's sealed runs are untouched"
    );
}
