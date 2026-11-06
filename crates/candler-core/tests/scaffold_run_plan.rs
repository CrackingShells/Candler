#![cfg(test)]
//! Development tests of run planning.

mod support;

use candler_core::runner::plan::{Force, PlanCtx, PlanError, PlanMode, plan_runs};
use support::run_plan::{Facts, experiment, fresh_store, resolution};

#[test]
fn three_cells_give_keys_with_attempt_one() {
    let (_dir, store, lock) = fresh_store();
    let ctx = PlanCtx {
        store,
        experiment_lock: lock,
    };
    let file = experiment(&["m1", "m2", "m3"], 1);
    let out = plan_runs(
        &ctx,
        &file,
        &resolution(&Facts::base()),
        Force::No,
        PlanMode::Hold,
    )
    .expect("plan");
    assert_eq!(out.plans.len(), 3);
    assert!(
        out.plans
            .iter()
            .all(|p| p.attempt == 1 && p.stored.is_none())
    );
    assert!(out.lock.is_some());
}

#[test]
fn a_second_planner_is_refused() {
    let (_dir, store, lock) = fresh_store();
    let ctx = PlanCtx {
        store,
        experiment_lock: lock,
    };
    let file = experiment(&["m1"], 1);
    let res = resolution(&Facts::base());
    let first = plan_runs(&ctx, &file, &res, Force::No, PlanMode::Hold).expect("plan");
    let second = plan_runs(&ctx, &file, &res, Force::No, PlanMode::Hold);
    assert!(matches!(second, Err(PlanError::ExperimentLocked)));
    let check = plan_runs(&ctx, &file, &res, Force::No, PlanMode::Check);
    assert!(matches!(check, Err(PlanError::ExperimentLocked)));
    drop(first);
    plan_runs(&ctx, &file, &res, Force::No, PlanMode::Hold).expect("free again");
}

#[test]
fn the_plan_is_stable_when_nothing_changed() {
    let (_dir, store, lock) = fresh_store();
    let ctx = PlanCtx {
        store,
        experiment_lock: lock,
    };
    let file = experiment(&["m1", "m2"], 2);
    let res = resolution(&Facts::base());
    let a = plan_runs(&ctx, &file, &res, Force::No, PlanMode::Check).expect("plan");
    let b = plan_runs(&ctx, &file, &res, Force::No, PlanMode::Check).expect("plan");
    assert_eq!(a.plans, b.plans);
    assert_eq!(a.plans.len(), 4);
}
