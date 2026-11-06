//! The orchestration of a job's runs: scheduling, caps and teardown.

pub mod caps;
pub mod schedule;
pub mod session;
pub mod teardown;

use futures::future::BoxFuture;

use crate::jobs::model::{JobControl, JobOutcome, JobSpec};
use crate::runner::Runtime;

/// Runs the runs of `spec` under `rt` and says how the job ended.
pub fn orchestrate(rt: Runtime, spec: JobSpec, ctl: JobControl) -> BoxFuture<'static, JobOutcome> {
    let _ = (rt, spec, ctl);
    Box::pin(async {
        JobOutcome {
            end: candler_schemas::execution::JobEnd::Failed,
            runs: Vec::new(),
        }
    })
}
