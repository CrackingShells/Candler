//! Executing one run: spawning its container, ending it, collecting its files and sealing it.

pub mod collect;
pub mod ends;
pub mod seal;
pub mod spawn;

use futures::future::BoxFuture;

use super::plan::RunPlan;
use candler_schemas::trace::EndedBy;

use super::{RunControl, RunOutcome, RunStatus, Runtime, Usage};
use crate::jobs::slots::Slot;

/// Executes the run `plan` while holding `slot`.
pub fn exec_run<'a>(
    rt: &'a Runtime,
    plan: &'a RunPlan,
    slot: &'a Slot,
    ctl: &'a RunControl,
) -> BoxFuture<'a, RunOutcome> {
    let _ = (rt, slot, ctl);
    Box::pin(async {
        RunOutcome {
            key: plan.key.clone(),
            status: RunStatus::Invalid,
            ended_by: EndedBy::Unknown,
            invalid: None,
            inspect: None,
            usage: Usage::default(),
        }
    })
}
