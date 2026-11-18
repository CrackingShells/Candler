//! The blocked tests on simulated experiments of known truth hold their nominal error rate.
//!
//! Each experiment is drawn from a seeded `SplitMix64` in the shape `analyze` reads: two arms of
//! `plan.model` (`alpha`, `beta`) over blocks of `plan.other`, a fixed number of runs per cell.
//! The manifests of a design do not depend on the draw, so a design is built once and each
//! simulation only chooses which outcome record every run carries.

use std::collections::BTreeSet;
use std::fs;
use std::sync::Arc;
use std::time::Instant;

use candler_core::analysis::report::{Comparison, Entry, Report, ReportInput, RunView, Tested};
use candler_core::analysis::splitmix::SplitMix64;
use candler_core::analysis::tally::RunFacts;
use candler_core::evaluation::selection::select::select;
use candler_schemas::execution::RunManifest;
use candler_schemas::outcome::OutcomeRecord;
use candler_schemas::primitives::NameVersion;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::support::job_orchestrate::{Rig, skill_repo, spec};
use crate::support::run_plan::Facts;
use candler_core::analysis::blocking::Battery;
use candler_core::jobs::model::{JobCause, JobControl, JobOutcome};
use candler_core::jobs::orchestrate::orchestrate_with;
use candler_schemas::execution::ExperimentFile;

const ALPHA: f64 = 0.05;
/// The two-sided 99 % normal quantile.
const Z99: f64 = 2.575_829_303_549;

fn digest_text(c: char) -> String {
    format!("sha256:{}", String::from(c).repeat(64))
}

/// The manifest of one run of `model` in `block` (`None` for no block factor). The key carries the
/// cell's real digest.
fn manifest(model: &str, block: Option<&str>, replicate: u32) -> RunManifest {
    let mut values = vec![json!({"path": "plan.model", "value": model})];
    let mut factors = vec!["plan.model"];
    if let Some(level) = block {
        values.push(json!({"path": "plan.other", "value": level}));
        factors.push("plan.other");
    }
    let mut sources: Vec<Value> = values
        .iter()
        .map(|v| json!({"path": v["path"], "source": {"kind": "experiment_file"}}))
        .collect();
    sources.push(json!({"path": "harness", "source": {"kind": "experiment_file"}}));
    sources.push(json!({"path": "treatment", "source": {"kind": "experiment_file"}}));
    let measured: Vec<Value> = [
        ("provenance.eval_infra.commit", json!("abc1234")),
        ("provenance.eval_infra.dirty", json!(false)),
        ("provenance.sandbox_contract", json!("candler-container@1")),
    ]
    .into_iter()
    .map(|(path, value)| json!({"path": path, "value": value, "source": {"kind": "experiment_file"}}))
    .collect();
    let body = |cell: &str| {
        json!({
            "schema": "run-manifest/1",
            "run": {"cell": cell, "replicate": replicate, "attempt": 1},
            "experiment": digest_text('0'),
            "factors": factors,
            "comparable": {
                "values": values,
                "harness": {"name": "h", "version": "1.0.0"},
                "treatment": {"name": "t"},
                "snapshots": [],
                "inputs": [],
                "environments": []
            },
            "sources": sources,
            "secrets": [],
            "measured": measured,
            "deviations": [],
            "integrity": {"kind": "valid"}
        })
    };
    let probe: RunManifest = serde_json::from_value(body(&digest_text('f'))).expect("a manifest");
    let cell = probe.cell_digest().to_string();
    serde_json::from_value(body(cell.as_str())).expect("a manifest") // BORROW: explicit .as_str()
}

fn record(
    run: &RunView,
    n: usize,
    expectation: &str,
    role: &str,
    status: &str,
    weight: Option<(u32, u32)>,
) -> OutcomeRecord {
    let key = run.manifest.run();
    let mut doc = json!({
        "run": {"cell": key.cell().to_string(), "replicate": key.replicate(), "attempt": key.attempt()},
        "expectation_id": expectation,
        "role": role,
        "evaluator": "suite-eval@1.0.0",
        "instruments": [],
        "suite_entry_digest": digest_text('1'),
        "context_digest": digest_text('2'),
        "request_id": format!("01926b3a-7c3e-7a1b-8c4d-{n:012x}"),
        "status": status,
        "evidence": [],
    });
    if let Some((num, den)) = weight {
        doc["weight"] = json!({"num": num, "den": den});
    }
    serde_json::from_value(doc).expect("an outcome record")
}

/// A cell of a design: its model, its block and its number of runs.
#[derive(Clone, Copy)]
struct Cell {
    model: &'static str,
    block: Option<&'static str>,
    runs: u32,
}

/// A design: the manifests of its runs and, for each run, the records it may carry, indexed by
/// the run's outcome.
struct Design {
    cells: Vec<Cell>,
    views: Vec<RunView>,
    records: Vec<Vec<OutcomeRecord>>,
    /// The cell of each run, by position in `cells`.
    cell_of: Vec<usize>,
}

impl Design {
    /// `weighted` designs grade `E3` with weights over 4 (outcome 0 to 4, a pass at 4); the others
    /// grade `E1` pass or fail (outcome 0 or 1).
    fn new(cells: &[Cell], weighted: bool) -> Self {
        Self::with_den(cells, weighted.then_some(4))
    }

    /// `den` of `Some(d)` grades `E3` with weights over `d` (outcome 0 to `d`, a pass at `d`).
    fn with_den(cells: &[Cell], den: Option<u32>) -> Self {
        let weighted = den.is_some();
        let mut views = Vec::new();
        let mut records = Vec::new();
        let mut cell_of = Vec::new();
        for (at, cell) in cells.iter().enumerate() {
            for replicate in 1..=cell.runs {
                let manifest = manifest(cell.model, cell.block, replicate);
                let view = RunView {
                    facts: RunFacts::new(manifest.run().clone(), manifest.integrity().clone()),
                    manifest,
                };
                let n = views.len() + 1;
                let held = if weighted {
                    let den = den.unwrap_or(4);
                    (0..=den)
                        .map(|num| {
                            let status = if num == den { "pass" } else { "fail" };
                            record(&view, n, "E3", "quality", status, Some((num, den)))
                        })
                        .collect()
                } else {
                    ["fail", "pass"]
                        .map(|status| record(&view, n, "E1", "gate", status, None))
                        .to_vec()
                };
                records.push(held);
                views.push(view);
                cell_of.push(at);
            }
        }
        Self {
            cells: cells.to_vec(),
            views,
            records,
            cell_of,
        }
    }

    /// The report over the runs carrying the record `outcomes` names for each.
    fn report(&self, outcomes: &[usize]) -> Report {
        let chosen = self
            .records
            .iter()
            .zip(outcomes)
            .map(|(held, outcome)| held.get(*outcome).expect("an outcome").clone());
        Report::build(ReportInput {
            selection: select(chosen),
            evaluator: "suite-eval@1.0.0"
                .parse::<NameVersion>()
                .expect("name@version"),
            runs: self.views.clone(),
            unsealed: Vec::new(),
        })
        .expect("the report builds")
    }
}

/// A uniform draw on `[0, 1)`.
fn uniform(rng: &mut SplitMix64) -> f64 {
    let high = u32::try_from(rng.next_u64() >> 32).unwrap_or(0);
    f64::from(high) / 4_294_967_296.0
}

/// Binomial draws of `trials` at rate `p`.
fn binomial(rng: &mut SplitMix64, trials: u32, p: f64) -> usize {
    (0..trials).filter(|_| uniform(rng) < p).count()
}

/// The comparison of `plan.model` and its one tested entry.
fn model_test(report: &Report) -> &Tested {
    let comparison: &Comparison = report
        .comparisons
        .iter()
        .find(|c| c.factor.to_string() == "plan.model")
        .expect("a comparison of plan.model");
    let [Entry::Tested(tested)] = comparison.entries.as_slice() else {
        panic!("one tested entry: {:?}", comparison.entries);
    };
    tested
}

/// Draws one experiment of a binary design whose cell `i` passes at rate `rates[i]` and returns
/// the passes of each cell.
fn draw_binary(design: &Design, rates: &[f64], rng: &mut SplitMix64) -> Vec<usize> {
    design
        .cells
        .iter()
        .zip(rates)
        .map(|(cell, p)| binomial(rng, cell.runs, *p))
        .collect()
}

/// The outcome of each run when the first `passes[i]` runs of cell `i` pass.
fn outcomes_of(design: &Design, passes: &[usize]) -> Vec<usize> {
    let mut seen = vec![0_usize; passes.len()];
    design
        .cell_of
        .iter()
        .map(|cell| {
            let count = seen.get_mut(*cell).expect("a cell");
            *count += 1;
            let limit = *passes.get(*cell).expect("a cell");
            usize::from(*count <= limit)
        })
        .collect()
}

/// The rejection rate and the half-width of its 99 % interval around the rate itself.
fn rate_of(rejections: u32, trials: u32) -> (f64, f64) {
    let rate = f64::from(rejections) / f64::from(trials);
    (rate, Z99 * (rate * (1.0 - rate) / f64::from(trials)).sqrt())
}

/// Whether `value` lies in the 99 % interval of the rejection rate, the normal one around `value`
/// with its own variance, so the check does not widen with a rate that strays.
fn holds(rejections: u32, trials: u32, value: f64) -> bool {
    let rate = f64::from(rejections) / f64::from(trials);
    (rate - value).abs() <= Z99 * (value * (1.0 - value) / f64::from(trials)).sqrt()
}

/// Workers a simulation spreads its experiments over.
const WORKERS: u64 = 4;

/// Runs `trials` experiments over `WORKERS` threads, each with its own stream of `seed`, and sums
/// the two counts `experiment` returns.
fn simulate<F>(trials: u32, seed: u64, experiment: F) -> (u32, u32)
where
    F: Fn(&mut SplitMix64) -> (u32, u32) + Sync,
{
    let share = trials / u32::try_from(WORKERS).expect("small");
    std::thread::scope(|scope| {
        let workers: Vec<_> = (1..=WORKERS)
            .map(|w| {
                let run = &experiment;
                scope.spawn(move || {
                    let mut rng = SplitMix64::new(seed ^ w.wrapping_mul(0x9E37_79B9_7F4A_7C15));
                    (0..share).fold((0, 0), |held, _| {
                        let (x, y) = run(&mut rng);
                        (held.0 + x, held.1 + y)
                    })
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().expect("a worker"))
            .fold((0, 0), |held, (x, y)| (held.0 + x, held.1 + y))
    })
}

/// The rejection count of `trials` binary experiments over `design` at `rates`, by the blocked
/// test of `plan.model`.
fn rejections_blocked(design: &Design, rates: &[f64], trials: u32, seed: u64) -> u32 {
    simulate(trials, seed, |rng| {
        let passes = draw_binary(design, rates, rng);
        let report = design.report(&outcomes_of(design, &passes));
        (u32::from(model_test(&report).p < ALPHA), 0)
    })
    .0
}

/// Two arms over two blocks, `runs` per cell in the order alpha x, alpha y, beta x, beta y.
fn two_by_two(runs: [u32; 4]) -> Vec<Cell> {
    let [ax, ay, bx, by] = runs;
    [
        ("alpha", "x", ax),
        ("alpha", "y", ay),
        ("beta", "x", bx),
        ("beta", "y", by),
    ]
    .map(|(model, block, runs)| Cell {
        model,
        block: Some(block),
        runs,
    })
    .to_vec()
}

/// Guards: the false-positive rate of the blocked binary test under H0. Both arms pass at 0.3 in
/// block x and 0.6 in block y (equal within blocks, unequal between them), 12 runs per cell; the
/// 99 % binomial interval of the rejection rate of 2 000 experiments at 0.05 contains 0.05.
/// Oracle: the planted truth (no difference within any block) and the binomial variance of a
/// rate of 0.05 over 2 000 trials.
/// Bite: report Fisher's p in place of the mid-p, or a p of 0.01, and the rate leaves the
/// interval.
#[test]
fn kt_h0_binary_nominal() {
    let design = Design::new(&two_by_two([12; 4]), false);
    let trials = 2_000;
    let start = Instant::now();
    let rejected = rejections_blocked(&design, &[0.3, 0.6, 0.3, 0.6], trials, 0x00C0_FFEE);
    let (rate, half) = rate_of(rejected, trials);
    println!(
        "kt_h0_binary_nominal: {rejected}/{trials} = {rate:.4} (+-{half:.4}), {:?}",
        start.elapsed()
    );
    assert!(
        holds(rejected, trials, ALPHA),
        "rejection rate {rate:.4} outside the 99 % interval of 0.05"
    );
}

/// Guards: the power of the blocked binary test at a difference of 0.2 (B minus A in every
/// block: 0.3 to 0.5 in block x, 0.5 to 0.7 in block y), 12 runs per cell, 1 500 experiments. It
/// is reported with its 95 % interval and must lie above alpha.
/// Oracle: the planted difference of 0.2; a test that rejects above its size has some power.
/// Bite: a test that never rejects (p of 1) turns it red; it asserts only that power exceeds
/// alpha.
#[test]
fn kt_power_at_0_2() {
    let design = Design::new(&two_by_two([12; 4]), false);
    let trials = 1_500;
    let rejected = rejections_blocked(&design, &[0.3, 0.5, 0.5, 0.7], trials, 0x0BAD_5EED);
    let power = f64::from(rejected) / f64::from(trials);
    let half = 1.96 * (power * (1.0 - power) / f64::from(trials)).sqrt();
    println!(
        "kt_power_at_0_2: {rejected}/{trials} = {power:.4} (95 % interval {:.4} to {:.4})",
        power - half,
        power + half
    );
    assert!(power > ALPHA, "power {power:.4} is not above alpha");
}

/// Guards: Simpson's paradox. H0 holds within blocks (both arms pass at 0.8 in block x and 0.2 in
/// block y) but alpha runs mostly in x (20 and 5 runs) and beta mostly in y (5 and 20), so the
/// pooled arms differ (about 0.68 against 0.32). Over 2 000 experiments the blocked rate holds
/// 0.05; the pooled test of the same counts rejects at a rate outside that interval.
/// Oracle: the planted truth, and the pooled rates computed from the allocation by arithmetic.
/// Bite: pool the blocks in `analyze` and the blocked rate leaves the interval.
#[test]
fn kt_simpson_blocked_holds() {
    let blocked = Design::new(&two_by_two([20, 5, 5, 20]), false);
    let pooled = Design::new(
        &[
            Cell {
                model: "alpha",
                block: None,
                runs: 25,
            },
            Cell {
                model: "beta",
                block: None,
                runs: 25,
            },
        ],
        false,
    );
    let trials = 2_000;
    let (by_block, by_pool) = simulate(trials, 0x0051_3350, |rng| {
        let passes = draw_binary(&blocked, &[0.8, 0.2, 0.8, 0.2], rng);
        let report = blocked.report(&outcomes_of(&blocked, &passes));
        let [ax, ay, bx, by] = passes.as_slice() else {
            panic!("four cells");
        };
        let flat = pooled.report(&outcomes_of(&pooled, &[ax + ay, bx + by]));
        (
            u32::from(model_test(&report).p < ALPHA),
            u32::from(model_test(&flat).p < ALPHA),
        )
    });
    println!("kt_simpson_blocked_holds: blocked {by_block}/{trials}, pooled {by_pool}/{trials}");
    assert!(
        holds(by_block, trials, ALPHA),
        "blocked rate {by_block}/{trials} outside the interval of 0.05"
    );
    assert!(
        !holds(by_pool, trials, ALPHA),
        "pooled rate {by_pool}/{trials} inside the interval of 0.05"
    );
}

/// Guards: the false-positive rate of the weighted (permutation) test under H0. Outcomes are
/// counts over 4 at rate 0.4 in block x and 0.7 in block y for both arms, 6 runs per cell; the
/// count of experiments is set by the measured runtime. The 99 % binomial interval of the rejection
/// rate at 0.05 contains 0.05.
/// Oracle: the planted truth and the binomial variance of a rate of 0.05.
/// Bite: a constant p of 0.01 leaves the interval.
#[test]
fn kt_h0_weighted_nominal() {
    const WEIGHTED_TRIALS: u32 = 1_000;
    let design = Design::new(&two_by_two([6; 4]), true);
    let rates = [0.4, 0.7, 0.4, 0.7];
    let start = Instant::now();
    let (rejected, _) = simulate(WEIGHTED_TRIALS, 0x0E16_47ED, |rng| {
        let outcomes: Vec<usize> = design
            .cell_of
            .iter()
            .map(|cell| binomial(rng, 4, *rates.get(*cell).expect("a rate")))
            .collect();
        let report = design.report(&outcomes);
        (u32::from(model_test(&report).p < ALPHA), 0)
    });
    let (rate, half) = rate_of(rejected, WEIGHTED_TRIALS);
    println!(
        "kt_h0_weighted_nominal: {rejected}/{WEIGHTED_TRIALS} = {rate:.4} (+-{half:.4}), {:?}",
        start.elapsed()
    );
    assert!(
        holds(rejected, WEIGHTED_TRIALS, ALPHA),
        "rejection rate {rate:.4} outside the 99 % interval of 0.05"
    );
}

/// Guards: the false-positive rate of the one-block binary test (the mid-p) under H0. Both arms
/// pass at 0.3 in the one block, 12 runs per arm; the 99 % binomial interval of the rejection
/// rate of 4 000 experiments at 0.05 contains 0.05.
/// Oracle: the planted truth (no difference) and the binomial variance of a rate of 0.05 over
/// 4 000 trials.
/// Bite: report Fisher's p in place of the mid-p on the one-block path and the rate leaves the
/// interval.
#[test]
fn kt_h0_one_block_nominal() {
    let cells = [("alpha", 12), ("beta", 12)].map(|(model, runs)| Cell {
        model,
        block: Some("x"),
        runs,
    });
    let design = Design::new(&cells, false);
    let trials = 4_000;
    let start = Instant::now();
    let rejected = rejections_blocked(&design, &[0.3, 0.3], trials, 0x0001_B10C);
    let (rate, half) = rate_of(rejected, trials);
    println!(
        "kt_h0_one_block_nominal: {rejected}/{trials} = {rate:.4} (+-{half:.4}), {:?}",
        start.elapsed()
    );
    assert!(
        holds(rejected, trials, ALPHA),
        "rejection rate {rate:.4} outside the 99 % interval of 0.05"
    );
}

/// Guards: the false-positive rate of the weighted (permutation) test under H0 when the weights
/// carry no ties. Weights are on a grid of 1 000 (uniform up to 0.4 in block x and 0.7 in block y
/// for both arms), 6 runs per cell, 1 000 experiments; the 99 % binomial interval of the
/// rejection rate at 0.05 contains 0.05. With the five weights of `kt_h0_weighted_nominal` the
/// statistic ties often and the rate falls to about half of alpha.
/// Oracle: the planted truth and the binomial variance of a rate of 0.05.
/// Bite: a constant p of 0.01 leaves the interval.
#[test]
fn kt_h0_weighted_continuous() {
    const DEN: u32 = 1_000;
    const TRIALS: u32 = 1_000;
    let design = Design::with_den(&two_by_two([6; 4]), Some(DEN));
    let upper = [400_u32, 700, 400, 700];
    let start = Instant::now();
    let (rejected, _) = simulate(TRIALS, 0x0E16_47ED, |rng| {
        let outcomes: Vec<usize> = design
            .cell_of
            .iter()
            .map(|cell| {
                let top = *upper.get(*cell).expect("a limit");
                let draw = u32::try_from(rng.next_u64() >> 32).unwrap_or(0);
                usize::try_from(draw % top).unwrap_or(0)
            })
            .collect();
        let report = design.report(&outcomes);
        (u32::from(model_test(&report).p < ALPHA), 0)
    });
    let (rate, half) = rate_of(rejected, TRIALS);
    println!(
        "kt_h0_weighted_continuous: {rejected}/{TRIALS} = {rate:.4} (+-{half:.4}), {:?}",
        start.elapsed()
    );
    assert!(
        holds(rejected, TRIALS, ALPHA),
        "rejection rate {rate:.4} outside the 99 % interval of 0.05"
    );
}

/// The treatment tags of the golden shape, and its models.
const TAGS: [&str; 4] = ["v1", "v2", "v3", "v4"];
const MODELS: [&str; 3] = ["m1", "m2", "m3"];
const REPLICATES: u32 = 4;

/// Plans and runs the golden job (4 treatment levels by 3 models, `REPLICATES` runs a cell) on the
/// fakes.
fn golden_job(rig: &Rig) -> JobOutcome {
    let models = MODELS.map(|m| format!("\"{m}\"")).join(", ");
    let names = ["htn", "t2", "t3", "t4"]
        .map(|n| format!("\"{n}\""))
        .join(", ");
    let text = format!(
        "schema = \"experiment/1\"\nreplicates = {REPLICATES}\n\n[[factors]]\npath = \"model.id\"\nlevels = [{models}]\n\n[[factors]]\npath = \"request.treatment\"\nlevels = [{names}]\n\n[budget.per_run]\nwall_s = 3600\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"htn\"\n"
    );
    let mut job = spec(&["m1"], &Facts::base(), 3600, 4);
    job.experiment = ExperimentFile::from_toml_str(&text).expect("an experiment file");
    let (repo, _) = skill_repo();
    let base = serde_json::to_value(&job.resolution).expect("JSON");
    job.levels = TAGS
        .map(|tag| {
            let mut json = base.clone();
            for value in json["values"]
                .as_array_mut()
                .expect("values")
                .iter_mut()
                .filter(|value| value["path"] == "treatment")
            {
                value["value"]["tag"] = json!(tag);
            }
            for setting in json["settings"]
                .as_array_mut()
                .expect("settings")
                .iter_mut()
            {
                if setting["path"] == "request.treatment" {
                    setting["value"] =
                        json!({"name": "htn", "tag": tag, "source": repo.display().to_string()});
                }
            }
            serde_json::from_value(json).expect("a resolution")
        })
        .to_vec();
    job.resolution = job.levels.first().expect("a level").clone();
    let (cancel, _keep) = watch::channel::<Option<JobCause>>(None);
    let ctl = JobControl {
        cancel: Arc::new(cancel),
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    runtime.block_on(orchestrate_with(rig.runtime(), job, ctl, rig.host()))
}

/// The tag a typed treatment reference carries.
fn tag_of(level: &Value) -> String {
    level["tag"].as_str().expect("a tag").to_owned() // BORROW: .to_owned() makes the owned tag
}

/// Guards: the golden shape end to end. A job of 4 treatment levels by 3 models, 4 replicates a
/// cell, is planned and run on the fakes; the outcomes planted afterwards are v1 passing 0 of 4 in
/// every model, v2 4 of 4, v3 and v4 2 of 4. `analyze` then gives 6 treatment comparisons, each
/// blocked by model (3 blocks), and 3 model comparisons, each blocked by treatment (4 blocks), the
/// CMH battery on the treatment pairs, and on the planted pair a p below alpha and the planted
/// direction (v2 over v1, B minus A) in every block (the interval of a 0 of 4 against 4 of 4 odds
/// ratio is undefined, so its reading is not asserted).
/// Oracle: the shape's arithmetic (C(4,2) = 6 pairs, C(3,2) = 3 pairs) and the planted counts.
/// Bite: leave the treatment out of the declared factors and the comparisons are not 6 and 3.
#[test]
fn kt_two_levels_end_to_end() {
    let rig = Rig::new(0);
    let outcome = golden_job(&rig);
    assert_eq!(
        outcome.runs.len(),
        48,
        "{:?}: every run of the plan ran",
        outcome.end
    );
    let root = rig.store.layout().root().join("runs");
    let mut views = Vec::new();
    for entry in fs::read_dir(&root).expect("the runs directory") {
        let path = entry.expect("an entry").path().join("manifest.json");
        // The fakes seal their runs as infrastructure failures with no evaluation provenance; the
        // planted outcomes need valid runs, so the integrity and the provenance are planted too.
        // Neither is part of the cell, so the keys stay the ones the plan gave.
        let mut body: Value =
            serde_json::from_slice(&fs::read(path).expect("a manifest")).expect("manifest JSON");
        body["integrity"] = json!({"kind": "valid"});
        for (name, value) in [
            ("provenance.eval_infra.commit", json!("abc1234")),
            ("provenance.eval_infra.dirty", json!(false)),
        ] {
            body["measured"]
                .as_array_mut()
                .expect("measured")
                .push(json!({"path": name, "value": value, "source": {"kind": "experiment_file"}}));
        }
        let manifest: RunManifest = serde_json::from_value(body).expect("a manifest");
        views.push(RunView {
            facts: RunFacts::new(manifest.run().clone(), manifest.integrity().clone()),
            manifest,
        });
    }
    assert_eq!(views.len(), 48, "every run is sealed with a manifest");
    let passes = |tag: &str| match tag {
        "v1" => 0,
        "v2" => REPLICATES,
        _ => REPLICATES / 2,
    };
    let records: Vec<OutcomeRecord> = views
        .iter()
        .enumerate()
        .map(|(n, view)| {
            let treatment =
                serde_json::to_value(view.manifest.comparable().treatment()).expect("JSON");
            let status = if view.manifest.run().replicate() <= passes(&tag_of(&treatment)) {
                "pass"
            } else {
                "fail"
            };
            record(view, n + 1, "E1", "gate", status, None)
        })
        .collect();
    let report = Report::build(ReportInput {
        selection: select(records),
        evaluator: "suite-eval@1.0.0"
            .parse::<NameVersion>()
            .expect("name@version"),
        runs: views,
        unsealed: Vec::new(),
    })
    .expect("the report builds");
    let by = |factor: &str| -> Vec<&Comparison> {
        report
            .comparisons
            .iter()
            .filter(|c| c.factor.to_string() == factor)
            .collect()
    };
    let (treatment, model) = (by("request.treatment"), by("model.id"));
    assert_eq!(
        treatment.len(),
        6,
        "{:?}",
        report
            .comparisons
            .iter()
            .map(|c| c.factor.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(model.len(), 3);
    for comparison in &treatment {
        let [Entry::Tested(tested)] = comparison.entries.as_slice() else {
            panic!("one tested entry: {:?}", comparison.entries);
        };
        assert_eq!(
            tested.blocks, 3,
            "a treatment pair is blocked by the 3 models"
        );
        assert!(matches!(
            tested.battery,
            Some(Battery::Cmh | Battery::ExactStrata)
        ));
    }
    for comparison in &model {
        let [Entry::Tested(tested)] = comparison.entries.as_slice() else {
            panic!("one tested entry: {:?}", comparison.entries);
        };
        assert_eq!(
            tested.blocks, 4,
            "a model pair is blocked by the 4 treatments"
        );
    }
    let tags: BTreeSet<(String, String)> = treatment
        .iter()
        .map(|c| {
            let (a, b) = c.levels.clone().expect("the levels");
            (tag_of(&a), tag_of(&b))
        })
        .collect();
    assert_eq!(tags.len(), 6, "six distinct level pairs");
    let planted = treatment
        .iter()
        .find(|c| {
            let (a, b) = c.levels.clone().expect("the levels");
            BTreeSet::from([tag_of(&a), tag_of(&b)])
                == BTreeSet::from(["v1".to_owned(), "v2".to_owned()])
        })
        .expect("the planted pair");
    let [Entry::Tested(tested)] = planted.entries.as_slice() else {
        panic!("one tested entry");
    };
    let (a, _) = planted.levels.clone().expect("the levels");
    let sign = if tag_of(&a) == "v1" { 1.0 } else { -1.0 };
    assert!(tested.p < ALPHA);
    for effect in &tested.per_block {
        assert!(
            (effect.diff - sign).abs() < 1e-12,
            "B minus A is {sign} in {:?}",
            effect.key
        );
    }
}
