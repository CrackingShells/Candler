//! Guards for the image service over the real docker client and a PATH-fake `docker`.
//!
//! The fake is the process-level environment of `tests/fixtures/docker/fake-docker`: its `build`
//! writes a marker that its `image inspect` reads, and it takes a second to build, so calls made
//! together really overlap. It is an environment, not an argv oracle: what is asserted is how
//! many builds the world saw, never an argv built by the code under test.
#![cfg(test)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::image::{
    BuildSetting, DockerImageService, ImageAssets, ImageService, LockDigest,
};
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::RunKey;
use sha2::Digest as _;

#[path = "support/mod.rs"]
mod support;

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// The image id the fake prints for every build and answers for every inspect.
const FAKE_ID: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// What the fake answers to `docker image inspect --format '{{json .}}'`.
const INSPECT_ANSWER: &str = concat!(
    r#"{"Id":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","#,
    r#""RootFS":{"Layers":["sha256:aa","sha256:bb"]},"#,
    r#""Config":{"Labels":{"candler.cc_version":"2.1.311"}}}"#
);

/// A service over the real client, a fake docker and a store, and the fake's log.
struct Rig {
    service: DockerImageService,
    log: PathBuf,
    _dir: tempfile::TempDir,
}

fn write_script(path: &Path, text: &str) {
    std::fs::write(path, text).expect("a script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("a mode");
}

impl Rig {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a directory");
        let canned = dir.path().join("canned");
        let bin = dir.path().join("bin");
        let records = dir.path().join("records");
        for sub in [&canned, &bin, &records] {
            std::fs::create_dir_all(sub).expect("a directory");
        }
        let log = dir.path().join("docker.log");
        std::fs::write(canned.join("build.out"), format!("{FAKE_ID}\n")).expect("an answer");
        std::fs::write(canned.join("build.delay"), "1").expect("a delay");
        std::fs::write(canned.join("image-inspect.out"), INSPECT_ANSWER).expect("an answer");
        std::fs::write(canned.join("run.out"), "2.1.311 (Claude Code)\n").expect("an answer");
        // The id the fake prints for a build exists, as it does in docker.
        // BORROW: `.to_owned()` makes the owned file name the marker needs
        // INDEX: the fake id starts with the `sha256:` prefix, which is seven bytes
        std::fs::write(canned.join("built-sha256_".to_owned() + &FAKE_ID[7..]), "")
            .expect("a marker");
        // The client's environment is an allowlist, so the fake's own settings live in a wrapper.
        let wrapper = bin.join("docker");
        write_script(
            &wrapper,
            &format!(
                "#!/bin/sh\nexport FAKE_DOCKER_LOG='{}' FAKE_DOCKER_DIR='{}'\nexec '{}' \"$@\"\n",
                log.display(),
                canned.display(),
                support::fake_docker().display()
            ),
        );
        // TRAIT_OBJECT: the supervisor takes its sink as a trait object
        let sink: Arc<dyn EventSink> = Arc::new(Silent);
        let supervisor = Arc::new(Supervisor::new(sink));
        let env = ClientEnv::from_process();
        let cli = DockerCli::with_binary(
            Arc::clone(&supervisor),
            wrapper,
            env.clone(), // BORROW: .clone() gives the service its own environment
            records,
        );
        let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/docker");
        let service = DockerImageService::new(
            supervisor,
            cli,
            env,
            ImageAssets {
                base_context: sources.join("base"),
                env_context: sources.join("env"),
                store_root: dir.path().join("store"),
            },
        );
        Self {
            service,
            log,
            _dir: dir,
        }
    }

    /// How many `docker build` calls the fake logged for `tag`.
    fn builds_of(&self, tag: &str) -> usize {
        let text = std::fs::read_to_string(&self.log).unwrap_or_default();
        text.lines()
            .filter_map(|line| serde_json::from_str::<Vec<String>>(line).ok())
            .filter(|argv| argv.first().is_some_and(|verb| verb == "build"))
            .filter(|argv| {
                argv.windows(2)
                    // INDEX: `windows(2)` yields pairs of two
                    .any(|pair| pair[0] == "-t" && pair[1] == tag)
            })
            .count()
    }
}

/// A lock and the setting and digest that name it.
fn lock(text: &str) -> (LockDigest, BuildSetting, String) {
    let bytes = text.as_bytes().to_vec();
    let hex = sha2::Sha256::digest(&bytes)
        .iter()
        .fold(String::new(), |mut text, byte| {
            use std::fmt::Write as _;
            let _ = write!(text, "{byte:02x}");
            text
        });
    (
        LockDigest(hex.clone()), // BORROW: .clone() gives the digest its own text
        BuildSetting {
            lock: bytes,
            extension: None,
        },
        hex,
    )
}

/// Guards: B05 — two concurrent `ensure_env` calls for one lock digest make exactly one build
/// (plan D2, flock per tag).
/// Oracle: the PATH-fake docker's argv log, counting build lines for the tag.
/// Bite: take the flock after the "tag exists" check, not before.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn b05_one_build_per_digest() {
    let rig = Rig::new();
    let (digest, setting, hex) = lock("numpy==2.0.0\n");
    let tag = format!("candler-env:{hex}");

    let (first, second) = tokio::join!(
        rig.service.ensure(&digest, &setting),
        rig.service.ensure(&digest, &setting)
    );
    let (first, second) = (first.expect("an image"), second.expect("an image"));
    // Provenance: plan D2 — one build of the env tag per lock digest.
    assert_eq!(
        rig.builds_of(&tag),
        1,
        "two concurrent calls built the env twice"
    );
    assert_eq!(first.id, second.id);
    assert_eq!(first.id.0, FAKE_ID);

    // A call after both finished finds the image and builds nothing.
    rig.service
        .ensure(&digest, &setting)
        .await
        .expect("an image");
    assert_eq!(rig.builds_of(&tag), 1);

    // Positive control: another lock digest is another tag and builds again.
    let (other_digest, other_setting, other_hex) = lock("numpy==2.0.1\n");
    let other_tag = format!("candler-env:{other_hex}");
    rig.service
        .ensure(&other_digest, &other_setting)
        .await
        .expect("an image");
    assert_eq!(rig.builds_of(&other_tag), 1);
    assert_eq!(rig.builds_of(&tag), 1);
}
