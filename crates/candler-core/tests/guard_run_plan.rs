#![cfg(test)]
//! Guards for run planning: that a resume deletes nothing, replays the stored cell and records
//! every forced deviation. Every check of stored files reads them with `std::fs` and
//! `serde_json`, never through the planner.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::lock::JobLock;
use candler_core::runner::plan::{Force, PlanError, PlanMode, plan_runs};
use candler_schemas::primitives::JobId;
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

    b26_lock_rows();
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
/// Oracle: literal paths and values, with the stored ones read back from hand-built manifests;
/// the bytes under `runs/` before and after.
/// Rows for a changed input digest (the fifth fact, named `inputs.build`), for a value a
/// configuration default changed after the cell was stored (the stored one is carried), for the
/// runs of another experiment (never adopted, never counted), for a secret that only an earlier
/// attempt of the cell saw, and for a past experiment with a cell this one does not answer.
/// Bite: skip the image-ID or the inputs comparison in `check_facts`; match stored cells by subset
/// only; read the secrets of the last attempt only.
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
        let mut observed = base.clone();
        (change.apply)(&mut observed);
        let ctx = plan_ctx(&store, &lock, &observed);
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

    b27_pin_row();
    b27_other_experiment_row();
    b27_secrets_row();
    b27_superset_row();
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
    for (plan, model) in out.plans.iter().zip(["m1", "m2"]) {
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
        .clone();
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
    let paths: Vec<&str> = deviations.iter().map(|d| d.path.as_str()).collect();
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
        let mut observed = base.clone();
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
