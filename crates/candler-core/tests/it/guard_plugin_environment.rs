#![cfg(test)]
//! Guards for the plug-in agent environment (R06 D4): an experiment that selects `p/e` with
//! `isolation.build.plugin` gets the environment's values and inputs as a layer below its own
//! file. Every experiment is resolved and planned through the route `candler run` takes, over a
//! plug-in directory written by the test.
//!
//! The expectations are the hand-written experiment that spells the same values out, the codes
//! of D4, and literals worked by hand.

use crate::support;

use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use candler_core::experiment::inputs::resolve_input_groups;
use candler_core::isolation::image::{
    BuildSetting, ImageError, ImageService, LockDigest, LockRequest,
};
use candler_core::isolation::model::{ImageId, ImageRef};
use candler_core::resolve::config::{Config, ConfigEnv};
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::{
    PreflightError, PreflightStep, ResolveContext, ResolveError, Resolver, StepReport,
};
use candler_core::resolve::steps::image::ImageIdStep;
use candler_core::resolve::steps::plugin_environment::{
    Fragment, FragmentError, PluginEnvironmentStep, plugin_egress, unite_egress,
};
use candler_core::runner::plan::{Force, PlanError, PlanMode, plan_runs};
use candler_core::store::Store;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::{
    Comparable, EventPayload, ExperimentFile, ExperimentInput, InputAudience,
};
use candler_schemas::primitives::{FieldPath, Name, NameToken, RunKey};
use candler_schemas::refusal::Refusal;
use candler_schemas::values::{JsonValue, ValueSource};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use support::run_plan::{Facts, fresh_store, plan_ctx};

fn fp(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

fn js(value: Value) -> JsonValue {
    JsonValue::new(value).expect("a JSON value")
}

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// What the image step asked the image service for.
#[derive(Debug, Default)]
struct Asked {
    /// The prune list of every `ensure` call.
    prunes: Vec<Vec<String>>,
    /// The name and bytes of the sources of every `lock` call.
    sources: Vec<Vec<(String, String)>>,
}

/// An image service that answers with one image, records what it was asked, and compiles a lock
/// that is the sources written one after the other, so two lists give one lock only when they
/// hold the same sources in the same order.
struct Recording(Arc<Mutex<Asked>>);

impl ImageService for Recording {
    fn lock<'a>(&'a self, request: &'a LockRequest) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        let held: Vec<(String, String)> = request
            .sources
            .iter()
            .map(|s| {
                (
                    s.name.clone(),
                    String::from_utf8_lossy(&s.bytes).into_owned(),
                )
            }) // BORROW: the record owns its texts
            .collect();
        let text: String = held
            .iter()
            .flat_map(|(n, b)| [n.as_str(), "\n", b.as_str(), "\n"])
            .collect();
        self.0.lock().expect("the record").sources.push(held);
        Box::pin(async move { Ok(text.into_bytes()) })
    }

    fn ensure<'a>(
        &'a self,
        _lock: &'a LockDigest,
        setting: &'a BuildSetting,
    ) -> BoxFuture<'a, Result<ImageRef, ImageError>> {
        // BORROW: .clone() gives the record its own prune list
        let prune = setting.prune.clone();
        self.0.lock().expect("the record").prunes.push(prune);
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

/// Derives the pinned treatment the `treatment:` sources are read from.
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
        let report = StepReport::new(self.name.clone())
            .derive(
                fp("harness"),
                js(json!({"name": "claude-code", "version": "2.1.311"})),
            )
            .derive(
                fp("treatment"),
                js(json!({"name": "htn", "commit": self.commit})),
            );
        Box::pin(async move { Ok(report) })
    }
}

/// A scratch tree: the project root with `candler.toml`, the plug-in `p` in `plug/`, and the
/// experiment's directory `exp/`.
struct Rig {
    tmp: tempfile::TempDir,
}

impl Rig {
    /// A tree whose plug-in manifest holds `environments` (TOML tables below
    /// `agent_environments`), and the files every test reads.
    fn new(environments: &str) -> Self {
        let tmp = tempfile::tempdir().expect("a temp dir");
        let root = tmp.path().canonicalize().expect("a real path");
        fs::create_dir_all(root.join("plug/docker/ctx")).expect("directories");
        fs::create_dir_all(root.join("exp")).expect("directories");
        fs::write(root.join("candler.toml"), "[plugins]\ndirs = [\"plug\"]\n").expect("config");
        fs::write(
            root.join("plug/plugin.toml"),
            format!(
                "schema = \"plugin-manifest/1\"\nname = \"p\"\nversion = \"1.0.0\"\ncommand = [\"run\"]\n\n\
                 [protocol]\nmajor = 1\nminor = 0\n\n[environment]\nkind = \"uv\"\nproject = \"env\"\nlock = \"env/uv.lock\"\n\n{environments}\n"
            ),
        )
        .expect("manifest");
        fs::write(root.join("plug/req.txt"), "plugin-dep==1\n").expect("file");
        fs::write(
            root.join("plug/docker/Dockerfile"),
            "ARG BASE\nFROM ${BASE}\n",
        )
        .expect("file");
        fs::write(root.join("plug/docker/ctx/data"), "d\n").expect("file");
        fs::write(root.join("exp/extra.txt"), "exp-dep==2\n").expect("file");
        fs::write(root.join("exp/Dockerfile"), "ARG BASE\nFROM ${BASE}\n").expect("file");
        fs::create_dir_all(root.join("exp/ctx")).expect("directories");
        fs::write(root.join("exp/ctx/data"), "e\n").expect("file");
        Self { tmp }
    }

    fn root(&self) -> PathBuf {
        self.tmp.path().canonicalize().expect("a real path")
    }

    fn plug(&self) -> PathBuf {
        self.root().join("plug")
    }

    fn exp(&self) -> PathBuf {
        self.root().join("exp")
    }
}

/// An experiment file whose `[fixed]` holds `extra` lines and one factor, with the pinned skill
/// repository as its treatment source.
fn file_with(extra: &str) -> ExperimentFile {
    let (repo, _) = support::job_orchestrate::skill_repo();
    let text = format!(
        "schema = \"experiment/1\"\nreplicates = 1\n\n[fixed]\n\"prompt.template\" = \"say hello\"\n{extra}\n\
         [[factors]]\npath = \"model.id\"\nlevels = [\"m1\"]\n\n\
         [budget.per_run]\nwall_s = 5\n\n[harness]\nname = \"claude-code\"\n\n\
         [treatment]\nname = \"htn\"\nsource = \"{}\"\n",
        repo.display()
    );
    // BORROW: .as_str() lends the document to the parser
    ExperimentFile::from_toml_str(text.as_str()).expect("an experiment file")
}

/// What one resolution and plan produced.
struct Planned {
    resolution: Resolution,
    cells: Vec<Comparable>,
    asked: Arc<Mutex<Asked>>,
}

/// Resolves `file` as `candler run` does, with the steps in the order `register_m2b_steps` gives
/// them.
async fn resolve(
    rig: &Rig,
    file: &ExperimentFile,
) -> Result<(Resolution, Arc<Mutex<Asked>>), ResolveError> {
    let asked = Arc::new(Mutex::new(Asked::default()));
    let config = Arc::new(Config::load(&rig.exp(), &ConfigEnv::default()).expect("a config"));
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let image = ImageIdStep::new(
        // TRAIT_OBJECT: the image service is a recording fake
        Arc::new(Recording(Arc::clone(&asked))),
        Arc::new(Supervisor::new(sink)),
    )
    .expect("a step");
    let (_, commit) = support::job_orchestrate::skill_repo();
    let pinned = Pinned {
        name: "pinned@1".parse().expect("a name@token"),
        commit: commit.clone(), // BORROW: the step owns its commit
    };
    let mut values: Vec<(FieldPath, JsonValue)> = file
        .fixed()
        .iter()
        .map(|(path, value)| (path.clone(), value.clone())) // BORROW: the layer owns its paths and values
        .collect();
    values.push((
        fp("request.harness"),
        js(serde_json::to_value(file.harness()).expect("JSON")),
    ));
    values.push((
        fp("request.treatment"),
        js(serde_json::to_value(file.treatment()).expect("JSON")),
    ));
    let layer = FileValues::new(values)
        .relative_to(&rig.exp())
        .expect("absolute paths");
    let resolution = Resolver::new(config)
        .register(Box::new(pinned))
        .expect("registers")
        .register(Box::new(PluginEnvironmentStep::new().expect("a step")))
        .expect("registers")
        .register(Box::new(image))
        .expect("registers")
        .resolve(ResolveInput::default().file(layer))
        .await?;
    Ok((resolution, asked))
}

/// Resolves and plans `file`.
async fn plan(rig: &Rig, file: &ExperimentFile) -> Result<Planned, PlanError> {
    let (resolution, asked) = resolve(rig, file).await.expect("resolves");
    plan_resolved(file, resolution, asked)
}

fn plan_resolved(
    file: &ExperimentFile,
    resolution: Resolution,
    asked: Arc<Mutex<Asked>>,
) -> Result<Planned, PlanError> {
    let (_keep, store, lock) = fresh_store();
    let ctx = plan_ctx(&store, &lock, &Facts::base());
    let outcome = plan_runs(&ctx, file, &resolution, Force::No, PlanMode::Check)?;
    Ok(Planned {
        cells: outcome.fresh.into_values().collect(),
        resolution,
        asked,
    })
}

/// The `isolation.egress` the only planned cell holds.
fn egress_in(planned: &Planned) -> Option<Value> {
    planned.cells.first().and_then(|cell| {
        cell.values()
            .iter()
            .find(|value| value.path().as_str() == "isolation.egress")
            .and_then(|value| value.value())
            .map(|value| value.as_value().clone())
    })
}

/// The value the resolution holds at `path`, among values and settings.
fn value<'a>(resolution: &'a Resolution, path: &str) -> Option<&'a Value> {
    resolution
        .values
        .iter()
        .chain(resolution.settings.iter())
        .find(|v| v.path().as_str() == path)
        .and_then(|v| v.value())
        .map(JsonValue::as_value)
}

fn source<'a>(resolution: &'a Resolution, path: &str) -> Option<&'a ValueSource> {
    resolution
        .sources
        .iter()
        .find(|entry| entry.path().as_str() == path)
        .map(candler_schemas::values::SourceEntry::source)
}

const WEB: &str = r#"
[agent_environments.web.values]
"isolation.build.requirements" = ["req.txt"]
"isolation.build.prune" = ["a.py", "b.py"]
"mcp.servers.tools.command" = "plug-cmd"
"mcp.servers.tools.args" = ["--serve"]
"#;

/// Guards: R06 D4, N1, N2 — an experiment that selects `p/web` resolves to the same cell, the
/// same `isolation.env_lock`, the same prune list and the same lock sources as the experiment
/// that writes the environment's values out by hand with the plug-in's paths: the plug-in's
/// requirement path reads against the plug-in's directory, not the working directory.
/// Oracle: the hand-written experiment through the existing path, with the plug-in directory
/// spelled out as the file layer spells a path.
/// Bite: in the step, join the fragment's paths to the working directory instead of the plug-in's.
#[tokio::test]
async fn equivalent_values_and_lock() {
    let rig = Rig::new(WEB);
    let selected = plan(&rig, &file_with("\"isolation.build.plugin\" = \"p/web\""))
        .await
        .expect("plans");
    let plug = rig.plug();
    let by_hand = plan(
        &rig,
        &file_with(&format!(
            "\"isolation.build.requirements\" = [\"{plug}/req.txt\"]\n\
             \"isolation.build.prune\" = [\"a.py\", \"b.py\"]\n\
             \"mcp.servers.tools.command\" = \"plug-cmd\"\n\
             \"mcp.servers.tools.args\" = [\"--serve\"]\n",
            plug = plug.display()
        )),
    )
    .await
    .expect("plans");
    assert_eq!(selected.cells.len(), 1);
    assert_eq!(
        selected.cells[0].digest(),
        by_hand.cells[0].digest(),
        "the same cell"
    );
    assert_eq!(
        value(&selected.resolution, "isolation.env_lock"),
        value(&by_hand.resolution, "isolation.env_lock"),
        "the same env lock"
    );
    assert_eq!(
        selected.asked.lock().expect("record").sources,
        by_hand.asked.lock().expect("record").sources,
        "the same lock sources"
    );
    assert_eq!(
        selected.asked.lock().expect("record").sources[0],
        vec![("req.txt".to_owned(), "plugin-dep==1\n".to_owned())]
    );
    assert_eq!(
        selected.asked.lock().expect("record").prunes,
        by_hand.asked.lock().expect("record").prunes
    );
}

/// Guards: R06 D4, N3 — an appended requirements list gives the lock of the same list written out
/// by hand: the plug-in's sources first, then the experiment's, a `treatment:` entry included, a
/// source the experiment repeats kept once at its first place.
/// Oracle: the hand-written list through the existing path; the lock is the sources in order, so
/// a list in another order or without the plug-in's entry gives another lock.
/// Bite: the plug-in's requirements replaced by the experiment's instead of appended.
#[tokio::test]
async fn equivalent_appended_requirements() {
    let rig = Rig::new(WEB);
    let plug = rig.plug();
    let treatment = ".claude/skills/htn/SKILL.md";
    let selected = plan(
        &rig,
        &file_with(&format!(
            "\"isolation.build.plugin\" = \"p/web\"\n\
             \"isolation.build.requirements\" = [\"extra.txt\", \"treatment:{treatment}\", \"{plug}/req.txt\"]\n",
            plug = plug.display()
        )),
    )
    .await
    .expect("plans");
    let by_hand = plan(
        &rig,
        &file_with(&format!(
            "\"isolation.build.requirements\" = [\"{plug}/req.txt\", \"extra.txt\", \"treatment:{treatment}\"]\n",
            plug = plug.display()
        )),
    )
    .await
    .expect("plans");
    let names: Vec<String> = selected.asked.lock().expect("record").sources[0]
        .iter()
        .map(|(name, _)| name.clone()) // BORROW: the test owns the names
        .collect();
    assert_eq!(names, ["req.txt", "extra.txt", "SKILL.md"]);
    assert_eq!(
        value(&selected.resolution, "isolation.env_lock"),
        value(&by_hand.resolution, "isolation.env_lock"),
        "the same lock as the hand-written list"
    );
    assert_eq!(
        selected.asked.lock().expect("record").sources,
        by_hand.asked.lock().expect("record").sources
    );
}

/// Guards: R06 D4 — an experiment scalar beats the plug-in's and the resolution records the
/// experiment file as its source; a value only the plug-in sets is the plug-in's, recorded as
/// derived by `plugin_environment` from `p/web`; the step's report holds both values with the
/// origin.
/// Oracle: the layer order of `resolve/layers.rs` (the file above a derived value), worked by hand.
/// Bite: the plug-in's value winning the scalar (a derived layer ranked above the file).
#[tokio::test]
async fn layer_precedence_records_both_sources() {
    let rig = Rig::new(WEB);
    let (resolution, _) = resolve(
        &rig,
        &file_with(
            "\"isolation.build.plugin\" = \"p/web\"\n\"mcp.servers.tools.command\" = \"own-cmd\"\n",
        ),
    )
    .await
    .expect("resolves");
    assert_eq!(
        value(&resolution, "mcp.servers.tools.command"),
        Some(&json!("own-cmd"))
    );
    assert_eq!(
        source(&resolution, "mcp.servers.tools.command"),
        Some(&ValueSource::ExperimentFile)
    );
    assert_eq!(
        value(&resolution, "mcp.servers.tools.args"),
        Some(&json!(["--serve"]))
    );
    let from = Some("p/web".to_owned());
    assert!(
        matches!(
            source(&resolution, "mcp.servers.tools.args"),
            Some(ValueSource::Derived { step, from: found, .. })
                if step.to_string() == "plugin_environment@1" && *found == from
        ),
        "{:?}",
        source(&resolution, "mcp.servers.tools.args")
    );
    let report = resolution
        .preflight
        .iter()
        .find(|r| r.step.name() == "plugin_environment")
        .expect("the step's report");
    let written: Vec<&str> = report.derived.iter().map(|(p, _)| p.as_str()).collect();
    assert!(
        written.contains(&"mcp.servers.tools.command"),
        "{written:?}"
    );
    assert!(
        !written.contains(&"request.inputs"),
        "inputs are never derived"
    );
    assert!(
        report.origins.iter().all(|(_, text)| text == "p/web"),
        "{:?}",
        report.origins
    );
}

/// Guards: R06 D4 — an unknown plug-in is refused naming it, and an unknown environment is
/// `isolation.plugin_environment_missing`, naming the environments the plug-in offers.
/// Oracle: the codes of D4 (`codes.toml`), and the manifest written by the test.
/// Bite: the missing environment reported as a failed step, which loses the code.
#[tokio::test]
async fn layer_unknown_plugin_and_environment() {
    let rig = Rig::new(WEB);
    let error = resolve(&rig, &file_with("\"isolation.build.plugin\" = \"q/web\""))
        .await
        .expect_err("an unknown plug-in");
    assert!(
        error.to_string().contains("no plug-in named `q`"),
        "{error}"
    );
    let error = resolve(&rig, &file_with("\"isolation.build.plugin\" = \"p/nope\""))
        .await
        .expect_err("an unknown environment");
    assert_eq!(
        error.code().to_string(),
        "isolation.plugin_environment_missing"
    );
    assert!(
        error.to_string().contains("`nope`") && error.to_string().contains("web"),
        "{error}"
    );
    let error = resolve(&rig, &file_with("\"isolation.build.plugin\" = \"nodash\""))
        .await
        .expect_err("a selection of one name");
    assert!(
        error.to_string().contains("<plugin>/<environment>"),
        "{error}"
    );
}

/// Guards: R06 D4, I14 — a path of an environment that leaves the plug-in directory is refused
/// at the reader: an absolute path, a `..` segment, a path through a symbolic link that points
/// out, and an input that does the same.
/// Oracle: the rule of I14 (a path stays in its directory), one row per way out.
/// Bite: the symbolic-link check dropped, so only the lexical checks remain.
#[test]
fn layer_paths_stay_in_the_plugin_directory() {
    let rows: [(&str, &str); 5] = [
        (
            "absolute",
            "[agent_environments.e.values]\n\"isolation.build.lock\" = \"/etc/hosts\"\n",
        ),
        (
            "dotdot",
            "[agent_environments.e.values]\n\"isolation.build.requirements\" = [\"../exp/extra.txt\"]\n",
        ),
        (
            "link",
            "[agent_environments.e.values]\n\"isolation.build.context\" = \"out/ctx\"\n",
        ),
        (
            "input dotdot",
            "[[agent_environments.e.inputs]]\nkind = \"k\"\npath = \"../exp\"\n",
        ),
        (
            "input link",
            "[[agent_environments.e.inputs]]\nkind = \"k\"\npath = \"out/more\"\n",
        ),
    ];
    for (label, environments) in rows {
        let rig = Rig::new(environments);
        let outside = rig.root().join("outside");
        fs::create_dir_all(outside.join("ctx")).expect("directory");
        fs::create_dir_all(outside.join("more")).expect("directory");
        symlink(&outside, rig.plug().join("out")).expect("a link");
        let error = Fragment::load(&[rig.plug()], "p/e").expect_err(label);
        assert!(
            matches!(error, FragmentError::Escapes { .. }),
            "{label}: {error}"
        );
    }
    // The same environment without the way out loads, with its paths below the plug-in directory.
    let rig = Rig::new(
        "[agent_environments.e.values]\n\"isolation.build.requirements\" = [\"req.txt\", \"treatment:x.txt\"]\n",
    );
    let fragment = Fragment::load(&[rig.plug()], "p/e").expect("loads");
    assert_eq!(
        fragment.list("isolation.build.requirements"),
        [
            format!("{}/req.txt", rig.plug().display()),
            "treatment:x.txt".to_owned()
        ]
    );
}

/// Guards: R06 D4, N3 — prune appends the plug-in's entries first and drops repeats; a plug-in's
/// egress, alone or beside the experiment's own, plans per cell as the canonical union, while an
/// entry that is no host or meets bridge keeps its own code.
/// Oracle: the prune list worked by hand (`a.py b.py c.py`), the unions worked by hand
/// (`a.com b.com` and `a.com b.com c.com`), and the codes of the egress rule.
/// Bite: the plug-in's egress dropped from the merge (the cell holds no list or only the
/// experiment's).
#[tokio::test]
async fn layer_lists_append() {
    let rig = Rig::new(
        r#"
[agent_environments.web.values]
"isolation.build.prune" = ["a.py", "b.py"]
"isolation.egress" = ["b.com:443", "a.com:443"]
"#,
    );
    let refusal =
        |planned: Result<Planned, PlanError>| planned.err().expect("refused").code().to_string();
    let alone = plan(&rig, &file_with("\"isolation.build.plugin\" = \"p/web\""))
        .await
        .expect("the plug-in's list alone plans");
    assert_eq!(
        egress_in(&alone),
        Some(json!(["a.com:443", "b.com:443"])),
        "the plug-in's list alone"
    );
    let beside = plan(
        &rig,
        &file_with(
            "\"isolation.build.plugin\" = \"p/web\"\n\"isolation.egress\" = [\"a.com:443\", \"c.com:443\"]\n",
        ),
    )
    .await
    .expect("beside the experiment's list plans");
    assert_eq!(
        egress_in(&beside),
        Some(json!(["a.com:443", "b.com:443", "c.com:443"])),
        "beside the experiment's list"
    );

    // Prune appends without any egress.
    let pruned = Rig::new(
        "[agent_environments.web.values]\n\"isolation.build.prune\" = [\"a.py\", \"b.py\"]\n",
    );
    let selected = plan(
        &pruned,
        &file_with(
            "\"isolation.build.plugin\" = \"p/web\"\n\"isolation.build.prune\" = [\"b.py\", \"c.py\"]\n",
        ),
    )
    .await
    .expect("plans");
    assert_eq!(
        selected.asked.lock().expect("record").prunes,
        [vec![
            "a.py".to_owned(),
            "b.py".to_owned(),
            "c.py".to_owned()
        ]]
    );

    // A plug-in entry that is no host, or meets bridge, is refused per cell with its own code
    // even when the experiment writes its own list.
    let bad =
        Rig::new("[agent_environments.web.values]\n\"isolation.egress\" = [\"10.0.0.1:80\"]\n");
    let invalid = refusal(
        plan(
            &bad,
            &file_with(
                "\"isolation.build.plugin\" = \"p/web\"\n\"isolation.egress\" = [\"a.com:443\"]\n",
            ),
        )
        .await,
    );
    assert_eq!(invalid, "isolation.egress_invalid");
    let bridged = refusal(
        plan(
            &rig,
            &file_with(
                "\"isolation.build.plugin\" = \"p/web\"\n\"isolation.egress\" = [\"a.com:443\"]\n\"isolation.network\" = \"bridge\"\n",
            ),
        )
        .await,
    );
    assert_eq!(bridged, "isolation.egress_with_bridge");
}

/// Guards: R06 D4, N3 — the per-cell egress merge reads the plug-in step's passing report and
/// nothing else: another step's derived egress and a failed report do not enter, and a job
/// re-planned from its stored resolution holds the plug-in's egress as the first plan does.
/// Oracle: reports built by hand (names, checks and values), and the list of the first plan.
/// Bite: the merge reading every report's `isolation.egress`, or the stored resolution losing the
/// reports (the stored cell holds no list).
#[tokio::test]
async fn layer_egress_merge_reads_the_plugin_report_only() {
    let report = |step: &str, egress: Value, passed: bool| {
        let name: NameToken = step.parse().expect("a name@token");
        let mut report = StepReport::new(name).derive(fp("isolation.egress"), js(egress));
        if !passed {
            report = report.check(candler_core::resolve::preflight::CheckResult::new(
                "failed@1".parse().expect("a name@token"),
                false,
                "no".to_owned(),
            ));
        }
        report
    };
    let reports = [
        report("other@1", json!(["other.com:443"]), true),
        report("plugin_environment@1", json!(["lost.com:443"]), false),
    ];
    assert_eq!(plugin_egress(&reports), None, "no passing plug-in report");
    let reports = [
        report("other@1", json!(["other.com:443"]), true),
        report("plugin_environment@1", json!(["mine.com:443"]), true),
    ];
    assert_eq!(plugin_egress(&reports), Some(&json!(["mine.com:443"])));
    assert_eq!(
        unite_egress(
            plugin_egress(&reports),
            Some(&json!(["own.com:443", "mine.com:443"]))
        ),
        Some(json!(["mine.com:443", "own.com:443", "mine.com:443"])),
        "the plug-in's entries first"
    );
    assert_eq!(unite_egress(None, Some(&json!("odd"))), Some(json!("odd")));
    assert_eq!(
        unite_egress(Some(&json!("odd")), Some(&json!(["a.com:443"]))),
        Some(json!("odd")),
        "a value that is no list reaches the rule that refuses it"
    );

    // A job re-planned from its stored resolution refuses as the first plan does.
    let rig =
        Rig::new("[agent_environments.web.values]\n\"isolation.egress\" = [\"pypi.org:443\"]\n");
    let file = file_with("\"isolation.build.plugin\" = \"p/web\"\n");
    let (resolution, asked) = resolve(&rig, &file).await.expect("resolves");
    let stored: Resolution =
        serde_json::from_slice(&serde_json::to_vec(&resolution).expect("JSON"))
            .expect("reads back");
    for (label, held) in [("fresh", resolution), ("stored", stored)] {
        let planned = plan_resolved(&file, held, Arc::clone(&asked)).expect("plans");
        assert_eq!(
            egress_in(&planned),
            Some(json!(["pypi.org:443"])),
            "{label}"
        );
    }
}

/// Guards: R06 D4 — the `dockerfile`/`context` pair comes from one layer: a plug-in that sets
/// one half is refused by the must-be-set-together rule, and so is a pair split between the
/// plug-in and the experiment; a pair from the plug-in, or from the experiment, resolves. The
/// same holds for the lock and the requirements list: one layer sets both, or the lock that would
/// silently beat the list is refused.
/// Oracle: D4's rule, one row per way the pair is set.
/// Bite: the layer check dropped, so the split pair builds from two directories; the lock check
/// dropped, so a plug-in lock beats the experiment's list (and the reverse) without a word.
#[tokio::test]
async fn layer_pair_from_two_layers() {
    let half =
        "[agent_environments.e.values]\n\"isolation.build.dockerfile\" = \"docker/Dockerfile\"\n";
    let both = "[agent_environments.e.values]\n\"isolation.build.dockerfile\" = \"docker/Dockerfile\"\n\"isolation.build.context\" = \"docker/ctx\"\n";
    let lock = "[agent_environments.e.values]\n\"isolation.build.lock\" = \"req.txt\"\n";
    let list = "[agent_environments.e.values]\n\"isolation.build.requirements\" = [\"req.txt\"]\n";
    let lock_and_list = "[agent_environments.e.values]\n\"isolation.build.lock\" = \"req.txt\"\n\"isolation.build.requirements\" = [\"req.txt\"]\n";
    let rows: [(&str, &str, &str, bool); 10] = [
        ("plug-in half", half, "", false),
        ("plug-in pair", both, "", true),
        (
            "split",
            both,
            "\"isolation.build.context\" = \"ctx\"\n",
            false,
        ),
        (
            "experiment pair over plug-in pair",
            both,
            "\"isolation.build.dockerfile\" = \"Dockerfile\"\n\"isolation.build.context\" = \"ctx\"\n",
            true,
        ),
        (
            "experiment half",
            half,
            "\"isolation.build.context\" = \"ctx\"\n",
            false,
        ),
        (
            "plug-in lock, experiment list",
            lock,
            "\"isolation.build.requirements\" = [\"extra.txt\"]\n",
            false,
        ),
        (
            "experiment lock, plug-in list",
            list,
            "\"isolation.build.lock\" = \"extra.txt\"\n",
            false,
        ),
        ("plug-in lock and list", lock_and_list, "", true),
        (
            "experiment lock and list",
            "[agent_environments.e.values]\n",
            "\"isolation.build.lock\" = \"extra.txt\"\n\"isolation.build.requirements\" = [\"extra.txt\"]\n",
            true,
        ),
        ("plug-in lock alone", lock, "", true),
    ];
    for (label, environments, own, resolves) in rows {
        let rig = Rig::new(environments);
        let outcome = resolve(
            &rig,
            &file_with(&format!("\"isolation.build.plugin\" = \"p/e\"\n{own}")),
        )
        .await;
        match (outcome, resolves) {
            (Ok(_), true) => {}
            (Err(error), false) => assert!(
                error.to_string().contains("must be set together"),
                "{label}: {error}"
            ),
            (Ok(_), false) => panic!("{label}: resolved"),
            (Err(error), true) => panic!("{label}: refused: {error}"),
        }
    }
}

/// Guards: R06 D4 — the environment's inputs resolve against the plug-in directory into the same
/// mounts as the same inputs written by hand in the experiment, at their `at`; a mount-point
/// collision with an experiment input, and a kind declared twice, are refused by the existing rules.
/// Oracle: the hand-written inputs through the existing resolution, and its refusal texts.
/// Bite: the plug-in's inputs resolved against the experiment's directory.
#[test]
fn layer_inputs_mount() {
    let rig = Rig::new(
        "[[agent_environments.e.inputs]]\nkind = \"tools\"\npath = \"docker/ctx\"\nat = \"tools\"\n",
    );
    let fragment = Fragment::load(&[rig.plug()], "p/e").expect("loads");
    let dir = tempfile::tempdir().expect("a temp dir");
    let store = Store::init(&dir.path().join("store")).expect("a store");
    let input = |kind: &str, path: &str, at: Option<&str>| {
        ExperimentInput::new(
            kind.parse::<Name>().expect("a name"),
            path.to_owned(),
            at.map(str::to_owned),
            InputAudience::Agent,
        )
        .expect("an input")
    };
    let own = [input("notes", "ctx", Some("notes"))];
    let exp = rig.exp();
    let merged = resolve_input_groups(
        &store,
        &[
            (&own, exp.as_path()),
            (fragment.inputs.as_slice(), fragment.dir.as_path()),
        ],
    )
    .expect("resolves");
    let by_hand_inputs = [
        input("notes", "exp/ctx", Some("notes")),
        input("tools", "plug/docker/ctx", Some("tools")),
    ];
    let root = rig.root();
    let by_hand =
        resolve_input_groups(&store, &[(&by_hand_inputs, root.as_path())]).expect("resolves");
    let mut mounts: Vec<_> = merged
        .mounts
        .iter()
        .map(|m| (m.path.clone(), m.digest.clone()))
        .collect();
    mounts.sort();
    let mut expected: Vec<_> = by_hand
        .mounts
        .iter()
        .map(|m| (m.path.clone(), m.digest.clone()))
        .collect();
    expected.sort();
    assert_eq!(mounts, expected, "the same mounts");
    assert_eq!(mounts[1].0, "tools/data");
    assert_eq!(merged.agent, by_hand.agent);

    let clash = [input("notes", "ctx", Some("tools"))];
    let error = resolve_input_groups(
        &store,
        &[
            (&clash, exp.as_path()),
            (fragment.inputs.as_slice(), fragment.dir.as_path()),
        ],
    )
    .expect_err("a mount point shared");
    assert!(
        error.to_string().contains("is mounted where input"),
        "{error}"
    );
    let twice = [input("tools", "ctx", Some("elsewhere"))];
    let error = resolve_input_groups(
        &store,
        &[
            (&twice, exp.as_path()),
            (fragment.inputs.as_slice(), fragment.dir.as_path()),
        ],
    )
    .expect_err("a kind declared twice");
    assert!(error.to_string().contains("is declared twice"), "{error}");
}
