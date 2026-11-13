//! The real-Docker guard of the harness version probe: the core reads the version of the real
//! `OpenCode` and Claude Code base images through the control container it gives the probe.
//!
//! The test builds each base image under a tag of its own and removes that image by exact id when
//! the build created it; it starts no other container than the probe's, which docker
//! removes on exit.
#![cfg(test)]

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::image::{DockerImageService, ImageAssets, ImageService};
use candler_core::isolation::model::{ImageId, ImageRef};
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::driver::HarnessDriver;
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_harness_opencode::driver::OpenCodeDriver;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::RunKey;

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// One base image: its build context under `assets/docker`, the tag it is built under, the
/// driver that probes it, the binary the oracle runs and the version the image carries.
struct Row {
    dir: &'static str,
    tag: &'static str,
    driver: Arc<dyn HarnessDriver>, // TRAIT_OBJECT: the rows hold different drivers
    binary: &'static str,
    want: &'static str,
}

/// The stdout of `docker <args>`, trimmed; empty when docker fails.
fn docker(args: &[&str]) -> String {
    let out = Command::new("docker")
        .args(args)
        .output()
        .expect("the docker CLI runs");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// Guards: R01 §Harness drivers — preflight reads the version of each real base image: the probe
/// runs in the read-only control container with the home and the state a run gives the harness,
/// so `opencode --version` creates its directories and prints, Claude Code's version is the one
/// it always read, and each version parses to the one the image carries.
/// Oracle: `docker run --rm --entrypoint <binary> <image> --version` on a writable root, and the
/// image's own label `candler.cc_version`, which agree on 2.2.7 and 2.1.311.
/// Bite: run the probe with no home and no state variables (the bare probe): `OpenCode` fails with
/// `makeDirectory (/home/agent/.local/share/opencode/log)` and exits 1, and the read is refused.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker"]
async fn opencode_probe_reads_each_real_base_image_version() {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/docker");
    let rows = vec![
        Row {
            dir: "base-opencode",
            tag: "candler-test-version-probe:opencode",
            driver: Arc::new(OpenCodeDriver::new().expect("the driver builds from constants")),
            binary: "opencode",
            want: "2.2.7",
        },
        Row {
            dir: "base",
            tag: "candler-test-version-probe:claude-code",
            driver: Arc::new(ClaudeCodeDriver::new().expect("the driver builds from constants")),
            binary: "claude",
            want: "2.1.311",
        },
    ];
    let mut failures = Vec::new();
    for Row {
        dir,
        tag,
        driver,
        binary,
        want,
    } in rows
    {
        let context = assets.join(dir);
        let context = context.to_str().expect("a UTF-8 path");
        let existing = docker(&["image", "inspect", "--format", "{{.Id}}", tag]);
        let id = docker(&["build", "--quiet", "-t", tag, context]);
        assert!(
            id.starts_with("sha256:"),
            "{dir}: the build printed an id: {id}"
        );
        let writable_root = docker(&["run", "--rm", "--entrypoint", binary, &id, "--version"]);
        let label = docker(&[
            "image",
            "inspect",
            "--format",
            "{{index .Config.Labels \"candler.cc_version\"}}",
            &id,
        ]);

        let tmp = tempfile::tempdir().expect("a scratch directory");
        // TRAIT_OBJECT: the supervisor takes its sink as a trait object
        let sink: Arc<dyn EventSink> = Arc::new(Silent);
        let sup = Arc::new(Supervisor::new(sink));
        let env = ClientEnv::from_process();
        let cli = DockerCli::new(Arc::clone(&sup), env.clone(), tmp.path().join("control"))
            .expect("a docker client"); // BORROW: .clone() gives the service its own environment
        let service = DockerImageService::new(
            sup,
            cli,
            env,
            ImageAssets {
                base_context: tmp.path().join("base"),
                env_context: tmp.path().join("env"),
                store_root: tmp.path().join("store"),
            },
            driver,
        );
        let image = ImageRef {
            id: ImageId(id.clone()), // BORROW: .clone() keeps the id for the cleanup
            harness_version: None,
        };
        let read = service.harness_version(&image).await;

        if existing.is_empty() {
            docker(&["rmi", &id]);
        }
        assert!(
            writable_root.contains(want),
            "{dir}: the oracle read {writable_root}"
        );
        assert_eq!(label, want, "{dir}: the image's label");
        match read {
            Ok(version) if version.to_string() == want => {}
            other => failures.push(format!("{dir}: the probe read {other:?}, not {want}")),
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
