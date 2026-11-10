#![cfg(test)]
//! Guards for the inputs an experiment declares: what is refused, how the build inputs and the
//! declared ones merge into the `inputs` root, and how the inputs reach the container and the
//! manifest.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use candler_core::experiment::inputs::resolve_inputs;
use candler_core::isolation::image::{
    BuildSetting, ImageError, ImageService, LockDigest, LockRequest,
};
use candler_core::isolation::model::{ImageId, ImageRef};
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, ResolveInput};
use candler_core::resolve::preflight::Resolver;
use candler_core::resolve::steps::image::ImageIdStep;
use candler_core::store::Store;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::{EventPayload, ExperimentInput, InputAudience};
use candler_schemas::primitives::{FieldPath, Name, RunKey};
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;
use serde_json::json;

fn name(text: &str) -> Name {
    Name::from_str(text).expect("a name")
}

fn input(kind: &str, path: &str, at: Option<&str>, audience: InputAudience) -> ExperimentInput {
    ExperimentInput::new(name(kind), path.to_owned(), at.map(str::to_owned), audience)
        .expect("a declaration the wire accepts")
}

/// A store in a temporary directory.
fn store(root: &Path) -> Store {
    Store::init(&root.join("store")).expect("a store")
}

/// An experiment directory holding `a.txt`, a directory `tree` with two files, and an empty
/// directory `empty`.
fn experiment_dir(root: &Path) -> std::path::PathBuf {
    let dir = root.join("exp");
    fs::create_dir_all(dir.join("tree/sub")).expect("directories");
    fs::create_dir_all(dir.join("empty")).expect("an empty directory");
    fs::write(dir.join("a.txt"), "alpha").expect("a file");
    fs::write(dir.join("tree/one.txt"), "one").expect("a file");
    fs::write(dir.join("tree/sub/two.txt"), "two").expect("a file");
    dir
}

/// Guards: I14 — an input the experiment declares wrongly is refused before any run, and the
/// refusal names the input's kind and the path at fault (authors).
/// Oracle: hand-written; each case is one way to declare an input that cannot be mounted or
/// hashed, with the words the message must hold.
#[test]
fn declared_input_refused_names_kind_and_path() {
    let root = tempfile::tempdir().expect("a directory");
    let dir = experiment_dir(root.path());
    symlink(dir.join("a.txt"), dir.join("tree/link.txt")).expect("a symlink");
    symlink(dir.join("a.txt"), dir.join("direct.txt")).expect("a symlink");
    fs::create_dir_all(dir.join("only_dirs/inner")).expect("nested empty directories");
    fs::create_dir_all(dir.join("dirs/d,ir")).expect("a directory");
    fs::write(dir.join("dirs/d,ir/inner.txt"), "x").expect("a file");
    fs::create_dir_all(dir.join("commas")).expect("a directory");
    fs::write(dir.join("commas/a,b.txt"), "x").expect("a file name with a comma");
    fs::create_dir_all(dir.join("lines")).expect("a directory");
    fs::write(dir.join("lines/nl\nfile.txt"), "x").expect("a file name with a newline");
    fs::write(dir.join("d,ir.txt"), "x").expect("a file input named with a comma");
    let store = store(root.path());
    let agent = InputAudience::Agent;
    let cases: Vec<(&str, Vec<ExperimentInput>, &str)> = vec![
        (
            "duplicate kind",
            vec![
                input("servers", "a.txt", None, agent),
                input("servers", "tree", None, agent),
            ],
            "servers",
        ),
        (
            "missing path",
            vec![input("gone", "nowhere.txt", None, agent)],
            "nowhere.txt",
        ),
        ("empty path", vec![input("blank", "", None, agent)], "blank"),
        (
            "absolute path",
            vec![input("abs", "/etc/hosts", None, agent)],
            "/etc/hosts",
        ),
        (
            "parent segment",
            vec![input("up", "../exp/a.txt", None, agent)],
            "../exp/a.txt",
        ),
        (
            "parent segment inside",
            vec![input(
                "up",
                "tree/../a.txt",
                None,
                InputAudience::Evaluators,
            )],
            "tree/../a.txt",
        ),
        (
            "symlink in a directory",
            vec![input("tree", "tree", None, agent)],
            "link.txt",
        ),
        (
            "symlink as the input",
            vec![input("direct", "direct.txt", None, agent)],
            "direct.txt",
        ),
        (
            "empty directory",
            vec![input("hollow", "empty", None, agent)],
            "empty",
        ),
        (
            "directory of empty directories",
            vec![input(
                "hollow",
                "only_dirs",
                None,
                InputAudience::Evaluators,
            )],
            "only_dirs",
        ),
        (
            "two files at one workspace path",
            vec![
                input("first", "a.txt", Some("data"), agent),
                input("second", "a.txt", Some("data"), agent),
            ],
            "data/a.txt",
        ),
        (
            "comma in a file name of a directory",
            vec![input("commas", "commas", None, agent)],
            "a,b.txt",
        ),
        (
            "comma in a directory name",
            vec![input("dirs", "dirs", None, agent)],
            "d,ir",
        ),
        (
            "newline in a file name",
            vec![input("lines", "lines", None, agent)],
            "nl",
        ),
        (
            "comma in the file name of a file input",
            vec![input("comma_file", "d,ir.txt", None, agent)],
            "d,ir.txt",
        ),
        (
            "comma in at",
            vec![input("where", "a.txt", Some("x,y"), agent)],
            "x,y",
        ),
        (
            "newline in at",
            vec![input(
                "where",
                "a.txt",
                Some("x\ny"),
                InputAudience::Evaluators,
            )],
            "x",
        ),
        (
            "a file inside another file's path",
            vec![
                input("file", "a.txt", Some("x"), agent),
                input("under", "tree/sub", Some("x/a.txt"), agent),
            ],
            "x/a.txt",
        ),
    ];
    for (label, declared, expect) in cases {
        let error = resolve_inputs(&store, &declared, &dir)
            .expect_err(label)
            .to_string();
        assert!(
            error.contains(expect),
            "{label}: `{error}` lacks `{expect}`"
        );
        let kind = declared.last().expect("an input").kind().to_string();
        assert!(
            error.contains(&kind),
            "{label}: `{error}` lacks the kind `{kind}`"
        );
    }
}

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// An image service that answers with one image and never builds.
struct OneImage;

impl ImageService for OneImage {
    fn lock<'a>(&'a self, _request: &'a LockRequest) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        Box::pin(async { Ok(b"lock".to_vec()) })
    }

    fn ensure<'a>(
        &'a self,
        _lock: &'a LockDigest,
        _setting: &'a BuildSetting,
    ) -> BoxFuture<'a, Result<ImageRef, ImageError>> {
        Box::pin(async {
            Ok(ImageRef {
                id: ImageId(format!("sha256:{}", "7".repeat(64))),
                cc_version: None,
            })
        })
    }

    fn cc_version<'a>(
        &'a self,
        _image: &'a ImageRef,
    ) -> BoxFuture<'a, Result<semver::Version, ImageError>> {
        Box::pin(async { Ok(semver::Version::new(2, 1, 311)) })
    }
}

fn fp(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

fn js(value: serde_json::Value) -> JsonValue {
    JsonValue::new(value).expect("a JSON value")
}

/// Resolves with the image step alone and returns the value of the `inputs` root.
async fn resolved_inputs(
    cwd: &Path,
    settings: Vec<(&str, serde_json::Value)>,
) -> serde_json::Value {
    let config = Arc::new(Config::new(
        cwd.to_owned(),
        None,
        UserConfig::default(),
        Some(cwd.join("state")),
    ));
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let step =
        ImageIdStep::new(Arc::new(OneImage), Arc::new(Supervisor::new(sink))).expect("a step");
    let values = settings
        .into_iter()
        .map(|(path, value)| (fp(path), js(value)))
        .collect();
    let resolution = Resolver::new(config)
        .register(Box::new(step))
        .expect("registers")
        .resolve(ResolveInput::default().file(FileValues::new(values)))
        .await
        .expect("resolves with no step deriving a path twice");
    resolution
        .values
        .iter()
        .find(|value| value.path().as_str() == "inputs")
        .and_then(|value| value.value())
        .expect("an inputs root")
        .as_value()
        .clone()
}

/// Guards: I14 — an experiment with a build extension and declared agent inputs resolves to one
/// `inputs` root that holds both, sorted by kind, and the one step that derives it derives it
/// once (the image step; agent environment; comparability).
/// Oracle: hand-written. The agent entries are given digests by hand; `build_inputs` is the
/// digest of the Dockerfile and context the step reads, so the test only checks its kind and
/// position, and that it is a `sha256:` digest.
#[tokio::test]
async fn declared_input_with_build_inputs_merge_into_one_sorted_root() {
    let root = tempfile::tempdir().expect("a directory");
    let cwd = root.path();
    fs::write(cwd.join("Dockerfile"), "FROM scratch\n").expect("a Dockerfile");
    fs::create_dir_all(cwd.join("ctx")).expect("a context");
    fs::write(cwd.join("ctx/f"), "x").expect("a file");
    fs::write(cwd.join("lock.txt"), "lock").expect("a lock");
    let agent = json!([
        {"kind": "zeta", "digest": format!("sha256:{}", "b".repeat(64))},
        {"kind": "alpha", "digest": format!("sha256:{}", "a".repeat(64))},
    ]);
    let settings = vec![
        ("isolation.build.lock", json!("lock.txt")),
        ("isolation.build.dockerfile", json!("Dockerfile")),
        ("isolation.build.context", json!("ctx")),
        ("request.inputs", agent.clone()),
    ];
    let merged = resolved_inputs(cwd, settings).await;
    let entries = merged.as_array().expect("a list");
    let kinds: Vec<&str> = entries
        .iter()
        .map(|entry| entry["kind"].as_str().expect("a kind"))
        .collect();
    assert_eq!(
        kinds,
        ["alpha", "build_inputs", "zeta"],
        "one sorted root: {merged}"
    );
    assert_eq!(
        entries[0], agent[1],
        "the declared digests pass through unchanged"
    );
    assert_eq!(
        entries[2], agent[0],
        "the declared digests pass through unchanged"
    );
    assert!(
        entries[1]["digest"]
            .as_str()
            .is_some_and(|d| d.starts_with("sha256:"))
    );
}

/// Guards: I14 — declared agent inputs reach the `inputs` root with no build extension too, and
/// an experiment that declares none has no `inputs` root.
/// Oracle: hand-written.
#[tokio::test]
async fn declared_input_without_build_inputs_stands_alone() {
    let root = tempfile::tempdir().expect("a directory");
    let cwd = root.path();
    fs::write(cwd.join("lock.txt"), "lock").expect("a lock");
    let agent = json!([{"kind": "alpha", "digest": format!("sha256:{}", "a".repeat(64))}]);
    let with = resolved_inputs(
        cwd,
        vec![
            ("isolation.build.lock", json!("lock.txt")),
            ("request.inputs", agent.clone()),
        ],
    )
    .await;
    assert_eq!(with, agent);
}
