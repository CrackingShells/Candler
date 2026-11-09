//! Tearing a job's session down: the gateway and both networks go through the container guard,
//! and a refusal or a failure is evented and reported, never fatal.

use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::ReasonCode;
use futures::future::BoxFuture;

use super::session::JobSession;
use super::session::SWEEP_LIMIT;
use crate::isolation::container::sweeper::ContainerSweeper;
use crate::isolation::docker::CALL_DEADLINE;
use crate::isolation::model::SweepScope;

/// How many docker calls the gateway's destruction may make: the check that it is the job's, the
/// stop, the look at what is left, and the removal.
const GATEWAY_CALLS: u32 = 4;
/// How many docker calls the destruction of the job's two networks may make: a look and a
/// removal for each.
const NETWORK_CALLS: u32 = 4;

/// The longest a session's teardown takes: the label sweep at its limit, then every docker call of
/// the gateway's and the networks' destruction at its deadline. A stopped job's end waits for it.
pub const TEARDOWN_ALLOWANCE: std::time::Duration = SWEEP_LIMIT
    .saturating_add(CALL_DEADLINE.saturating_mul(GATEWAY_CALLS.saturating_add(NETWORK_CALLS)));

/// What a teardown could not remove.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TeardownReport {
    /// What stayed behind, one line each: the gateway, or a network by name, and why.
    pub incomplete: Vec<String>,
}

impl TeardownReport {
    /// Whether everything the session made is gone.
    #[must_use]
    pub const fn complete(&self) -> bool {
        self.incomplete.is_empty()
    }
}

impl JobSession {
    /// Sweeps the job's containers by label, then destroys the gateway, the job's internal network
    /// and its egress network, each attempted even when another fails. Every run has ended by
    /// now, so the sweep reaches only what a failed run left attached. A refusal or an error emits
    /// `container.sweep_incomplete` and is named in the report; the job's end stays what its runs
    /// said.
    pub fn close(self, scope: &SweepScope) -> BoxFuture<'_, TeardownReport> {
        Box::pin(async move { self.destroy_all(scope, true).await })
    }

    /// Destroys what a gateway that failed to start may have left: whatever the job's records and
    /// labels show, then both networks, all through the guard.
    pub(super) async fn close_unstarted(&self) -> TeardownReport {
        self.destroy_all(&SweepScope::CurrentJob, true).await
    }

    /// Says that a part of the teardown did not finish.
    fn note(&self, scope: &SweepScope, report: &mut TeardownReport, what: &str, why: &str) {
        report.incomplete.push(format!("{what}: {why}"));
        if let Ok(reason) = "teardown_incomplete".parse::<ReasonCode>() {
            self.events.emit(
                None,
                EventPayload::container_sweep_incomplete(None, scope.wire(), reason),
            );
        }
    }

    async fn destroy_all(&self, scope: &SweepScope, sweep: bool) -> TeardownReport {
        let mut report = TeardownReport::default();
        if sweep {
            // the sweeper reports an incomplete sweep itself
            if let Err(error) = self.sweeper.sweep(scope).await {
                self.note(scope, &mut report, "sweep", &error.to_string());
            }
        }
        if let Some(gateway) = &self.gateway
            && let Err(error) = self.guard.destroy(gateway, scope).await
        {
            self.note(scope, &mut report, "gateway", &error.to_string());
        }
        for network in &self.networks {
            if let Err(error) = self.guard.destroy_network(network, scope).await {
                self.note(
                    scope,
                    &mut report,
                    &format!("{network:?}"),
                    &error.to_string(),
                );
            }
        }
        report
    }
}
