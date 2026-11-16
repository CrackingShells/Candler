#![cfg(test)]
//! Guards for the refusal of ground truth in what the agent receives: an experiment whose
//! evaluator-only input has the bytes of a file in the image layer's Dockerfile or build context
//! (the experiment's or a plug-in environment's), in a plug-in environment's agent input, or in
//! the treatment's skill tree is refused at resolve with `isolation.ground_truth_exposed`.
//!
//! Every experiment is resolved as `candler run` resolves it, over a tree the test writes. The
//! expectations are digests worked by hand with `shasum -a 256` and the messages the author reads.

use crate::support;

use std::fs;
use std::path::PathBuf;
use std::str::FromStr as _;
use std::sync::Arc;

use candler_core::resolve::config::{Config, ConfigEnv};
use candler_core::resolve::layers::{FileValues, ResolveInput};
use candler_core::resolve::preflight::{
    PreflightError, PreflightStep, ResolveContext, ResolveError, Resolver, StepReport,
};
use candler_core::resolve::steps::ground_truth::{GroundTruth, GroundTruthStep, TruthFile};
use candler_core::resolve::steps::plugin_environment::PluginEnvironmentStep;
use candler_core::supervise::{EventSink, Supervisor};
use candler_harness::skills::SkillTree;
use candler_schemas::execution::EventPayload;
use candler_schemas::primitives::{Digest, FieldPath, NameToken, RunKey};
use candler_schemas::refusal::Refusal;
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;
use serde_json::{Value, json};

/// `printf 'CANARY-TRUTH\n' | shasum -a 256`.
const CANARY_SHA: &str = "e75708c483136fd4fcf30d513fd66ec9083bfc8c7000f5de6b97091da5469af2";
/// `printf 'skill\n' | shasum -a 256`: the bytes of the pinned test skill's `SKILL.md`.
const SKILL_SHA: &str = "3088e5b60779a95389e4ed08d2ecee6eaac2311c590dab3f2e4beb3090a54f00";
const CANARY: &str = "CANARY-TRUTH\n";

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

/// Which place of the tree holds the canary, and what the plug-in manifest declares.
struct Tree {
    tmp: tempfile::TempDir,
}

impl Tree {
    /// A tree with the experiment `exp/` (a Dockerfile and a context) and the plug-in `plug/`
    /// (an environment `e` with a Dockerfile, a context and the agent input `k`). Nothing holds
    /// the canary yet.
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("a temp dir");
        let root = tmp.path().canonicalize().expect("a real path");
        for dir in ["exp/ctx", "plug/docker/ctx", "plug/data"] {
            fs::create_dir_all(root.join(dir)).expect("directories");
        }
        fs::write(root.join("candler.toml"), "[plugins]\ndirs = [\"plug\"]\n").expect("config");
        fs::write(
            root.join("plug/plugin.toml"),
            "schema = \"plugin-manifest/1\"\nname = \"p\"\nversion = \"1.0.0\"\ncommand = [\"run\"]\n\n\
             [protocol]\nmajor = 1\nminor = 0\n\n[environment]\nkind = \"uv\"\nproject = \"env\"\nlock = \"env/uv.lock\"\n\n\
             [agent_environments.e.values]\n\"isolation.build.dockerfile\" = \"docker/Dockerfile\"\n\"isolation.build.context\" = \"docker/ctx\"\n\n\
             [[agent_environments.e.inputs]]\nkind = \"k\"\npath = \"data\"\n",
        )
        .expect("manifest");
        for file in ["exp/Dockerfile", "plug/docker/Dockerfile"] {
            fs::write(root.join(file), "ARG BASE\nFROM ${BASE}\n").expect("file");
        }
        fs::write(root.join("exp/ctx/data"), "e\n").expect("file");
        fs::write(root.join("plug/docker/ctx/data"), "d\n").expect("file");
        fs::write(root.join("plug/data/notes.txt"), "n\n").expect("file");
        Self { tmp }
    }

    fn root(&self) -> PathBuf {
        self.tmp.path().canonicalize().expect("a real path")
    }

    /// Writes the canary at `relative` below the tree, under another name than the truth's.
    fn plant(&self, relative: &str) {
        fs::write(self.root().join(relative), CANARY).expect("the canary");
    }
}

fn truth(sha: &str) -> GroundTruth {
    GroundTruth::new(vec![TruthFile {
        kind: "truth".to_owned(),
        path: "gt.txt".to_owned(),
        digest: Digest::from_str(&format!("sha256:{sha}")).expect("a digest"),
    }])
}

/// Resolves the tree's experiment as `candler run` does. `plugin` selects `p/e`; `own_layer`
/// gives the experiment's own Dockerfile and context.
async fn resolve(
    tree: &Tree,
    truth: GroundTruth,
    plugin: bool,
    own_layer: bool,
) -> Result<(), ResolveError> {
    let exp = tree.root().join("exp");
    let config = Arc::new(Config::load(&exp, &ConfigEnv::default()).expect("a config"));
    let (repo, commit) = support::job_orchestrate::skill_repo();
    let mut values: Vec<(FieldPath, JsonValue)> = Vec::new();
    if own_layer {
        values.push((fp("isolation.build.dockerfile"), js(json!("Dockerfile"))));
        values.push((fp("isolation.build.context"), js(json!("ctx"))));
    }
    if plugin {
        values.push((fp("isolation.build.plugin"), js(json!("p/e"))));
    }
    values.push((fp("request.harness"), js(json!({"name": "claude-code"}))));
    values.push((
        fp("request.treatment"),
        js(json!({"name": "htn", "source": repo})),
    ));
    let layer = FileValues::new(values)
        .relative_to(&exp)
        .expect("absolute paths");
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let step = GroundTruthStep::new(
        truth,
        Arc::new(Supervisor::new(sink)),
        SkillTree::new(".claude/skills", ".claude/skills"),
    )
    .expect("a step");
    Resolver::new(config)
        .register(Box::new(Pinned {
            name: "pinned@1".parse().expect("a name@token"),
            commit: commit.clone(), // BORROW: the step owns its commit
        }))
        .expect("registers")
        .register(Box::new(PluginEnvironmentStep::new().expect("a step")))
        .expect("registers")
        .register(Box::new(step))
        .expect("registers")
        .resolve(ResolveInput::default().file(layer))
        .await
        .map(|_| ())
}

/// Guards: R01 §Contracts I8 — an evaluator-only input whose bytes sit in the experiment's build
/// context, in a plug-in environment's context, in a plug-in environment's agent input or in the
/// treatment's skill tree is refused with `isolation.ground_truth_exposed`, and the message names
/// the input and the place, even when the copy has another file name.
/// Oracle: the canary's and the skill's sha-256, worked by hand with `shasum -a 256`.
/// Bite: skip the plug-in environment's agent inputs, and the `plug-in input` row passes.
#[tokio::test]
async fn refuses_ground_truth_in_every_agent_bound_place() {
    // (label, file to write the canary to, plug-in selected, own layer, the truth's sha, the place)
    let rows = [
        (
            "experiment context",
            Some("exp/ctx/renamed.bin"),
            false,
            true,
            CANARY_SHA,
            "`renamed.bin` in the experiment's build context",
        ),
        (
            "experiment Dockerfile",
            Some("exp/Dockerfile"),
            false,
            true,
            CANARY_SHA,
            "the Dockerfile of the experiment's build context",
        ),
        (
            "plug-in context",
            Some("plug/docker/ctx/deep.bin"),
            true,
            false,
            CANARY_SHA,
            "`deep.bin` in the plug-in environment's build context",
        ),
        (
            "plug-in input",
            Some("plug/data/inner.txt"),
            true,
            false,
            CANARY_SHA,
            "`inner.txt` in agent input `k` of the plug-in environment `p/e`",
        ),
        (
            "treatment tree",
            None,
            false,
            false,
            SKILL_SHA,
            "`.claude/skills/htn/SKILL.md` in the treatment's skill tree",
        ),
    ];
    for (label, planted, plugin, own_layer, sha, place) in rows {
        let tree = Tree::new();
        if let Some(relative) = planted {
            tree.plant(relative);
        }
        let error = resolve(&tree, truth(sha), plugin, own_layer)
            .await
            .expect_err(label);
        assert_eq!(
            error.code().as_str(),
            "isolation.ground_truth_exposed",
            "{label}: {error}"
        );
        let text = error.to_string();
        assert!(
            text.contains("evaluator-only input `truth` (gt.txt)") && text.contains(place),
            "{label}: {text}"
        );
    }
}

/// Guards: R01 §Contracts I8 — an experiment whose agent-bound bytes hold no copy of its
/// evaluator-only input resolves, and an experiment with no evaluator-only input reads nothing:
/// its build context does not even exist.
/// Oracle: hand-written; the canary is written nowhere, and the second tree points at a context
/// directory that is absent, which any walk of it would fail on.
#[tokio::test]
async fn passes_without_a_copy_and_reads_nothing_without_ground_truth() {
    let tree = Tree::new();
    resolve(&tree, truth(CANARY_SHA), true, true)
        .await
        .expect("no copy, no refusal");
    fs::remove_dir_all(tree.root().join("exp/ctx")).expect("removes the context");
    resolve(&tree, GroundTruth::default(), false, true)
        .await
        .expect("nothing is read when there is no ground truth");
}

/// Guards: R01 §Contracts I8 / D9 — an empty evaluator-only file next to an empty file in the
/// build context, in a plug-in environment's context and in its agent input is no copy, so the
/// experiment resolves: every empty file has one digest.
/// Oracle: the sha-256 of no bytes, `printf '' | shasum -a 256`.
/// Bite: remove the empty-digest filter in `GroundTruth::named`, and the experiment is refused.
#[tokio::test]
async fn passes_when_only_empty_files_match() {
    const EMPTY_SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let tree = Tree::new();
    for relative in [
        "exp/ctx/__init__.py",
        "plug/docker/ctx/__init__.py",
        "plug/data/__init__.py",
    ] {
        fs::write(tree.root().join(relative), "").expect("an empty file");
    }
    resolve(&tree, truth(EMPTY_SHA), true, true)
        .await
        .expect("an empty file is no copy");
}
