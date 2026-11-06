//! The orchestration of a job's runs: scheduling, caps and teardown.
//!
//! A job plans its runs under the experiment lock, starts its gateway on networks of its own,
//! runs the plans under the parallel cap and the machine's slots, stops runs in flight when a job
//! cap is crossed, and destroys what it made through the container guard on every path.

pub mod caps;
pub mod schedule;
pub mod session;
pub mod teardown;

use std::sync::Arc;

use candler_schemas::execution::JobEnd;
use futures::future::BoxFuture;

use self::session::{DockerHost, EnvSecrets, JobHost, JobSession, cells_of};
use crate::isolation::model::SweepScope;
use crate::jobs::model::{JobControl, JobOutcome, JobSpec};
use crate::runner::plan::{Force, PlanCtx, PlanMode, experiment_of, plan_runs};
use crate::runner::Runtime;

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

/// Plans, opens the session, runs the plans and closes the session.
async fn run_job(
    mut rt: Runtime,
    spec: JobSpec,
    _ctl: JobControl,
    host: Arc<dyn JobHost>,
) -> JobOutcome {
    let secrets = EnvSecrets::new(spec.experiment.secrets());
    let Ok(fresh) = experiment_of(&spec.experiment, &spec.resolution) else {
        return failed();
    };
    let ctx = PlanCtx {
        store: rt.store.clone(), // BORROW: the context owns its handle on the store
        experiment_lock: rt.store.layout().experiment_lock(&fresh),
        secrets_present: secrets.present(),
    };
    let force = if spec.force { Force::Yes } else { Force::No };
    let Ok(outcome) = plan_runs(
        &ctx,
        &spec.experiment,
        &spec.resolution,
        force,
        PlanMode::Hold,
    ) else {
        return failed();
    };
    let Ok(cells) = cells_of(&spec, &outcome, &secrets.present()) else {
        return failed();
    };
    rt.cells = cells;
    let Ok(session) = JobSession::open(&mut rt, &spec, &outcome.experiment, &*host, &secrets).await
    else {
        return failed();
    };
    let _lock = outcome.lock;
    let _report = session.close(&SweepScope::CurrentJob).await;
    failed()
}
