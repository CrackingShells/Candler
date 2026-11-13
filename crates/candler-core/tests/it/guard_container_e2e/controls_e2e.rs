//! The real-Docker guard of the session controls: the production controls run the real Claude
//! Code in the agent base image, behind the production gateway sidecar, against the scripted fake
//! model (`assets/docker/fakemodel`), through the production isolation and run loop. No token is
//! spent and no real model is called.
//!
//! The test removes, by exact id, the images it made, and checks that it left no container.
#![cfg(test)]

use std::path::PathBuf;
use std::sync::Arc;

use candler_harness::driver::HarnessDriver as _;

use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::image::{DockerImageService, ImageAssets, context_digest};
use candler_core::isolation::model::ImageRef;
use candler_core::jobs::model::JobControl;
use candler_core::jobs::orchestrate::controls::{ControlContexts, DockerControlRunner, ensure};
use candler_core::jobs::slots::Slots;
use candler_core::runner::Runtime;
use candler_core::runner::evidence::provenance::EvalInfra;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::RunKey;
use serde_json::{Value, json};

use crate::support::gateway as gateway_support;

use crate::support::job_orchestrate::{Rig, spec};
use crate::support::run_plan::Facts;
use gateway_support::{GATEWAY_CONTEXT, docker};

/// The sidecar's tag, as the binary builds it.
const SIDECAR_TAG: &str = "candler-gateway:latest";

/// Discards every event.
struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// The id of the image `reference`, when docker holds it.
fn image_id(reference: &str) -> Option<String> {
    let found = docker(&["image", "inspect", "--format", "{{.Id}}", reference]);
    found.ok().then(|| found.out.trim().to_owned())
}

/// The agent base image of `context`, found or built by the core's image service, as a job's
/// image step finds or builds it.
async fn base_image(context: &str) -> ImageRef {
    let tmp = tempfile::tempdir().expect("a scratch directory");
    // TRAIT_OBJECT: the supervisor takes its sink as a trait object
    let sink: Arc<dyn EventSink> = Arc::new(NullSink);
    let supervisor = Arc::new(Supervisor::new(sink));
    let env = ClientEnv::from_process();
    let cli = DockerCli::new(
        Arc::clone(&supervisor),
        env.clone(),
        tmp.path().join("records"),
    )
    .expect("a docker client"); // BORROW: .clone() gives the service its own environment
    let service = DockerImageService::new(
        supervisor,
        cli,
        env,
        ImageAssets {
            base_context: PathBuf::from(context),
            env_context: tmp.path().join("env"),
            store_root: tmp.path().join("store"),
        },
        Arc::new(ClaudeCodeDriver::new()),
    );
    service.base().await.expect("the agent base image builds")
}

/// The sorted lines `docker` prints for `args`.
fn listed(args: &[&str]) -> Vec<String> {
    let mut lines: Vec<String> = docker(args).out.lines().map(str::to_owned).collect();
    lines.sort();
    lines
}

/// Guards: R01 §Session runner — the real controls pass on the pinned Claude Code in the
/// agent base image: C1 waits for its background job and closes, C2 stops once it has answered,
/// the passing record is written below the store's `controls/` and names both controls, and the
/// world the controls built leaves no container behind.
/// Oracle: the fake model's scripts `c1` and `c2` and the close rule, as the verdict reads them;
/// the containers docker lists before and after the test.
/// Bite: leave `-i` out of the container's command line (the harness reads its end of input at
/// once, C2 records no turn and the control is refused).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Docker; run with --run-ignored only"]
async fn controls_e2e_pinned_harness_passes() {
    let base_context = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/base");
    let fake_context = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/fakemodel");
    let containers = listed(&["ps", "-aq"]);
    let sidecar_made = image_id(SIDECAR_TAG).is_none();
    if sidecar_made {
        docker(&["build", "--quiet", "-t", SIDECAR_TAG, GATEWAY_CONTEXT]).must("docker build");
    }
    let fake_tag = format!(
        "candler-fakemodel:src-{}",
        context_digest(std::path::Path::new(fake_context))
            .expect("the fake's digest")
            .get(..12)
            .expect("twelve digits")
    );
    let fake_made = image_id(&fake_tag).is_none();

    let base_tag = candler_core::isolation::image::base_tag(
        std::path::Path::new(base_context),
        &ClaudeCodeDriver::new().base_image().build_args,
    )
    .expect("a tag");
    let base_made = image_id(&base_tag).is_none();
    let base = base_image(base_context).await;
    let version = docker(&[
        "image",
        "inspect",
        "--format",
        "{{index .Config.Labels \"candler.cc_version\"}}",
        &base.id.0,
    ])
    .must("the agent base image's harness label")
    .trim()
    .to_owned();

    let rig = Rig::new(0);
    let sink: Arc<dyn EventSink> = Arc::new(NullSink);
    let supervisor = Arc::new(Supervisor::new(Arc::clone(&sink)));
    let records = rig.dir.path().join("records");
    std::fs::create_dir_all(&records).expect("a directory");
    let cli = DockerCli::new(Arc::clone(&supervisor), ClientEnv::from_process(), records)
        .expect("a docker client");
    let slots = rig.dir.path().join("slots");
    std::fs::create_dir_all(&slots).expect("a directory");
    let rt = Runtime {
        supervisor,
        store: rig.store.clone(),
        sink,
        slots: Slots::open(&slots, 2).expect("the slots"),
        docker: Arc::new(cli),
        isolation: None,
        gateway: None,
        job: rig.job.clone(),
        driver: Arc::new(ClaudeCodeDriver::new()),
        cells: std::collections::BTreeMap::new(),
        eval_infra: EvalInfra::default(),
    };
    let facts = Facts {
        secrets: Vec::new(),
        ..Facts::base()
    };
    let mut job = spec(&["m"], &facts, 600, 1);
    let harness = json!({"name": "claude-code", "version": version});
    let mut resolved = serde_json::to_value(&job.resolution).expect("a resolution");
    for value in resolved["values"].as_array_mut().expect("values") {
        if value["path"] == "harness" {
            value["value"] = harness.clone();
        }
    }
    job.resolution = serde_json::from_value(resolved).expect("a resolution");

    let runner = DockerControlRunner::new(ControlContexts {
        base: base_context.into(),
        base_build_args: ClaudeCodeDriver::new().base_image().build_args,
        fakemodel: fake_context.into(),
    });
    let ctl = JobControl {
        cancel: Arc::new(tokio::sync::watch::channel(None).0),
    };
    let result = ensure(&rt, &job, &runner, &ctl).await;

    // what the test made goes, by exact id, whatever the result was
    if fake_made && let Some(id) = image_id(&fake_tag) {
        docker(&["image", "rm", &id]).must("removing the fake model's image");
    }
    if sidecar_made && let Some(id) = image_id(SIDECAR_TAG) {
        docker(&["image", "rm", &id]).must("removing the sidecar image");
    }
    if base_made {
        docker(&["image", "rm", &base.id.0]).must("removing the agent base image");
    }

    let reference = result.unwrap_or_else(|error| panic!("the controls did not pass: {error}"));
    let record: Value = serde_json::from_slice(
        &std::fs::read(rig.store.layout().root().join(&reference.path)).expect("the record"),
    )
    .expect("a record");
    let names: Vec<&str> = record["results"]
        .as_array()
        .expect("results")
        .iter()
        .filter_map(|result| result["control"].as_str())
        .collect();
    assert_eq!(names, ["c1", "c2"], "the record names both controls");
    assert_eq!(
        listed(&["ps", "-aq"]),
        containers,
        "no container is left behind"
    );
}
