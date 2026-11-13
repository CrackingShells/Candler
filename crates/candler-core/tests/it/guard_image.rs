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
    BuildSetting, DockerImageService, ImageAssets, ImageService, LockDigest, PackageProbe,
};
use candler_core::isolation::model::{ImageId, ImageRef};
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, ResolveInput};
use candler_core::resolve::preflight::Resolver;
use candler_core::resolve::steps::image::ImageIdStep;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::FieldPath;
use candler_schemas::primitives::RunKey;
use candler_schemas::values::JsonValue;
use serde_json::json;
use sha2::Digest as _;

use crate::support;

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
    service: Arc<DockerImageService>,
    sup: Arc<Supervisor>,
    wrapper: PathBuf,
    canned: PathBuf,
    store_root: PathBuf,
    log: PathBuf,
    _dir: tempfile::TempDir,
}

fn write_script(path: &Path, text: &str) {
    std::fs::write(path, text).expect("a script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("a mode");
}

/// The tag of the base image built from `assets/docker/base` with `CC_VERSION=2.1.311`, as docker
/// holds it: the first 12 hex digits of the sha256 of the sources' digest (each file's name, a
/// NUL, its bytes and a NUL, in name order), a NUL, then `CC_VERSION`, a NUL, `2.1.311` and a
/// NUL, computed with `shasum`.
const BASE_SUFFIX: &str = "src-12bee9345901";

/// The env tag's part for the sources of `assets/docker/env`: the first 12 hex digits of the
/// sha256 of each file's name, a NUL, its bytes and a NUL, in name order, computed with `shasum`.
const ENV_SUFFIX: &str = "env-e7ca5eaba54a";

/// The directory `assets/docker/<name>` of the repository.
fn assets(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/docker")
        .join(name)
}

impl Rig {
    fn new() -> Self {
        Self::over(&assets("base"), &assets("env"))
    }

    /// A world whose base image is built from the context `base` and whose env layer from the
    /// sources in `env`.
    fn over(base: &Path, env_sources: &Path) -> Self {
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
            wrapper.clone(), // BORROW: .clone() keeps the wrapper for the probe
            env.clone(),     // BORROW: .clone() gives the service its own environment
            records,
        );
        let store_root = dir.path().join("store");
        let service = Arc::new(DockerImageService::new(
            Arc::clone(&supervisor),
            cli,
            env,
            ImageAssets {
                base_context: base.to_path_buf(),
                env_context: env_sources.to_path_buf(),
                store_root: store_root.clone(), // BORROW: .clone() keeps the root for the probe
            },
            Arc::new(ClaudeCodeDriver::new()),
        ));
        Self {
            service,
            sup: supervisor,
            wrapper,
            canned,
            store_root,
            log,
            _dir: dir,
        }
    }

    /// A probe of the packages `names` over this world's client.
    fn probe(&self, names: &[&str]) -> PackageProbe {
        PackageProbe::new(
            Arc::clone(&self.sup),
            self.wrapper.clone(), // BORROW: .clone() gives the probe its own path
            ClientEnv::from_process(),
            self.store_root.clone(), // BORROW: .clone() gives the probe its own path
            names.iter().map(|name| (*name).to_owned()).collect(), // BORROW: owned names
        )
    }

    /// The tags of the env images the fake built, in order.
    fn env_tags_built(&self) -> Vec<String> {
        let text = std::fs::read_to_string(&self.log).unwrap_or_default();
        text.lines()
            .filter_map(|line| serde_json::from_str::<Vec<String>>(line).ok())
            .filter(|argv| argv.first().is_some_and(|verb| verb == "build"))
            .flat_map(|argv| {
                argv.windows(2)
                    // INDEX: `windows(2)` yields pairs of two
                    .filter(|pair| pair[0] == "-t" && pair[1].starts_with("candler-env:"))
                    // INDEX: `windows(2)` yields pairs of two
                    .map(|pair| pair[1].clone()) // BORROW: owned tag
                    .collect::<Vec<_>>()
            })
            .collect()
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

/// A lock and the setting and digest that name it, pruning nothing.
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
            prune: Vec::new(),
        },
        hex,
    )
}

/// `setting` with the files `names` pruned.
fn pruning(setting: &BuildSetting, names: &[&str]) -> BuildSetting {
    BuildSetting {
        // BORROW: .clone() gives the setting its own lock bytes
        lock: setting.lock.clone(),
        extension: None,
        prune: names.iter().map(|name| (*name).to_owned()).collect(), // BORROW: owned names
    }
}

/// Guards: B05 — two concurrent `ensure_env` calls for one lock digest make exactly one build
/// (plan D2, flock per tag).
/// Oracle: the PATH-fake docker's argv log, counting build lines for the tag.
/// Bite: take the flock after the "tag exists" check, not before.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn b05_one_build_per_digest() {
    let rig = Rig::new();
    let (digest, setting, hex) = lock("numpy==2.0.0\n");
    let tag = format!("candler-env:{hex}-{BASE_SUFFIX}-{ENV_SUFFIX}");

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
    let other_tag = format!("candler-env:{other_hex}-{BASE_SUFFIX}-{ENV_SUFFIX}");
    rig.service
        .ensure(&other_digest, &other_setting)
        .await
        .expect("an image");
    assert_eq!(rig.builds_of(&other_tag), 1);
    assert_eq!(rig.builds_of(&tag), 1);
}

/// Guards: A1 (R01 §Agent environment and secrets) — the env image is keyed on the lock, on the
/// files the experiment prunes and on its base image: one lock with two prune lists gives two
/// images, one lock with one list, in any order, gives one, and one lock on two bases gives two.
/// Oracle: the tags below are the sha256 of the lock's hex digest followed by a NUL and each
/// sorted prune name, computed with `shasum`, not with Candler, then `-` and the base's tag
/// suffix as docker holds it, then `-` and the env sources' part; the PATH-fake docker's argv
/// log counts the builds of each tag.
/// Bite: key the env tag on the lock digest (and prune list) alone.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a1_prune_is_part_of_the_env_key() {
    const LOCK: &str = "face025c59f708edef23a8040f867bc583a365a5aa87fcae3047c8e1a4b0f8a5";
    const KEY_A: &str = "c5cf7377617741ee1d18541006fc0e0a83f690249c32d75b56fae2c90296cb97";
    const KEY_B: &str = "8f34d42a94dd37105522fb6e8f4229ae004669a0b04498d7ddb561a5c99e6830";
    const KEY_AB: &str = "416062981ea22b25817cc0f8a8bf8ae3988e32090f7758b70e8815e03df0a1ad";
    let rig = Rig::new();
    let (digest, plain, hex) = lock("numpy==2.0.0\n");
    assert_eq!(hex, LOCK, "the oracle's lock is the lock under test");
    let tag = |key: &str| format!("candler-env:{key}-{BASE_SUFFIX}-{ENV_SUFFIX}");

    for names in [
        &["a.py"][..],
        &["b.py"],
        &["b.py", "a.py"],
        &["a.py", "b.py"],
    ] {
        rig.service
            .ensure(&digest, &pruning(&plain, names))
            .await
            .expect("an image");
    }
    assert_eq!(rig.builds_of(&tag(KEY_A)), 1, "one list, one image");
    assert_eq!(rig.builds_of(&tag(KEY_B)), 1, "another list, another image");
    assert_eq!(
        rig.builds_of(&tag(KEY_AB)),
        1,
        "one list in two orders, one image"
    );
    assert_eq!(
        rig.builds_of(&tag(LOCK)),
        0,
        "no pruned image is keyed on the lock alone"
    );

    // Positive control: nothing pruned keeps the lock's own tag.
    rig.service.ensure(&digest, &plain).await.expect("an image");
    assert_eq!(rig.builds_of(&tag(LOCK)), 1);

    // Another base on the same lock is another tag: its sources are another directory.
    let other_base = tempfile::tempdir().expect("a directory");
    std::fs::write(other_base.path().join("Dockerfile"), "FROM scratch\n").expect("a source");
    let other = Rig::over(other_base.path(), &assets("env"));
    other
        .service
        .ensure(&digest, &plain)
        .await
        .expect("an image");
    let built = other.env_tags_built();
    assert_eq!(built.len(), 1, "one build on the other base");
    assert_ne!(
        built[0],
        tag(LOCK),
        "two bases on one lock took one env tag"
    );
    assert!(
        built[0].starts_with(&format!("candler-env:{LOCK}-src-"))
            && !built[0].contains(BASE_SUFFIX),
        "the env tag carries the other base's tag: {}",
        built[0]
    );
}

/// Guards: A1 — the env image is keyed on the env sources too: one lock on one base builds a
/// second env image once the env Dockerfile is edited, so an edit never reuses a stale image.
/// Oracle: the PATH-fake docker's argv log of the env tags built, over a copy of
/// `assets/docker/env` whose Dockerfile gains one comment line; before the edit the copy keys
/// as the original does.
/// Bite: leave the env sources out of `env_key`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_env_dockerfile_edit_keys_another_env_image() {
    let env = tempfile::tempdir().expect("a directory");
    for name in ["Dockerfile", ".dockerignore"] {
        std::fs::copy(assets("env").join(name), env.path().join(name)).expect("a copy");
    }
    let rig = Rig::over(&assets("base"), env.path());
    let (digest, setting, hex) = lock("numpy==2.0.0\n");
    rig.service
        .ensure(&digest, &setting)
        .await
        .expect("an image");
    let before = format!("candler-env:{hex}-{BASE_SUFFIX}-{ENV_SUFFIX}");
    assert_eq!(
        rig.env_tags_built(),
        [before.as_str()],
        "the copy keys as the original"
    );

    let dockerfile = env.path().join("Dockerfile");
    let mut text = std::fs::read_to_string(&dockerfile).expect("the Dockerfile");
    text.push_str("# edited\n");
    std::fs::write(&dockerfile, text).expect("the edit");
    rig.service
        .ensure(&digest, &setting)
        .await
        .expect("an image");
    let built = rig.env_tags_built();
    assert_eq!(built.len(), 2, "the edit built again: {built:?}");
    assert_ne!(built[1], before, "the edited sources reused the stale tag");
    assert!(
        built[1].starts_with(&format!("candler-env:{hex}-{BASE_SUFFIX}-env-")),
        "only the env part moved: {}",
        built[1]
    );
}

/// Guards: A2 (R01 §Run manifest) — a read of named packages returns, for each name, the
/// distribution and version the container printed: strings for one distribution, lists, in step,
/// for none or several; a name the experiment may not record is refused before any container runs.
/// Oracle: the container's printed answer is written by hand below, and the expected readings are
/// written by hand from it; the PATH-fake docker's log shows whether a container ran.
/// Bite: return the list form for a single distribution, or let a name with a quote reach the
/// script.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a2_package_read_pairs_each_name_with_its_distributions() {
    let rig = Rig::new();
    std::fs::write(
        rig.canned.join("run.out"),
        r#"{"x": [["x-dist", "1.2.3"]], "y": [], "z": [["z-a", "1"], ["z-b", "2"]]}"#,
    )
    .expect("an answer");
    let image = ImageRef {
        id: ImageId(FAKE_ID.to_owned()), // BORROW: the reference owns the id
        harness_version: None,
    };

    let read = rig
        .probe(&["x", "y", "z"])
        .read(&image)
        .await
        .expect("a read");
    let seen = |name: &str| {
        let reading = read.get(name).expect("a reading per name");
        (reading.distribution.clone(), reading.version.clone()) // BORROW: .clone() owns the values
    };
    assert_eq!(seen("x"), (json!("x-dist"), json!("1.2.3")));
    assert_eq!(seen("y"), (json!([]), json!([])));
    assert_eq!(seen("z"), (json!(["z-a", "z-b"]), json!(["1", "2"])));

    let runs = || {
        std::fs::read_to_string(&rig.log)
            .unwrap_or_default()
            .lines()
            .filter(|line| line.starts_with("[\"run\""))
            .count()
    };
    let before = runs();
    for bad in ["x'); import os #", "X", "1x", "x-y", ""] {
        assert!(
            rig.probe(&[bad]).read(&image).await.is_err(),
            "{bad:?} is not an import name"
        );
    }
    assert_eq!(runs(), before, "a refused name starts no container");
}

/// Resolves `values` through the image preflight step over the rig's service, and returns the
/// refusal's text, or `None` when the resolution succeeded.
async fn image_step_refusal(rig: &Rig, values: &[(&str, serde_json::Value)]) -> Option<String> {
    let cwd = tempfile::tempdir().expect("a directory");
    // A requirement source, so that a step that got as far as the lock would run a compile.
    std::fs::write(cwd.path().join("req.txt"), "numpy\n").expect("a requirement source");
    let config = Arc::new(Config::new(
        cwd.path().to_owned(), // BORROW: .to_owned() makes the owned working directory
        None,
        UserConfig::default(),
        Some(cwd.path().join("state")),
    ));
    // TRAIT_OBJECT: the step takes its image service as a trait object
    let images: Arc<dyn ImageService> = rig.service.clone(); // BORROW: .clone() shares the service
    let step = ImageIdStep::new(images, Arc::clone(&rig.sup)).expect("a step");
    let sources = ("isolation.build.requirements", json!(["req.txt"]));
    let file = FileValues::new(
        values
            .iter()
            .chain(std::iter::once(&sources))
            .map(|(path, value)| {
                (
                    path.parse::<FieldPath>().expect("a field path"),
                    JsonValue::new(value.clone()).expect("a JSON value"), // BORROW: .clone() owns the value
                )
            })
            .collect(),
    );
    Resolver::new(config)
        .register(Box::new(step))
        .expect("registers")
        .resolve(ResolveInput::default().file(file))
        .await
        .err()
        .map(|error| error.to_string())
}

/// Guards: R01 §Agent environment and secrets — the image preflight step refuses a malformed
/// `isolation.build.prune` or `isolation.record_packages` before any docker call, and its message
/// names the setting and the bad value: a name with a `/`, `.` or `..`, an empty name, a name with
/// a space, a name that is not an import name, a non-string entry and a value that is not a list.
/// Oracle: the rows are written by hand from the two name rules (`^[A-Za-z0-9._-]+$` for prune,
/// `^[a-z_][a-z0-9_]*$` for `record_packages`); the PATH-fake docker's log shows no docker argv.
/// Bite: accept every name in `prune_of` and `check_record_packages`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn image_step_refuses_malformed_names_before_docker() {
    const PRUNE: &str = "isolation.build.prune";
    const RECORD: &str = "isolation.record_packages";
    let rows = [
        (PRUNE, json!(["ok.py", "a/b.py"]), r#""a/b.py""#),
        (PRUNE, json!([".."]), r#""..""#),
        (PRUNE, json!(["."]), r#""."#),
        (PRUNE, json!([""]), r#""""#),
        (PRUNE, json!(["a b"]), r#""a b""#),
        (PRUNE, json!([3]), "3"),
        (PRUNE, json!("a.py"), r#""a.py""#),
        (RECORD, json!(["x", "Foo"]), r#""Foo""#),
        (RECORD, json!(["1x"]), r#""1x""#),
        (RECORD, json!(["a-b"]), r#""a-b""#),
        (RECORD, json!(["a.b"]), r#""a.b""#),
        (RECORD, json!([""]), r#""""#),
        (RECORD, json!(["x y"]), r#""x y""#),
        (RECORD, json!([7]), "7"),
        (RECORD, json!("x"), r#""x""#),
    ];
    let docker_calls = |rig: &Rig| std::fs::read_to_string(&rig.log).unwrap_or_default();
    for (path, value, shown) in rows {
        let rig = Rig::new();
        let refusal = image_step_refusal(&rig, &[(path, value.clone())]) // BORROW: .clone() owns the value
            .await
            .unwrap_or_else(|| panic!("{path} = {value} was accepted"));
        assert!(
            refusal.contains(path),
            "the refusal names {path}: {refusal}"
        );
        assert!(
            refusal.contains(shown),
            "the refusal names the bad value {shown}: {refusal}"
        );
        assert_eq!(
            docker_calls(&rig),
            "",
            "{path} = {value} reached docker before it was refused"
        );
    }

    // Positive control: well-formed lists pass, and the step then does reach docker.
    let rig = Rig::new();
    let accepted = image_step_refusal(
        &rig,
        &[
            (PRUNE, json!(["a.py", "b-1_2.so"])),
            (RECORD, json!(["x", "_y2"])),
        ],
    )
    .await;
    assert_eq!(accepted, None, "well-formed lists are accepted");
    assert!(
        !docker_calls(&rig).is_empty(),
        "an accepted resolution reaches docker, so an empty log means a refusal"
    );
}
