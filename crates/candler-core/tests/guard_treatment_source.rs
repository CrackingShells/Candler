#![cfg(test)]
//! Guards for the treatment's source: the experiment file's `[treatment] source` is where the
//! checkout is, it is recorded where the treatment came from, and it never reaches a cell digest.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::{
    PreflightError, PreflightStep, ResolveContext, Resolver, StepReport,
};
use candler_core::resolve::steps::identity::TreatmentIdentityStep;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::{Comparable, EventPayload};
use candler_schemas::primitives::{FieldPath, NameToken, RunKey};
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;
use serde_json::{Value, json};

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

fn fp(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

fn js(value: Value) -> JsonValue {
    JsonValue::new(value).expect("a JSON value")
}

/// Derives the four typed roots the treatment step does not.
struct OtherRoots {
    name: NameToken,
}

impl PreflightStep for OtherRoots {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        _ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        let digest = format!("sha256:{}", "a".repeat(64));
        let report = StepReport::new(self.name.clone())
            .derive(
                fp("harness"),
                js(json!({"name": "claude-code", "version": "2.0.0"})),
            )
            .derive(
                fp("inputs"),
                js(json!([{"kind": "build_inputs", "digest": digest}])),
            )
            .derive(
                fp("snapshots"),
                js(json!([{"kind": "catalog", "digest": digest}])),
            )
            .derive(
                fp("environments"),
                js(json!([{"plugin": "p@1.0.0", "lock_digest": digest}])),
            );
        Box::pin(async move { Ok(report) })
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8(out.stdout)
        .expect("UTF-8")
        .trim()
        .to_owned()
}

/// Resolves a file that names the treatment `request` and sets nothing else, from the working
/// directory `cwd`, through the real treatment step.
async fn resolve(request: Value, cwd: &Path) -> Result<Resolution, String> {
    // TRAIT_OBJECT: the supervisor reports to no job
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let supervisor = Arc::new(Supervisor::new(sink));
    let config = Arc::new(Config::new(
        cwd.to_owned(),
        None,
        UserConfig::default(),
        Some(cwd.join("state")),
    ));
    let treatment = TreatmentIdentityStep::new(supervisor).expect("a step");
    let others = OtherRoots {
        name: "roots@1".parse().expect("a name@token"),
    };
    let values = FileValues::new(vec![(fp("request.treatment"), js(request))])
        .relative_to(cwd)
        .expect("paths");
    Resolver::new(config)
        .register(Box::new(treatment))
        .expect("registers")
        .register(Box::new(others))
        .expect("registers")
        .resolve(ResolveInput::default().file(values))
        .await
        .map_err(|e| e.to_string())
}

fn comparable(resolution: &Resolution) -> Comparable {
    let settings: Vec<FieldPath> = resolution
        .settings
        .iter()
        .map(|setting| setting.path().clone())
        .collect();
    Comparable::from_resolution(&resolution.values, &resolution.sources, &settings)
        .expect("a cell")
        .0
}

/// The source entry of `treatment`, as JSON.
fn treatment_source(resolution: &Resolution) -> Value {
    let entry = resolution
        .sources
        .iter()
        .find(|entry| entry.path().as_str() == "treatment")
        .expect("a source for the treatment");
    serde_json::to_value(entry).expect("JSON")
}

/// Two checkouts of one commit: the original and a clone of it.
fn two_checkouts() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let root = tempfile::tempdir().expect("a temp dir");
    let one = root.path().join("one");
    std::fs::create_dir_all(&one).expect("a directory");
    git(&one, &["init", "-q"]);
    std::fs::write(one.join("SKILL.md"), "# skill\n").expect("a file");
    git(&one, &["add", "."]);
    git(&one, &["commit", "-q", "-m", "skill"]);
    git(&one, &["tag", "v1"]);
    let two = root.path().join("two");
    let out = Command::new("git")
        .arg("clone")
        .arg("-q")
        .arg(&one)
        .arg(&two)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "{out:?}");
    (root, one, two)
}

/// Guards: I7, I12 — a treatment's source says where its files were read, and nothing about what
/// they are: two experiment files that differ only in `[treatment] source`, naming two checkouts
/// of one commit, resolve to one cell digest, and their treatment's source entries differ only
/// in `from`.
/// Oracle: `git rev-parse` in each checkout.
/// Bite: write the source into the derived `treatment` (its `tag`), so it reaches `comparable.treatment`.
#[tokio::test]
async fn source_only_changes_the_source_entry() {
    let (root, one, two) = two_checkouts();
    let (commit_one, commit_two) = (
        git(&one, &["rev-parse", "v1^{commit}"]),
        git(&two, &["rev-parse", "v1^{commit}"]),
    );
    assert_eq!(commit_one, commit_two, "the checkouts hold one commit");

    let treatment =
        |dir: &Path| json!({"name": "demo", "tag": "v1", "source": dir.to_str().expect("UTF-8")});
    let first = resolve(treatment(&one), root.path())
        .await
        .expect("resolves");
    let second = resolve(treatment(&two), root.path())
        .await
        .expect("resolves");
    let (cell_one, cell_two) = (comparable(&first), comparable(&second));

    assert_eq!(
        cell_one
            .treatment()
            .commit
            .as_ref()
            .map(ToString::to_string),
        Some(commit_one),
        "the tag resolves to the commit git names"
    );
    assert_eq!(
        cell_one.digest(),
        cell_two.digest(),
        "the source moves no digest"
    );

    let (mut entry_one, mut entry_two) = (treatment_source(&first), treatment_source(&second));
    assert_ne!(entry_one, entry_two, "the entries tell the checkouts apart");
    let from = |entry: &mut Value| {
        entry["source"]
            .as_object_mut()
            .expect("a source object")
            .remove("from")
    };
    assert_eq!(
        from(&mut entry_one),
        Some(json!(one.to_str().expect("UTF-8")))
    );
    assert_eq!(
        from(&mut entry_two),
        Some(json!(two.to_str().expect("UTF-8")))
    );
    assert_eq!(entry_one, entry_two, "the entries differ only in `from`");
}

/// Guards: I12 — a derived value with no location it was read from records none: its source
/// entry holds the step and no `from`, so the entries of a manifest keep their bytes.
/// Oracle: the entry written by hand, `{"path": "treatment", "source": {"kind": "derived",
/// "step": "plain@1"}}`.
/// Bite: record an empty `from` for a value derived without one.
#[tokio::test]
async fn source_only_is_recorded_when_there_is_one() {
    struct Plain {
        name: NameToken,
    }
    impl PreflightStep for Plain {
        fn name<'a>(&'a self) -> &'a NameToken {
            &self.name
        }
        fn run(
            &self,
            _ctx: Arc<ResolveContext>,
        ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
            let report = StepReport::new(self.name.clone())
                .derive(fp("treatment"), js(json!({"name": "demo"})));
            Box::pin(async move { Ok(report) })
        }
    }
    let resolution = Resolver::new(Arc::new(Config::new(
        PathBuf::from("/work"),
        None,
        UserConfig::default(),
        None,
    )))
    .register(Box::new(Plain {
        name: "plain@1".parse().expect("a name@token"),
    }))
    .expect("registers")
    .resolve(ResolveInput::default())
    .await
    .expect("resolves");
    assert_eq!(
        treatment_source(&resolution),
        json!({
            "path": "treatment",
            "source": {"kind": "derived", "step": "plain@1"}
        })
    );
}

/// Guards: I12 — one checkout is one `sources[]` entry however its path is spelled: a trailing
/// slash, a `.` segment and a `..` segment resolve to equal `from`, and to one cell digest.
/// Oracle: the checkout's path as the test created it, written by hand.
/// Bite: record the path without removing its `.` and `..` segments and its trailing slash.
#[tokio::test]
async fn source_only_one_checkout_is_one_entry() {
    let (root, one, _two) = two_checkouts();
    let spelled = [
        format!("{}/", one.display()),
        format!("{}/./", one.display()),
        format!("{}/../one", one.display()),
        format!("{}/../one/./", root.path().join("one").display()),
    ];
    let plain = resolve(
        json!({"name": "demo", "tag": "v1", "source": one.to_str().expect("UTF-8")}),
        root.path(),
    )
    .await
    .expect("resolves");
    for text in &spelled {
        let other = resolve(
            json!({"name": "demo", "tag": "v1", "source": text}),
            root.path(),
        )
        .await
        .expect("resolves");
        assert_eq!(
            treatment_source(&other),
            treatment_source(&plain),
            "`{text}` records the entry of the plain path"
        );
        assert_eq!(comparable(&other).digest(), comparable(&plain).digest());
    }
    assert_eq!(
        treatment_source(&plain)["source"]["from"],
        json!(one.to_str().expect("UTF-8"))
    );
}
