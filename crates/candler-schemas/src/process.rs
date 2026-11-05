//! The child record and the exit status of a supervised process.
//!
//! The supervisor writes one [`ChildRecord`] for every child it spawns. The record names the
//! child by process id, process group and start time, and carries the marker the child received
//! in `CANDLER_RUN`. A record whose [`ProcessStart`] is `unavailable` never reads as alive: with
//! no start time, a process id cannot be told from a later process that reuses it. Whether a
//! process is alive is decided by the supervisor and not here.
//!
//! The record is a standalone document and carries the version marker
//! [`CHILD_RECORD_SCHEMA`] in its `schema` field. The reader refuses any other marker, a record
//! written as a list, and a process id or process group of 0. Unknown fields are ignored on input.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::generate::GeneratedSchema;
use crate::map_only::MapOnly;
use crate::primitives::{BootId, Marker, Timestamp, object_only, schema_tag};

/// The version marker every child record carries in its `schema` field.
pub const CHILD_RECORD_SCHEMA: &str = "child-record/1";

/// The wire root schemas this module publishes.
pub(crate) fn roots() -> Vec<GeneratedSchema> {
    Vec::new()
}

/// Why a child record could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ProcessError {
    /// A process id or process group of 0, which names no process.
    #[error("{field} is 0 (process ids count from 1)")]
    Zero {
        /// The name of the field: `pid` or `pgid`.
        field: &'static str,
    },
    /// A builder that was asked to build before a required field was set.
    #[error("child record field is missing (`{field}` was not set)")]
    MissingField {
        /// The name of the field.
        field: &'static str,
    },
}

schema_tag! {
    /// The constant `schema` field of a child record.
    SchemaTag = CHILD_RECORD_SCHEMA
}

/// When a process started, in the form its host reports: an object whose `kind` is `linux` (with `boot_id` and `ticks`), `darwin` (with `sec` and `usec`) or `unavailable`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum ProcessStart {
    /// A Linux process: the boot it ran in, and its start time in clock ticks since that boot.
    #[non_exhaustive]
    Linux {
        /// The identifier of the boot, so ticks of different boots are never compared.
        boot_id: BootId,
        /// The start time in clock ticks since boot, as `/proc/<pid>/stat` reports it.
        #[schemars(extend(
            "$comment" = "Fits in 64 bits; a reader that holds numbers as doubles is exact only below 2^53."
        ))]
        ticks: u64,
    },
    /// A macOS process: its start time as seconds and microseconds since the Unix epoch.
    #[non_exhaustive]
    Darwin {
        /// Whole seconds since the Unix epoch.
        #[schemars(extend(
            "$comment" = "Fits in 64 bits; a reader that holds numbers as doubles is exact only below 2^53."
        ))]
        sec: u64,
        /// Microseconds, as the host reports them.
        #[schemars(extend("$comment" = "Fits in 32 bits; the reader enforces it."))]
        usec: u32,
    },
    /// The host reported no start time; the record never reads as alive.
    Unavailable,
}

object_only!(ProcessStart);

impl ProcessStart {
    /// A Linux start time.
    #[must_use]
    pub const fn linux(boot_id: BootId, ticks: u64) -> Self {
        Self::Linux { boot_id, ticks }
    }

    /// A macOS start time.
    #[must_use]
    pub const fn darwin(sec: u64, usec: u32) -> Self {
        Self::Darwin { sec, usec }
    }
}

/// The record the supervisor writes for a child: its process id, process group, start time, marker and spawn time, with the version marker `child-record/1` in `schema`.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "ChildRecord")]
struct RawChildRecord {
    /// The version marker of the document: always `child-record/1`.
    schema: SchemaTag,
    /// The process id of the child, from 1.
    #[schemars(range(min = 1))]
    #[schemars(extend("$comment" = "Fits in 32 bits; the reader enforces it."))]
    pid: u32,
    /// The process group of the child, from 1.
    #[schemars(range(min = 1))]
    #[schemars(extend("$comment" = "Fits in 32 bits; the reader enforces it."))]
    pgid: u32,
    /// When the process started.
    start: ProcessStart,
    /// The value of `CANDLER_RUN` the child received.
    marker: Marker,
    /// When the supervisor spawned the child.
    spawned_at: Timestamp,
}

impl TryFrom<MapOnly<RawChildRecord>> for ChildRecord {
    type Error = ProcessError;

    fn try_from(raw: MapOnly<RawChildRecord>) -> Result<Self, Self::Error> {
        let RawChildRecord {
            schema: SchemaTag,
            pid,
            pgid,
            start,
            marker,
            spawned_at,
        } = raw.0;
        Self::checked(pid, pgid, start, marker, spawned_at)
    }
}

/// The record the supervisor writes for a child: its process id, process group, start time, marker and spawn time, with the version marker `child-record/1` in `schema`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawChildRecord>")]
#[non_exhaustive]
pub struct ChildRecord {
    schema: SchemaTag,
    pid: u32,
    pgid: u32,
    start: ProcessStart,
    marker: Marker,
    spawned_at: Timestamp,
}

impl ChildRecord {
    /// Starts a record; every field must be set before it builds.
    #[must_use]
    pub fn builder() -> ChildRecordBuilder {
        ChildRecordBuilder::default()
    }

    fn checked(
        process: u32,
        group: u32,
        start: ProcessStart,
        marker: Marker,
        spawned_at: Timestamp,
    ) -> Result<Self, ProcessError> {
        if process == 0 {
            return Err(ProcessError::Zero { field: "pid" });
        }
        if group == 0 {
            return Err(ProcessError::Zero { field: "pgid" });
        }
        Ok(Self {
            schema: SchemaTag,
            pid: process,
            pgid: group,
            start,
            marker,
            spawned_at,
        })
    }

    /// The process id of the child.
    #[must_use]
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// The process group of the child.
    #[must_use]
    pub const fn pgid(&self) -> u32 {
        self.pgid
    }

    /// When the process started.
    #[must_use]
    pub const fn start<'a>(&'a self) -> &'a ProcessStart {
        &self.start
    }

    /// The value of `CANDLER_RUN` the child received.
    #[must_use]
    pub const fn marker<'a>(&'a self) -> &'a Marker {
        &self.marker
    }

    /// When the supervisor spawned the child.
    #[must_use]
    pub const fn spawned_at<'a>(&'a self) -> &'a Timestamp {
        &self.spawned_at
    }
}

/// Builds a [`ChildRecord`].
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ChildRecordBuilder {
    pid: Option<u32>,
    pgid: Option<u32>,
    start: Option<ProcessStart>,
    marker: Option<Marker>,
    spawned_at: Option<Timestamp>,
}

impl ChildRecordBuilder {
    /// Sets the process id.
    #[must_use]
    pub const fn pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }

    /// Sets the process group.
    #[must_use]
    pub const fn pgid(mut self, pgid: u32) -> Self {
        self.pgid = Some(pgid);
        self
    }

    /// Sets when the process started.
    #[must_use]
    pub fn start(mut self, start: ProcessStart) -> Self {
        self.start = Some(start);
        self
    }

    /// Sets the marker the child received.
    #[must_use]
    pub fn marker(mut self, marker: Marker) -> Self {
        self.marker = Some(marker);
        self
    }

    /// Sets when the supervisor spawned the child.
    #[must_use]
    pub fn spawned_at(mut self, spawned_at: Timestamp) -> Self {
        self.spawned_at = Some(spawned_at);
        self
    }

    /// Builds the record.
    ///
    /// # Errors
    /// Returns [`ProcessError::MissingField`] if a field was not set, and [`ProcessError::Zero`]
    /// if the process id or the process group is 0.
    pub fn build(self) -> Result<ChildRecord, ProcessError> {
        let missing = |field| ProcessError::MissingField { field };
        ChildRecord::checked(
            self.pid.ok_or_else(|| missing("pid"))?,
            self.pgid.ok_or_else(|| missing("pgid"))?,
            self.start.ok_or_else(|| missing("start"))?,
            self.marker.ok_or_else(|| missing("marker"))?,
            self.spawned_at.ok_or_else(|| missing("spawned_at"))?,
        )
    }
}

/// How a process ended: an object whose `kind` is `code` (with the integer `code`) or `signal` (with the integer `signal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self", tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
#[allow(clippy::exhaustive_enums)] // EXHAUSTIVE: a process ends by exiting or by a signal, and in no third way
pub enum ExitStatus {
    /// The process exited with a status code.
    Code {
        /// The exit code.
        code: i32,
    },
    /// A signal ended the process.
    Signal {
        /// The signal number.
        signal: i32,
    },
}

object_only!(ExitStatus);
