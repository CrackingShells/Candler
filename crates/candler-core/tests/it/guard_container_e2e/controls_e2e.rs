//! The real-Docker guard of the session controls: the production controls run the real Claude
//! Code in the agent base image, behind the production gateway sidecar, against the scripted fake
//! model (`assets/docker/fakemodel`), through the production isolation and run loop. No token is
//! spent and no real model is called.
//!
//! The test removes, by exact id, the images it made, and checks that it left no container.
#![cfg(test)]

use std::path::PathBuf;
use std::sync::Arc;

use candler_harness::driver::HarnessDriver;

use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::image::{
    DockerImageService, ImageAssets, context_digest, sidecar_tag,
};
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
use crate::support::test_image::{SIDECAR_CONTEXT, TestImage};
use gateway_support::docker;

/// Discards every event.
struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// The id of the image `reference`, when docker holds it.
pub(super) fn image_id(reference: &str) -> Option<String> {
    let found = docker(&["image", "inspect", "--format", "{{.Id}}", reference]);
    found.ok().then(|| found.out.trim().to_owned())
}

/// The agent base image of `context`, found or built by the core's image service, as a job's
/// image step finds or builds it.
async fn base_image(
    driver: Arc<dyn HarnessDriver>, // TRAIT_OBJECT: the driver is chosen at run time
    context: &str,
) -> ImageRef {
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
        driver,
    );
    service.base().await.expect("the agent base image builds")
}

/// The sorted lines `docker` prints for `args`.
pub(crate) fn listed(args: &[&str]) -> Vec<String> {
    let mut lines: Vec<String> = docker(args).out.lines().map(str::to_owned).collect();
    lines.sort();
    lines
}

/// The production controls' world for one harness: the agent base image, the fake model's image,
/// the production gateway sidecar's image, and a runtime and job over them. [`ControlWorld::finish`]
/// removes, by exact id, the images this world made and checks that no container of its job is left.
pub(crate) struct ControlWorld {
    pub rig: Rig,
    pub rt: Runtime,
    pub job: candler_core::jobs::model::JobSpec,
    pub runner: DockerControlRunner,
    sidecar: TestImage,
    fake_tag: String,
    fake_made: bool,
    base: ImageRef,
    base_made: bool,
}

impl ControlWorld {
    /// Builds the world for `driver`, whose base image is built from `base_dir` below `assets/docker`.
    pub(crate) async fn new(
        driver: Arc<dyn HarnessDriver>, // TRAIT_OBJECT: the driver is chosen at run time
        base_dir: &str,
    ) -> Self {
        let base_context = format!(
            "{}/../../assets/docker/{base_dir}",
            env!("CARGO_MANIFEST_DIR")
        );
        let fake_context = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/fakemodel");
        // The production gateway reads the sidecar under the tag of its build context.
        let sidecar_tag = sidecar_tag(std::path::Path::new(SIDECAR_CONTEXT)).expect("a tag");
        let sidecar = TestImage::tagged(SIDECAR_CONTEXT, &sidecar_tag);
        let fake_tag = format!(
            "candler-fakemodel:src-{}",
            context_digest(std::path::Path::new(fake_context))
                .expect("the fake's digest")
                .get(..12)
                .expect("twelve digits")
        );
        let fake_made = image_id(&fake_tag).is_none();
        let base_args = driver.base_image().build_args;
        let base_tag = candler_core::isolation::image::base_tag(
            std::path::Path::new(&base_context),
            &base_args,
        )
        .expect("a tag");
        let base_made = image_id(&base_tag).is_none();
        let base = base_image(Arc::clone(&driver), &base_context).await;
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
            driver: Arc::clone(&driver),
            cells: std::collections::BTreeMap::new(),
            eval_infra: EvalInfra::default(),
        };
        let facts = Facts {
            secrets: Vec::new(),
            ..Facts::base()
        };
        let mut job = spec(&["m"], &facts, 600, 1);
        let harness = json!({"name": driver.id().to_string(), "version": version});
        let mut resolved = serde_json::to_value(&job.resolution).expect("a resolution");
        for value in resolved["values"].as_array_mut().expect("values") {
            if value["path"] == "harness" {
                value["value"] = harness.clone();
            }
        }
        job.resolution = serde_json::from_value(resolved).expect("a resolution");
        let runner = DockerControlRunner::new(ControlContexts {
            base: base_context.into(),
            base_build_args: base_args,
            fakemodel: fake_context.into(),
            gateway: SIDECAR_CONTEXT.into(),
        });
        Self {
            rig,
            rt,
            job,
            runner,
            sidecar,
            fake_tag,
            fake_made,
            base,
            base_made,
        }
    }

    /// The harness value of the job's resolution.
    pub(crate) fn harness(&self) -> Value {
        self.job
            .resolution
            .values
            .iter()
            .find(|value| value.path().as_str() == "harness")
            .and_then(|value| value.value())
            .map(|value| value.as_value().clone())
            .expect("the resolution names a harness")
    }

    /// Removes what the world made, by exact id, and checks that no container is left behind.
    pub(crate) fn finish(self) {
        if self.fake_made
            && let Some(id) = image_id(&self.fake_tag)
        {
            docker(&["image", "rm", &id]).must("removing the fake model's image");
        }
        drop(self.sidecar);
        if self.base_made {
            docker(&["image", "rm", &self.base.id.0]).must("removing the agent base image");
        }
        for key in ["candler.job", "candler.control_job"] {
            let label = format!("label={key}={}", self.rig.job);
            assert_eq!(
                listed(&["ps", "-aq", "--filter", &label]),
                Vec::<String>::new(),
                "no container of the job is left behind ({key})"
            );
        }
    }
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
    let world = ControlWorld::new(Arc::new(ClaudeCodeDriver::new()), "base").await;
    let ctl = JobControl {
        cancel: Arc::new(tokio::sync::watch::channel(None).0),
    };
    let result = ensure(&world.rt, &world.job, &world.runner, &ctl).await;
    let record_path = result
        .as_ref()
        .ok()
        .map(|reference| world.rig.store.layout().root().join(&reference.path));
    let record = record_path.map(|path| std::fs::read(path).expect("the record"));
    world.finish();

    let reference = result.unwrap_or_else(|error| panic!("the controls did not pass: {error}"));
    drop(reference);
    let record: Value =
        serde_json::from_slice(&record.expect("the record bytes")).expect("a record");
    let names: Vec<&str> = record["results"]
        .as_array()
        .expect("results")
        .iter()
        .filter_map(|result| result["control"].as_str())
        .collect();
    assert_eq!(names, ["c1", "c2"], "the record names both controls");
}
