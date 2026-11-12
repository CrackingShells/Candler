//! The real-Docker guard of the env image: what an experiment's requirements file and prune list
//! build, read back from the image.
//!
//! The test builds through [`DockerImageService`] over the real docker CLI, from the experiment's
//! requirements (`tests/fixtures/env_image/requirements.txt`) and a prune list. Everything it
//! inspects runs in control containers it creates with `docker create`, labelled
//! `candler.test=env_image`. Every container and image it created is removed by exact id, by
//! [`Created::finish`] and, when a test panics first, by its `Drop`.
#![cfg(test)]

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::image::{
    BuildSetting, DockerImageService, ImageAssets, ImageService, LockDigest, LockRequest,
    LockSource, PackageProbe,
};
use candler_core::isolation::model::ImageRef;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::RunKey;
use serde_json::{Value, json};
use sha2::Digest as _;

/// The distribution the fixture requirements pin, and its pinned version: a pure-Python wheel
/// with no dependencies.
const PACKAGE: &str = "colorama";
const PINNED: &str = "0.4.6";

/// A file the wheel ships (its `RECORD` lists it) and no seed package ships.
const PRUNED: &str = "isatty_test.py";

/// A file of the same wheel that the prune list does not name.
const KEPT: &str = "ansi_test.py";

/// The site-packages directory of the env image's interpreter.
const SITE: &str = "/opt/env/lib/python3.12/site-packages";

/// The packages `uv venv --seed` installs (uv's documentation of `--seed`).
const SEED: [&str; 3] = ["pip", "setuptools", "wheel"];

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// Runs `docker <args>` and returns its output, which a test reads and never trusts.
fn docker(args: &[&str]) -> std::process::Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("the docker CLI runs")
}

/// The lines `docker <args>` prints.
fn docker_lines(args: &[&str]) -> BTreeSet<String> {
    String::from_utf8_lossy(&docker(args).stdout)
        .lines()
        .map(str::to_owned) // BORROW: the set owns its ids
        .collect()
}

/// Every image id the daemon holds, in full.
fn all_images() -> BTreeSet<String> {
    docker_lines(&["images", "-aq", "--no-trunc"])
}

/// The lower-case hex sha256 of `bytes`.
fn hex_sha256(bytes: &[u8]) -> String {
    sha2::Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// The ids of the containers and images one test created, and the image ids that were there
/// before it started. Nothing outside these lists is ever removed.
struct Created {
    before: BTreeSet<String>,
    containers: Vec<String>,
    images: Vec<String>,
    lock_hex: Option<String>,
}

impl Created {
    fn new() -> Self {
        Self {
            before: all_images(),
            containers: Vec::new(),
            images: Vec::new(),
            lock_hex: None,
        }
    }

    /// Adds to the image list the env images that carry the test's lock label and the base image
    /// of the shared tag, when none of them was there before the test.
    fn adopt_built(&mut self) {
        let mut found = BTreeSet::new();
        if let Some(hex) = &self.lock_hex {
            let filter = format!("label=candler.lock={hex}");
            found.extend(docker_lines(&[
                "images",
                "-aq",
                "--no-trunc",
                "--filter",
                &filter,
            ]));
        }
        found.extend(docker_lines(&[
            "images",
            "-aq",
            "--no-trunc",
            "--filter",
            "reference=candler-base:src-*",
        ]));
        for id in found {
            if !self.before.contains(&id) && !self.images.contains(&id) {
                self.images.push(id);
            }
        }
    }

    /// Removes the containers, then the images, in reverse order of creation, by exact id.
    fn remove_all(&mut self) {
        self.adopt_built();
        for id in self.containers.drain(..) {
            let _ = docker(&["rm", &id]);
        }
        // The env image is a child of the base image: it goes first.
        for id in self.images.iter().rev() {
            let _ = docker(&["rmi", id]);
        }
    }

    /// Removes what the test created and checks that docker no longer knows any of it.
    fn finish(mut self) {
        self.remove_all();
        for id in &self.images {
            assert!(
                !docker(&["image", "inspect", id]).status.success(),
                "the image {id} is still there after the test"
            );
        }
        eprintln!(
            "env_image_e2e created and removed the images {:?}",
            self.images
        );
    }
}

impl Drop for Created {
    fn drop(&mut self) {
        self.remove_all();
    }
}

/// One test's service, scratch store and cleanup.
struct World {
    service: DockerImageService,
    probe_args: (Arc<Supervisor>, PathBuf, PathBuf),
    created: Created,
    _tmp: tempfile::TempDir,
}

/// Copies the files of `from` into `to`, one level deep.
fn copy_files(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a directory");
    for entry in std::fs::read_dir(from).expect("a directory to copy") {
        let path = entry.expect("an entry").path();
        let name = path.file_name().expect("a file name").to_owned(); // BORROW: the copy owns its name
        std::fs::copy(&path, to.join(name)).expect("a copied file");
    }
}

impl World {
    fn new() -> Self {
        let created = Created::new();
        let tmp = tempfile::tempdir().expect("a scratch directory");
        let root = tmp.path().canonicalize().expect("a canonical path");
        let store_root = root.join("store");
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/docker");
        // The service builds the env image from a copy of the Dockerfile; the repository's own is
        // never edited.
        let env_context = root.join("env-context");
        copy_files(&assets.join("env"), &env_context);
        // TRAIT_OBJECT: the supervisor takes its sink as a trait object
        let sink: Arc<dyn EventSink> = Arc::new(Silent);
        let sup = Arc::new(Supervisor::new(sink));
        let env = ClientEnv::from_process();
        let cli = DockerCli::new(Arc::clone(&sup), env.clone(), root.join("control"))
            .expect("a docker client"); // BORROW: .clone() gives the service its own environment
        let binary = cli.binary().to_path_buf(); // BORROW: the probe owns its path
        let service = DockerImageService::new(
            Arc::clone(&sup),
            cli,
            env,
            ImageAssets {
                base_context: assets.join("base"),
                env_context,
                store_root: store_root.clone(), // BORROW: .clone() keeps the root for the probe
            },
            Arc::new(ClaudeCodeDriver::new().expect("the driver builds from constants")),
        );
        Self {
            service,
            probe_args: (sup, binary, store_root),
            created,
            _tmp: tmp,
        }
    }

    /// Compiles `sources` into a lock and builds the env image of that lock with `prune`.
    async fn build(&mut self, sources: Vec<LockSource>, prune: &[&str]) -> ImageRef {
        let lock = self
            .service
            .lock(&LockRequest { sources })
            .await
            .expect("the lock compiles");
        let hex = hex_sha256(&lock);
        self.created.lock_hex = Some(hex.clone()); // BORROW: .clone() keeps the hex for the label filter
        let setting = BuildSetting {
            lock,
            extension: None,
            prune: prune.iter().map(|name| (*name).to_owned()).collect(), // BORROW: owned names
        };
        let built = self.service.ensure(&LockDigest(hex), &setting).await;
        self.created.adopt_built();
        built.expect("the env image builds")
    }

    /// Runs `entrypoint args…` in a new control container of `image` with no network and returns
    /// its stdout. The container is removed by its id.
    fn run_in(&mut self, image: &ImageRef, entrypoint: &str, args: &[&str]) -> String {
        let mut create = vec![
            "create",
            "--network",
            "none",
            "--label",
            "candler.test=env_image",
            "--entrypoint",
            entrypoint,
            image.id.0.as_str(), // BORROW: `.as_str()` borrows the image id
        ];
        create.extend_from_slice(args);
        let made = docker(&create);
        assert!(
            made.status.success(),
            "docker create: {}",
            String::from_utf8_lossy(&made.stderr)
        );
        let id = String::from_utf8_lossy(&made.stdout).trim().to_owned(); // BORROW: the list owns its id
        self.created.containers.push(id.clone()); // BORROW: .clone() keeps the id for the removal
        let ran = docker(&["start", "-a", &id]);
        assert!(
            ran.status.success(),
            "{entrypoint} {args:?}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        let _ = docker(&["rm", &id]);
        self.created.containers.retain(|known| *known != id);
        String::from_utf8_lossy(&ran.stdout).into_owned() // BORROW: the text outlives the output
    }

    /// The packages `uv pip list` shows in `image`, as `(name, version)`.
    fn uv_list(&mut self, image: &ImageRef) -> Vec<(String, String)> {
        let out = self.run_in(image, "uv", &["pip", "list", "--format", "json"]);
        let rows: Vec<Value> = serde_json::from_str(&out).expect("uv prints a JSON list");
        rows.iter()
            .map(|row| {
                let text = |key: &str| {
                    row.get(key)
                        .and_then(Value::as_str)
                        .expect("a string field")
                        .to_owned() // BORROW: the pair owns its text
                };
                (text("name"), text("version"))
            })
            .collect()
    }

    /// The reading of the import package `name` in `image`, through the service's probe.
    async fn read_package(&self, image: &ImageRef, name: &str) -> (Value, Value) {
        let (sup, binary, store_root) = &self.probe_args;
        let probe = PackageProbe::new(
            Arc::clone(sup),
            binary.clone(), // BORROW: .clone() gives the probe its own path
            ClientEnv::from_process(),
            store_root.clone(),    // BORROW: .clone() gives the probe its own path
            vec![name.to_owned()], // BORROW: the probe owns its names
        );
        let readings = probe.read(image).await.expect("the package read");
        let reading = readings.get(name).expect("a reading for the name");
        (reading.distribution.clone(), reading.version.clone()) // BORROW: .clone() copies the values out
    }
}

/// The fixture's requirements file.
fn requirements() -> Vec<LockSource> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/env_image/requirements.txt");
    vec![LockSource {
        name: String::from("requirements.txt"),
        bytes: std::fs::read(path).expect("the fixture requirements"),
    }]
}

/// Guards: X1 (agent environment) — an experiment's requirements file and prune list build an
/// env image in which the pruned file is absent and the pinned package is read back with its
/// distribution and version.
/// Oracle: the pin in `fixtures/env_image/requirements.txt` (`colorama==0.4.6`) and the wheel's
/// own `RECORD` in the container, which lists the pruned file, so the file was shipped; `uv pip
/// list` in the container. Candler's reader is the thing under test, never an oracle.
/// Bite: the env Dockerfile ignoring the prune argument.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs a Docker daemon and PyPI"]
async fn x1_env_image_prunes_the_named_file_and_reads_the_pin() {
    let mut world = World::new();
    let image = world.build(requirements(), &[PRUNED]).await;

    let record = world.run_in(
        &image,
        "cat",
        &[&format!("{SITE}/{PACKAGE}-{PINNED}.dist-info/RECORD")],
    );
    assert!(
        record.contains(&format!("{PACKAGE}/tests/{PRUNED},")),
        "the wheel does not ship {PRUNED}, so the prune proves nothing: {record}"
    );
    let pruned = world.run_in(&image, "find", &["/opt/env", "-name", PRUNED]);
    assert_eq!(pruned.trim(), "", "{PRUNED} is still under /opt/env");
    let kept = world.run_in(&image, "find", &["/opt/env", "-name", KEPT]);
    assert!(
        kept.contains(KEPT),
        "the prune removed {KEPT}, which the list does not name"
    );

    let (distribution, version) = world.read_package(&image, PACKAGE).await;
    assert_eq!(distribution, json!(PACKAGE), "the distribution read");
    assert_eq!(version, json!(PINNED), "the version read");
    let listed = world.uv_list(&image);
    assert!(
        listed.contains(&(PACKAGE.to_owned(), PINNED.to_owned())), // BORROW: owned pair to compare
        "uv pip list does not show {PACKAGE} {PINNED}: {listed:?}"
    );
    world.created.finish();
}

/// Guards: X1 (agent environment) — an experiment with no requirements gets an env image with no
/// third-party package.
/// Oracle: `uv pip list` in the container, against the packages uv's documentation says `--seed`
/// installs (pip, setuptools, wheel).
/// Bite: the lock carrying a package the experiment did not name.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs a Docker daemon"]
async fn x1_env_image_without_requirements_has_only_seed_packages() {
    let mut world = World::new();
    let image = world.build(Vec::new(), &[]).await;
    let listed = world.uv_list(&image);
    let names: BTreeSet<&str> = listed.iter().map(|(name, _)| name.as_str()).collect(); // BORROW: `.as_str()` borrows the names
    assert!(names.contains("pip"), "the seed is missing: {listed:?}");
    let extra: Vec<&&str> = names.iter().filter(|name| !SEED.contains(name)).collect();
    assert!(extra.is_empty(), "third-party packages present: {extra:?}");
    let (distribution, version) = world.read_package(&image, PACKAGE).await;
    assert_eq!(distribution, json!([]), "{PACKAGE} is installed");
    assert_eq!(version, json!([]), "{PACKAGE} is installed");
    world.created.finish();
}
