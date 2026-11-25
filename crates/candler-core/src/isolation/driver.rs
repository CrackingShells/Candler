//! The two seams to docker. [`DockerDriver`] reads and runs; [`DestroyDriver`] removes and is
//! held only by the container guard, so that nothing else can destroy a container.

use std::time::Duration;

use candler_schemas::primitives::ContainerId;
use futures::future::BoxFuture;

use super::docker::{AttachedRun, DockerError, ExecOutput, attached_request};
use super::model::{
    BuildRequest, ContainerRef, ImageInfo, Inspect, LabelSel, Labels, NetworkInfo, NetworkName,
};
use crate::supervise::SuperviseError;
use crate::supervise::spawn::SpawnRequest;

/// The read and run half of docker.
pub trait DockerDriver: Send + Sync {
    /// Inspects one container.
    ///
    /// # Errors
    /// [`DockerError`] when docker fails or the container is unknown.
    fn inspect<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Inspect, DockerError>>;

    /// Inspects several containers in one docker call. The answers come in the order of `ids`; a
    /// container docker does not have answers [`DockerError::NotFound`] for its id alone, and the
    /// others keep their facts.
    fn inspect_many<'a>(
        &'a self,
        ids: &'a [String],
    ) -> BoxFuture<'a, Vec<Result<Inspect, DockerError>>> {
        Box::pin(async move {
            let mut answers = Vec::with_capacity(ids.len());
            for id in ids {
                // BORROW: explicit .as_str() lends the id to the single call
                answers.push(self.inspect(id.as_str()).await);
            }
            answers
        })
    }

    /// The supervised request that runs the client `run` describes, led by the docker binary this
    /// driver runs. The default finds `docker` on this process's `PATH`.
    ///
    /// # Errors
    /// [`SuperviseError`] when the request is refused.
    fn attach(&self, run: AttachedRun) -> Result<SpawnRequest, SuperviseError> {
        attached_request(run)
    }

    /// Creates a container with `docker create <args>` and returns its id. The container is
    /// created and not started.
    ///
    /// # Errors
    /// [`DockerError::Unavailable`] for a driver that creates nothing, and [`DockerError`] when
    /// docker fails or prints no container id.
    fn create(&self, args: Vec<String>) -> BoxFuture<'_, Result<ContainerId, DockerError>> {
        let _ = args;
        Box::pin(async {
            Err(DockerError::Unavailable {
                detail: String::from("create"),
            })
        })
    }

    /// Lists the containers that carry the label `sel`.
    ///
    /// # Errors
    /// [`DockerError`] when docker fails.
    fn ps_by_label<'a>(
        &'a self,
        sel: &'a LabelSel,
    ) -> BoxFuture<'a, Result<Vec<ContainerRef>, DockerError>>;

    /// Lists the containers that carry the label `sel`, with their inspect facts. The default
    /// lists, then inspects them all in one call; docker does both in two calls.
    ///
    /// # Errors
    /// [`DockerError`] when docker fails.
    fn ps_inspect_by_label<'a>(
        &'a self,
        sel: &'a LabelSel,
    ) -> BoxFuture<'a, Result<Vec<Inspect>, DockerError>> {
        Box::pin(async move {
            let listed = self.ps_by_label(sel).await?;
            let ids: Vec<String> = listed
                .iter()
                .map(|container| container.cid.as_str().to_owned()) // BORROW: .to_owned() gives the call its own ids
                .collect();
            let mut found = Vec::new();
            for answer in self.inspect_many(&ids).await {
                match answer {
                    Ok(inspected) => found.push(inspected),
                    Err(DockerError::NotFound { .. }) => {}
                    Err(error) => return Err(error),
                }
            }
            Ok(found)
        })
    }

    /// Lists the networks of `job`, its internal, egress and allow networks, by the job label
    /// (one `docker network ls`). A driver that does not list answers none.
    ///
    /// # Errors
    /// [`DockerError`] when docker fails.
    fn networks_of_job<'a>(
        &'a self,
        job: &'a candler_schemas::primitives::JobId,
    ) -> BoxFuture<'a, Result<Vec<NetworkName>, DockerError>> {
        let _ = job;
        Box::pin(async { Ok(Vec::new()) })
    }

    /// Creates a network.
    ///
    /// # Errors
    /// [`DockerError`] when docker fails.
    fn network_create<'a>(
        &'a self,
        name: &'a NetworkName,
        internal: bool,
        labels: &'a Labels,
    ) -> BoxFuture<'a, Result<(), DockerError>>;

    /// Inspects a network.
    ///
    /// # Errors
    /// [`DockerError`] when docker fails or the network is unknown.
    fn network_inspect<'a>(
        &'a self,
        name: &'a NetworkName,
    ) -> BoxFuture<'a, Result<NetworkInfo, DockerError>>;

    /// Inspects an image; `None` when docker does not have it.
    ///
    /// # Errors
    /// [`DockerError`] when docker fails for another reason.
    fn image_inspect<'a>(
        &'a self,
        reference: &'a str,
    ) -> BoxFuture<'a, Result<Option<ImageInfo>, DockerError>>;

    /// Builds an image and returns its info.
    ///
    /// # Errors
    /// [`DockerError`] when the build fails.
    fn build<'a>(
        &'a self,
        request: &'a BuildRequest,
    ) -> BoxFuture<'a, Result<ImageInfo, DockerError>>;

    /// Runs `docker <args>` for at most `deadline` and returns what it printed on standard
    /// output. The arguments are a container call that destroys nothing: a `docker exec` in a
    /// container, or a `docker run --rm` of a control container.
    ///
    /// # Errors
    /// [`DockerError::Unavailable`] for a driver that runs no such call, and [`DockerError`]
    /// when docker fails or the call outlives `deadline`.
    fn capture(
        &self,
        args: Vec<String>,
        deadline: Duration,
    ) -> BoxFuture<'_, Result<Vec<u8>, DockerError>> {
        let _ = (args, deadline);
        Box::pin(async {
            Err(DockerError::Unavailable {
                detail: String::from("capture"),
            })
        })
    }

    /// Runs `docker <args>` as [`DockerDriver::capture`] does and returns how it ended with what
    /// it printed on each stream, whatever the exit status. The default answers as `capture`
    /// does, with an exit of `0`.
    ///
    /// # Errors
    /// [`DockerError::Unavailable`] for a driver that runs no such call, and [`DockerError`] when
    /// docker cannot be run or the call outlives `deadline`.
    fn capture_exec(
        &self,
        args: Vec<String>,
        deadline: Duration,
    ) -> BoxFuture<'_, Result<ExecOutput, DockerError>> {
        let printed = self.capture(args, deadline);
        Box::pin(async move { printed.await.map(ExecOutput::printed) })
    }
}

/// The destroy half of docker: stop, remove, remove a network. Only the guard holds one.
pub trait DestroyDriver: Send + Sync {
    /// Stops a container.
    ///
    /// # Errors
    /// [`DockerError`] when docker fails.
    fn stop<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>>;

    /// Removes a container.
    ///
    /// # Errors
    /// [`DockerError`] when docker fails.
    fn rm<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>>;

    /// Removes a network.
    ///
    /// # Errors
    /// [`DockerError`] when docker fails.
    fn network_rm<'a>(&'a self, name: &'a NetworkName) -> BoxFuture<'a, Result<(), DockerError>>;
}
