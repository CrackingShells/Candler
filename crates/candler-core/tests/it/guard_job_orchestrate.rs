//! Guards for job orchestration: what a job cap stops, and what it leaves alone.
//!
//! The job runs end to end over a store in a temporary directory, a PATH-fake docker client whose
//! `run` verb sleeps, a gateway that serves nothing and a harness that reads no output. The real
//! supervisor spawns and stops the fake runs; no container exists.
#![cfg(test)]

use crate::support;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use candler_core::jobs::model::{JobCause, JobControl, JobOutcome};
use candler_core::jobs::orchestrate::caps::{LiveRuns, stop_live};
use candler_core::jobs::orchestrate::orchestrate_with;
use candler_core::jobs::orchestrate::session::cells_of;
use candler_core::runner::plan::{Force, Levels, PlanMode, plan_runs};
use candler_core::runner::{RunStatus, run_order, seed_of};
use candler_core::supervise::EventSink;
use candler_schemas::execution::{InvalidReason, JobEnd};
use candler_schemas::primitives::{Digest, RunKey};
use candler_schemas::trace::EndedBy;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use support::job_orchestrate::{FakeHost, Rig, spec};
use support::job_runtime::Recorder;
use support::run_plan::{Facts, Run, manifest, snapshot, write_run};
use tokio::sync::watch;

/// The facts of every cell of the table: no secret is declared, so none is present.
fn facts() -> Facts {
    Facts {
        secrets: Vec::new(),
        commit: support::job_orchestrate::skill_repo().1.clone(), // BORROW: the facts own the commit text
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

/// The experiment digest the manifest of `key` stores, read from disk.
fn experiment_on_disk(root: &Path, key: &RunKey) -> Digest {
    let path = root
        .join("runs")
        .join(key.to_string())
        .join("manifest.json");
    let manifest: Value =
        serde_json::from_slice(&fs::read(path).expect("a manifest file")).expect("manifest JSON");
    manifest["experiment"]
        .as_str()
        .expect("an experiment digest")
        .parse()
        .expect("a digest")
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

/// Runs the job of `rig` over `models` on `host` to its end.
fn run_job_on(rig: &Rig, models: &[&str], wall_s: u32, host: Arc<FakeHost>) -> JobOutcome {
    let job = spec(models, &facts(), wall_s, 2);
    let (cancel, _keep) = watch::channel::<Option<JobCause>>(None);
    let ctl = JobControl {
        cancel: Arc::new(cancel),
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    runtime.block_on(orchestrate_with(rig.runtime(), job, ctl, host))
}

/// Runs the job of `rig` over `models` to its end and returns the kinds of the events the job's
/// sink took, in order, with the run each one names.
fn run_job_events(
    rig: &Rig,
    models: &[&str],
    wall_s: u32,
) -> (JobOutcome, Vec<(String, Option<RunKey>)>) {
    let job = spec(models, &facts(), wall_s, 2);
    let (cancel, _keep) = watch::channel::<Option<JobCause>>(None);
    let ctl = JobControl {
        cancel: Arc::new(cancel),
    };
    let recorder = Arc::new(Recorder::default());
    // TRAIT_OBJECT: the job takes its sink as a trait object
    let sink: Arc<dyn EventSink> = recorder.clone();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let outcome = runtime.block_on(orchestrate_with(
        rig.runtime_with_sink(sink),
        job,
        ctl,
        rig.host(),
    ));
    let kinds = recorder
        .events()
        .into_iter()
        // BORROW: .to_owned() gives the list its own kind text
        .map(|(run, payload)| (payload.kind().to_owned(), run))
        .collect();
    (outcome, kinds)
}

/// The table: six cells of one replicate each, in the order they are planned.
const TABLE: [&str; 6] = ["m1", "m2", "m3", "m4", "m5", "m6"];

/// Guards: I20 — crossing a job cap gives invalid(budget_stop) on live runs only; sealed runs keep
/// their status; no new run starts. F4: the gateway's `container.created` comes before the first
/// `run.started`.
/// Oracle: R01 §Budget; a literal run table. Which two of the unsealed runs are in flight is the
/// order `run_order` gives on the job's own seed, the digest the sealed runs store, so the guard
/// takes the first `max_parallel` unsealed runs of that order as the expected pair. Using
/// `run_order` as the fixture is sound because the `b3_` rows of `guard_run_plan` pin it.
/// Bite: apply the cap to all_runs instead of live_runs. F4: replace the gateway `container_created`
/// payload in `jobs/orchestrate/session.rs` with `job_started`; it fails at the `container.created
/// for the gateway precedes run.started` assertion.
#[test]
#[expect(
    clippy::doc_markdown,
    reason = "the guard header names identifiers in the form the guard index reads"
)]
fn b34_cap_stops_live_runs_only() {
    // The literal table: m1 and m2 are sealed valid before the job; two of m3 to m6 are in flight,
    // because `max_parallel` is 2 and the fake `run` sleeps; the other two are pending. The planned
    // order, from `run_order`, says which two.
    let facts = facts();
    let rig = Rig::new(30);
    let root = rig.store.layout().root().to_path_buf();
    for model in ["m1", "m2"] {
        write_run(&rig.store, &row(model, &facts));
    }
    let sealed = ["m1", "m2"].map(|model| key_of(model, &facts));
    let unsealed = ["m3", "m4", "m5", "m6"].map(|model| key_of(model, &facts));
    let in_flight = 2;
    let seed = seed_of(&experiment_on_disk(&root, &sealed[0]));
    let mut expected: Vec<RunKey> = run_order(TABLE.len(), 1, seed)
        .into_iter()
        .filter(|(cell, _)| *cell >= sealed.len())
        .filter_map(|(cell, _)| TABLE.get(cell).map(|model| key_of(model, &facts)))
        .take(in_flight)
        .collect();
    expected.sort();
    let before: Vec<_> = sealed.iter().map(|key| hashes(&root, key)).collect();
    assert!(
        before.iter().all(|files| !files.is_empty()),
        "the sealed runs hold files before the job"
    );

    // wall_s crossed: the job's cap is one second on a clock that stays at zero until both runs
    // are live. The fake client logs a `run` call only once its run is in flight, so the clock
    // moves past the cap only after m3 and m4 are both live; the fake `run` sleeps thirty seconds.
    let ticks = Arc::new(AtomicU64::new(0));
    let outcome = std::thread::scope(|scope| {
        let job = scope.spawn(|| run_job_on(&rig, &TABLE, 1, rig.host_on(&ticks)));
        while rig.runs_logged() < in_flight {
            assert!(
                !job.is_finished(),
                "the job ended before both runs were live"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        ticks.store(2_000, Ordering::SeqCst);
        job.join().expect("the job's thread")
    });

    assert_eq!(
        outcome.end,
        JobEnd::BudgetStop,
        "the job ends by its budget"
    );
    let mut ended: Vec<RunKey> = outcome.runs.iter().map(|run| run.key.clone()).collect();
    ended.sort();
    assert_eq!(
        ended, expected,
        "only the first `max_parallel` runs of the planned order ended"
    );
    let live = ended;
    let pending: Vec<RunKey> = unsealed
        .iter()
        .filter(|key| !live.contains(key))
        .cloned() // BORROW: the pending keys are owned copies
        .collect();
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
    let (calm, events) = run_job_events(&control, &TABLE, 600);
    // F4: the gateway's record is written when the session opens, and its event comes before
    // the first run starts (A55: `job.started, container.created, run.started, ...`).
    let first_run = events.iter().position(|(kind, _)| kind == "run.started");
    let first_container = events
        .iter()
        .position(|(kind, _)| kind == "container.created");
    assert!(
        first_container.is_some() && first_container < first_run,
        "container.created for the gateway precedes run.started: {events:?}"
    );
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

/// Guards: I20 — a run that enters after a stop starts already told to stop; a table never
/// stopped tells nothing.
/// Oracle: R01 §Budget ("crossing a job cap ... live runs"); literal causes.
/// Bite: drop the seeding of the channel in `LiveRuns::enter`.
#[test]
fn b34_run_entering_after_a_stop_hears_it() {
    let facts = facts();
    let key = key_of("m3", &facts);
    let calm = LiveRuns::new();
    assert_eq!(
        *calm.enter(&key).borrow(),
        None,
        "a table that was never stopped tells a new run nothing"
    );
    let stopped = LiveRuns::new();
    stop_live(&stopped, JobCause::BudgetStop);
    let late = stopped.enter(&key);
    assert_eq!(
        *late.borrow(),
        Some(JobCause::BudgetStop),
        "a run that enters after a stop starts told to stop"
    );
    stop_live(&stopped, JobCause::Killed);
    assert_eq!(
        *late.borrow(),
        Some(JobCause::Killed),
        "a later kill replaces the budget stop"
    );
    assert_eq!(
        *stopped.enter(&key_of("m4", &facts)).borrow(),
        Some(JobCause::Killed),
        "a run that enters after the kill starts killed"
    );
}

/// An experiment over the model `m1` and the treatment levels `levels`, one replicate.
fn two_level_file(levels: &[&str]) -> candler_schemas::execution::ExperimentFile {
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

/// The base resolution of a level: its treatment tag, the checkout its `request.treatment`
/// setting names and the harness version it observed.
fn level_of(tag: &str, source: &str, version: &str) -> candler_core::resolve::layers::Resolution {
    let facts = Facts {
        version: version.to_owned(), // BORROW: the facts own their version text
        ..facts()
    };
    let mut json = serde_json::to_value(support::run_plan::resolution(&facts)).expect("JSON");
    for value in json["values"]
        .as_array_mut()
        .expect("values")
        .iter_mut()
        .filter(|value| value["path"] == "treatment")
    {
        value["value"]["tag"] = json!(tag);
    }
    json["settings"] = json!([
        {"path": "request.treatment", "value": {"name": "htn", "tag": tag, "source": source}},
    ]);
    serde_json::from_value(json).expect("a resolution")
}

/// The cells `cells_of` builds for the job over `file` whose levels are `levels`, by the tag of
/// each cell's treatment.
fn cells_by_tag(
    file: &candler_schemas::execution::ExperimentFile,
    levels: &[candler_core::resolve::layers::Resolution],
) -> BTreeMap<String, candler_core::runner::exec::cell::CellRun> {
    let facts = facts();
    let (_dir, store, lock) = support::run_plan::fresh_store();
    let ctx = support::run_plan::plan_ctx(&store, &lock, &facts);
    let outcome =
        plan_runs(&ctx, file, Levels::from(levels), Force::No, PlanMode::Check).expect("a plan");
    let mut base = spec(&["m1"], &facts, 3600, 1);
    base.experiment = file.clone(); // BORROW: the spec owns its file
    base.resolution = levels[0].clone(); // BORROW: the spec owns its primary level
    let mut json = serde_json::to_value(&base).expect("JSON");
    if levels.len() > 1 {
        json["levels"] = serde_json::to_value(levels).expect("JSON");
    }
    let job: candler_core::jobs::model::JobSpec = serde_json::from_value(json).expect("a spec");
    cells_of(&job, &outcome, &BTreeSet::new(), &BTreeMap::new())
        .expect("cells")
        .values()
        .map(|cell| {
            let treatment = serde_json::to_value(cell.comparable.treatment()).expect("JSON");
            let tag = treatment["tag"].as_str().expect("a tag").to_owned(); // BORROW: owned tag
            (tag, (**cell).clone())
        })
        .collect()
}

/// Guards: T3 wiring (C3) -- each cell of a two-level job carries its own level's treatment
/// checkout and harness, never the first level's.
/// Oracle: the checkouts `/src/a` and `/src/b` and the harness versions `2.1.0` and `2.2.0`
/// written below, one pair for each level's tag.
/// Bite: build every cell from the job's primary level.
#[test]
fn t3_each_cell_runs_its_own_treatment() {
    let levels = [
        level_of("v1", "/src/a", "2.1.0"),
        level_of("v2", "/src/b", "2.2.0"),
    ];
    let cells = cells_by_tag(&two_level_file(&["htn", "other"]), &levels);
    assert_eq!(cells.len(), 2, "one cell per level");
    for (tag, source, version) in [("v1", "/src/a", "2.1.0"), ("v2", "/src/b", "2.2.0")] {
        let cell = cells.get(tag).expect("the level's cell");
        assert_eq!(
            cell.treatment_source,
            Some(PathBuf::from(source)),
            "the checkout of {tag}"
        );
        let harness = serde_json::to_value(cell.comparable.harness()).expect("JSON");
        assert_eq!(harness["version"], json!(version), "the harness of {tag}");
    }
}

/// Guards: T3 wiring (C3) -- a one-level job gives the cell it gave before levels: its
/// treatment checkout and harness come from the one resolution.
/// Oracle: the checkout `/src/a` and the version `2.1.0` written below.
/// Bite: look the cell's level up among the job's levels and refuse a job that holds none.
#[test]
fn t3_one_level_job_unchanged() {
    let levels = [level_of("v1", "/src/a", "2.1.0")];
    let cells = cells_by_tag(&two_level_file(&["htn"]), &levels);
    assert_eq!(cells.len(), 1, "one level, one cell");
    let cell = cells.get("v1").expect("the cell");
    assert_eq!(cell.treatment_source, Some(PathBuf::from("/src/a")));
    let harness = serde_json::to_value(cell.comparable.harness()).expect("JSON");
    assert_eq!(harness["version"], json!("2.1.0"));
}

/// Guards: C3 -- a spec of one level serialises with the keys it had before levels existed, and a
/// spec of two levels adds `levels`.
/// Oracle: the key list written below, which is the spec's field list before levels.
/// Bite: serialise an empty `levels`.
#[test]
fn t3_one_level_spec_has_no_levels_key() {
    let mut one = spec(&["m1"], &facts(), 3600, 1);
    let mut keys: Vec<String> = serde_json::to_value(&one)
        .expect("JSON")
        .as_object()
        .expect("an object")
        .keys()
        .cloned() // BORROW: owned key names
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "budget",
            "evaluation_inputs",
            "experiment",
            "force",
            "inputs",
            "plans",
            "resolution"
        ]
    );
    one.levels = vec![one.resolution.clone(), one.resolution.clone()]; // BORROW: the spec owns its levels
    assert!(serde_json::to_value(&one).expect("JSON")["levels"].is_array());
}
