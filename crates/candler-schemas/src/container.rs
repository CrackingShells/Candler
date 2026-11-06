//! The container record and the vocabulary the container events share.
//!
//! The supervisor writes one [`ContainerRecord`] for every container it creates, as soon as the
//! container id is known. The record names the container by its id, carries the labels the
//! container was created with, and says which role it plays. Whether a container may be destroyed
//! is decided by the supervisor and not here.
//!
//! The record is a standalone document and carries the version marker [`CONTAINER_RECORD_SCHEMA`]
//! in its `schema` field. The reader refuses any other marker and a record written as a list.
//! Unknown fields are ignored on input.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::generate::GeneratedSchema;
use crate::map_only::MapOnly;
use crate::primitives::{ContainerId, Digest, JobId, Marker, Timestamp, object_only, schema_tag};

/// The version marker every container record carries in its `schema` field.
pub const CONTAINER_RECORD_SCHEMA: &str = "container-record/1";

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    vec![GeneratedSchema::of::<ContainerRecord>("container-record")]
}

/// Why a container record could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ContainerError {
    /// A builder that was asked to build before a required field was set.
    #[error("container record field is missing (`{field}` was not set)")]
    MissingField {
        /// The name of the field.
        field: &'static str,
    },
}

schema_tag! {
    /// The constant `schema` field of a container record.
    SchemaTag = CONTAINER_RECORD_SCHEMA
}

/// The part a container plays in a run: `agent` or `gateway`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ContainerRole {
    /// The container runs the agent.
    Agent,
    /// The container is the gateway the agent reaches its provider through.
    Gateway,
}

/// How a container was found when it was destroyed: `recorded` or `labelled`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ContainerFound {
    /// The job directory holds a record of the container.
    Recorded,
    /// Only the labels of the container named it.
    Labelled,
}

/// How far a container sweep reached: `run`, `current_job` or `lost`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SweepScope {
    /// The containers of one run.
    Run,
    /// The containers of the job that is running.
    CurrentJob,
    /// The containers of a job whose supervisor was lost.
    Lost,
}

/// The labels a container carries: the job, the run and the experiment it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct ContainerLabels {
    /// The job the container belongs to: the value of the `candler.job` label.
    pub job: JobId,
    /// The run the container belongs to, in its path form: the value of the `candler.run` label.
    pub run: Marker,
    /// The digest of the experiment: the value of the `candler.experiment` label.
    pub experiment: Digest,
}

object_only!(ContainerLabels);

impl ContainerLabels {
    /// The labels of a container.
    #[must_use]
    pub const fn new(job: JobId, run: Marker, experiment: Digest) -> Self {
        Self {
            job,
            run,
            experiment,
        }
    }
}

/// The record the supervisor writes for a container: its id, labels, creation time, image, role and spawn time, with the version marker `container-record/1` in `schema`.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "ContainerRecord")]
struct RawContainerRecord {
    /// The version marker of the document: always `container-record/1`.
    schema: SchemaTag,
    /// The id of the container.
    cid: ContainerId,
    /// The labels the container was created with.
    labels: ContainerLabels,
    /// When the container was created, as the container engine reports it.
    created_at: Timestamp,
    /// The id of the image the container was created from.
    image_id: Digest,
    /// The part the container plays.
    role: ContainerRole,
    /// When the supervisor spawned the container.
    spawned_at: Timestamp,
}

impl TryFrom<MapOnly<RawContainerRecord>> for ContainerRecord {
    type Error = ContainerError;

    fn try_from(raw: MapOnly<RawContainerRecord>) -> Result<Self, Self::Error> {
        let RawContainerRecord {
            schema: SchemaTag,
            cid,
            labels,
            created_at,
            image_id,
            role,
            spawned_at,
        } = raw.0;
        Ok(Self {
            schema: SchemaTag,
            cid,
            labels,
            created_at,
            image_id,
            role,
            spawned_at,
        })
    }
}

/// The record the supervisor writes for a container: its id, labels, creation time, image, role and spawn time, with the version marker `container-record/1` in `schema`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawContainerRecord>")]
#[non_exhaustive]
pub struct ContainerRecord {
    schema: SchemaTag,
    cid: ContainerId,
    labels: ContainerLabels,
    created_at: Timestamp,
    image_id: Digest,
    role: ContainerRole,
    spawned_at: Timestamp,
}

impl ContainerRecord {
    /// Starts a record; every field must be set before it builds.
    #[must_use]
    pub fn builder() -> ContainerRecordBuilder {
        ContainerRecordBuilder::default()
    }

    /// The id of the container.
    #[must_use]
    pub const fn cid<'a>(&'a self) -> &'a ContainerId {
        &self.cid
    }

    /// The labels the container was created with.
    #[must_use]
    pub const fn labels<'a>(&'a self) -> &'a ContainerLabels {
        &self.labels
    }

    /// When the container was created.
    #[must_use]
    pub const fn created_at<'a>(&'a self) -> &'a Timestamp {
        &self.created_at
    }

    /// The id of the image the container was created from.
    #[must_use]
    pub const fn image_id<'a>(&'a self) -> &'a Digest {
        &self.image_id
    }

    /// The part the container plays.
    #[must_use]
    pub const fn role(&self) -> ContainerRole {
        self.role
    }

    /// When the supervisor spawned the container.
    #[must_use]
    pub const fn spawned_at<'a>(&'a self) -> &'a Timestamp {
        &self.spawned_at
    }
}

/// Builds a [`ContainerRecord`].
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ContainerRecordBuilder {
    cid: Option<ContainerId>,
    labels: Option<ContainerLabels>,
    created_at: Option<Timestamp>,
    image_id: Option<Digest>,
    role: Option<ContainerRole>,
    spawned_at: Option<Timestamp>,
}

impl ContainerRecordBuilder {
    /// Sets the container id.
    #[must_use]
    pub fn cid(mut self, cid: ContainerId) -> Self {
        self.cid = Some(cid);
        self
    }

    /// Sets the labels.
    #[must_use]
    pub fn labels(mut self, labels: ContainerLabels) -> Self {
        self.labels = Some(labels);
        self
    }

    /// Sets when the container was created.
    #[must_use]
    pub fn created_at(mut self, created_at: Timestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }

    /// Sets the image id.
    #[must_use]
    pub fn image_id(mut self, image_id: Digest) -> Self {
        self.image_id = Some(image_id);
        self
    }

    /// Sets the role.
    #[must_use]
    pub const fn role(mut self, role: ContainerRole) -> Self {
        self.role = Some(role);
        self
    }

    /// Sets when the supervisor spawned the container.
    #[must_use]
    pub fn spawned_at(mut self, spawned_at: Timestamp) -> Self {
        self.spawned_at = Some(spawned_at);
        self
    }

    /// Builds the record.
    ///
    /// # Errors
    /// Returns [`ContainerError::MissingField`] if a field was not set.
    pub fn build(self) -> Result<ContainerRecord, ContainerError> {
        let missing = |field| ContainerError::MissingField { field };
        Ok(ContainerRecord {
            schema: SchemaTag,
            cid: self.cid.ok_or_else(|| missing("cid"))?,
            labels: self.labels.ok_or_else(|| missing("labels"))?,
            created_at: self.created_at.ok_or_else(|| missing("created_at"))?,
            image_id: self.image_id.ok_or_else(|| missing("image_id"))?,
            role: self.role.ok_or_else(|| missing("role"))?,
            spawned_at: self.spawned_at.ok_or_else(|| missing("spawned_at"))?,
        })
    }
}

/// A container a run left behind: its id and role.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub struct StrayContainer {
    /// The id of the container.
    pub cid: ContainerId,
    /// The part the container plays.
    pub role: ContainerRole,
}

object_only!(StrayContainer);

impl StrayContainer {
    /// A stray container.
    #[must_use]
    pub const fn new(cid: ContainerId, role: ContainerRole) -> Self {
        Self { cid, role }
    }
}
