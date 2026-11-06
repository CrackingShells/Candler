#![cfg(test)]
//! Guards for run planning: that a resume deletes nothing, replays the stored cell and records
//! every forced deviation. Every check of stored files reads them with `std::fs` and
//! `serde_json`, never through the planner.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use candler_core::runner::plan::{Force, PlanCtx, PlanError, PlanMode, plan_runs};
use serde_json::{Value, json};
use support::run_plan::{
    Facts, Run, attempts_on_disk, experiment, fresh_store, manifest, resolution, snapshot,
    stored_cell, write_run,
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

/// One fact that changes between the stored cells and the fresh resolution.
struct Change {
    /// The row's name.
    name: &'static str,
    /// Applies the change to the facts the resolution observes.
    apply: fn(&mut Facts),
    /// The path the deviation names.
    path: &'static str,
    /// The stored value, as JSON.
    stored: Value,
    /// The observed value, as JSON.
    observed: Value,
}

/// The four facts a resume re-checks.
///
/// Provenance: the stored values are the ones `Facts::base()` writes into each manifest (image
/// of 64 `a`, Claude Code 2.1.0, a commit of 40 `1`, secret `API_KEY` present); the observed
/// values are the ones each `apply` writes.
fn changes() -> Vec<Change> {
    let base = Facts::base();
    vec![
        Change {
            name: "image ID",
            apply: |facts| facts.image = format!("sha256:{}", "b".repeat(64)),
            path: "isolation.image",
            stored: json!(format!("sha256:{}", "a".repeat(64))),
            observed: json!(format!("sha256:{}", "b".repeat(64))),
        },
        Change {
            name: "Claude Code version",
            apply: |facts| facts.version = "2.2.0".to_owned(),
            path: "harness.version",
            stored: json!("2.1.0"),
            observed: json!("2.2.0"),
        },
        Change {
            name: "secret absent",
            apply: |facts| facts.secrets.clear(),
            path: "secrets",
            stored: json!(base.secrets),
            observed: json!([]),
        },
        Change {
            name: "tag moved",
            apply: |facts| facts.commit = "2".repeat(40),
            path: "treatment",
            stored: json!({"name": "htn", "tag": "v1", "commit": "1".repeat(40)}),
            observed: json!({"name": "htn", "tag": "v1", "commit": "2".repeat(40)}),
        },
    ]
}

/// A store of two cells whose replicate 1 is sealed valid and whose replicate 2 is to start.
fn two_cells() -> (tempfile::TempDir, candler_core::store::Store, PathBuf) {
    let (dir, store, lock) = fresh_store();
    let facts = Facts::base();
    for model in ["m1", "m2"] {
        write_run(&store, &run(model, &facts, 1, 1, true));
    }
    (dir, store, lock)
}

/// Guards: B27 (I11, I3) — a resume replays the stored cell: a changed image ID, Claude Code
/// version, secret or treatment tag is refused without `--force`, naming the changed path, and a
/// refusal writes nothing; with nothing changed the resume plans, and with `--force` the plan
/// still carries the stored cell.
/// Oracle: literal paths and values, with the stored ones read back from hand-built manifests;
/// the bytes under `runs/` before and after.
/// Bite: skip the image-ID comparison in `check_facts`.
#[test]
fn b27_changed_fact_is_refused_and_the_cell_is_pinned() {
    let models = ["m1", "m2"];
    let file = experiment(&models, 2);
    let base = Facts::base();

    // Control: nothing changed, so the resume plans replicate 2 of each cell.
    let (_dir, store, lock) = two_cells();
    let ctx = PlanCtx {
        store,
        experiment_lock: lock,
    };
    let out = plan_runs(&ctx, &file, &resolution(&base), Force::No, PlanMode::Hold)
        .map_err(|e| e.to_string())
        .expect("the control plans");
    assert_eq!(out.plans.len(), 2, "one replicate left per cell");
    for (plan, model) in out.plans.iter().zip(models) {
        assert_eq!(
            plan.stored,
            Some(stored_cell(model, &base)),
            "the stored cell is replayed"
        );
        assert!(
            plan.deviations.is_empty(),
            "nothing changed, nothing deviates"
        );
    }
    drop(out);

    for change in changes() {
        let (_dir, store, lock) = two_cells();
        let root = store.layout().root().to_path_buf();
        let ctx = PlanCtx {
            store,
            experiment_lock: lock,
        };
        let mut observed = base.clone();
        (change.apply)(&mut observed);
        let res = resolution(&observed);

        // Refused without force, naming the path, and nothing is written.
        let before = snapshot(&root);
        let refusal = plan_runs(&ctx, &file, &res, Force::No, PlanMode::Hold);
        let Err(PlanError::Changed(deviations)) = refusal else {
            panic!("{}: expected a refusal", change.name);
        };
        let paths: BTreeSet<&str> = deviations.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(paths, BTreeSet::from([change.path]), "{}", change.name);
        for deviation in &deviations {
            let stored = deviation.stored.as_ref().map(|v| v.as_value().clone());
            let seen = deviation.observed.as_ref().map(|v| v.as_value().clone());
            assert_eq!(stored, Some(change.stored.clone()), "{}", change.name);
            assert_eq!(seen, Some(change.observed.clone()), "{}", change.name);
        }
        assert_eq!(
            snapshot(&root),
            before,
            "{}: a refusal writes nothing",
            change.name
        );

        // Pinned: forced, the plan still replays the stored cell, not the observed one.
        let out = plan_runs(&ctx, &file, &res, Force::Yes, PlanMode::Hold)
            .map_err(|e| e.to_string())
            .expect("a forced plan");
        assert_eq!(out.plans.len(), 2, "{}", change.name);
        for (plan, model) in out.plans.iter().zip(models) {
            assert_eq!(
                plan.stored,
                Some(stored_cell(model, &base)),
                "{}",
                change.name
            );
            let key = manifest(&run(model, &base, 1, 1, true))
                .run()
                .cell()
                .clone();
            assert_eq!(
                plan.key.cell(),
                &key,
                "{}: the stored digest keys the run",
                change.name
            );
        }
        drop(out);
    }
}

/// Guards: B28 (I11, I3) — with `--force` over each of the four changed facts, every new attempt
/// (two cells by two replicates) carries a `deviations[] {path, stored, observed}` entry, and the
/// bytes of the sealed runs never change.
/// Oracle: the literal path, stored and observed values of each change, and the bytes of every
/// sealed run's files before and after.
/// Bite: record the deviations on the first new attempt only.
#[test]
fn b28_forced_deviations_are_recorded_on_every_new_attempt() {
    let models = ["m1", "m2"];
    let file = experiment(&models, 2);
    let base = Facts::base();
    for change in changes() {
        // Each cell holds one sealed run that is invalid, so both replicates need a new attempt.
        let (_dir, store, lock) = fresh_store();
        let root = store.layout().root().to_path_buf();
        for model in models {
            let mut sealed = run(model, &base, 1, 1, true);
            sealed.integrity = json!({"kind": "invalid", "reason": "infrastructure"});
            write_run(&store, &sealed);
        }
        let ctx = PlanCtx {
            store,
            experiment_lock: lock,
        };
        let mut observed = base.clone();
        (change.apply)(&mut observed);

        let before = snapshot(&root);
        let out = plan_runs(
            &ctx,
            &file,
            &resolution(&observed),
            Force::Yes,
            PlanMode::Hold,
        )
        .map_err(|e| e.to_string())
        .expect("a forced plan");
        assert_eq!(
            out.plans.len(),
            4,
            "{}: two cells by two replicates",
            change.name
        );
        let attempts: BTreeSet<(u32, u32)> = out
            .plans
            .iter()
            .map(|p| (p.key.replicate(), p.attempt))
            .collect();
        assert_eq!(
            attempts,
            BTreeSet::from([(1, 2), (2, 1)]),
            "{}",
            change.name
        );
        for plan in &out.plans {
            assert_eq!(
                plan.deviations.len(),
                1,
                "{}: one deviation per attempt",
                change.name
            );
            let deviation = plan.deviations.first().expect("a deviation");
            assert_eq!(deviation.path.as_str(), change.path, "{}", change.name);
            let stored = deviation.stored.as_ref().map(|v| v.as_value().clone());
            let seen = deviation.observed.as_ref().map(|v| v.as_value().clone());
            assert_eq!(stored, Some(change.stored.clone()), "{}", change.name);
            assert_eq!(seen, Some(change.observed.clone()), "{}", change.name);
        }
        drop(out);

        // No sealed run's file changed, and none was added or removed.
        assert_eq!(
            snapshot(&root),
            before,
            "{}: planning writes no run file",
            change.name
        );
    }
}
