//! The sweep of containers by label, for a run, for the current job and for a lost job.
//!
//! The sweeper finds candidates in two ways, by the `candler.job` label docker lists and by the
//! records in the job directory, and hands every one to the [`ContainerGuard`]. It destroys
//! nothing itself: what the guard refuses is reported as refused, never retried by another path.
//! A sweep that could not look everywhere says so with `unknown`: a record that cannot be read,
//! a job directory that cannot be listed, a docker call that failed midway, or the time limit.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use candler_schemas::container::SweepScope as WireScope;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{ContainerId, JobId, Marker, ReasonCode};
use futures::future::BoxFuture;

use super::guard::{ContainerGuard, GuardError, LostProof};
use crate::isolation::docker::DockerError;
use crate::isolation::driver::DockerDriver;
use crate::isolation::layout::{LABEL_JOB, LABEL_ROLE, LABEL_RUN};
use crate::isolation::model::{
    ContainerRef, ContainerSweep, LabelSel, NetworkName, Refusal, SweepScope,
};
use crate::jobs::view::{ChildEntry, JobView};
use crate::supervise::EventSink;

/// Why a sweep could not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SweepError {
    /// The sweeper is not available.
    #[error("sweeper is not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
    /// Docker failed while listing.
    #[error("the sweep could not list containers: {detail}")]
    List {
        /// What docker reported.
        detail: String,
    },
}

/// Finds and destroys the containers in a scope.
pub trait ContainerSweeper: Send + Sync {
    /// Destroys what the guard allows in `scope` and says what it did.
    ///
    /// # Errors
    /// [`SweepError::List`] when the containers cannot be listed.
    fn sweep<'a>(
        &'a self,
        scope: &'a SweepScope,
    ) -> BoxFuture<'a, Result<ContainerSweep, SweepError>>;

    /// Destroys what the guard allows of the lost job `proof` is for. A sweeper that cannot show
    /// the proof to its guard is unavailable.
    ///
    /// # Errors
    /// [`SweepError::List`] when the containers cannot be listed, [`SweepError::Unavailable`]
    /// when the sweeper does not sweep lost jobs.
    fn sweep_lost<'a>(
        &'a self,
        proof: &'a LostProof,
    ) -> BoxFuture<'a, Result<ContainerSweep, SweepError>> {
        Box::pin(async move {
            let _ = proof;
            Err(SweepError::Unavailable {
                detail: "sweep_lost".to_owned(), // BORROW: .to_owned() gives the error its own text
            })
        })
    }
}

/// A record in the job directory that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableRecord {
    /// The file.
    pub path: PathBuf,
    /// Why it could not be read.
    pub error: String,
}

/// What a sweep did, with the records it could not read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SweepReport {
    /// What the sweep destroyed and refused, and whether it is complete.
    pub sweep: ContainerSweep,
    /// The records that could not be read; when there are any, `sweep.unknown` is set.
    pub unreadable: Vec<UnreadableRecord>,
}

/// The sweeper that finds containers by their labels and by the job's records.
pub struct LabelSweeper {
    // TRAIT_OBJECT: the driver is docker, or a recording fake in tests
    docker: Arc<dyn DockerDriver>,
    guard: Arc<ContainerGuard>,
    // TRAIT_OBJECT: the view is the job directory, or a fake in tests
    view: Arc<dyn JobView>,
    // TRAIT_OBJECT: the sink is the job's event log, or none
    events: Option<Arc<dyn EventSink>>,
    limit: Duration,
}

impl LabelSweeper {
    /// A sweeper that lists through `docker`, reads records through `view` and destroys through
    /// `guard`, and gives up after `limit`.
    #[must_use]
    pub fn new(
        docker: Arc<dyn DockerDriver>,
        guard: Arc<ContainerGuard>,
        view: Arc<dyn JobView>,
        limit: Duration,
    ) -> Self {
        // TRAIT_OBJECT: the driver and the view are chosen by the job runtime and by tests
        Self {
            docker,
            guard,
            view,
            events: None,
            limit,
        }
    }

    /// The same sweeper, reporting an incomplete sweep of the job as an event to `events`.
    #[must_use]
    pub fn with_events(mut self, events: Arc<dyn EventSink>) -> Self {
        // TRAIT_OBJECT: the sink is chosen by the job runner
        self.events = Some(events);
        self
    }

    /// Sweeps `scope` and reports the records it could not read besides what it did.
    ///
    /// # Errors
    /// [`SweepError::List`] when docker cannot list the job's containers.
    pub async fn sweep_report(&self, scope: &SweepScope) -> Result<SweepReport, SweepError> {
        self.sweep_with(scope, None).await
    }

    /// Sweeps the lost job `proof` is for, and reports like [`LabelSweeper::sweep_report`].
    ///
    /// # Errors
    /// [`SweepError::List`] when docker cannot list the job's containers.
    pub async fn sweep_lost_report(&self, proof: &LostProof) -> Result<SweepReport, SweepError> {
        let scope = SweepScope::Lost(proof.job().clone()); // BORROW: .clone() gives the scope its own job
        self.sweep_with(&scope, Some(proof)).await
    }

    async fn sweep_with(
        &self,
        scope: &SweepScope,
        proof: Option<&LostProof>,
    ) -> Result<SweepReport, SweepError> {
        let mut report = SweepReport::default();
        let mut failure = None;
        let timed = tokio::time::timeout(
            self.limit,
            self.collect(scope, proof, &mut report, &mut failure),
        )
        .await;
        if let Some(error) = failure {
            return Err(error);
        }
        if timed.is_err() {
            report.sweep.unknown = true;
        }
        if report.sweep.unknown {
            self.say_incomplete(scope);
        }
        Ok(report)
    }

    /// The job whose containers `scope` reaches.
    fn job_of<'a>(&'a self, scope: &'a SweepScope) -> &'a JobId {
        match scope {
            SweepScope::Lost(job) => job,
            SweepScope::Run(_) | SweepScope::CurrentJob => self.view.job(),
        }
    }

    /// Does the sweep into `report`, so that what was done survives a timeout.
    async fn collect(
        &self,
        scope: &SweepScope,
        proof: Option<&LostProof>,
        report: &mut SweepReport,
        failure: &mut Option<SweepError>,
    ) {
        let job = self.job_of(scope);
        let recorded = self.recorded(scope, job, report);
        let selector = LabelSel {
            key: LABEL_JOB.to_owned(), // BORROW: .to_owned() gives the selector its own text
            value: job.to_string(),
        };
        let mut found = match self.docker.ps_by_label(&selector).await {
            Ok(found) => found,
            Err(error) => {
                *failure = Some(SweepError::List {
                    detail: error.to_string(),
                });
                return;
            }
        };
        if let SweepScope::Run(key) = scope {
            let wanted = key.to_string();
            // BORROW: .as_str() compares the label with the run
            found.retain(|container| container.labels.get(LABEL_RUN) == Some(wanted.as_str()));
        }
        for cid in recorded {
            if found.iter().any(|container| container.cid == cid) {
                continue;
            }
            // BORROW: .as_str() hands the id to docker
            match self.docker.inspect(cid.as_str()).await {
                Ok(inspected) => found.push(ContainerRef::new(
                    inspected.id,
                    inspected.labels,
                    inspected.created,
                )),
                Err(DockerError::NotFound { .. }) => {}
                Err(
                    DockerError::Unavailable { .. }
                    | DockerError::Failed { .. }
                    | DockerError::Unreadable { .. }
                    | DockerError::NotAFullId { .. }
                    | DockerError::NotAJobNetwork { .. },
                ) => report.sweep.unknown = true,
            }
        }
        // agents go before the gateway they talk to
        found.sort_by_key(|container| container.labels.get(LABEL_ROLE) != Some("agent"));
        for container in &found {
            self.destroy_one(container, scope, proof, report).await;
        }
        if let SweepScope::Lost(lost) = scope {
            for network in [
                NetworkName::for_job(lost),
                NetworkName::egress_for_job(lost),
            ] {
                let gone = match proof {
                    Some(held) => self.guard.destroy_network_lost(&network, held).await,
                    None => self.guard.destroy_network(&network, scope).await,
                };
                if gone.is_err() {
                    report.sweep.unknown = true;
                }
            }
        }
    }

    /// The ids the job's records name within `scope`. A record that cannot be read, or a
    /// directory that cannot be listed, makes the sweep unknown. A lost job has no view here.
    fn recorded(
        &self,
        scope: &SweepScope,
        job: &JobId,
        report: &mut SweepReport,
    ) -> Vec<ContainerId> {
        if matches!(scope, SweepScope::Lost(_)) {
            return Vec::new();
        }
        let Ok(entries) = self.view.children() else {
            report.sweep.unknown = true;
            return Vec::new();
        };
        let mut ids = Vec::new();
        for entry in entries {
            match entry {
                ChildEntry::Container(record) => {
                    let in_scope = record.labels().job == *job
                        && match scope {
                            SweepScope::Run(key) => record.labels().run == Marker::for_run(key),
                            SweepScope::CurrentJob | SweepScope::Lost(_) => true,
                        };
                    if in_scope {
                        // BORROW: .clone() gives the list its own id
                        ids.push(record.cid().clone());
                    }
                }
                ChildEntry::Unreadable { path, error } => {
                    report.sweep.unknown = true;
                    report.unreadable.push(UnreadableRecord { path, error });
                }
                ChildEntry::Process(_) => {}
            }
        }
        ids
    }

    /// Hands one container to the guard and files the answer in `report`.
    async fn destroy_one(
        &self,
        container: &ContainerRef,
        scope: &SweepScope,
        proof: Option<&LostProof>,
        report: &mut SweepReport,
    ) {
        let done = match proof {
            Some(held) => self.guard.destroy_lost(container, held).await,
            None => self.guard.destroy(container, scope).await,
        };
        match done {
            Ok(done) if !done.absent => {
                // BORROW: .clone() gives the report its own copy of the container
                report.sweep.destroyed.push(container.clone());
            }
            Ok(_) => {}
            Err(GuardError::Refused(why)) => report.sweep.refused.push(Refusal {
                // BORROW: .clone() and .to_owned() give the refusal its own id and text
                container: container.cid.clone(),
                reason: why.code().to_owned(),
            }),
            Err(GuardError::Docker { .. } | GuardError::View { .. }) => {
                report.sweep.unknown = true;
            }
        }
    }

    /// Reports an incomplete sweep of the job. A run's sweep is reported by the ladder that
    /// called the hook, so it is not reported twice.
    fn say_incomplete(&self, scope: &SweepScope) {
        if matches!(scope.wire(), WireScope::Run) {
            return;
        }
        let Some(events) = &self.events else {
            return;
        };
        if let Ok(reason) = "sweep_unknown".parse::<ReasonCode>() {
            events.emit(
                None,
                EventPayload::container_sweep_incomplete(None, scope.wire(), reason),
            );
        }
    }
}

impl ContainerSweeper for LabelSweeper {
    fn sweep<'a>(
        &'a self,
        scope: &'a SweepScope,
    ) -> BoxFuture<'a, Result<ContainerSweep, SweepError>> {
        Box::pin(async move { self.sweep_report(scope).await.map(|report| report.sweep) })
    }

    fn sweep_lost<'a>(
        &'a self,
        proof: &'a LostProof,
    ) -> BoxFuture<'a, Result<ContainerSweep, SweepError>> {
        Box::pin(async move {
            self.sweep_lost_report(proof)
                .await
                .map(|report| report.sweep)
        })
    }
}
