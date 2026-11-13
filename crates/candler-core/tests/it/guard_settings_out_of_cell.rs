#![cfg(test)]
//! Guards for the configuration surface (R01 §Agent environment and secrets): a setting an
//! experiment file writes reaches the resolver as a setting and never the cell. Every experiment
//! is planned through the route `candler run` takes: the file's `[fixed]`, `request.harness` and
//! `request.treatment` are one file layer, resolved by the image step, then `plan_runs`.

use crate::support;

use std::path::Path;
use std::sync::{Arc, Mutex};

use candler_core::isolation::image::{
    BuildSetting, ImageError, ImageService, LockDigest, LockRequest,
};
use candler_core::isolation::model::{ImageId, ImageRef};
use candler_core::jobs::orchestrate::session::record_packages;
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::{
    PreflightError, PreflightStep, ResolveContext, Resolver, StepReport,
};
use candler_core::resolve::steps::image::ImageIdStep;
use candler_core::runner::plan::{Force, PlanError, PlanMode, plan_runs};
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::{Comparable, EventPayload, ExperimentFile};
use candler_schemas::primitives::{FieldPath, NameToken, RunKey};
use candler_schemas::values::JsonValue;
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
#[derive(Default)]
struct Asked {
    /// The prune list of every `ensure` call.
    prunes: Vec<Vec<String>>,
    /// The file names of the sources of every `lock` call.
    sources: Vec<Vec<String>>,
}

/// An image service that answers with one image and records what it was asked.
struct Recording(Arc<Mutex<Asked>>);

impl ImageService for Recording {
    fn lock<'a>(&'a self, request: &'a LockRequest) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        let names = request.sources.iter().map(|s| s.name.clone()).collect(); // BORROW: the record owns the names
        self.0.lock().expect("the record").sources.push(names);
        Box::pin(async { Ok(b"lock".to_vec()) })
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

/// Derives the typed roots the image step does not.
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
                fp("treatment"),
                js(json!({"name": "htn", "commit": "1".repeat(40)})),
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

/// An experiment file whose `[fixed]` holds a real cell value and `extra`, and whose factors are
/// `model.id` over one level and `more`.
fn file_with(extra: &str, more: &str) -> ExperimentFile {
    let text = format!(
        "schema = \"experiment/1\"\nreplicates = 1\n\n[fixed]\n\"prompt.template\" = \"say hello\"\n{extra}\n\
         [[factors]]\npath = \"model.id\"\nlevels = [\"m1\"]\n{more}\n\
         [budget.per_run]\nwall_s = 5\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"htn\"\n"
    );
    // BORROW: .as_str() lends the document to the parser
    ExperimentFile::from_toml_str(text.as_str()).expect("an experiment file")
}

/// What planning one file produced.
struct Planned {
    cells: Vec<Comparable>,
    resolution: Resolution,
    asked: Arc<Mutex<Asked>>,
}

/// Resolves and plans `file` as `candler run` does.
async fn plan(file: &ExperimentFile, dir: &Path) -> Planned {
    try_plan(file, dir).await.expect("plans")
}

/// Resolves `file` as `candler run` does and plans it, or says why planning refused.
async fn try_plan(file: &ExperimentFile, dir: &Path) -> Result<Planned, PlanError> {
    let asked = Arc::new(Mutex::new(Asked::default()));
    let config = Arc::new(Config::new(
        dir.to_owned(),
        None,
        UserConfig::default(),
        Some(dir.join("state")),
    ));
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let image = ImageIdStep::new(
        // TRAIT_OBJECT: the image service is a recording fake
        Arc::new(Recording(Arc::clone(&asked))),
        Arc::new(Supervisor::new(sink)),
    )
    .expect("a step");
    let roots = OtherRoots {
        name: "roots@1".parse().expect("a name@token"),
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
        .relative_to(dir)
        .expect("absolute paths");
    let resolution = Resolver::new(config)
        .register(Box::new(roots))
        .expect("registers")
        .register(Box::new(image))
        .expect("registers")
        .resolve(ResolveInput::default().file(layer))
        .await
        .expect("resolves");
    let (_keep, store, lock) = fresh_store();
    let ctx = plan_ctx(&store, &lock, &Facts::base());
    let outcome = plan_runs(&ctx, file, &resolution, Force::No, PlanMode::Check)?;
    Ok(Planned {
        cells: outcome.fresh.into_values().collect(),
        resolution,
        asked,
    })
}

/// The value the resolution holds as a setting at `path`.
fn setting<'a>(resolution: &'a Resolution, path: &str) -> Option<&'a Value> {
    resolution
        .settings
        .iter()
        // BORROW: .as_str() compares the path text
        .find(|value| value.path().as_str() == path)
        .and_then(|value| value.value())
        .map(JsonValue::as_value)
}

/// Whether `cell` holds a value at `path` or below it.
fn holds(cell: &Comparable, path: &str) -> bool {
    cell.values().iter().any(|value| {
        // BORROW: .as_str() compares the path text
        let text = value.path().as_str();
        text == path || text.starts_with(&format!("{path}."))
    })
}

/// The paths of a cell's values.
fn paths_of(cell: &Comparable) -> Vec<String> {
    cell.values()
        .iter()
        .map(|value| value.path().to_string())
        .collect()
}

/// One row: a setting written in the file, its value, and where the resolution must show it.
struct Row {
    label: &'static str,
    toml: &'static str,
    path: &'static str,
    value: Value,
}

/// The rows of the table; a location setting reads as a path below `dir`, the file's directory.
fn rows(dir: &Path) -> Vec<Row> {
    vec![
        Row {
            label: "requirements",
            toml: "\"isolation.build.requirements\" = [\"req.txt\"]",
            path: "isolation.build.requirements",
            value: json!([dir.join("req.txt").to_str().expect("UTF-8")]),
        },
        Row {
            label: "prune",
            toml: "\"isolation.build.prune\" = [\"a.py\", \"b.py\"]",
            path: "isolation.build.prune",
            value: json!(["a.py", "b.py"]),
        },
        Row {
            label: "record_packages",
            toml: "\"isolation.record_packages\" = [\"numpy\"]",
            path: "isolation.record_packages",
            value: json!(["numpy"]),
        },
        Row {
            label: "token_secret",
            toml: "\"model.token_secret\" = \"MODEL_TOKEN\"",
            path: "model.token_secret",
            value: json!("MODEL_TOKEN"),
        },
    ]
}

/// Guards: R01 §Agent environment and secrets (the configuration surface) — a setting written in
/// the experiment file's `[fixed]` is a setting of the run and not a value of the cell: the
/// cell holds no path of it, its digest is the digest of the same experiment without it, and the
/// resolution still holds it as a setting that the image step and the provenance step read.
/// Oracle: the hand-written table of rows, each with the setting's value as written, and the
/// control row's digest, the same file with no setting.
/// Bite: drop the `is_setting` filter of the file's cell values in `fresh_cells`.
#[tokio::test]
async fn settings_out_of_cell_fixed_setting_never_reaches_the_cell() {
    let root = tempfile::tempdir().expect("a directory");
    let dir = root.path();
    std::fs::write(dir.join("req.txt"), "numpy\n").expect("a requirements file");
    let control = plan(&file_with("", ""), dir).await;
    assert_eq!(control.cells.len(), 1);
    // INDEX: the control has one cell
    let want = control.cells[0].digest();

    assert_eq!(
        // INDEX: the control has one cell
        paths_of(&control.cells[0]),
        [
            "budget.per_run.wall_s",
            "isolation.env_lock",
            "isolation.image",
            "model.id",
            "prompt.template"
        ],
        "the control keeps every cell value of the file"
    );
    let mut problems: Vec<String> = Vec::new();
    for row in rows(dir) {
        let planned = plan(&file_with(row.toml, ""), dir).await;
        let [cell] = planned.cells.as_slice() else {
            problems.push(format!(
                "{}: {} cells, one expected",
                row.label,
                planned.cells.len()
            ));
            continue;
        };
        if holds(cell, row.path) {
            problems.push(format!(
                "{}: `{}` is a setting and must not be in `comparable.values`: {:?}",
                row.label,
                row.path,
                paths_of(cell)
            ));
        }
        if cell.digest() != want {
            problems.push(format!(
                "{}: the cell digest differs from the control's",
                row.label
            ));
        }
        if setting(&planned.resolution, row.path) != Some(&row.value) {
            problems.push(format!("{}: the resolution lost the setting", row.label));
        }
        let asked = planned.asked.lock().expect("the record");
        let reached = match row.label {
            "prune" => asked.prunes == [vec!["a.py".to_owned(), "b.py".to_owned()]],
            "requirements" => asked.sources == [vec!["req.txt".to_owned()]],
            "record_packages" => record_packages(&planned.resolution) == ["numpy"],
            _ => true,
        };
        if !reached {
            problems.push(format!(
                "{}: the step that reads the setting did not see it",
                row.label
            ));
        }
    }

    // `request.inputs` feeds the `inputs` root by design, so the digest moves with it; the
    // setting itself stays out of the values, which are the control's.
    let declared = json!([{"kind": "alpha", "digest": format!("sha256:{}", "b".repeat(64))}]);
    let toml = format!(
        "\"request.inputs\" = [{{ kind = \"alpha\", digest = \"sha256:{}\" }}]",
        "b".repeat(64)
    );
    let planned = plan(&file_with(toml.as_str(), ""), dir).await;
    let [cell] = planned.cells.as_slice() else {
        panic!("request.inputs: one cell expected");
    };
    if holds(cell, "request.inputs") {
        problems.push(format!(
            "request.inputs: a setting in `comparable.values`: {:?}",
            paths_of(cell)
        ));
    }
    if paths_of(cell) != paths_of(&control.cells[0]) {
        problems.push("request.inputs: the values differ from the control's".to_owned());
    }
    if serde_json::to_value(cell.inputs()).expect("JSON") != declared {
        problems.push("request.inputs: the inputs root is not the declared list".to_owned());
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// Guards: R01 §Agent environment and secrets (the configuration surface) — an experiment that
/// gives a setting as a factor is refused by planning, with an error that names the path, so no
/// level of it is lost; a factor that is no setting (`model.id`, two levels) still plans two
/// cells.
/// Rows: the factor at the setting itself, at its parent table (`isolation.build`, `isolation`,
/// `model`, `budget` filling `budget.max_runs`), a parent table that fills only `model.id`, and
/// `budget` filling only `budget.per_run.turns`.
/// Oracle: the hand-written leaf path in the message, and the levels counted by hand.
/// Bite: remove the refusal at the top of `fresh_cells`, or test the factor's own path only; the two levels of the setting collapse
/// into one cell and no setting reaches the resolution.
#[tokio::test]
async fn settings_out_of_cell_setting_as_factor_is_refused() {
    let root = tempfile::tempdir().expect("a directory");
    let dir = root.path();
    let rows = [
        (
            "isolation.build.prune",
            "[[factors]]\npath = \"isolation.build.prune\"\nlevels = [[\"a.py\"], [\"b.py\"]]\n",
        ),
        (
            "isolation.build.prune",
            "[[factors]]\npath = \"isolation.build\"\nlevels = [{ prune = [\"a.py\"] }, { prune = [\"b.py\"] }]\n",
        ),
        (
            "isolation.build.prune",
            "[[factors]]\npath = \"isolation\"\nlevels = [{ build = { prune = [\"a.py\"] } }, { build = { prune = [\"b.py\"] } }]\n",
        ),
        (
            "model.token_secret",
            "[[factors]]\npath = \"model\"\nlevels = [{ token_secret = \"A\" }, { token_secret = \"B\" }]\n",
        ),
        (
            "budget.max_runs",
            "[[factors]]\npath = \"budget\"\nlevels = [{ max_runs = 1 }, { max_runs = 2 }]\n",
        ),
    ];
    for (leaf, more) in rows {
        let refused = try_plan(&file_with("", more), dir).await.err();
        let Some(error) = refused else {
            panic!("`{leaf}` given as a factor was planned instead of refused: {more}");
        };
        assert!(
            matches!(&error, PlanError::SettingAsFactor { path } if path == leaf),
            "{more}: {error:?}"
        );
        assert!(
            error.to_string().contains(leaf),
            "the message names the path: {error}"
        );
    }

    // `budget` is a setting prefix, but a table level that fills only `budget.per_run.*` cell values
    // varies no setting
    let budget = "[[factors]]\npath = \"budget\"\nlevels = [{ per_run = { turns = 5 } }, { per_run = { turns = 6 } }]\n";
    let text = format!(
        "schema = \"experiment/1\"\nreplicates = 1\n\n{budget}\n[budget.per_run]\nwall_s = 3600\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"htn\"\n"
    );
    // BORROW: .as_str() lends the document to the parser
    let file = ExperimentFile::from_toml_str(text.as_str()).expect("an experiment file");
    assert_eq!(plan(&file, dir).await.cells.len(), 2, "two turn caps");

    // a factor at a parent table whose levels fill no setting still plans one cell per level
    let parent = "[[factors]]\npath = \"model\"\nlevels = [{ id = \"a\" }, { id = \"b\" }]\n";
    let text = format!(
        "schema = \"experiment/1\"\nreplicates = 1\n\n{parent}\n[budget.per_run]\nwall_s = 5\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"htn\"\n"
    );
    // BORROW: .as_str() lends the document to the parser
    let file = ExperimentFile::from_toml_str(text.as_str()).expect("an experiment file");
    assert_eq!(
        plan(&file, dir).await.cells.len(),
        2,
        "model.id over two levels"
    );

    let models = "[[factors]]\npath = \"model.id\"\nlevels = [\"m1\", \"m2\"]\n";
    let text = format!(
        "schema = \"experiment/1\"\nreplicates = 1\n\n{models}\n[budget.per_run]\nwall_s = 5\n\n[harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"htn\"\n"
    );
    // BORROW: .as_str() lends the document to the parser
    let control = ExperimentFile::from_toml_str(text.as_str()).expect("an experiment file");
    assert_eq!(
        plan(&control, dir).await.cells.len(),
        2,
        "two models, two cells"
    );
}
