#![cfg(test)]
//! Guards for `isolation.build.context = "treatment:<path>"`: the experiment's own layer is built
//! from `<path>` of the treatment's checkout as the pinned commit holds it, the ground-truth
//! refusal reads that same tree before any image is built, and the digest of the context follows
//! its bytes.
//!
//! Every experiment is resolved through the steps `candler run` registers, over a git repository
//! the test writes: a tiny setuptools package `pkg` at three commits. No image is built: the
//! image service is a fake that reads the context it is handed.

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::{Arc, Mutex};

use candler_core::isolation::image::{
    BuildSetting, ImageError, ImageService, LockDigest, LockRequest,
};
use candler_core::isolation::model::{ImageId, ImageRef};
use candler_core::resolve::config::{Config, ConfigEnv};
use candler_core::resolve::layers::{FileValues, ResolveInput};
use candler_core::resolve::preflight::{
    PreflightError, PreflightStep, ResolveContext, ResolveError, Resolver, StepReport,
};
use candler_core::resolve::steps::ground_truth::{GroundTruth, GroundTruthStep, TruthFile};
use candler_core::resolve::steps::image::ImageIdStep;
use candler_core::resolve::steps::plugin_environment::PluginEnvironmentStep;
use candler_core::supervise::{EventSink, Supervisor};
use candler_core::tree::BuildContext;
use candler_harness::skills::SkillTree;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{Digest, FieldPath, NameToken, RunKey};
use candler_schemas::refusal::Refusal;
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;
use serde_json::{Value, json};

/// `printf 'CANARY-TRUTH\n' | shasum -a 256`.
const CANARY_SHA: &str = "e75708c483136fd4fcf30d513fd66ec9083bfc8c7000f5de6b97091da5469af2";

fn fp(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

fn js(value: Value) -> JsonValue {
    JsonValue::new(value).expect("a JSON value")
}

struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// What the image service was asked: one entry per `ensure`, the digest of the build inputs it was
/// handed and the text of the package's module in its context.
#[derive(Debug, Default)]
struct Built(Vec<(String, String)>);

struct Fake(Arc<Mutex<Built>>);

impl ImageService for Fake {
    fn lock<'a>(&'a self, _request: &'a LockRequest) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        Box::pin(async { Ok(b"lock\n".to_vec()) })
    }

    fn ensure<'a>(
        &'a self,
        _lock: &'a LockDigest,
        setting: &'a BuildSetting,
    ) -> BoxFuture<'a, Result<ImageRef, ImageError>> {
        let layer = setting.extension.as_ref().expect("an extension");
        let inputs =
            BuildContext::read(&layer.dockerfile, &layer.context).expect("readable inputs");
        let digest = inputs.digest().expect("a digest");
        let module = fs::read_to_string(layer.context.join("demo/__init__.py")).expect("module");
        self.0
            .lock()
            .expect("the record")
            .0
            .push((digest.hex().to_owned(), module)); // BORROW: the record owns its texts
        Box::pin(async {
            Ok(ImageRef {
                id: ImageId(format!("sha256:{}", "7".repeat(64))),
                harness_version: None,
            })
        })
    }

    fn harness_version<'a>(
        &'a self,
        _image: &'a ImageRef,
    ) -> BoxFuture<'a, Result<semver::Version, ImageError>> {
        Box::pin(async { Ok(semver::Version::new(2, 1, 311)) })
    }
}

/// Derives the pinned treatment, as the identity step does.
struct Pinned {
    name: NameToken,
    commit: String,
}

impl PreflightStep for Pinned {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        _ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        let report = StepReport::new(self.name.clone()).derive(
            fp("treatment"),
            js(json!({"name": "htn", "commit": self.commit})),
        );
        Box::pin(async move { Ok(report) })
    }
}

/// A repository holding the skill `htn` and `pkg`, a setuptools package, at the commits `one`, `two` and `three`:
/// `two` changes the module and adds a file named for the canary, `three` changes only a file
/// outside `pkg`. The working tree is then changed to a fourth, uncommitted text.
struct Repo {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    exp: PathBuf,
    one: String,
    two: String,
    three: String,
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(["-c", "commit.gpgsign=false", "-C"])
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?} failed");
    String::from_utf8(out.stdout)
        .expect("git prints text")
        .trim()
        .to_owned() // BORROW: owned text
}

impl Repo {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("a temp dir");
        let base = tmp.path().canonicalize().expect("a real path");
        let root = base.join("skills");
        let exp = base.join("exp");
        fs::create_dir_all(root.join("pkg/demo")).expect("directories");
        fs::create_dir_all(root.join(".claude/skills/htn")).expect("directories");
        fs::write(root.join(".claude/skills/htn/SKILL.md"), "skill\n").expect("the skill");
        fs::create_dir_all(&exp).expect("directories");
        fs::write(exp.join("Dockerfile"), "ARG BASE\nFROM ${BASE}\n").expect("the Dockerfile");
        fs::write(
            root.join("pkg/pyproject.toml"),
            "[build-system]\nrequires = [\"setuptools\"]\nbuild-backend = \"setuptools.build_meta\"\n\n\
             [project]\nname = \"demo\"\nversion = \"0.1\"\n",
        )
        .expect("pyproject");
        fs::write(root.join("pkg/demo/__init__.py"), "VALUE = 1\n").expect("module");
        fs::write(root.join("README"), "one\n").expect("readme");
        git(&root, &["init", "-q"]);
        git(&root, &["add", "."]);
        git(&root, &["commit", "-q", "-m", "one"]);
        let one = git(&root, &["rev-parse", "HEAD"]);
        fs::write(root.join("pkg/demo/__init__.py"), "VALUE = 2\n").expect("module");
        fs::write(root.join("pkg/demo/answer.txt"), "CANARY-TRUTH\n").expect("canary");
        git(&root, &["add", "."]);
        git(&root, &["commit", "-q", "-m", "two"]);
        let two = git(&root, &["rev-parse", "HEAD"]);
        fs::write(root.join("README"), "three\n").expect("readme");
        git(&root, &["commit", "-q", "-a", "-m", "three"]);
        let three = git(&root, &["rev-parse", "HEAD"]);
        fs::write(root.join("pkg/demo/__init__.py"), "VALUE = 99\n").expect("working tree");
        Self {
            _tmp: tmp,
            root,
            exp,
            one,
            two,
            three,
        }
    }
}

fn truth(sha: &str) -> GroundTruth {
    GroundTruth::new(vec![TruthFile {
        kind: "truth".to_owned(),
        path: "gt.txt".to_owned(),
        digest: Digest::from_str(&format!("sha256:{sha}")).expect("a digest"),
    }])
}

/// Resolves an experiment whose context is `context` and whose treatment is pinned to `commit`,
/// through the steps in the order `register_m2b_steps` gives them: the ground truth, a gate,
/// before the image.
async fn resolve(
    repo: &Repo,
    commit: &str,
    context: &str,
    truth: GroundTruth,
) -> (Result<(), ResolveError>, Arc<Mutex<Built>>) {
    let built = Arc::new(Mutex::new(Built::default()));
    let config = Arc::new(Config::load(&repo.exp, &ConfigEnv::default()).expect("a config"));
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let supervisor = Arc::new(Supervisor::new(sink));
    let image = ImageIdStep::new(
        // TRAIT_OBJECT: the image service is a fake
        Arc::new(Fake(Arc::clone(&built))),
        Arc::clone(&supervisor),
    )
    .expect("a step");
    let gate = GroundTruthStep::new(
        truth,
        supervisor,
        SkillTree::new(".claude/skills", ".claude/skills"),
    )
    .expect("a step");
    let values = vec![
        (fp("isolation.build.dockerfile"), js(json!("Dockerfile"))),
        (fp("isolation.build.context"), js(json!(context))),
        (fp("request.harness"), js(json!({"name": "claude-code"}))),
        (
            fp("request.treatment"),
            js(json!({"name": "htn", "source": repo.root})),
        ),
    ];
    let layer = FileValues::new(values)
        .relative_to(&repo.exp)
        .expect("absolute paths");
    let outcome = Resolver::new(config)
        .register(Box::new(Pinned {
            name: "pinned@1".parse().expect("a name@token"),
            commit: commit.to_owned(), // BORROW: the step owns its commit
        }))
        .expect("registers")
        .register(Box::new(PluginEnvironmentStep::new().expect("a step")))
        .expect("registers")
        .register(Box::new(gate))
        .expect("registers")
        .register(Box::new(image))
        .expect("registers")
        .resolve(ResolveInput::new(Vec::new()).file(layer))
        .await
        .map(|_| ());
    (outcome, built)
}

/// Guards: G2 — a `treatment:` context is `<path>` of the pinned commit, not of the working tree,
/// and its digest follows the bytes: two commits with equal `pkg` subtrees give one digest, two
/// commits that differ give two.
/// Oracle: the module texts written by hand at each commit, and the working tree's `VALUE = 99`,
/// which must appear nowhere; the digests are compared with one another.
/// Bite: read the context from the working tree (`cwd.join(path)`): the module reads `VALUE = 99`.
#[tokio::test]
async fn context_is_the_pinned_commits_tree() {
    let repo = Repo::new();
    let mut seen = Vec::new();
    for commit in [&repo.one, &repo.two, &repo.three] {
        let (outcome, built) =
            resolve(&repo, commit, "treatment:pkg", GroundTruth::default()).await;
        outcome.expect("resolves");
        let built = built.lock().expect("the record");
        assert_eq!(built.0.len(), 1, "one image build");
        seen.push(built.0[0].clone()); // BORROW: owned entry
    }
    assert_eq!(seen[0].1, "VALUE = 1\n", "commit one's bytes");
    assert_eq!(seen[1].1, "VALUE = 2\n", "commit two's bytes");
    assert_ne!(seen[0].0, seen[1].0, "different trees, different digests");
    assert_eq!(
        seen[1], seen[2],
        "equal subtrees at two commits, one digest"
    );
}

/// Guards: G2, R01 §Contracts I8 — an evaluator-only file whose bytes sit in the treatment's
/// context is refused at resolve with `isolation.ground_truth_exposed`, naming the file, and no
/// image is built; the commit before the file was added resolves.
/// Oracle: the canary's sha-256, worked by hand with `shasum -a 256`; the build count of the fake.
/// Bite: skip the `treatment:` context in `context_exposures`, or register the ground truth after
/// the image step with no gate: the refusal is missing, or the fake built an image.
#[tokio::test]
async fn ground_truth_in_the_treatment_context_is_refused_before_any_build() {
    let repo = Repo::new();
    let (outcome, built) = resolve(&repo, &repo.two, "treatment:pkg", truth(CANARY_SHA)).await;
    let error = outcome.expect_err("the canary is in the context");
    assert_eq!(
        error.code().as_str(),
        "isolation.ground_truth_exposed",
        "{error}"
    );
    let text = error.to_string();
    assert!(
        text.contains("`demo/answer.txt` in the experiment's build context"),
        "{text}"
    );
    assert!(
        built.lock().expect("the record").0.is_empty(),
        "no image was built"
    );
    let (outcome, built) = resolve(&repo, &repo.one, "treatment:pkg", truth(CANARY_SHA)).await;
    outcome.expect("commit one holds no copy");
    assert_eq!(built.lock().expect("the record").0.len(), 1);
}

/// Guards: G2 — a `treatment:` context that names no directory of the commit is refused, and a
/// path that climbs out of the checkout is refused.
/// Oracle: hand-written paths: `nope` is in no commit, `../x` leaves the checkout.
/// Bite: pass the path to git unchecked: `../x` reaches git instead of the refusal.
#[tokio::test]
async fn a_context_outside_the_commit_is_refused() {
    let repo = Repo::new();
    for path in ["treatment:nope", "treatment:../x"] {
        let (outcome, built) = resolve(&repo, &repo.one, path, GroundTruth::default()).await;
        outcome.expect_err(path);
        assert!(built.lock().expect("the record").0.is_empty(), "{path}");
    }
}
