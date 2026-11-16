//! The images the real-Docker guards need besides the agent base: the test agent, the fake model
//! and the gateway sidecar. Each is found or built through the core's docker client, keyed by the
//! digest of its build context, and removed by id once the test that built it ends.
//!
//! The file is shared by the core's and the binary's test crates, which include it by path, so it
//! uses only the public surface of `candler_core`.
#![allow(
    dead_code,
    reason = "two test crates include this file and each uses only part of it"
)]

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::driver::DockerDriver as _;
use candler_core::isolation::image::context_digest;
use candler_core::isolation::model::{BuildRequest, ImageId, ImageRef, Labels};
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::RunKey;

/// The build context of the test agent: busybox with `nc`, `wget` and `sh`.
pub const TESTAGENT_CONTEXT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/testagent");

/// The build context of the gateway sidecar.
pub const SIDECAR_CONTEXT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/gateway");

/// The label of every image this module builds, valued with the digits of its context's digest.
const LABEL: &str = "candler.test_image";

/// How many digits of the context's digest key an image.
const KEY_DIGITS: usize = 12;

/// Whether a call built the image or found it already there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Built {
    /// The image was there: the call built nothing and removes nothing.
    Found,
    /// The call built the image.
    Made,
}

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// The image of the build context `context`, tagged `tag` or, without one, by the digest of the
/// context: found when docker holds that tag, built through the core's client when it does not.
///
/// A tag keyed by the context's digest names exactly that content, so an image left by an older
/// context is never taken for the current one, and an image that is already there is used as it
/// is and never removed by the call that found it.
///
/// # Panics
/// When the context cannot be read or the build fails.
#[must_use]
pub fn test_image_as(context: &Path, tag: Option<&str>) -> (ImageId, Built) {
    let digest = context_digest(context).expect("the context's digest");
    let key = digest.get(..KEY_DIGITS).expect("twelve digits").to_owned(); // BORROW: the key owns its text
    let tag = tag.map_or_else(
        || {
            let name = context.file_name().map_or_else(
                || String::from("context"),
                |name| name.to_string_lossy().into_owned(),
            );
            format!("candler-test-{name}:{key}")
        },
        str::to_owned, // BORROW: the request owns its tag
    );
    let records = tempfile::tempdir().expect("a record directory");
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let cli = DockerCli::new(
        Arc::new(Supervisor::new(sink)),
        ClientEnv::from_process(),
        records.path().to_path_buf(), // BORROW: the client owns its record directory
    )
    .expect("a docker client");
    let request = BuildRequest {
        context: context.to_path_buf(), // BORROW: the request owns its context
        tag: tag.clone(),               // BORROW: .clone() keeps the tag for the lookup
        build_args: Vec::new(),
        labels: Labels::new().with(LABEL, &key),
    };
    // The test may run inside a runtime, so the call runs in a thread of its own.
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("a runtime")
                    .block_on(async {
                        if let Some(info) = cli.image_inspect(&tag).await.expect("an inspect") {
                            (info.id, Built::Found)
                        } else {
                            let info = cli.build(&request).await.expect("the image builds");
                            (info.id, Built::Made)
                        }
                    })
            })
            .join()
            .expect("the build thread")
    })
}

/// The image of `context` under its digest-keyed tag. See [`test_image_as`].
#[must_use]
pub fn test_image(context: &Path) -> (ImageId, Built) {
    test_image_as(context, None)
}

/// An image a test uses, removed by its id when the value is dropped if the test built it.
pub struct TestImage {
    id: ImageId,
    built: Built,
}

impl TestImage {
    /// The image of `context`, found or built.
    #[must_use]
    pub fn new(context: &str) -> Self {
        let (id, built) = test_image(Path::new(context));
        Self { id, built }
    }

    /// The image of `context` under the tag `tag`, found or built.
    #[must_use]
    pub fn tagged(context: &str, tag: &str) -> Self {
        let (id, built) = test_image_as(Path::new(context), Some(tag));
        Self { id, built }
    }

    /// The image's id, which docker takes as an image reference.
    #[must_use]
    pub fn id<'a>(&'a self) -> &'a str {
        self.id.0.as_str() // BORROW: explicit .as_str()
    }

    /// The image as the core names one.
    #[must_use]
    pub fn reference(&self) -> ImageRef {
        ImageRef {
            id: self.id.clone(), // BORROW: .clone() gives the reference its own id
            harness_version: None,
        }
    }
}

impl Drop for TestImage {
    fn drop(&mut self) {
        if self.built == Built::Made {
            // The id is removed; a container still on it, or a second tag, keeps the image and
            // is reported, never forced.
            let removed = Command::new("docker").args(["rmi", self.id()]).output();
            if !removed.is_ok_and(|output| output.status.success()) {
                eprintln!("the test image {} was not removed", self.id());
            }
        }
    }
}
