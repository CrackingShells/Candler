//! The container guard: the one place that destroys a container or a network.
//!
//! A container is destroyed only when the rule below passes on a fresh `docker inspect` of its
//! full id, and the rule is applied again to a second inspect before the last verb:
//!
//! 1. the id is 64 lowercase hex digits and is what docker answers to, with the labels and the
//!    creation time the caller claims;
//! 2. `candler.job` names the job of the scope: the current job for a run or the current job, a
//!    lost job for [`SweepScope::Lost`];
//! 3. `candler.role` is `agent` or `gateway`; `control` is never destroyed;
//! 4. a run's scope needs the container's `candler.run` to be that run;
//! 5. a record in the job directory that names the container must agree with its labels;
//! 6. for the current job, the container must have a record: a labelled container no record names
//!    is refused as [`GuardRefusal::Unrecorded`].
//!
//! The verbs are `inspect`, check, `stop`, `inspect`, check, `rm`, always by id and never with
//! `-f`. `rm` is `rm -v`, which removes only the container's own anonymous volumes. The guard is
//! the only holder of a [`DestroyDriver`].

use std::str::FromStr;
use std::sync::Arc;

use candler_schemas::container::{ContainerFound, ContainerRole};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{ContainerId, JobId, RunKey};
use futures::future::BoxFuture;

use crate::isolation::docker::{DockerCli, DockerError};
use crate::isolation::driver::{DestroyDriver, DockerDriver};
use crate::isolation::layout::{LABEL_JOB, LABEL_ROLE, LABEL_RUN};
use crate::isolation::model::{ContainerRef, Inspect, Labels, NetworkName, SweepScope};
use crate::jobs::dir::JobDir;
use crate::jobs::lock::JobLock;
use crate::jobs::lost::find_end;
use crate::jobs::view::{ChildEntry, JobView};
use crate::supervise::EventSink;

/// Why the guard refused to destroy a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GuardRefusal {
    /// Nothing records the container and its labels do not put it in the scope.
    Unrecorded,
    /// The container belongs to another job.
    OtherJob,
    /// The container carries no usable candler labels.
    Unlabelled,
    /// The container's labels, or what the caller claimed, differ from what a record or an
    /// earlier inspect said.
    Mismatch,
    /// The container is a control container.
    Control,
    /// The id is not a full container id.
    NotFullId,
    /// The container belongs to another run of the job.
    OtherRun,
}

impl GuardRefusal {
    /// The refusal as a reason code, in lowercase words joined by `_`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unrecorded => "unrecorded",
            Self::OtherJob => "other_job",
            Self::Unlabelled => "unlabelled",
            Self::Mismatch => "mismatch",
            Self::Control => "control",
            Self::NotFullId => "not_full_id",
            Self::OtherRun => "other_run",
        }
    }
}

/// Why the guard could not decide or act.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GuardError {
    /// The guard refused.
    #[error("the guard refused: {0:?}")]
    Refused(GuardRefusal),
    /// Docker failed, or its answer could not be read; nothing was destroyed after that call.
    #[error("docker failed under the guard: {detail}")]
    Docker {
        /// What docker said.
        detail: String,
    },
    /// The job directory could not be read, so the guard could not tell what is recorded.
    #[error("the guard could not read the job's records: {detail}")]
    View {
        /// What went wrong.
        detail: String,
    },
}

/// What a destroy did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destroyed {
    /// Whether a record named the container, as opposed to its labels.
    pub recorded: bool,
    /// Whether the container was already gone when the guard looked, so that it destroyed
    /// nothing.
    pub absent: bool,
}

/// The proof that a job is lost: the lock of its directory, held by the caller. A holder of the
/// lock is the only process that could be that job's supervisor, so no live one exists, and the
/// proof names the job the lock was taken for. It can be made only from a held [`JobLock`].
#[derive(Debug)]
pub struct LostProof {
    dir: JobDir,
}

impl LostProof {
    /// The proof of the job whose lock `lock` is.
    #[must_use]
    pub fn held(lock: &JobLock) -> Self {
        Self {
            dir: lock.dir().clone(), // BORROW: the proof owns the directory of the lock
        }
    }

    /// The job the proof is for.
    #[must_use]
    pub fn job<'a>(&'a self) -> &'a JobId {
        self.dir.job()
    }

    /// Whether the job's log has a `job.ended`, read now.
    fn ended(&self) -> Result<bool, GuardError> {
        find_end(&self.dir)
            .map(|end| end.is_some())
            .map_err(|error| GuardError::View {
                detail: error.to_string(),
            })
    }
}

/// A container that passed the rule.
struct Verified {
    inspect: Inspect,
    job: JobId,
    role: ContainerRole,
    recorded: bool,
}

/// Destroys containers and networks, and only those it may.
pub struct ContainerGuard {
    // TRAIT_OBJECT: the destroy driver is docker, or a recording fake in tests
    destroy: Arc<dyn DestroyDriver>,
    // TRAIT_OBJECT: the read driver is docker, or a recording fake in tests
    docker: Arc<dyn DockerDriver>,
    // TRAIT_OBJECT: the view is the job directory, or a fake in tests
    view: Arc<dyn JobView>,
    // TRAIT_OBJECT: the sink is the job's event log, or none
    events: Option<Arc<dyn EventSink>>,
}

impl ContainerGuard {
    /// A guard over a destroy driver, a read driver and the view of the job's records.
    #[must_use]
    pub fn new(
        destroy: Arc<dyn DestroyDriver>,
        docker: Arc<dyn DockerDriver>,
        view: Arc<dyn JobView>,
    ) -> Self {
        // TRAIT_OBJECT: the drivers and the view are chosen by the job runtime and by tests
        Self {
            destroy,
            docker,
            view,
            events: None,
        }
    }

    /// A guard that reads and destroys through the docker client `cli`. This is how the runtime
    /// builds one: the destroy half of `cli` is given to no other holder.
    #[must_use]
    pub fn over_docker(cli: &DockerCli, view: Arc<dyn JobView>) -> Self {
        Self::new(
            Arc::new(cli.destroyer()),
            Arc::new(cli.clone()), // BORROW: .clone() gives the guard its own client
            view,
        )
    }

    /// The same guard, reporting each destroy as a `container.destroyed` event to `events`.
    #[must_use]
    pub fn with_events(mut self, events: Arc<dyn EventSink>) -> Self {
        // TRAIT_OBJECT: the sink is chosen by the job runner
        self.events = Some(events);
        self
    }

    /// The read driver the guard inspects through, for callers that look before they ask the
    /// guard to act. It holds no destroy verb.
    #[must_use]
    pub fn reader(&self) -> Arc<dyn DockerDriver> {
        // TRAIT_OBJECT: the read driver is docker, or a recording fake in tests
        Arc::clone(&self.docker)
    }

    /// Says whether `container` may be destroyed within `scope`: `Some` names how the guard
    /// found it, `None` says docker no longer has it. It acts on nothing.
    pub fn destroyable<'a>(
        &'a self,
        container: &'a ContainerRef,
        scope: &'a SweepScope,
    ) -> BoxFuture<'a, Result<Option<ContainerFound>, GuardError>> {
        Box::pin(async move {
            Ok(self
                .verify(container, scope, None)
                .await?
                .map(|verified| found_of(verified.recorded)))
        })
    }

    /// Asks `container` to stop when the guard allows it within `scope`. A container docker no
    /// longer has is not an error.
    pub fn stop<'a>(
        &'a self,
        container: &'a ContainerRef,
        scope: &'a SweepScope,
    ) -> BoxFuture<'a, Result<(), GuardError>> {
        Box::pin(async move {
            if self.verify(container, scope, None).await?.is_some() {
                self.stop_by_id(&container.cid).await?;
            }
            Ok(())
        })
    }

    /// Destroys `container` when the guard allows it within `scope`: inspect, check, stop,
    /// inspect, check that the id and labels are the same, check the job again, remove.
    pub fn destroy<'a>(
        &'a self,
        container: &'a ContainerRef,
        scope: &'a SweepScope,
    ) -> BoxFuture<'a, Result<Destroyed, GuardError>> {
        Box::pin(self.destroy_with(container, scope, None))
    }

    /// Destroys `container` of the lost job `proof` is for, by the same rule as
    /// [`ContainerGuard::destroy`] within [`SweepScope::Lost`]. The job must have no
    /// `job.ended` at each check.
    pub fn destroy_lost<'a>(
        &'a self,
        container: &'a ContainerRef,
        proof: &'a LostProof,
    ) -> BoxFuture<'a, Result<Destroyed, GuardError>> {
        Box::pin(async move {
            let scope = SweepScope::Lost(proof.job().clone()); // BORROW: .clone() gives the scope its own job
            self.destroy_with(container, &scope, Some(proof)).await
        })
    }

    async fn destroy_with(
        &self,
        container: &ContainerRef,
        scope: &SweepScope,
        proof: Option<&LostProof>,
    ) -> Result<Destroyed, GuardError> {
        let Some(verified) = self.verify(container, scope, proof).await? else {
            return Ok(Destroyed {
                recorded: false,
                absent: true,
            });
        };
        let gone = Destroyed {
            recorded: verified.recorded,
            absent: true,
        };
        if !self.stop_by_id(&container.cid).await? {
            return Ok(gone);
        }
        // BORROW: .as_str() hands the id to docker
        let again = match self.docker.inspect(container.cid.as_str()).await {
            Ok(again) => again,
            Err(DockerError::NotFound { .. }) => return Ok(gone),
            Err(error) => return Err(docker_failed(&error)),
        };
        if !same_container(&verified.inspect, &again) {
            return Err(GuardError::Refused(GuardRefusal::Mismatch));
        }
        // the job may have recovered while the container was stopping
        self.check_job(&verified.job, scope, proof)?;
        // BORROW: .as_str() hands the id to docker
        match self.destroy.rm(container.cid.as_str()).await {
            Ok(()) => {}
            Err(DockerError::NotFound { .. }) => return Ok(gone),
            Err(error) => return Err(docker_failed(&error)),
        }
        self.report(&container.cid, verified.role, verified.recorded, scope);
        Ok(Destroyed {
            recorded: verified.recorded,
            absent: false,
        })
    }

    /// Removes the network `name` when its job is the job `scope` reaches, by the same job rule
    /// as a container: the current job's networks for [`SweepScope::Run`] and
    /// [`SweepScope::CurrentJob`], and none for [`SweepScope::Lost`], which needs
    /// [`ContainerGuard::destroy_network_lost`]. Docker refuses to remove a network that still has
    /// containers on it, and that refusal is the error. A network docker no longer has is not an
    /// error.
    pub fn destroy_network<'a>(
        &'a self,
        name: &'a NetworkName,
        scope: &'a SweepScope,
    ) -> BoxFuture<'a, Result<(), GuardError>> {
        Box::pin(self.destroy_network_with(name, scope, None))
    }

    /// Removes the network `name` of the lost job `proof` is for, when the job has no
    /// `job.ended`.
    pub fn destroy_network_lost<'a>(
        &'a self,
        name: &'a NetworkName,
        proof: &'a LostProof,
    ) -> BoxFuture<'a, Result<(), GuardError>> {
        Box::pin(async move {
            let scope = SweepScope::Lost(proof.job().clone()); // BORROW: .clone() gives the scope its own job
            self.destroy_network_with(name, &scope, Some(proof)).await
        })
    }

    async fn destroy_network_with(
        &self,
        name: &NetworkName,
        scope: &SweepScope,
        proof: Option<&LostProof>,
    ) -> Result<(), GuardError> {
        let info = match self.docker.network_inspect(name).await {
            Ok(info) => info,
            Err(DockerError::NotFound { .. }) => return Ok(()),
            Err(error) => return Err(docker_failed(&error)),
        };
        let job = info
            .labels
            .get(LABEL_JOB)
            .and_then(|text| JobId::from_str(text).ok())
            .ok_or(GuardError::Refused(GuardRefusal::Unlabelled))?;
        let ours =
            NetworkName::for_job(&job) == *name || NetworkName::egress_for_job(&job) == *name;
        if !ours || info.name != *name {
            return Err(GuardError::Refused(GuardRefusal::Mismatch));
        }
        self.check_job(&job, scope, proof)?;
        match self.destroy.network_rm(name).await {
            Ok(()) | Err(DockerError::NotFound { .. }) => Ok(()),
            Err(error) => Err(docker_failed(&error)),
        }
    }

    /// Stops a container whose rule has just passed. `false` when docker no longer has it.
    async fn stop_by_id(&self, cid: &ContainerId) -> Result<bool, GuardError> {
        // BORROW: .as_str() hands the id to docker
        match self.destroy.stop(cid.as_str()).await {
            Ok(()) => Ok(true),
            Err(DockerError::NotFound { .. }) => Ok(false),
            Err(error) => Err(docker_failed(&error)),
        }
    }

    /// Inspects `container` and applies the rule to the answer. `None` when docker has no such
    /// container.
    async fn verify(
        &self,
        container: &ContainerRef,
        scope: &SweepScope,
        proof: Option<&LostProof>,
    ) -> Result<Option<Verified>, GuardError> {
        // BORROW: `.as_str()` borrows the text of the container id
        if container.cid.as_str().len() != 64 {
            return Err(GuardError::Refused(GuardRefusal::NotFullId));
        }
        // BORROW: .as_str() hands the id to docker
        let inspect = match self.docker.inspect(container.cid.as_str()).await {
            Ok(inspect) => inspect,
            Err(DockerError::NotFound { .. }) => return Ok(None),
            Err(error) => return Err(docker_failed(&error)),
        };
        if inspect.id != container.cid || !claims_hold(container, &inspect) {
            return Err(GuardError::Refused(GuardRefusal::Mismatch));
        }
        let job = inspect
            .labels
            .get(LABEL_JOB)
            .and_then(|text| JobId::from_str(text).ok())
            .ok_or(GuardError::Refused(GuardRefusal::Unlabelled))?;
        let role = role_of(&inspect.labels).map_err(GuardError::Refused)?;
        self.check_job(&job, scope, proof)?;
        check_run(&inspect.labels, scope)?;
        let recorded = self.check_record(&inspect, &job, role, scope)?;
        Ok(Some(Verified {
            inspect,
            job,
            role,
            recorded,
        }))
    }

    /// Checks that `job` is the job `scope` reaches. A lost job is reached only with the proof of
    /// that job, and only while its log has no `job.ended`; the view is not asked.
    fn check_job(
        &self,
        job: &JobId,
        scope: &SweepScope,
        proof: Option<&LostProof>,
    ) -> Result<(), GuardError> {
        let other = GuardError::Refused(GuardRefusal::OtherJob);
        match scope {
            SweepScope::Run(_) | SweepScope::CurrentJob => {
                if job == self.view.job() {
                    Ok(())
                } else {
                    Err(other)
                }
            }
            SweepScope::Lost(lost) => match proof {
                Some(held) if job == lost && held.job() == lost && lost != self.view.job() => {
                    if held.ended()? {
                        Err(other)
                    } else {
                        Ok(())
                    }
                }
                Some(_) | None => Err(other),
            },
        }
    }

    /// Checks the container against the record of the job directory, and says whether one names
    /// it. The current job's container needs a record unless its run is the scope's run.
    fn check_record(
        &self,
        inspect: &Inspect,
        job: &JobId,
        role: ContainerRole,
        scope: &SweepScope,
    ) -> Result<bool, GuardError> {
        if matches!(scope, SweepScope::Lost(_)) {
            return Ok(false);
        }
        let entries = self.view.children().map_err(|error| GuardError::View {
            detail: error.to_string(),
        })?;
        let record = entries.iter().find_map(|entry| match entry {
            ChildEntry::Container(record) if *record.cid() == inspect.id => Some(record),
            ChildEntry::Container(_) | ChildEntry::Process(_) | ChildEntry::Unreadable { .. } => {
                None
            }
        });
        let Some(record) = record else {
            return match scope {
                SweepScope::Run(_) => Ok(false),
                SweepScope::CurrentJob | SweepScope::Lost(_) => {
                    Err(GuardError::Refused(GuardRefusal::Unrecorded))
                }
            };
        };
        let labels = record.labels();
        let agrees = labels.job == *job
            && record.role() == role
            && record.created_at().instant() == inspect.created.instant()
            && inspect
                .labels
                .get(LABEL_RUN)
                // BORROW: `.as_str()` borrows the text of the run marker
                .is_none_or(|run| run == labels.run.as_str());
        if agrees {
            Ok(true)
        } else {
            Err(GuardError::Refused(GuardRefusal::Mismatch))
        }
    }

    /// Reports a destroyed container to the sink, if the guard has one.
    fn report(&self, cid: &ContainerId, role: ContainerRole, recorded: bool, scope: &SweepScope) {
        if let Some(events) = &self.events {
            let run: Option<&RunKey> = match scope {
                SweepScope::Run(key) => Some(key),
                SweepScope::CurrentJob | SweepScope::Lost(_) => None,
            };
            events.emit(
                run,
                EventPayload::container_destroyed(
                    cid.clone(), // BORROW: .clone() gives the event its own id
                    role,
                    found_of(recorded),
                ),
            );
        }
    }
}

/// How a container was found.
const fn found_of(recorded: bool) -> ContainerFound {
    if recorded {
        ContainerFound::Recorded
    } else {
        ContainerFound::Labelled
    }
}

/// The error of a docker call that failed under the guard.
fn docker_failed(error: &DockerError) -> GuardError {
    GuardError::Docker {
        detail: error.to_string(),
    }
}

/// Whether every label and the creation time the caller claimed is what docker says.
fn claims_hold(claimed: &ContainerRef, inspect: &Inspect) -> bool {
    claimed.created_at.instant() == inspect.created.instant()
        && claimed
            .labels
            .iter()
            .all(|(key, value)| inspect.labels.get(key) == Some(value))
}

/// Whether the second inspect is of the same container as the first: the same id, creation time
/// and labels.
fn same_container(first: &Inspect, second: &Inspect) -> bool {
    first.id == second.id
        && first.created.instant() == second.created.instant()
        && first.labels == second.labels
}

/// The role a container's labels give it. A control container is never destroyed.
fn role_of(labels: &Labels) -> Result<ContainerRole, GuardRefusal> {
    match labels.get(LABEL_ROLE) {
        Some("agent") => Ok(ContainerRole::Agent),
        Some("gateway") => Ok(ContainerRole::Gateway),
        Some("control") => Err(GuardRefusal::Control),
        Some(_) | None => Err(GuardRefusal::Unlabelled),
    }
}

/// Checks that a run's scope reaches the container's run.
fn check_run(labels: &Labels, scope: &SweepScope) -> Result<(), GuardError> {
    match scope {
        SweepScope::Run(key) => {
            // BORROW: `.as_str()` borrows the text of the rendered run key
            if labels.get(LABEL_RUN) == Some(key.to_string().as_str()) {
                Ok(())
            } else {
                Err(GuardError::Refused(GuardRefusal::OtherRun))
            }
        }
        SweepScope::CurrentJob | SweepScope::Lost(_) => Ok(()),
    }
}
