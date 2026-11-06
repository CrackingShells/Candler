#![cfg(test)]
//! Development tests of the image step: the lock, the treatment source and the extension.

use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

use candler_core::isolation::image::{
    BuildSetting, Extension, ImageError, ImageService, LockDigest, LockRequest,
};
use candler_core::isolation::model::{ImageId, ImageRef};
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::{ResolveError, Resolver};
use candler_core::resolve::steps::identity::TreatmentIdentityStep;
use candler_core::resolve::steps::image::ImageIdStep;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{FieldPath, RunKey};
use candler_schemas::values::{JsonValue, ResolvedValue};
use futures::future::BoxFuture;
use serde_json::{Value, json};

#[derive(Default)]
struct Seen {
    requests: Vec<LockRequest>,
    settings: Vec<BuildSetting>,
}

/// Compiles a lock that echoes its sources, and records what it was given.
#[derive(Default)]
struct FakeImages {
    seen: Mutex<Seen>,
}

impl ImageService for FakeImages {
    fn lock<'a>(&'a self, request: &'a LockRequest) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        let mut bytes = Vec::new();
        for source in &request.sources {
            bytes.extend_from_slice(&source.bytes);
        }
        self.seen
            .lock()
            .expect("lock")
            .requests
            .push(request.clone());
        Box::pin(async move { Ok(bytes) })
    }

    fn ensure<'a>(
        &'a self,
        lock: &'a LockDigest,
        setting: &'a BuildSetting,
    ) -> BoxFuture<'a, Result<ImageRef, ImageError>> {
        self.seen
            .lock()
            .expect("lock")
            .settings
            .push(setting.clone());
        let id = format!("sha256:{}", lock.0);
        Box::pin(async move {
            Ok(ImageRef {
                id: ImageId(id),
                cc_version: None,
            })
        })
    }

    fn cc_version<'a>(
        &'a self,
        _image: &'a ImageRef,
    ) -> BoxFuture<'a, Result<semver::Version, ImageError>> {
        Box::pin(async { Ok(semver::Version::new(2, 1, 0)) })
    }
}

struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

fn fp(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

async fn resolve(
    images: &Arc<FakeImages>,
    cwd: &Path,
    mut file: Vec<(&str, Value)>,
) -> Result<Resolution, ResolveError> {
    if !file.iter().any(|(path, _)| *path == "request.treatment") {
        file.push(("request.treatment", json!({"name": "t"})));
    }
    let sink: Arc<dyn EventSink> = Arc::new(NullSink);
    let supervisor = Arc::new(Supervisor::new(sink));
    let service: Arc<dyn ImageService> = images.clone();
    let config = Arc::new(Config::new(
        cwd.to_owned(),
        None,
        UserConfig::default(),
        Some(cwd.join("state")),
    ));
    let resolver = Resolver::new(config)
        .register(Box::new(
            TreatmentIdentityStep::new(Arc::clone(&supervisor)).expect("a step"),
        ))?
        .register(Box::new(
            ImageIdStep::new(service, supervisor).expect("a step"),
        ))?;
    let values = file
        .into_iter()
        .map(|(path, value)| (fp(path), JsonValue::new(value).expect("a JSON value")))
        .collect();
    resolver
        .resolve(ResolveInput::default().file(FileValues::new(values)))
        .await
}

fn derived<'a>(resolution: &'a Resolution, path: &str) -> Option<&'a Value> {
    resolution
        .values
        .iter()
        .find(|v| v.path().as_str() == path)
        .and_then(ResolvedValue::value)
        .map(JsonValue::as_value)
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

fn lock_of(resolution: &Resolution) -> Value {
    derived(resolution, "isolation.env_lock")
        .cloned()
        .expect("an env_lock")
}

#[tokio::test]
async fn two_locks_give_two_env_locks_and_the_lock_file_is_verbatim() {
    let cwd = tempfile::tempdir().expect("a temp dir");
    std::fs::write(cwd.path().join("a.lock"), b"numpy==1\n").expect("a file");
    std::fs::write(cwd.path().join("b.lock"), b"numpy==2\n").expect("a file");
    let images = Arc::new(FakeImages::default());
    let run = |name: &'static str| {
        let images = Arc::clone(&images);
        let cwd = cwd.path().to_owned();
        async move {
            resolve(&images, &cwd, vec![("isolation.build.lock", json!(name))])
                .await
                .expect("resolves")
        }
    };
    let first = run("a.lock").await;
    let second = run("b.lock").await;
    assert_ne!(lock_of(&first), lock_of(&second));
    let again = run("a.lock").await;
    assert_eq!(lock_of(&first), lock_of(&again));
    let seen = images.seen.lock().expect("lock");
    assert_eq!(seen.requests.len(), 0);
    assert_eq!(seen.settings[0].lock, b"numpy==1\n");
    assert!(seen.settings[0].extension.is_none());
    let expected = format!("sha256:{}", {
        let out = Command::new("shasum")
            .args(["-a", "256"])
            .arg(cwd.path().join("a.lock"))
            .output()
            .expect("shasum");
        String::from_utf8(out.stdout).expect("UTF-8")[..64].to_owned()
    });
    assert_eq!(lock_of(&first), json!(expected));
}

#[tokio::test]
async fn a_treatment_source_is_read_at_the_pinned_commit() {
    let cwd = tempfile::tempdir().expect("a temp dir");
    let repo = cwd.path().join("repo");
    std::fs::create_dir_all(repo.join("skill")).expect("a dir");
    git(&repo, &["init", "-q"]);
    std::fs::write(repo.join("skill/req.in"), b"old\n").expect("a file");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "one"]);
    git(&repo, &["tag", "v1"]);
    std::fs::write(repo.join("skill/req.in"), b"new\n").expect("a file");
    git(&repo, &["commit", "-q", "-am", "two"]);
    std::fs::write(cwd.path().join("local.in"), b"local\n").expect("a file");

    let images = Arc::new(FakeImages::default());
    let resolved = resolve(
        &images,
        cwd.path(),
        vec![
            ("request.treatment", json!({"name": "t", "tag": "v1"})),
            ("request.treatment_source", json!("repo/skill")),
            (
                "isolation.build.requirements",
                json!(["treatment:req.in", "local.in"]),
            ),
        ],
    )
    .await
    .expect("resolves");
    {
        let seen = images.seen.lock().expect("lock");
        let sources = &seen.requests[0].sources;
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].name, "req.in");
        assert_eq!(sources[0].bytes, b"old\n");
        assert_eq!(sources[1].bytes, b"local\n");
    }
    assert!(derived(&resolved, "isolation.image").is_some());
    for bad in ["treatment:../x", "treatment:/etc/passwd", "treatment:-p"] {
        let refused = resolve(
            &images,
            cwd.path(),
            vec![
                ("request.treatment", json!({"name": "t", "tag": "v1"})),
                ("request.treatment_source", json!("repo/skill")),
                ("isolation.build.requirements", json!([bad])),
            ],
        )
        .await;
        assert!(
            matches!(refused, Err(ResolveError::Preflight { .. })),
            "{bad}"
        );
    }
}

#[tokio::test]
async fn an_extension_changes_inputs_and_its_absence_leaves_none() {
    let cwd = tempfile::tempdir().expect("a temp dir");
    std::fs::create_dir(cwd.path().join("ctx")).expect("a dir");
    std::fs::write(cwd.path().join("Dockerfile"), b"FROM x\n").expect("a file");
    std::fs::write(cwd.path().join("ctx/f"), b"1").expect("a file");
    let images = Arc::new(FakeImages::default());
    let with = |extra: &'static str| {
        let images = Arc::clone(&images);
        let cwd = cwd.path().to_owned();
        async move {
            std::fs::write(cwd.join("ctx/f"), extra).expect("a file");
            resolve(
                &images,
                &cwd,
                vec![
                    ("isolation.build.dockerfile", json!("Dockerfile")),
                    ("isolation.build.context", json!("ctx")),
                ],
            )
            .await
            .expect("resolves")
        }
    };
    let one = with("1").await;
    let two = with("2").await;
    assert_ne!(derived(&one, "inputs"), derived(&two, "inputs"));
    assert_eq!(
        derived(&one, "inputs").expect("inputs")[0]["kind"],
        json!("build_inputs")
    );
    let none = resolve(&images, cwd.path(), vec![])
        .await
        .expect("resolves");
    assert!(derived(&none, "inputs").is_none());
    let seen = images.seen.lock().expect("lock");
    assert!(seen.settings[0].extension.is_some());
    assert_eq!(
        seen.settings[0].extension,
        Some(Extension {
            dockerfile: cwd.path().join("Dockerfile"),
            context: cwd.path().join("ctx"),
        })
    );
    assert!(seen.settings[2].extension.is_none());
    assert_eq!(seen.requests[2].sources.len(), 0);
}
