//! Every kind of control container runs on the real agent base image.

use std::sync::Arc;
use std::time::Duration;

use candler_core::isolation::container::exec::collect_spec;
use candler_core::isolation::container::mounts::{MountMode, MountSpec};
use candler_core::isolation::container::run::catalog_spec;
use candler_core::isolation::control::outcome;
use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::driver::DockerDriver as _;
use candler_core::isolation::image::{
    DockerImageService, ImageAssets, ImageService as _, PackageProbe, base_tag, probe_env,
    probe_spec,
};
use candler_core::isolation::model::{
    ContainerPlan, ImageRef, Labels, Limits, NetworkName, UserSpec,
};
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::driver::HarnessDriver as _;
use candler_harness::spec::CommandSpec;
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{JobId, RunKey};

use super::controls_e2e_runs::image_id;
use crate::support::gateway::docker;

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// The command `argv`, as the control containers take one.
fn command(argv: &[&str]) -> CommandSpec {
    CommandSpec::builder()
        .argv(argv.iter().map(|word| (*word).to_owned()).collect()) // BORROW: the command owns its words
        .build()
        .expect("a command")
}

/// Guards: R01 §Process supervision (control containers) — each kind of control container the
/// core builds, the harness version probe, the package probe, the catalog capture and the collect
/// of a run's files, runs on the real agent base image, built through `DockerImageService::base`,
/// reads what its caller put there, and gets a home the read-only root leaves writable.
/// Oracle: the image's own `candler.cc_version` label as docker reports it, the distribution
/// `pip` that the python in the image ships, and the strings the commands print and write; each
/// is read from the container, not from Candler's reader.
/// Bite: a probe builder that gives the container no `HOME` of its own (the image's `HOME` is not
/// writable on a read-only root, the M4 failure of the `OpenCode` probe), or a builder that drops
/// `--read-only` from only one kind.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs a Docker daemon and the network to build the agent base image"]
async fn control_each_kind_runs_on_the_real_base() {
    let base_context = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/base");
    let driver = ClaudeCodeDriver::new();
    let base_tag = base_tag(
        std::path::Path::new(base_context),
        &driver.base_image().build_args,
    )
    .expect("a tag");
    let base_made = image_id(&base_tag).is_none();

    let scratch = tempfile::tempdir_in(std::env::var("HOME").expect("a home directory"))
        .expect("a scratch directory under the home");
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let supervisor = Arc::new(Supervisor::new(sink));
    let env = ClientEnv::from_process();
    let cli = DockerCli::new(
        Arc::clone(&supervisor),
        env.clone(), // BORROW: .clone() gives the client its own environment
        scratch.path().join("records"),
    )
    .expect("a docker client");
    let service = DockerImageService::new(
        Arc::clone(&supervisor),
        cli.clone(), // BORROW: .clone() gives the service its own client
        env.clone(), // BORROW: .clone() gives the service its own environment
        ImageAssets {
            base_context: base_context.into(),
            env_context: scratch.path().join("env"),
            store_root: scratch.path().join("store"),
        },
        Arc::new(ClaudeCodeDriver::new()),
    );
    let base: ImageRef = service.base().await.expect("the base image builds");
    let deadline = Duration::from_secs(60);

    let outcome_of = |printed: Result<_, _>| outcome(printed.expect("the container ran"));

    // The version probe: the version the service reads is the label the image carries.
    let label = docker(&[
        "image",
        "inspect",
        "--format",
        "{{index .Config.Labels \"candler.cc_version\"}}",
        &base.id.0,
    ])
    .must("the base image's harness label");
    let read = service
        .harness_version(&base)
        .await
        .expect("the probe reads the version");
    assert_eq!(
        read.to_string(),
        label.trim(),
        "the probe's version is the label's"
    );

    // The probe's home is writable on a read-only root.
    let spec = probe_spec(
        &base,
        &probe_env(&driver),
        &[
            String::from("sh"),
            String::from("-c"),
            String::from("mkdir -p \"$HOME\" && test -w \"$HOME\" && echo writable"),
        ],
    );
    let printed = outcome_of(cli.control(&spec, deadline, 4096).await)
        .expect("a probe with a writable home exits 0");
    assert_eq!(String::from_utf8_lossy(&printed).trim(), "writable");

    // The package probe reads a distribution the python in the image ships.
    let probe = PackageProbe::new(
        Arc::clone(&supervisor),
        cli.binary().to_path_buf(), // BORROW: the probe owns the client's binary path
        env.clone(),                // BORROW: the probe owns its environment
        scratch.path().join("store"),
        vec![String::from("pip")],
    );
    let readings = probe.read(&base).await.expect("the package probe reads");
    assert_eq!(
        readings
            .get("pip")
            .map(|reading| reading.distribution.clone()),
        Some(serde_json::Value::String(String::from("pip"))),
        "the distribution that provides `pip`"
    );

    // The catalog capture, over the limits its caller gives.
    let limits = Limits {
        pids_limit: 64,
        memory: String::from("512m"),
        cpus: 1.0,
    };
    let spec = catalog_spec(
        &base.id,
        &limits,
        Duration::from_secs(20),
        &command(&["sh", "-c", "echo catalog-ran"]),
    )
    .expect("a catalog spec");
    let printed = outcome_of(cli.control(&spec, deadline, 4096).await).expect("the capture ran");
    assert_eq!(String::from_utf8_lossy(&printed).trim(), "catalog-ran");

    // The collect reads what the run's mounts hold, as the engine's user.
    let project = scratch.path().join("project");
    let work = project.join("work");
    let home = project.join("home");
    for dir in [&work, &home] {
        std::fs::create_dir_all(dir).expect("a mount directory");
        std::fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o777))
            .expect("a world-writable mount");
    }
    let job: JobId = "01900000-0000-7000-8000-00000000c011"
        .parse()
        .expect("a job id");
    let plan = ContainerPlan {
        image: base.clone(), // BORROW: .clone() gives the plan its own image
        labels: Labels::new(),
        mounts: vec![
            MountSpec {
                source: work.clone(), // BORROW: .clone() gives the mount its own path
                target: String::from("/work"),
                mode: MountMode::ReadWrite,
            },
            MountSpec {
                source: home.clone(), // BORROW: .clone() gives the mount its own path
                target: String::from("/home/agent"),
                mode: MountMode::ReadWrite,
            },
        ],
        env: vec![(String::from("HOME"), String::from("/home/agent"))],
        gateway: None,
        state: Vec::new(),
        network: NetworkName::for_job(&job),
        egress: None,
        limits,
        record_dir: project.clone(),
        user: UserSpec::Engine,
        workdir: String::from("/work"),
        home: String::from("/home/agent"),
        expiry: Duration::from_secs(30),
        project_root: project.clone(), // BORROW: .clone() gives the plan its own path
        store_root: scratch.path().join("store"),
        interactive: false,
    };
    std::fs::create_dir_all(&plan.store_root).expect("a store directory");
    let spec = collect_spec(
        &plan,
        &command(&["sh", "-c", "echo collected > /work/out && cat /work/out"]),
        Duration::from_secs(20),
    )
    .expect("a collect spec");
    let printed = cli
        .capture(spec.args(), deadline)
        .await
        .expect("the collect ran");
    assert_eq!(String::from_utf8_lossy(&printed).trim(), "collected");
    assert_eq!(
        std::fs::read_to_string(work.join("out"))
            .expect("the file the container wrote")
            .trim(),
        "collected",
        "the container wrote through the mount"
    );

    // what the test built goes, by exact id
    if base_made {
        docker(&["image", "rm", &base.id.0]).must("removing the agent base image");
    }
}
