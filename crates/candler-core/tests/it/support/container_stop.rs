//! Helpers for the guards of the container stop and guard: builders and fixtures, never expected values.
use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex, PoisonError};

use candler_core::isolation::docker::DockerError;
use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
use candler_core::isolation::model::{
    BuildRequest, ContainerRef, ImageId, ImageInfo, Inspect, InspectState, LabelSel, Labels,
    NetworkInfo, NetworkName,
};
use candler_core::isolation::testing::RecordingDestroyer;
use candler_schemas::container::{ContainerLabels, ContainerRecord, ContainerRole};
use candler_schemas::primitives::{ContainerId, Digest, JobId, Marker, RunKey, Timestamp};
use futures::future::BoxFuture;

/// A job id that is a valid `UUIDv7`, distinct for each `n`.
pub fn job(n: u32) -> JobId {
    JobId::from_str(&format!("{n:08x}-0000-7000-8000-{n:012x}")).expect("a UUIDv7")
}

/// A full container id, distinct for each `n`.
pub fn cid(n: u32) -> ContainerId {
    ContainerId::from_str(&format!("{n:064x}")).expect("64 hex digits")
}

/// A digest, distinct for each `n`.
pub fn digest(n: u32) -> Digest {
    Digest::from_hex(&format!("{n:064x}")).expect("64 hex digits")
}

/// A run key of the cell `n`, first replicate, first attempt.
pub fn run_key(n: u32) -> RunKey {
    RunKey::new(digest(n), 1, 1).expect("a run key")
}

/// A fixed creation time.
pub fn created() -> Timestamp {
    Timestamp::from_str("2026-01-02T03:04:05.123456789Z").expect("a timestamp")
}

/// The labels of a container of `job` in `role`, with the run when there is one.
pub fn labels(job: &JobId, role: &str, run: Option<&RunKey>) -> Labels {
    let labels = Labels::new()
        .with("candler.job", job.as_str()) // BORROW: explicit .as_str() instead of Deref coercion
        .with("candler.role", role);
    match run {
        Some(key) => labels.with("candler.run", &key.to_string()),
        None => labels,
    }
}

/// What docker says about a running container with `labels`.
pub fn inspect(id: &ContainerId, labels: Labels) -> Inspect {
    Inspect::new(
        id.clone(),                      // BORROW: .clone() gives the answer its own id
        ImageId("sha256:aa".to_owned()), // BORROW: .to_owned() gives the id its own text
        created(),
        labels,
        InspectState::new(
            "running".to_owned(), // BORROW: .to_owned() gives the state its own text
            true,
            0,
            false,
            Some(created()),
            None,
        ),
    )
}

/// The container as a caller names it: the id, the labels and the creation time docker gave.
pub fn container_ref(inspected: &Inspect) -> ContainerRef {
    ContainerRef::new(
        inspected.id.clone(),      // BORROW: .clone() gives the reference its own id
        inspected.labels.clone(),  // BORROW: .clone() gives the reference its own labels
        inspected.created.clone(), // BORROW: .clone() gives the reference its own time
    )
}

/// The record the supervisor would have written for `id`.
pub fn record(id: &ContainerId, job: &JobId, run: &RunKey, role: ContainerRole) -> ContainerRecord {
    ContainerRecord::builder()
        .cid(id.clone()) // BORROW: .clone() gives the record its own id
        .labels(ContainerLabels::new(
            job.clone(), // BORROW: .clone() gives the labels their own job
            Marker::for_run(run),
            digest(7),
        ))
        .created_at(created())
        .image_id(digest(8))
        .role(role)
        .spawned_at(created())
        .build()
        .expect("a record")
}

/// An ordered log that several fakes append to.
#[derive(Debug, Clone, Default)]
pub struct Log(Arc<Mutex<Vec<String>>>);

impl Log {
    /// An empty log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a line.
    pub fn push(&self, line: String) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(line);
    }

    /// The lines so far.
    pub fn lines(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone() // BORROW: .clone() copies the lines out of the lock
    }
}

/// A [`DockerDriver`] with a script of inspect answers per id: the first inspect gets the first
/// answer, the second the second, and the last answer repeats. It logs each call into the
/// shared [`Log`] and reaches no docker.
pub struct ScriptedDocker {
    log: Log,
    script: Mutex<BTreeMap<String, Vec<Inspect>>>,
    listed: Vec<ContainerRef>,
    networks: BTreeMap<String, NetworkInfo>,
}

impl ScriptedDocker {
    /// A fake that knows nothing and logs into `log`.
    pub fn new(log: &Log) -> Self {
        Self {
            log: log.clone(), // BORROW: .clone() gives the fake its own handle on the log
            script: Mutex::new(BTreeMap::new()),
            listed: Vec::new(),
            networks: BTreeMap::new(),
        }
    }

    /// The inspect answers for `id`, in order.
    #[must_use]
    pub fn script(self, id: &ContainerId, answers: Vec<Inspect>) -> Self {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id.as_str().to_owned(), answers); // BORROW: .as_str() and .to_owned() key the script
        self
    }

    /// What `network_inspect(name)` answers.
    #[must_use]
    pub fn network(mut self, name: &NetworkName, info: NetworkInfo) -> Self {
        self.networks.insert(name.as_str().to_owned(), info); // BORROW: .as_str() and .to_owned() key the answer
        self
    }
}

impl DockerDriver for ScriptedDocker {
    fn inspect<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Inspect, DockerError>> {
        self.log.push(format!("inspect {id}"));
        let mut script = self.script.lock().unwrap_or_else(PoisonError::into_inner);
        let answer = match script.get_mut(id) {
            Some(answers) if answers.len() > 1 => Some(answers.remove(0)),
            Some(answers) => answers.first().cloned(), // BORROW: .cloned() repeats the last answer
            None => None,
        };
        Box::pin(async move {
            answer.ok_or_else(|| DockerError::NotFound {
                detail: format!("No such container: {id}"),
            })
        })
    }

    fn ps_by_label<'a>(
        &'a self,
        sel: &'a LabelSel,
    ) -> BoxFuture<'a, Result<Vec<ContainerRef>, DockerError>> {
        self.log.push(format!("ps {}={}", sel.key, sel.value));
        let listed = self.listed.clone(); // BORROW: .clone() copies the listing for the future
        Box::pin(async move { Ok(listed) })
    }

    fn network_create<'a>(
        &'a self,
        name: &'a NetworkName,
        _internal: bool,
        _labels: &'a Labels,
    ) -> BoxFuture<'a, Result<(), DockerError>> {
        self.log.push(format!("network create {}", name.as_str())); // BORROW: explicit .as_str()
        Box::pin(async { Ok(()) })
    }

    fn network_inspect<'a>(
        &'a self,
        name: &'a NetworkName,
    ) -> BoxFuture<'a, Result<NetworkInfo, DockerError>> {
        self.log.push(format!("network inspect {}", name.as_str())); // BORROW: explicit .as_str()
        let answer = self.networks.get(name.as_str()).cloned(); // BORROW: .as_str() and .cloned()
        Box::pin(async move {
            answer.ok_or_else(|| DockerError::NotFound {
                detail: "network not found".to_owned(), // BORROW: .to_owned() gives the error its own text
            })
        })
    }

    fn image_inspect<'a>(
        &'a self,
        _reference: &'a str,
    ) -> BoxFuture<'a, Result<Option<ImageInfo>, DockerError>> {
        Box::pin(async { Ok(None) })
    }

    fn build<'a>(
        &'a self,
        _request: &'a BuildRequest,
    ) -> BoxFuture<'a, Result<ImageInfo, DockerError>> {
        Box::pin(async {
            Err(DockerError::Unavailable {
                detail: "ScriptedDocker::build".to_owned(), // BORROW: .to_owned() gives the error its own text
            })
        })
    }
}

/// A [`DestroyDriver`] that logs each verb into the shared [`Log`] and then records it in the
/// test-support [`RecordingDestroyer`], which destroys nothing.
pub struct LoggingDestroyer {
    log: Log,
    inner: Arc<RecordingDestroyer>,
}

impl LoggingDestroyer {
    /// A destroyer that logs into `log` and records into `inner`.
    pub fn new(log: &Log, inner: Arc<RecordingDestroyer>) -> Self {
        Self {
            log: log.clone(), // BORROW: .clone() gives the fake its own handle on the log
            inner,
        }
    }
}

impl DestroyDriver for LoggingDestroyer {
    fn stop<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        self.log.push(format!("stop {id}"));
        self.inner.stop(id)
    }

    fn rm<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        self.log.push(format!("rm {id}"));
        self.inner.rm(id)
    }

    fn network_rm<'a>(&'a self, name: &'a NetworkName) -> BoxFuture<'a, Result<(), DockerError>> {
        self.log.push(format!("network rm {}", name.as_str())); // BORROW: explicit .as_str()
        self.inner.network_rm(name)
    }
}
