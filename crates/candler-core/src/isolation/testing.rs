//! Fakes of the docker seams for tests, behind the `test-support` feature: a recording
//! [`DockerDriver`] with hand-written answers, a recording [`DestroyDriver`], the
//! [`AllowlistDriver`] that panics on an id it was not told about, and a [`FakeJobView`].
//!
//! A fake never starts a process and never reaches docker.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use candler_schemas::primitives::JobId;
use futures::future::BoxFuture;

use super::docker::DockerError;
use super::driver::{DestroyDriver, DockerDriver};
use super::model::{
    BuildRequest, ContainerRef, ImageInfo, Inspect, LabelSel, Labels, NetworkInfo, NetworkName,
};
use crate::jobs::view::{ChildEntry, JobView, JobViewError};

/// A capture answer: the needle of the call it answers and what it gives.
type Canned = (String, Result<Vec<u8>, DockerError>);

/// Locks `mutex`, going on with the data of a holder that panicked.
fn held<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A [`DockerDriver`] that answers from what a test wrote and records every call as a line.
#[derive(Debug, Default)]
pub struct RecordingDocker {
    calls: Mutex<Vec<String>>,
    inspects: Mutex<BTreeMap<String, Inspect>>,
    listed: Mutex<Vec<ContainerRef>>,
    images: Mutex<BTreeMap<String, ImageInfo>>,
    captures: Mutex<Vec<Canned>>,
}

impl RecordingDocker {
    /// A fake that knows nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Answers `inspect(id)` with `answer`.
    #[must_use]
    pub fn with_inspect(self, id: &str, answer: Inspect) -> Self {
        held(&self.inspects).insert(id.to_owned(), answer);
        self
    }

    /// Answers every `ps_by_label` with `containers`.
    #[must_use]
    pub fn with_listed(self, containers: Vec<ContainerRef>) -> Self {
        *held(&self.listed) = containers;
        self
    }

    /// Answers `image_inspect(reference)` with `info`, and a `build` whose tag is `reference`.
    #[must_use]
    pub fn with_image(self, reference: &str, info: ImageInfo) -> Self {
        held(&self.images).insert(reference.to_owned(), info);
        self
    }

    /// Answers a `capture` whose arguments, joined by spaces, contain `needle` with `answer`. The
    /// first rule that matches answers; a capture that matches none fails.
    #[must_use]
    pub fn with_capture(self, needle: &str, answer: Result<Vec<u8>, DockerError>) -> Self {
        held(&self.captures).push((needle.to_owned(), answer)); // BORROW: the rule owns its needle
        self
    }

    /// The calls so far, one line each, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<String> {
        held(&self.calls).clone()
    }

    fn record(&self, line: String) {
        held(&self.calls).push(line);
    }
}

impl DockerDriver for RecordingDocker {
    fn inspect<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Inspect, DockerError>> {
        self.record(format!("inspect {id}"));
        let answer = held(&self.inspects).get(id).cloned();
        Box::pin(async move {
            answer.ok_or_else(|| DockerError::Failed {
                detail: format!("no such container: {id}"),
            })
        })
    }

    fn ps_by_label<'a>(
        &'a self,
        sel: &'a LabelSel,
    ) -> BoxFuture<'a, Result<Vec<ContainerRef>, DockerError>> {
        self.record(format!("ps {}={}", sel.key, sel.value));
        let listed = held(&self.listed).clone();
        Box::pin(async move { Ok(listed) })
    }

    fn network_create<'a>(
        &'a self,
        name: &'a NetworkName,
        internal: bool,
        labels: &'a Labels,
    ) -> BoxFuture<'a, Result<(), DockerError>> {
        let _ = labels;
        self.record(format!(
            "network create {} internal={internal}",
            name.as_str()
        ));
        Box::pin(async { Ok(()) })
    }

    fn network_inspect<'a>(
        &'a self,
        name: &'a NetworkName,
    ) -> BoxFuture<'a, Result<NetworkInfo, DockerError>> {
        self.record(format!("network inspect {}", name.as_str()));
        Box::pin(async {
            Err(DockerError::Failed {
                detail: "no such network".to_owned(),
            })
        })
    }

    fn image_inspect<'a>(
        &'a self,
        reference: &'a str,
    ) -> BoxFuture<'a, Result<Option<ImageInfo>, DockerError>> {
        self.record(format!("image inspect {reference}"));
        let info = held(&self.images).get(reference).cloned();
        Box::pin(async move { Ok(info) })
    }

    fn capture(
        &self,
        args: Vec<String>,
        deadline: std::time::Duration,
    ) -> BoxFuture<'_, Result<Vec<u8>, DockerError>> {
        let line = args.join(" ");
        self.record(format!("capture {line}"));
        let _ = deadline;
        let answer = held(&self.captures)
            .iter()
            .find(|(needle, _)| line.contains(needle.as_str())) // BORROW: explicit .as_str()
            .map(|(_, answer)| answer.clone()); // BORROW: .clone() gives the call its own answer
        Box::pin(async move {
            answer.unwrap_or_else(|| {
                Err(DockerError::Failed {
                    detail: format!("no scripted answer for: {line}"),
                })
            })
        })
    }

    fn build<'a>(
        &'a self,
        request: &'a BuildRequest,
    ) -> BoxFuture<'a, Result<ImageInfo, DockerError>> {
        self.record(format!(
            "build {} {}",
            request.tag,
            request.context.display()
        ));
        let built = held(&self.images).get(&request.tag).cloned();
        Box::pin(async move {
            built.ok_or_else(|| DockerError::Unavailable {
                detail: "RecordingDocker::build has no image scripted for this tag".to_owned(),
            })
        })
    }
}

/// A [`DestroyDriver`] that destroys nothing and records every call as a line.
#[derive(Debug, Default)]
pub struct RecordingDestroyer {
    calls: Mutex<Vec<String>>,
}

impl RecordingDestroyer {
    /// A destroyer with no calls.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The calls so far, one line each, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<String> {
        held(&self.calls).clone()
    }
}

impl DestroyDriver for RecordingDestroyer {
    fn stop<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        held(&self.calls).push(format!("stop {id}"));
        Box::pin(async { Ok(()) })
    }

    fn rm<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        held(&self.calls).push(format!("rm {id}"));
        Box::pin(async { Ok(()) })
    }

    fn network_rm<'a>(&'a self, name: &'a NetworkName) -> BoxFuture<'a, Result<(), DockerError>> {
        held(&self.calls).push(format!("network rm {}", name.as_str()));
        Box::pin(async { Ok(()) })
    }
}

/// Wraps a driver and panics when asked to destroy a container or a network it was not told
/// about. A test allows the ids it created; a destroy of any other id is a bug in the code under
/// test, and a panic cannot be swallowed as an error.
#[derive(Debug)]
pub struct AllowlistDriver<D> {
    inner: D,
    containers: Mutex<BTreeSet<String>>,
    networks: Mutex<BTreeSet<String>>,
}

impl<D> AllowlistDriver<D> {
    /// A wrapper around `inner` that allows nothing yet.
    #[must_use]
    pub fn new(inner: D) -> Self {
        Self {
            inner,
            containers: Mutex::new(BTreeSet::new()),
            networks: Mutex::new(BTreeSet::new()),
        }
    }

    /// Allows destroying the container `id`.
    pub fn allow_container(&self, id: &str) {
        held(&self.containers).insert(id.to_owned());
    }

    /// Allows removing the network `name`.
    pub fn allow_network(&self, name: &str) {
        held(&self.networks).insert(name.to_owned());
    }

    /// The wrapped driver.
    #[must_use]
    pub const fn inner(&self) -> &D {
        &self.inner
    }

    fn check_container(&self, verb: &str, id: &str) {
        assert!(
            held(&self.containers).contains(id),
            "{verb} of the container `{id}`, which the test did not create"
        );
    }
}

impl<D: DockerDriver> DockerDriver for AllowlistDriver<D> {
    fn inspect<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Inspect, DockerError>> {
        self.inner.inspect(id)
    }

    fn ps_by_label<'a>(
        &'a self,
        sel: &'a LabelSel,
    ) -> BoxFuture<'a, Result<Vec<ContainerRef>, DockerError>> {
        self.inner.ps_by_label(sel)
    }

    fn network_create<'a>(
        &'a self,
        name: &'a NetworkName,
        internal: bool,
        labels: &'a Labels,
    ) -> BoxFuture<'a, Result<(), DockerError>> {
        self.allow_network(name.as_str());
        self.inner.network_create(name, internal, labels)
    }

    fn network_inspect<'a>(
        &'a self,
        name: &'a NetworkName,
    ) -> BoxFuture<'a, Result<NetworkInfo, DockerError>> {
        self.inner.network_inspect(name)
    }

    fn image_inspect<'a>(
        &'a self,
        reference: &'a str,
    ) -> BoxFuture<'a, Result<Option<ImageInfo>, DockerError>> {
        self.inner.image_inspect(reference)
    }

    fn build<'a>(
        &'a self,
        request: &'a BuildRequest,
    ) -> BoxFuture<'a, Result<ImageInfo, DockerError>> {
        self.inner.build(request)
    }

    fn capture(
        &self,
        args: Vec<String>,
        deadline: std::time::Duration,
    ) -> BoxFuture<'_, Result<Vec<u8>, DockerError>> {
        self.inner.capture(args, deadline)
    }
}

impl<D: DestroyDriver> DestroyDriver for AllowlistDriver<D> {
    fn stop<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        self.check_container("stop", id);
        self.inner.stop(id)
    }

    fn rm<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        self.check_container("rm", id);
        self.inner.rm(id)
    }

    fn network_rm<'a>(&'a self, name: &'a NetworkName) -> BoxFuture<'a, Result<(), DockerError>> {
        assert!(
            held(&self.networks).contains(name.as_str()),
            "network rm of `{}`, which the test did not create",
            name.as_str()
        );
        self.inner.network_rm(name)
    }
}

/// A [`JobView`] that answers from what a test wrote.
#[derive(Debug, Clone)]
pub struct FakeJobView {
    job: JobId,
    children: Vec<ChildEntry>,
    lost: Vec<JobId>,
}

impl FakeJobView {
    /// A view of `job` with no records and no lost job.
    #[must_use]
    pub const fn new(job: JobId) -> Self {
        Self {
            job,
            children: Vec::new(),
            lost: Vec::new(),
        }
    }

    /// The records the view returns.
    #[must_use]
    pub fn with_children(mut self, children: Vec<ChildEntry>) -> Self {
        self.children = children;
        self
    }

    /// The jobs the view calls lost.
    #[must_use]
    pub fn with_lost(mut self, lost: Vec<JobId>) -> Self {
        self.lost = lost;
        self
    }

    /// The view as the guard holds it.
    #[must_use]
    pub fn shared(self) -> Arc<dyn JobView> {
        // TRAIT_OBJECT: the guard holds the view as a trait object
        Arc::new(self)
    }
}

impl JobView for FakeJobView {
    fn job<'a>(&'a self) -> &'a JobId {
        &self.job
    }

    fn children(&self) -> Result<Vec<ChildEntry>, JobViewError> {
        Ok(self.children.clone())
    }

    fn is_lost(&self, job: &JobId) -> Result<bool, JobViewError> {
        Ok(self.lost.contains(job))
    }
}
