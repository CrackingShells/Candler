#![cfg(test)]
//! Guards for run planning: that a resume deletes nothing, replays the stored cell and records
//! every forced deviation. Every check of stored files reads them with `std::fs` and
//! `serde_json`, never through the planner.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use candler_core::runner::plan::{Force, PlanCtx, PlanMode, plan_runs};
use serde_json::{Value, json};
use support::run_plan::{
    Facts, Run, attempts_on_disk, experiment, fresh_store, manifest, resolution, snapshot,
    write_run,
};

type Files = BTreeMap<PathBuf, Vec<u8>>;

/// The paths whose bytes changed, the paths that vanished and the paths that appeared.
fn diff(
    before: &Files,
    after: &Files,
) -> (BTreeSet<PathBuf>, BTreeSet<PathBuf>, BTreeSet<PathBuf>) {
    let changed = before
        .iter()
        .filter(|(path, bytes)| after.get(*path).is_some_and(|now| now != *bytes))
        .map(|(path, _)| path.clone())
        .collect();
    let removed = before
        .keys()
        .filter(|p| !after.contains_key(*p))
        .cloned()
        .collect();
    let added = after
        .keys()
        .filter(|p| !before.contains_key(*p))
        .cloned()
        .collect();
    (changed, removed, added)
}

/// A run of `model` with the base facts.
fn run<'a>(
    model: &'a str,
    facts: &'a Facts,
    replicate: u32,
    attempt: u32,
    sealed: bool,
) -> Run<'a> {
    Run {
        model,
        replicate,
        attempt,
        facts,
        integrity: json!({"kind": "valid"}),
        sealed,
    }
}

/// The JSON of a stored manifest, read from disk.
fn manifest_on_disk(store_root: &std::path::Path, relative: &std::path::Path) -> Value {
    serde_json::from_slice(&fs::read(store_root.join(relative)).expect("a manifest file"))
        .expect("manifest JSON")
}

/// Guards: B26 (I11, I20) — a resume after two kills deletes nothing, seals each unfinished
/// attempt `invalid(interrupted)` and numbers the attempts of a cell and replicate 1, 2, 3.
/// Oracle: the file system, read by the test: the bytes of every file under `runs/` before and
/// after each plan, and a literal attempt list.
/// Bite: `remove_dir_all` on an attempt without `run.done` in `seal_open`.
#[test]
fn b26_resume_deletes_nothing_and_numbers_attempts() {
    let (_dir, store, lock) = fresh_store();
    let root = store.layout().root().to_path_buf();
    let facts = Facts::base();
    let models = ["m1", "m2"];
    for model in models {
        write_run(&store, &run(model, &facts, 1, 1, true));
        write_run(&store, &run(model, &facts, 2, 1, false));
    }
    let file = experiment(&models, 2);
    let res = resolution(&facts);
    let ctx = PlanCtx {
        store: store.clone(),
        experiment_lock: lock,
    };

    // First plan, over two sealed runs and two unfinished attempts.
    let before = snapshot(&root);
    let out = plan_runs(&ctx, &file, &res, Force::No, PlanMode::Hold).expect("first plan");
    drop(out.lock);
    let after = snapshot(&root);
    let (changed, removed, added) = diff(&before, &after);
    let unfinished: Vec<_> = models
        .iter()
        .map(|m| manifest(&run(m, &facts, 2, 1, false)).run().to_string())
        .collect();
    let want_changed: BTreeSet<PathBuf> = unfinished
        .iter()
        .map(|key| PathBuf::from(format!("runs/{key}/manifest.json")))
        .collect();
    let want_added: BTreeSet<PathBuf> = unfinished
        .iter()
        .map(|key| PathBuf::from(format!("runs/{key}/run.done")))
        .collect();
    assert!(removed.is_empty(), "nothing is deleted: {removed:?}");
    assert_eq!(
        changed, want_changed,
        "only the sealed attempts' manifests change"
    );
    assert_eq!(added, want_added, "only their run.done files appear");
    for path in &want_changed {
        let integrity = manifest_on_disk(&root, path)["integrity"].clone();
        assert_eq!(
            integrity,
            json!({"kind": "invalid", "reason": "interrupted"})
        );
    }
    assert_eq!(
        out.plans.iter().map(|p| p.attempt).collect::<Vec<_>>(),
        [2, 2]
    );
    assert_eq!(
        out.plans
            .iter()
            .map(|p| p.key.replicate())
            .collect::<Vec<_>>(),
        [2, 2]
    );
    assert_eq!(out.sealed.len(), 2, "the two valid sealed runs are skipped");

    // The supervisor is killed again: the new attempts are left unfinished.
    for model in models {
        write_run(&store, &run(model, &facts, 2, 2, false));
    }
    let before = snapshot(&root);
    let out = plan_runs(&ctx, &file, &res, Force::No, PlanMode::Hold).expect("second plan");
    drop(out.lock);
    let after = snapshot(&root);
    let (changed, removed, added) = diff(&before, &after);
    let second: Vec<_> = models
        .iter()
        .map(|m| manifest(&run(m, &facts, 2, 2, false)).run().to_string())
        .collect();
    let want_changed: BTreeSet<PathBuf> = second
        .iter()
        .map(|key| PathBuf::from(format!("runs/{key}/manifest.json")))
        .collect();
    assert!(removed.is_empty(), "nothing is deleted: {removed:?}");
    assert_eq!(
        changed, want_changed,
        "attempt 1 stays byte-identical, attempt 2 is sealed"
    );
    assert_eq!(added.len(), 2, "one run.done per sealed attempt");
    assert_eq!(
        out.plans.iter().map(|p| p.attempt).collect::<Vec<_>>(),
        [3, 3]
    );

    // The third attempts start; each cell and replicate holds 1, 2, 3 and no gap.
    for model in models {
        write_run(&store, &run(model, &facts, 2, 3, false));
        let cell = manifest(&run(model, &facts, 2, 1, false))
            .run()
            .cell()
            .hex()
            .to_owned();
        assert_eq!(
            attempts_on_disk(&root, &cell, 2),
            [1, 2, 3],
            "replicate 2 of {model}"
        );
        assert_eq!(
            attempts_on_disk(&root, &cell, 1),
            [1],
            "replicate 1 of {model}"
        );
    }
}
