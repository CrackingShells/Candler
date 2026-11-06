#![cfg(test)]
//! Development tests of the experiment resolution leaf.

use std::time::Duration;

use candler_core::experiment::budget::to_caps;
use candler_core::experiment::surface::{Surface, SurfaceError};
use candler_harness::spec::Cell;
use candler_schemas::execution::Budget;
use candler_schemas::primitives::FieldPath;
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::{Value, json};

fn rv(path: &str, value: Value) -> ResolvedValue {
    ResolvedValue::recorded(
        path.parse::<FieldPath>().expect("a field path"),
        JsonValue::new(value).expect("a JSON value"),
    )
}

fn base() -> Vec<ResolvedValue> {
    vec![
        rv("model.id", json!("m1")),
        rv("model.endpoint", json!("https://up.example/v1")),
        rv("permission_mode", json!("default")),
        rv("prompt.template", json!("do {task}")),
    ]
}

fn budget(per_run: &Value) -> Budget {
    serde_json::from_value(json!({ "per_run": per_run, "max_cost_usd": 99.0 })).expect("a budget")
}

#[test]
fn a_minimal_cell_reads() {
    let values = base();
    let surface = Surface::from_cell(&Cell::new(&values)).expect("a surface");
    assert_eq!(surface.model_id, "m1");
    assert_eq!(surface.tools_allowed.len(), 0);
    assert!(surface.mcp.is_none());
}

#[test]
fn every_required_path_is_required() {
    for drop in [
        "model.id",
        "model.endpoint",
        "permission_mode",
        "prompt.template",
    ] {
        let values: Vec<_> = base()
            .into_iter()
            .filter(|v| v.path().as_str() != drop)
            .collect();
        let error = Surface::from_cell(&Cell::new(&values)).expect_err(drop);
        assert!(
            matches!(error, SurfaceError::Missing(ref p) if p.as_str() == drop),
            "{drop}"
        );
    }
}

#[test]
fn wrong_types_and_absolute_paths_are_refused() {
    let mut values = base();
    values.push(rv("tools.allowed", json!("Bash")));
    assert!(matches!(
        Surface::from_cell(&Cell::new(&values)),
        Err(SurfaceError::Type { .. })
    ));
    let mut values = base();
    values.push(rv("mcp", json!({"fs": {"command": "/usr/bin/x"}})));
    assert!(matches!(
        Surface::from_cell(&Cell::new(&values)),
        Err(SurfaceError::AbsolutePath { .. })
    ));
}

#[test]
fn a_value_below_a_reserved_root_is_refused() {
    let mut values = base();
    values.push(rv("harness.extra", json!("x")));
    assert!(matches!(
        Surface::from_values(&values),
        Err(SurfaceError::ReservedRoot { .. })
    ));
}

#[test]
fn caps_become_flags_and_expiry() {
    let caps = to_caps(&budget(
        &json!({"wall_s": 3600, "turns": 7, "max_cost_usd": 0.5}),
    ));
    assert_eq!(
        caps.flags(),
        ["--max-turns", "7", "--max-budget-usd", "0.5"]
    );
    assert_eq!(caps.expiry(), Some(Duration::from_mins(61)));
    let bare = to_caps(&budget(&json!({"wall_s": 10})));
    assert_eq!(bare.flags().len(), 0);
}

// ---- steps ----

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use candler_core::isolation::image::{
    BuildSetting, ImageError, ImageService, LockDigest, LockRequest,
};
use candler_core::isolation::model::ImageRef;
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::{PreflightStep, ResolveError, Resolver};
use candler_core::resolve::steps::identity::{HarnessIdentityStep, TreatmentIdentityStep};
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::RunKey;
use futures::future::BoxFuture;

struct FakeImages {
    version: semver::Version,
}

impl ImageService for FakeImages {
    fn lock<'a>(&'a self, _request: &'a LockRequest) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn ensure<'a>(
        &'a self,
        _lock: &'a LockDigest,
        _setting: &'a BuildSetting,
    ) -> BoxFuture<'a, Result<ImageRef, ImageError>> {
        Box::pin(async {
            Err(ImageError::Unavailable {
                detail: "fake".to_owned(),
            })
        })
    }

    fn cc_version<'a>(
        &'a self,
        _image: &'a ImageRef,
    ) -> BoxFuture<'a, Result<semver::Version, ImageError>> {
        let version = self.version.clone();
        Box::pin(async move { Ok(version) })
    }
}

struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

fn config(cwd: &Path) -> Arc<Config> {
    Arc::new(Config::new(
        cwd.to_owned(),
        None,
        UserConfig::default(),
        Some(cwd.join("state")),
    ))
}

fn fp(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

async fn resolve(
    steps: Vec<Box<dyn PreflightStep>>,
    cwd: &Path,
    file: Vec<(&str, Value)>,
) -> Result<Resolution, ResolveError> {
    let mut resolver = Resolver::new(config(cwd));
    for step in steps {
        resolver = resolver.register(step)?;
    }
    let values = file
        .into_iter()
        .map(|(path, value)| (fp(path), JsonValue::new(value).expect("a JSON value")))
        .collect();
    let input = ResolveInput::default().file(FileValues::new(values));
    resolver.resolve(input).await
}

fn derived<'a>(resolution: &'a Resolution, path: &str) -> Option<&'a Value> {
    resolution
        .values
        .iter()
        .find(|v| v.path().as_str() == path)
        .and_then(ResolvedValue::value)
        .map(JsonValue::as_value)
}

const IMAGE: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000001";

fn harness_step(version: &str, supported: &str) -> Box<dyn PreflightStep> {
    Box::new(
        HarnessIdentityStep::new(
            Arc::new(FakeImages {
                version: version.parse().expect("a version"),
            }),
            supported.parse().expect("a requirement"),
        )
        .expect("a step"),
    )
}

#[tokio::test]
async fn the_harness_identity_is_derived_and_a_version_outside_either_requirement_is_refused() {
    let cwd = tempfile::tempdir().expect("a temp dir");
    let file = |request: Value| {
        vec![
            ("isolation.image", json!(IMAGE)),
            ("request.harness", request),
        ]
    };
    let ok = resolve(
        vec![harness_step("2.1.5", ">=2, <3")],
        cwd.path(),
        file(json!({"name": "claude-code", "version": "^2.1"})),
    )
    .await
    .expect("resolves");
    assert_eq!(
        derived(&ok, "harness"),
        Some(&json!({"name": "claude-code", "version": "2.1.5"}))
    );
    let outside_supported = resolve(
        vec![harness_step("3.0.0", ">=2, <3")],
        cwd.path(),
        file(json!({"name": "claude-code"})),
    )
    .await;
    assert!(matches!(
        outside_supported,
        Err(ResolveError::Preflight { .. })
    ));
    let outside_request = resolve(
        vec![harness_step("2.1.5", ">=2, <3")],
        cwd.path(),
        file(json!({"name": "claude-code", "version": "^2.2"})),
    )
    .await;
    assert!(matches!(
        outside_request,
        Err(ResolveError::Preflight { .. })
    ));
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8(out.stdout)
        .expect("UTF-8")
        .trim()
        .to_owned()
}

fn treatment_step() -> Box<dyn PreflightStep> {
    let sink: Arc<dyn EventSink> = Arc::new(NullSink);
    Box::new(TreatmentIdentityStep::new(Arc::new(Supervisor::new(sink))).expect("a step"))
}

#[tokio::test]
async fn the_treatment_tag_resolves_to_its_commit() {
    let cwd = tempfile::tempdir().expect("a temp dir");
    let repo = cwd.path().join("repo");
    std::fs::create_dir(&repo).expect("a dir");
    git(&repo, &["init", "-q"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "one"]);
    git(&repo, &["tag", "v1"]);
    let commit = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "two"]);
    let other = git(&repo, &["rev-parse", "HEAD"]);

    let run = |treatment: Value| {
        resolve(
            vec![treatment_step()],
            cwd.path(),
            vec![
                ("request.treatment", treatment),
                ("request.treatment_source", json!("repo")),
            ],
        )
    };
    let ok = run(json!({"name": "cand", "tag": "v1"}))
        .await
        .expect("resolves");
    assert_eq!(
        derived(&ok, "treatment"),
        Some(&json!({"name": "cand", "tag": "v1", "commit": commit}))
    );
    let agreeing = run(json!({"name": "cand", "tag": "v1", "commit": commit})).await;
    assert!(agreeing.is_ok());
    let disagreeing = run(json!({"name": "cand", "tag": "v1", "commit": other})).await;
    assert!(matches!(disagreeing, Err(ResolveError::Preflight { .. })));
    let missing = run(json!({"name": "cand", "tag": "nope"})).await;
    assert!(matches!(missing, Err(ResolveError::Preflight { .. })));
    let option_like = run(json!({"name": "cand", "tag": "--all"})).await;
    assert!(matches!(option_like, Err(ResolveError::Preflight { .. })));
}

#[tokio::test]
async fn a_treatment_with_no_tag_and_no_commit_runs_no_git() {
    let cwd = tempfile::tempdir().expect("a temp dir");
    let ok = resolve(
        vec![treatment_step()],
        cwd.path(),
        vec![("request.treatment", json!({"name": "cand"}))],
    )
    .await
    .expect("resolves");
    assert_eq!(derived(&ok, "treatment"), Some(&json!({"name": "cand"})));
}
