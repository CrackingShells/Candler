#![cfg(test)]
//! Guards for run planning: that a resume deletes nothing, replays the stored cell and records
//! every forced deviation. Every check of stored files reads them with `std::fs` and
//! `serde_json`, never through the planner.

use crate::support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::lock::JobLock;
use candler_core::runner::plan::{
    CellId, Force, Levels, PlanError, PlanMode, check_facts, plan_runs,
};
use candler_core::runner::{run_order, seed_of};
use candler_schemas::primitives::{Digest, JobId};
use serde_json::{Value, json};
use support::run_plan::{
    Facts, Run, attempts_on_disk, digest_text, experiment, fresh_store, manifest, plan_ctx,
    resolution, snapshot, stored_cell, write_run,
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
        .map(|(path, _)| path.clone()) // BORROW: the result owns its paths
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
        job: None,
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
/// Rows for the experiment lock and for a live job: a second planner is refused with the bytes
/// unchanged, a leftover lock file whose flock is free plans, a job that holds its lock keeps its
/// unfinished attempt unsealed, and a full attempt counter is refused.
/// Bite: `remove_dir_all` on an attempt without `run.done` in `seal_open`; `take_lock` without the
/// flock; `job_is_live` that is always false.
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
    let ctx = plan_ctx(&store, &lock, &facts);

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
        // INDEX: every sealed manifest has an `integrity` member
        let integrity = manifest_on_disk(&root, path)["integrity"].clone(); // BORROW: the integrity member is copied out of the parsed manifest
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
            .to_owned(); // BORROW: the cell hex is owned
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

    b26_lock_rows();
    b26_group_row();
    b26_liveness_rows();
    b26_overflow_row();
}

/// Lock row: a second `Hold` and a `Check` are refused while one planner holds the experiment, and
/// a lock file left behind with a free flock plans.
fn b26_lock_rows() {
    let (_dir, store, lock) = two_cells();
    let root = store.layout().root().to_path_buf();
    let facts = Facts::base();
    let file = experiment(&["m1", "m2"], 2);
    let res = resolution(&facts);
    let ctx = plan_ctx(&store, &lock, &facts);
    let held = plan_runs(&ctx, &file, &res, Force::No, PlanMode::Hold).expect("the first planner");
    let before = snapshot(&root);
    for mode in [PlanMode::Hold, PlanMode::Check] {
        let refusal = plan_runs(&ctx, &file, &res, Force::No, mode);
        assert!(
            matches!(refusal, Err(PlanError::ExperimentLocked)),
            "a held experiment refuses {mode:?}"
        );
        assert_eq!(
            snapshot(&root),
            before,
            "a refusal writes nothing under runs/"
        );
    }
    drop(held);
    assert!(lock.exists(), "the lock file stays behind");
    let again = plan_runs(&ctx, &file, &res, Force::No, PlanMode::Hold);
    assert!(
        again.is_ok(),
        "a leftover lock file with a free flock plans"
    );
}

/// Group row.
///
/// Guards: B26 (I11) — two planners over one stored group, each with an experiment lock
/// path of its own, never both hold the group: the second is refused under `Hold` and `Check`,
/// and plans once the first lets go.
/// Oracle: the stored group's digest `e` differs from the fresh digest, so the planners share
/// no fresh lock file; the refusal is `PlanError::ExperimentLocked`.
/// Bite: delete the group-lock block of `plan_runs` (`if experiment != fresh_experiment`).
fn b26_group_row() {
    let (_dir, store, lock) = two_cells();
    let facts = Facts::base();
    let file = experiment(&["m1", "m2"], 2);
    let res = resolution(&facts);
    let first = plan_ctx(&store, &lock, &facts);
    let mut second = plan_ctx(&store, &lock, &facts);
    second.experiment_lock = lock.with_file_name("other-experiment.lock");
    let held = plan_runs(&first, &file, &res, Force::No, PlanMode::Hold).expect("the first");
    for mode in [PlanMode::Hold, PlanMode::Check] {
        let refusal = plan_runs(&second, &file, &res, Force::No, mode);
        assert!(
            matches!(refusal, Err(PlanError::ExperimentLocked)),
            "a held stored group refuses {mode:?} under another fresh lock path"
        );
    }
    drop(held.lock);
    let again = plan_runs(&second, &file, &res, Force::No, PlanMode::Hold);
    assert!(again.is_ok(), "the group plans once the first lock drops");
}

/// Liveness row: an unfinished attempt whose job holds its lock on another descriptor refuses the
/// plan and is not sealed; once the lock is free it is sealed, and so is one whose job directory
/// is gone.
fn b26_liveness_rows() {
    let (_dir, store, lock) = fresh_store();
    let root = store.layout().root().to_path_buf();
    let facts = Facts::base();
    // Provenance: lowercase hyphenated UUIDv7 literals.
    let live_text = "0190a5e0-7c3a-7000-8000-000000000001";
    let live: JobId = live_text.parse().expect("a job id");
    for model in ["m1", "m2"] {
        write_run(&store, &run(model, &facts, 1, 1, true));
    }
    let mut owned = run("m1", &facts, 2, 1, false);
    owned.job = Some(live_text);
    write_run(&store, &owned);
    let mut orphan = run("m2", &facts, 2, 1, false);
    orphan.job = Some("0190a5e0-7c3a-7000-8000-000000000002");
    write_run(&store, &orphan);
    let path = store.layout().job_dir(&live);
    fs::create_dir_all(&path).expect("a job directory");
    let holder = JobLock::acquire(&JobDir::new(live, path)).expect("the job lock");

    let file = experiment(&["m1", "m2"], 2);
    let res = resolution(&facts);
    let ctx = plan_ctx(&store, &lock, &facts);
    let before = snapshot(&root);
    let refusal = plan_runs(&ctx, &file, &res, Force::No, PlanMode::Hold);
    assert!(
        matches!(refusal, Err(PlanError::ExperimentLocked)),
        "a live job refuses the plan"
    );
    assert_eq!(
        snapshot(&root),
        before,
        "no attempt is sealed while a job is live"
    );

    drop(holder);
    let out = plan_runs(&ctx, &file, &res, Force::No, PlanMode::Hold).expect("the job let go");
    drop(out);
    let done = snapshot(&root)
        .keys()
        .filter(|path| path.ends_with("run.done"))
        .count();
    assert_eq!(
        done, 4,
        "both unfinished attempts are sealed, the one with no job directory too"
    );
}

/// Overflow row: a cell and replicate whose highest attempt is `u32::MAX` is refused.
fn b26_overflow_row() {
    let (_dir, store, lock) = fresh_store();
    let facts = Facts::base();
    let mut last = run("m1", &facts, 1, u32::MAX, true);
    last.integrity = json!({"kind": "invalid", "reason": "infrastructure"});
    write_run(&store, &last);
    let ctx = plan_ctx(&store, &lock, &facts);
    let refusal = plan_runs(
        &ctx,
        &experiment(&["m1"], 1),
        &resolution(&facts),
        Force::No,
        PlanMode::Hold,
    );
    assert!(
        matches!(
            refusal,
            Err(PlanError::AttemptOverflow { replicate: 1, .. })
        ),
        "no attempt number is left"
    );
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

/// The five facts a resume re-checks.
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
            apply: |facts| facts.version = "2.2.0".to_owned(), // BORROW: the facts own their version text
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
        Change {
            name: "input digest",
            apply: |facts| facts.input = Some('d'),
            path: "inputs.build",
            stored: json!(format!("sha256:{}", "c".repeat(64))),
            observed: json!(format!("sha256:{}", "d".repeat(64))),
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
/// The order of the plans comes from `run_order`, so a plan is found by its cell.
/// Oracle: literal paths and values, with the stored ones read back from hand-built manifests;
/// the bytes under `runs/` before and after.
/// Rows for a changed input digest (the fifth fact, named `inputs.build`), for a value a
/// configuration default changed after the cell was stored (the stored one is carried), for the
/// runs of another experiment (never adopted, never counted), for a secret that only an earlier
/// attempt of the cell saw, for a past experiment with a cell this one does not answer, and for
/// two input names that map to one deviation path.
/// Bite: skip the image-ID or the inputs comparison in `check_facts`; drop `check_names`; match
/// stored cells by subset only; read the secrets of the last attempt only.
#[test]
fn b27_changed_fact_is_refused_and_the_cell_is_pinned() {
    let models = ["m1", "m2"];
    let file = experiment(&models, 2);
    let base = Facts::base();

    // Control: nothing changed, so the resume plans replicate 2 of each cell.
    let (_dir, store, lock) = two_cells();
    let ctx = plan_ctx(&store, &lock, &base);
    let out = plan_runs(&ctx, &file, &resolution(&base), Force::No, PlanMode::Hold)
        .map_err(|e| e.to_string())
        .expect("the control plans");
    assert_eq!(out.plans.len(), 2, "one replicate left per cell");
    for model in models {
        let stored = stored_cell(model, &base);
        let plan = out
            .plans
            .iter()
            .find(|plan| plan.cell == stored.digest())
            .expect("a plan for the cell");
        assert_eq!(plan.stored, Some(stored), "the stored cell is replayed");
        assert!(
            plan.deviations.is_empty(),
            "nothing changed, nothing deviates"
        );
    }
    drop(out);

    for change in changes() {
        let (_dir, store, lock) = two_cells();
        let root = store.layout().root().to_path_buf();
        let mut observed = base.clone(); // BORROW: the changed facts are an owned copy of the base
        (change.apply)(&mut observed);
        let ctx = plan_ctx(&store, &lock, &observed);
        let res = resolution(&observed);

        // Refused without force, naming the path, and nothing is written.
        let before = snapshot(&root);
        let refusal = plan_runs(&ctx, &file, &res, Force::No, PlanMode::Hold);
        let Err(PlanError::Changed(deviations)) = refusal else {
            panic!("{}: expected a refusal", change.name);
        };
        let paths: BTreeSet<&str> = deviations.iter().map(|d| d.path.as_str()).collect(); // BORROW: the set holds the deviation paths as text
        assert_eq!(paths, BTreeSet::from([change.path]), "{}", change.name);
        for deviation in &deviations {
            let stored = deviation.stored.as_ref().map(|v| v.as_value().clone()); // BORROW: the compared values are owned copies
            let seen = deviation.observed.as_ref().map(|v| v.as_value().clone()); // BORROW: the compared values are owned copies
            assert_eq!(stored, Some(change.stored.clone()), "{}", change.name); // BORROW: the expected values are owned copies
            assert_eq!(seen, Some(change.observed.clone()), "{}", change.name); // BORROW: the expected values are owned copies
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
        for model in models {
            let plan = out
                .plans
                .iter()
                .find(|plan| plan.cell == stored_cell(model, &base).digest())
                .expect("a plan for the stored cell");
            assert_eq!(
                plan.stored,
                Some(stored_cell(model, &base)),
                "{}",
                change.name
            );
            let key = manifest(&run(model, &base, 1, 1, true))
                .run()
                .cell()
                .clone(); // BORROW: the key is an owned copy of the cell
            assert_eq!(
                plan.key.cell(),
                &key,
                "{}: the stored digest keys the run",
                change.name
            );
        }
        drop(out);
    }

    b27_pin_row();
    b27_other_experiment_row();
    b27_secrets_row();
    b27_superset_row();
    b27_name_collision_row();
}

/// Pin row: a project-config default (a turn cap) changed after the cell was stored, so the plan
/// carries the stored value and records nothing.
fn b27_pin_row() {
    let stored_facts = Facts {
        turns: Some(10),
        ..Facts::base()
    };
    let now = Facts {
        turns: Some(20),
        ..Facts::base()
    };
    let (_dir, store, lock) = fresh_store();
    for model in ["m1", "m2"] {
        write_run(&store, &run(model, &stored_facts, 1, 1, true));
    }
    let ctx = plan_ctx(&store, &lock, &now);
    let out = plan_runs(
        &ctx,
        &experiment(&["m1", "m2"], 2),
        &resolution(&now),
        Force::No,
        PlanMode::Hold,
    )
    .map_err(|e| e.to_string())
    .expect("a changed default is not a refusal");
    assert_eq!(out.plans.len(), 2, "replicate 2 of each cell");
    for model in ["m1", "m2"] {
        let plan = out
            .plans
            .iter()
            .find(|plan| plan.cell == stored_cell(model, &stored_facts).digest())
            .expect("a plan for the stored cell");
        assert_eq!(
            plan.stored,
            Some(stored_cell(model, &stored_facts)),
            "10 is carried"
        );
        assert_ne!(
            plan.stored,
            Some(stored_cell(model, &now)),
            "20 is not adopted"
        );
        assert!(
            plan.deviations.is_empty(),
            "a default is not a re-checked fact"
        );
    }
}

/// Cross-experiment row: experiment A stored a cell with one more declared cap and has a sealed
/// replicate 1 and an unfinished replicate 2; experiment B (the same model, without the cap) plans
/// fresh, counts none of A's runs and touches none of A's files.
fn b27_other_experiment_row() {
    let theirs = Facts {
        turns: Some(10),
        experiment: 'a',
        ..Facts::base()
    };
    let (_dir, store, lock) = fresh_store();
    let root = store.layout().root().to_path_buf();
    write_run(&store, &run("m1", &theirs, 1, 1, true));
    write_run(&store, &run("m1", &theirs, 2, 1, false));
    let a_cell = manifest(&run("m1", &theirs, 1, 1, true))
        .run()
        .cell()
        .clone(); // BORROW: the key is an owned copy of the cell
    let ours = Facts::base();
    let ctx = plan_ctx(&store, &lock, &ours);
    let before = snapshot(&root);
    let out = plan_runs(
        &ctx,
        &experiment(&["m1"], 2),
        &resolution(&ours),
        Force::No,
        PlanMode::Hold,
    )
    .map_err(|e| e.to_string())
    .expect("another experiment's cell is ignored");
    assert!(out.sealed.is_empty(), "none of A's runs counts for B");
    assert_eq!(out.plans.len(), 2, "both replicates of B are to run");
    for plan in &out.plans {
        assert_eq!(plan.attempt, 1, "B's attempts start at 1");
        assert!(plan.stored.is_none(), "nothing is replayed from A");
        assert_ne!(plan.key.cell(), &a_cell, "B's cell is not A's");
    }
    assert_eq!(
        snapshot(&root),
        before,
        "A's unfinished attempt is not sealed"
    );
}

/// Secrets row: attempt 1 saw `API_KEY` and attempt 2, the last, saw none; with no secret present
/// now the resume is refused naming `secrets`, as the union of the attempts says it was present.
fn b27_secrets_row() {
    let saw = Facts::base();
    let none = Facts {
        secrets: Vec::new(),
        ..Facts::base()
    };
    let (_dir, store, lock) = fresh_store();
    let mut first = run("m1", &saw, 1, 1, true);
    first.integrity = json!({"kind": "invalid", "reason": "infrastructure"});
    write_run(&store, &first);
    write_run(&store, &run("m1", &none, 1, 2, true));
    let ctx = plan_ctx(&store, &lock, &none);
    let refusal = plan_runs(
        &ctx,
        &experiment(&["m1"], 2),
        &resolution(&none),
        Force::No,
        PlanMode::Hold,
    );
    let Err(PlanError::Changed(deviations)) = refusal else {
        panic!("an earlier attempt's secret counts");
    };
    let paths: Vec<&str> = deviations.iter().map(|d| d.path.as_str()).collect(); // BORROW: the list holds the deviation paths as text
    assert_eq!(paths, ["secrets"]);
}

/// Superset row: a past experiment with cells m1 and m2 holds the cell this experiment (m1 only)
/// would answer, so the plan is refused naming the past experiment and the cell nothing answers.
fn b27_superset_row() {
    let theirs = Facts {
        experiment: 'a',
        ..Facts::base()
    };
    let (_dir, store, lock) = fresh_store();
    for model in ["m1", "m2"] {
        write_run(&store, &run(model, &theirs, 1, 1, true));
    }
    let m2_cell = manifest(&run("m2", &theirs, 1, 1, true))
        .run()
        .cell()
        .to_string();
    let ours = Facts::base();
    let ctx = plan_ctx(&store, &lock, &ours);
    let refusal = plan_runs(
        &ctx,
        &experiment(&["m1"], 1),
        &resolution(&ours),
        Force::No,
        PlanMode::Hold,
    );
    let Err(PlanError::StoredCellUnknown { experiment, cells }) = refusal else {
        panic!("a past experiment with an unmatched cell is refused");
    };
    // Provenance: `digest_text('a')` is the experiment digest `theirs` writes into its manifests.
    assert_eq!(experiment, digest_text('a'));
    assert_eq!(cells, [m2_cell]);
}

/// Collision row: a cell with inputs named `build-a` and `build_a`, which map to one deviation
/// path, is refused naming both, forced or not.
fn b27_name_collision_row() {
    let facts = Facts {
        clash: true,
        ..Facts::base()
    };
    let (_dir, store, lock) = fresh_store();
    write_run(&store, &run("m1", &facts, 1, 1, true));
    let ctx = plan_ctx(&store, &lock, &facts);
    for force in [Force::No, Force::Yes] {
        let refusal = plan_runs(
            &ctx,
            &experiment(&["m1"], 2),
            &resolution(&facts),
            force,
            PlanMode::Hold,
        );
        // Provenance: the inputs of a cell are sorted, and `-` sorts before `_`.
        assert!(
            matches!(
                &refusal,
                Err(PlanError::NameCollision { first, second })
                    if first == "build-a" && second == "build_a"
            ),
            "two names of one segment are refused"
        );
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
        let mut observed = base.clone(); // BORROW: the changed facts are an owned copy of the base
        (change.apply)(&mut observed);
        let ctx = plan_ctx(&store, &lock, &observed);

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
            assert_eq!(deviation.path.as_str(), change.path, "{}", change.name); // BORROW: the path is compared as text
            let stored = deviation.stored.as_ref().map(|v| v.as_value().clone()); // BORROW: the compared values are owned copies
            let seen = deviation.observed.as_ref().map(|v| v.as_value().clone()); // BORROW: the compared values are owned copies
            assert_eq!(stored, Some(change.stored.clone()), "{}", change.name); // BORROW: the expected values are owned copies
            assert_eq!(seen, Some(change.observed.clone()), "{}", change.name); // BORROW: the expected values are owned copies
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

/// The cell of the single-model experiment over `model`: the digest of a cell does not depend on
/// the other cells of an experiment.
fn single_cell(model: &str) -> CellId {
    let facts = Facts::base();
    let (_dir, store, lock) = fresh_store();
    let ctx = plan_ctx(&store, &lock, &facts);
    let out = plan_runs(
        &ctx,
        &experiment(&[model], 1),
        &resolution(&facts),
        Force::No,
        PlanMode::Check,
    )
    .map_err(|e| e.to_string())
    .expect("a plan");
    out.plans.first().expect("one run").cell.clone() // BORROW: the cell outlives the outcome
}

/// The `(model index, replicate)` of each planned run of `models`, in plan order, and the digest
/// the plan seeds its order from.
fn planned_order(models: &[&str], replicates: u32) -> (Vec<(usize, u32)>, Digest) {
    let cells: Vec<CellId> = models.iter().map(|model| single_cell(model)).collect();
    let facts = Facts::base();
    let (_dir, store, lock) = fresh_store();
    let ctx = plan_ctx(&store, &lock, &facts);
    let out = plan_runs(
        &ctx,
        &experiment(models, replicates),
        &resolution(&facts),
        Force::No,
        PlanMode::Check,
    )
    .map_err(|e| e.to_string())
    .expect("a plan");
    let order = out
        .plans
        .iter()
        .map(|plan| {
            let index = cells
                .iter()
                .position(|cell| *cell == plan.cell)
                .expect("a planned run of a declared cell");
            (index, plan.key.replicate())
        })
        .collect();
    (order, out.experiment.clone()) // BORROW: the digest outlives the outcome
}

/// Guards: B3 — every replicate round holds each cell exactly once, and rounds come in replicate
/// order, for several seeds and design sizes.
/// Oracle: the arithmetic of the design: a round of `cells` runs is a permutation of `0..cells`
/// and round `r` carries replicate `r`.
/// Bite: shuffle across rounds instead of within each round.
#[test]
fn b3_each_round_holds_each_cell_once() {
    for seed in [0, 1, 0x9E37_79B9_7F4A_7C15, u64::MAX] {
        for (cells, replicates) in [(1_usize, 1_u32), (2, 3), (6, 4), (13, 5)] {
            let order = run_order(cells, replicates, seed);
            let total = cells * usize::try_from(replicates).expect("a small count");
            assert_eq!(order.len(), total, "seed {seed}, {cells} cells");
            for (round, replicate) in order.chunks(cells).zip(1..=replicates) {
                let mut seen: Vec<usize> = round.iter().map(|(cell, _)| *cell).collect();
                seen.sort_unstable();
                assert_eq!(
                    seen,
                    (0..cells).collect::<Vec<_>>(),
                    "seed {seed}, {cells} cells, replicate {replicate}: each cell once"
                );
                assert!(
                    round.iter().all(|(_, r)| *r == replicate),
                    "seed {seed}, {cells} cells: the round of replicate {replicate} holds only it"
                );
            }
        }
    }
}

/// Guards: B3 — one seed gives one order, however often it is asked.
/// Oracle: equality of two calls with the same arguments.
/// Bite: draw the shuffle from a clock or a counter.
#[test]
fn b3_same_seed_same_order() {
    for seed in [0, 7, u64::MAX] {
        assert_eq!(run_order(8, 3, seed), run_order(8, 3, seed), "seed {seed}");
    }
}

/// Guards: B3 — two seeds give two orders for a design of at least 6 cells.
/// Oracle: inequality of the orders of seeds 1 and 2 on 8 cells and 3 replicates, which is chance
/// 1 in 40 320 per round if the seed were ignored.
/// Bite: ignore the seed.
#[test]
fn b3_other_seed_other_order() {
    assert_ne!(run_order(8, 3, 1), run_order(8, 3, 2));
    assert_ne!(run_order(6, 1, 1), run_order(6, 1, 3));
}

/// Guards: B3 — `plan_runs` orders from the experiment's digest: the same file planned twice plans
/// one order, a file whose cells digest differently plans another, and the seed is the first 8
/// bytes of the digest, little-endian.
/// Oracle: two plans of one file; two files of 8 cells and 2 replicates that differ in the model
/// ids only (one chance in 40 320 per round of agreeing if the digest were ignored); the literal
/// bytes `01..08` of a digest read as the number `0x0807060504030201`.
/// Bite: seed the order with a constant.
#[test]
fn b3_plan_runs_uses_the_experiment_seed() {
    let first = ["m1", "m2", "m3", "m4", "m5", "m6", "m7", "m8"];
    let second = ["n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8"];
    let (order, digest) = planned_order(&first, 2);
    let (again, again_digest) = planned_order(&first, 2);
    let (other, other_digest) = planned_order(&second, 2);
    assert_eq!(digest, again_digest);
    assert_ne!(digest, other_digest);
    assert_eq!(order, again, "the same file plans one order");
    assert_ne!(order, other, "another digest plans another order");

    let literal = Digest::from_hex(&format!("0102030405060708{}", "0".repeat(48)))
        .expect("a digest of 64 hex digits");
    assert_eq!(seed_of(&literal), 0x0807_0605_0403_0201);
}

/// Guards: T3 — a file with one treatment resolves once and plans the runs it planned before
/// treatment levels existed: the same cells, the same experiment digest, the same order.
/// Oracle: the cell digests, the experiment digest and the run order written below, taken from
/// the planner before per-level resolution was written.
/// Bite: key the cell by a level index, or add the level to a one-level cell.
#[test]
fn t3_one_level_plans_the_old_bytes() {
    let facts = Facts::base();
    let (_dir, store, lock) = fresh_store();
    let ctx = plan_ctx(&store, &lock, &facts);
    let out = plan_runs(
        &ctx,
        &experiment(&["m1", "m2"], 2),
        &resolution(&facts),
        Force::No,
        PlanMode::Check,
    )
    .map_err(|e| e.to_string())
    .expect("a plan");
    let planned: Vec<(String, u32)> = out
        .plans
        .iter()
        .map(|plan| (plan.cell.to_string(), plan.key.replicate()))
        .collect();
    let m1 = "sha256:c8c025d07f891ace920b103b2dd93ae78cdbe49fbf4269085554aa79d72dfab2";
    let m2 = "sha256:528ad7c2a0587eda8c5fbeee1760c89687f211a181a125f7dfc74b7be074c13a";
    let expected: Vec<(String, u32)> = [(m1, 1), (m2, 1), (m1, 2), (m2, 2)]
        .into_iter()
        .map(|(cell, replicate)| (cell.to_owned(), replicate))
        .collect();
    assert_eq!(
        (out.experiment.to_string(), planned),
        (
            "sha256:2a0b3bf188be1e42e471095c7973dfaa390f5d0288cbb6733f15ac6ce4e670e7".to_owned(),
            expected
        )
    );
}

/// An experiment over the cell `m1` and the treatment levels `levels`, one replicate.
fn treatment_file(levels: &[&str]) -> candler_schemas::execution::ExperimentFile {
    let list = levels
        .iter()
        .map(|level| format!("\"{level}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let text = format!(
        "schema = \"experiment/1\"\nreplicates = 1\n\n[[factors]]\npath = \"model.id\"\nlevels = [\"m1\"]\n\n[[factors]]\npath = \"request.treatment\"\nlevels = [{list}]\n\n[budget.per_run]\nwall_s = 3600\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"htn\"\n"
    );
    candler_schemas::execution::ExperimentFile::from_toml_str(&text).expect("an experiment file")
}

/// The base resolution with its treatment record at the tag `tag`: a level that hashes apart from
/// the base level.
fn level_resolution(tag: &str) -> candler_core::resolve::layers::Resolution {
    let mut json = serde_json::to_value(resolution(&Facts::base())).expect("JSON");
    let values = json["values"].as_array_mut().expect("values");
    for value in values
        .iter_mut()
        .filter(|value| value["path"] == "treatment")
    {
        value["value"]["tag"] = json!(tag);
    }
    serde_json::from_value(json).expect("a resolution")
}

/// The planned runs of `file` under `levels`, planned against an empty store.
fn plan_levels(
    file: &candler_schemas::execution::ExperimentFile,
    levels: Levels<'_>,
) -> Result<candler_core::runner::plan::PlanOutcome, PlanError> {
    let facts = Facts::base();
    let (_dir, store, lock) = fresh_store();
    let ctx = plan_ctx(&store, &lock, &facts);
    plan_runs(&ctx, file, levels, Force::No, PlanMode::Check)
}

/// The digest of the one cell of the one-level file under `level`.
fn cell_of_level(level: &candler_core::resolve::layers::Resolution) -> Digest {
    let out = plan_levels(&treatment_file(&["htn"]), Levels::from(level)).expect("a plan");
    out.plans.first().expect("one run").cell.clone() // BORROW: the cell outlives the outcome
}

/// Guards: T3 — two treatment levels give two cells, each built from its own level's resolution,
/// so the cells differ in digest and each holds its own level's treatment record.
/// Oracle: the digest of the one cell a one-level file plans under each level's resolution, and
/// the tag each level's resolution names.
/// Bite: build every cell from the first level.
#[test]
fn t3_each_cell_uses_its_level() {
    let levels = [resolution(&Facts::base()), level_resolution("v2")];
    let out = plan_levels(
        &treatment_file(&["htn", "other"]),
        Levels::from(&levels[..]),
    )
    .expect("a plan");
    let planned: BTreeSet<Digest> = out.plans.iter().map(|plan| plan.cell.clone()).collect(); // BORROW: the set owns its digests
    let expected: BTreeSet<Digest> = levels.iter().map(cell_of_level).collect();
    assert_eq!(expected.len(), 2, "two levels hold two digests");
    assert_eq!(planned, expected);
    for (level, tag) in levels.iter().zip(["v1", "v2"]) {
        let cell = out
            .fresh
            .get(&cell_of_level(level))
            .expect("the level's cell");
        let treatment = serde_json::to_value(cell.treatment()).expect("JSON");
        assert_eq!(
            treatment["tag"],
            json!(tag),
            "the cell of the level at {tag}"
        );
    }
}

/// Guards: T3 — a file with two or more treatment levels given fewer resolutions fails the plan,
/// naming the first level without a resolution; a failing level cannot reach the plan as a
/// resolution, so this is the plan failing on a level.
/// Oracle: the level names written in the file.
/// Bite: skip a level without a resolution and plan the others.
#[test]
fn t3_unresolved_level_fails_the_plan() {
    let first = resolution(&Facts::base());
    let both = [resolution(&Facts::base()), level_resolution("v2")];
    let two = treatment_file(&["htn", "other"]);
    let three = treatment_file(&["htn", "other", "third"]);
    let cases = [
        ("one resolution", &two, Levels::from(&first), "other"),
        ("two of three", &three, Levels::from(&both[..]), "third"),
    ];
    for (name, file, levels, missing) in cases {
        let error = plan_levels(file, levels).map(|_| ()).expect_err(name);
        assert!(
            error.to_string().contains(missing),
            "{name}: the error names {missing}: {error}"
        );
    }
}

/// Guards: T3 — `check_facts` compares a stored cell with the resolution of its own level: the
/// cell of one level reports no deviation under that level and a treatment deviation under
/// another.
/// Oracle: the tags the two resolutions name, and the path `treatment`.
/// Bite: check every cell against the first level's resolution.
#[test]
fn t3_check_facts_per_level() {
    let levels = [resolution(&Facts::base()), level_resolution("v2")];
    let out = plan_levels(
        &treatment_file(&["htn", "other"]),
        Levels::from(&levels[..]),
    )
    .expect("a plan");
    let second = out
        .fresh
        .get(&cell_of_level(&levels[1]))
        .expect("the second cell");
    assert_eq!(check_facts(second, second, &levels[1]), Vec::new());
    let paths: Vec<String> = check_facts(second, second, &levels[0])
        .iter()
        .map(|deviation| deviation.path.to_string())
        .collect();
    assert_eq!(paths, vec!["treatment".to_owned()]);
}

/// The base resolution with its treatment record at the commit `commit`.
fn commit_resolution(commit: &str) -> candler_core::resolve::layers::Resolution {
    let mut json = serde_json::to_value(resolution(&Facts::base())).expect("JSON");
    let values = json["values"].as_array_mut().expect("values");
    for value in values
        .iter_mut()
        .filter(|value| value["path"] == "treatment")
    {
        value["value"]["commit"] = json!(commit);
    }
    serde_json::from_value(json).expect("a resolution")
}

/// Guards: T3 — the commit of a treatment reference is part of its cell: two one-level plans that
/// differ only in the treatment commit give two cell digests.
/// Oracle: `__design__/architecture.md` §Treatment and §Comparable, which hash the reference
/// `{name, tag, commit}` and leave out only `source`; the two commits are written below.
/// Bite: leave the commit out of the treatment record the cell hashes.
#[test]
fn t3_probe_treatment_commit_in_digest() {
    let first = cell_of_level(&commit_resolution(&"1".repeat(40)));
    let second = cell_of_level(&commit_resolution(&"2".repeat(40)));
    assert_ne!(
        first, second,
        "the treatment commit does not enter the cell digest"
    );
}

/// Guards: ruling 2 -- two declared treatment levels whose resolved references are equal are
/// refused at plan time, naming both levels; two that differ in the commit alone plan.
/// Oracle: the level names written in the file, and the probe above, which shows the commit
/// enters the cell.
/// Bite: skip the equality check between levels.
#[test]
fn t3_equal_levels_are_refused() {
    let same = [resolution(&Facts::base()), resolution(&Facts::base())];
    let error = plan_levels(&treatment_file(&["htn", "other"]), Levels::from(&same[..]))
        .map(|_| ())
        .expect_err("equal levels");
    let text = error.to_string();
    assert!(
        text.contains("htn") && text.contains("other"),
        "the refusal names both levels: {text}"
    );
    let apart = [
        commit_resolution(&"1".repeat(40)),
        commit_resolution(&"2".repeat(40)),
    ];
    plan_levels(&treatment_file(&["htn", "other"]), Levels::from(&apart[..]))
        .expect("levels that differ in the commit plan");
}

/// An experiment over the cell `m1` whose treatment factor has the table levels `levels` (TOML).
fn table_levels_file(levels: &str) -> candler_schemas::execution::ExperimentFile {
    let text = format!(
        "schema = \"experiment/1\"\nreplicates = 1\n\n[[factors]]\npath = \"model.id\"\nlevels = [\"m1\"]\n\n[[factors]]\npath = \"request.treatment\"\nlevels = {levels}\n\n[budget.per_run]\nwall_s = 3600\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"htn\"\n"
    );
    candler_schemas::execution::ExperimentFile::from_toml_str(&text).expect("an experiment file")
}

/// Guards: ruling 4 -- a table level of `request.treatment` fills the four fields of a treatment
/// and plans one cell per level; a leaf that is not one of the four, or a table where a field
/// takes a value, is refused naming its path.
/// Oracle: the fields written in the tables below (`name`, `tag`, `commit`, `source` are the
/// fields of the `[treatment]` table) and the two tags each level's resolution names.
/// Bite: admit every leaf under `request.treatment`.
#[test]
fn t3_treatment_table_levels_admit_four_fields() {
    let levels = [resolution(&Facts::base()), level_resolution("v2")];
    let planned = plan_levels(
        &table_levels_file(
            "[{name = \"htn\", tag = \"v1\", source = \"a\"}, {name = \"htn\", tag = \"v2\", source = \"a\"}]",
        ),
        Levels::from(&levels[..]),
    )
    .expect("two table levels plan");
    assert_eq!(planned.plans.len(), 2, "one cell per level");
    for (table, path) in [
        (
            "[{name = \"htn\", foo = \"x\"}, {name = \"other\"}]",
            "request.treatment.foo",
        ),
        (
            "[{name = \"htn\", source = {a = 1}}, {name = \"other\"}]",
            "request.treatment.source",
        ),
    ] {
        let error = plan_levels(&table_levels_file(table), Levels::from(&levels[..]))
            .map(|_| ())
            .expect_err(path);
        assert!(
            error.to_string().contains(path),
            "the refusal names {path}: {error}"
        );
    }
}

/// Guards: I12 — `replicates` is an operational setting, so the plan reads the resolved value: a
/// file that writes `replicates = 5` and an explicit `replicates = 1` plan one run per cell, and
/// the resolution names the explicit argument as the value's source.
/// Oracle: two cells times one replicate is two runs; the file's own count would plan ten.
/// Bite: read the file's `replicates` again in the plan.
#[test]
fn set_replicates_is_honoured() {
    use candler_core::resolve::config::{Config, UserConfig};
    use candler_core::resolve::layers::{FileValues, Layered, ResolveInput};

    let root = tempfile::tempdir().expect("a temp dir");
    let config = Config::new(
        root.path().to_owned(), // BORROW: the config owns its working directory
        None,
        UserConfig::default(),
        Some(root.path().join("state")),
    );
    let replicates =
        |count: u32| candler_schemas::values::JsonValue::new(json!(count)).expect("a JSON value");
    let input = ResolveInput::default()
        .explicit("replicates".parse().expect("a field path"), replicates(1))
        .file(FileValues::new(vec![(
            "replicates".parse().expect("a field path"),
            replicates(5),
        )]));
    let layered = Layered::new(&config, input).expect("layers");
    let layers = layered.resolve();

    let facts = Facts::base();
    let mut json = serde_json::to_value(resolution(&facts)).expect("JSON");
    json["settings"] = serde_json::to_value(&layers.settings).expect("JSON");
    let resolved: candler_core::resolve::layers::Resolution =
        serde_json::from_value(json).expect("a resolution");

    let (_dir, store, lock) = fresh_store();
    let ctx = plan_ctx(&store, &lock, &facts);
    let out = plan_runs(
        &ctx,
        &experiment(&["m1", "m2"], 5),
        &resolved,
        Force::No,
        PlanMode::Check,
    )
    .map_err(|e| e.to_string())
    .expect("a plan");
    assert_eq!(out.plans.len(), 2);

    let sources = serde_json::to_string(&layers.sources).expect("JSON");
    assert!(
        sources.contains("explicit"),
        "the source of `replicates`: {sources}"
    );
}

/// Guards: I11 — the summary of a plan bounds the wall clock by the new runs times the per-run
/// cap the runs get (an explicit setting first, then the file's), and names the job caps, as
/// the experiment file wrote them.
/// Oracle: two cells times two replicates is four runs of 600 s, or of 300 s under an explicit
/// setting of 300, and the caps are the file's own
/// numbers written below.
/// Bite: read the caps from the resolution, which holds neither (seen red that way).
#[test]
fn the_summary_of_a_plan_holds_the_caps_of_the_file() {
    use candler_core::resolve::config::{Config, UserConfig};
    use candler_core::resolve::layers::{Layered, ResolveInput};
    let text = "schema = \"experiment/1\"\nreplicates = 2\n\n[[factors]]\npath = \"model.id\"\nlevels = [\"m1\", \"m2\"]\n\n[budget]\nmax_runs = 7\nmax_parallel = 2\n\n[budget.per_run]\nwall_s = 600\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"htn\"\n";
    let file = candler_schemas::execution::ExperimentFile::from_toml_str(text).expect("a file");
    let facts = Facts::base();
    let (_dir, store, lock) = fresh_store();
    let ctx = plan_ctx(&store, &lock, &facts);
    let resolved = resolution(&facts);
    let out = plan_runs(&ctx, &file, &resolved, Force::No, PlanMode::Check)
        .map_err(|e| e.to_string())
        .expect("a plan");
    let summary = out.summary(file.budget());
    assert_eq!(
        (summary.cells, summary.new_runs, summary.reused_runs),
        (2, 4, 0)
    );
    assert_eq!(summary.wall_s_bound, Some(2400));
    assert_eq!(
        summary.job_caps,
        vec![
            ("budget.max_runs".to_owned(), json!(7)),
            ("budget.max_parallel".to_owned(), json!(2)),
        ]
    );

    // An explicit `--set budget.per_run.wall_s` over the file's 600 is the cap the runs get, so
    // it is the cap the bound uses.
    let root = tempfile::tempdir().expect("a temp dir");
    let config = Config::new(
        root.path().to_owned(), // BORROW: the config owns its working directory
        None,
        UserConfig::default(),
        Some(root.path().join("state")),
    );
    let input = ResolveInput::default().explicit(
        "budget.per_run.wall_s".parse().expect("a field path"),
        candler_schemas::values::JsonValue::new(json!(300)).expect("a JSON value"),
    );
    let layers = Layered::new(&config, input).expect("layers").resolve();
    let mut held = serde_json::to_value(resolution(&facts)).expect("JSON");
    for key in ["values", "sources"] {
        let mut all = held[key].as_array().cloned().unwrap_or_default();
        all.extend(
            serde_json::to_value(match key {
                "values" => serde_json::to_value(&layers.values).expect("JSON"),
                _ => serde_json::to_value(&layers.sources).expect("JSON"),
            })
            .expect("JSON")
            .as_array()
            .cloned()
            .unwrap_or_default(),
        );
        held[key] = serde_json::Value::Array(all);
    }
    let explicit: candler_core::resolve::layers::Resolution =
        serde_json::from_value(held).expect("a resolution");
    let (_dir2, store2, lock2) = fresh_store();
    let ctx2 = plan_ctx(&store2, &lock2, &facts);
    let out = plan_runs(&ctx2, &file, &explicit, Force::No, PlanMode::Check)
        .map_err(|e| e.to_string())
        .expect("a plan");
    assert_eq!(out.summary(file.budget()).wall_s_bound, Some(1200));
}
