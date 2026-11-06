//! After the child exits: collect the run's files once, and decide whether a gateway failure is
//! tried again.

use candler_harness::spec::Workspace;
use candler_schemas::primitives::RunKey;

use super::ends::{End, Retry};
use crate::isolation::model::{ContainerOutcome, ContainerPlan};
use crate::isolation::{EndFacts, Isolation};
use crate::supervise::stop::{Ended, Exit};

/// Whether the caller opens another attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RetryDecision {
    /// Open the next attempt with the store and run it again.
    Retry,
    /// The end is final.
    Final,
}

/// Whether `end` is tried again: only a gateway failure of the first attempt is.
#[must_use]
pub const fn retry_decision(end: &End) -> RetryDecision {
    match end.retry {
        Retry::Once => RetryDecision::Retry,
        Retry::Never | Retry::None => RetryDecision::Final,
    }
}

/// What an attempt left behind once the child was waited for.
#[derive(Debug)]
#[non_exhaustive]
pub struct Finished {
    /// The inspect and the destroy result of the container, when they could be had.
    pub outcome: Option<ContainerOutcome>,
    /// Whether the files of the run were collected.
    pub collect_ok: bool,
}

/// Finishes an attempt after its child was waited for.
///
/// A child that exited is collected from and destroyed here, once; a child that hit its deadline
/// or was stopped was destroyed by the stop hook, so only its final inspect is read. A failed
/// collection is reported, never raised, and leaves what the facts hold.
// TRAIT_OBJECT: the isolation and the facts are held as trait objects by the runtime
pub async fn finish_attempt(
    isolation: &dyn Isolation,
    plan: &ContainerPlan,
    workspace: &Workspace<'_>,
    exit: &Exit,
    key: &RunKey,
    facts: &dyn EndFacts,
) -> Finished {
    match exit.ended {
        Ended::Exited => match isolation
            .collect_and_destroy(plan, workspace, exit, key)
            .await
        {
            Ok(outcome) => Finished {
                outcome: Some(outcome),
                collect_ok: true,
            },
            Err(_) => Finished {
                outcome: facts.final_inspect(),
                collect_ok: false,
            },
        },
        Ended::Deadline | Ended::Stopped => Finished {
            outcome: facts.final_inspect(),
            collect_ok: true,
        },
    }
}
