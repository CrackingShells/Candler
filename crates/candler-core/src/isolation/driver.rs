//! The two seams to docker. [`DockerDriver`] reads and runs; [`DestroyDriver`] removes and is
//! held only by the container guard, so that nothing else can destroy a container.

use futures::future::BoxFuture;

use super::docker::DockerError;
use super::model::{
    BuildRequest, ContainerRef, ImageInfo, Inspect, LabelSel, Labels, NetworkInfo, NetworkName,
};

/// The read and run half of docker.
pub trait DockerDriver: Send + Sync {
    /// Inspects one container.
    ///
    /// # Errors
    /// [`DockerError`] when docker fails or the container is unknown.
    fn inspect<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Inspect, DockerError>>;

    /// Lists the containers that carry the label `sel`.
    ///
    /// # Errors
    /// [`DockerError`] when docker fails.
    fn ps_by_label<'a>(
        &'a self,
        sel: &'a LabelSel,
    ) -> BoxFuture<'a, Result<Vec<ContainerRef>, DockerError>>;

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
