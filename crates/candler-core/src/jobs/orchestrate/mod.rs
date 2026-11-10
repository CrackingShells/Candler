//! The orchestration of a job's runs: scheduling, caps and teardown.
//!
//! A job plans its runs under the experiment lock, starts its gateway on networks of its own,
//! runs the plans under the parallel cap and the machine's slots, stops runs in flight when a job
//! cap is crossed, and destroys what it made through the container guard on every path.

pub mod caps;
pub mod schedule;
pub mod session;
pub mod teardown;

use std::collections::BTreeMap;
use std::sync::Arc;

use candler_schemas::execution::JobEnd;
use futures::future::BoxFuture;

use self::schedule::Scheduler;
use self::session::{
    DockerHost, EnvSecrets, JobHost, JobSession, cells_of, image_of, record_packages,
};
use crate::isolation::model::{ImageId, ImageRef, SweepScope};
use crate::jobs::model::{JobControl, JobOutcome, JobSpec};
use crate::runner::plan::{Force, PlanCtx, PlanMode, experiment_of, plan_runs};
use crate::runner::{JobMeter, Runtime};

/// Runs the runs of `spec` under `rt` on the machine's docker and says how the job ended.
pub fn orchestrate(rt: Runtime, spec: JobSpec, ctl: JobControl) -> BoxFuture<'static, JobOutcome> {
    orchestrate_with(rt, spec, ctl, Arc::new(DockerHost))
}

/// Runs the runs of `spec` under `rt` on `host` and says how the job ended.
pub fn orchestrate_with(
    rt: Runtime,
    spec: JobSpec,
    ctl: JobControl,
    // TRAIT_OBJECT: the host is docker, or a fake in tests
    host: Arc<dyn JobHost>,
) -> BoxFuture<'static, JobOutcome> {
    Box::pin(run_job(rt, spec, ctl, host))
}

/// The outcome of a job that never started a run.
const fn failed() -> JobOutcome {
    JobOutcome {
        end: JobEnd::Failed,
        runs: Vec::new(),
    }
}

/// Writes one line to the supervisor's stderr that names why a job could not start. Errors name
/// a secret, never its value.
fn refused(what: &str, error: &impl std::fmt::Display) -> JobOutcome {
    #[expect(clippy::print_stderr, reason = "the supervisor's stderr is its log")]
    {
        eprintln!("orchestrate: {what}: {error}");
    }
    failed()
}

/// Plans, opens the session, runs the plans and closes the session.
async fn run_job(
    mut rt: Runtime,
    spec: JobSpec,
    ctl: JobControl,
    // TRAIT_OBJECT: the host is the process's own host, or a fake in tests
    host: Arc<dyn JobHost>,
) -> JobOutcome {
    let secrets = EnvSecrets::new(spec.experiment.secrets());
    let fresh = match experiment_of(&spec.experiment, &spec.resolution) {
        Ok(fresh) => fresh,
        Err(error) => return refused("the experiment cannot be identified", &error),
    };
    let ctx = PlanCtx {
        store: rt.store.clone(), // BORROW: the context owns its handle on the store
        experiment_lock: rt.store.layout().experiment_lock(&fresh),
        secrets_present: secrets.present(),
    };
    let force = if spec.force { Force::Yes } else { Force::No };
    let outcome = match plan_runs(
        &ctx,
        &spec.experiment,
        &spec.resolution,
        force,
        PlanMode::Hold,
    ) {
        Ok(outcome) => outcome,
        Err(error) => return refused("the plan was refused", &error),
    };
    let names = record_packages(&spec.resolution);
    let mut packages = BTreeMap::new();
    if !names.is_empty() {
        // EXPLICIT: the loop fills `packages` and skips images it already holds; .map() would hide the state update
        for comparable in outcome.fresh.values() {
            let Some(id) = image_of(comparable) else {
                continue;
            };
            if packages.contains_key(&id) {
                continue;
            }
            // BORROW: the image reference owns the id it names
            let image = ImageRef {
                id: ImageId(id.clone()),
                cc_version: None,
            };
            if let Some(readings) = host.packages(&rt, &image, &names).await {
                packages.insert(id, readings);
            }
        }
    }
    let cells = match cells_of(&spec, &outcome, &secrets.present(), &packages) {
        Ok(cells) => cells,
        Err(error) => return refused("the cells cannot be built", &error),
    };
    rt.cells = cells;
    let session =
        match JobSession::open(&mut rt, &spec, &outcome.experiment, &*host, &secrets).await {
            Ok(session) => session,
            Err(error) => return refused("the job's session cannot be opened", &error),
        };
    let _lock = outcome.lock;
    let scheduler = Scheduler::with_clock(
        Arc::new(rt),
        spec.budget.clone(), // BORROW: the scheduler owns the job's caps
        Arc::new(JobMeter::new()),
        host.clock(),
    );
    let schedule = scheduler.run(outcome.plans, ctl.cancel.subscribe()).await;
    // the report names what stayed behind; the job's end stays what its runs said
    let _report = session.close(&SweepScope::CurrentJob).await;
    JobOutcome {
        end: schedule.end,
        runs: schedule.runs,
    }
}
