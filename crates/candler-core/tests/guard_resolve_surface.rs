#![cfg(test)]
//! Guards for the classification of the configuration surface and for the split of the budget.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use candler_core::experiment::budget::to_caps;
use candler_core::experiment::surface::{Surface, SurfaceError};
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::{
    PreflightError, PreflightStep, ResolveContext, Resolver, StepReport,
};
use candler_schemas::execution::{Budget, Comparable};
use candler_schemas::primitives::{FieldPath, NameToken};
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;
use serde_json::{Value, json};

fn fp(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

fn js(value: Value) -> JsonValue {
    JsonValue::new(value).expect("a JSON value")
}

/// Derives the five typed roots, as the preflight steps do; `flip` names the one root whose value
/// differs from the base.
struct Roots {
    name: NameToken,
    flip: Option<&'static str>,
}

impl PreflightStep for Roots {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        _ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        let flip = self.flip;
        let digit = move |root: &str| if flip == Some(root) { 'b' } else { 'a' };
        let digest = move |root: &str| format!("sha256:{}", digit(root).to_string().repeat(64));
        let derived = [
            (
                "harness",
                json!({"name": "claude-code", "version": if flip == Some("harness") {"2.0.1"} else {"2.0.0"}}),
            ),
            (
                "treatment",
                json!({"name": if flip == Some("treatment") {"t-b"} else {"t-a"}}),
            ),
            (
                "inputs",
                json!([{"kind": "build_inputs", "digest": digest("inputs")}]),
            ),
            (
                "snapshots",
                json!([{"kind": "catalog", "digest": digest("snapshots")}]),
            ),
            (
                "environments",
                json!([{"plugin": "p@1.0.0", "lock_digest": digest("environments")}]),
            ),
        ];
        // BORROW: `.clone()` gives the report its own step name
        let mut report = StepReport::new(self.name.clone());
        for (path, value) in derived {
            report = report.derive(fp(path), js(value));
        }
        Box::pin(async move { Ok(report) })
    }
}

fn config(cwd: &Path) -> Arc<Config> {
    Arc::new(Config::new(
        // BORROW: `.to_owned()` makes the owned working directory
        cwd.to_owned(),
        None,
        UserConfig::default(),
        Some(cwd.join("state")),
    ))
}

/// Resolves the file values with the typed roots derived, as the registered steps derive them.
async fn resolve(file: &[(&str, Value)], flip: Option<&'static str>) -> Resolution {
    let cwd = PathBuf::from("/work");
    let step = Roots {
        name: "roots@1".parse().expect("a name@token"),
        flip,
    };
    let values = file
        .iter()
        // BORROW: `.clone()` gives the file its own value
        .map(|(path, value)| (fp(path), js(value.clone())))
        .collect();
    Resolver::new(config(&cwd))
        .register(Box::new(step))
        .expect("registers")
        .resolve(ResolveInput::default().file(FileValues::new(values)))
        .await
        .expect("resolves")
}

fn comparable(
    resolution: &Resolution,
) -> Result<Comparable, candler_schemas::execution::ExecutionError> {
    let settings: Vec<FieldPath> = resolution
        .settings
        .iter()
        // BORROW: `.clone()` makes the owned setting path
        .map(|setting| setting.path().clone())
        .collect();
    Comparable::from_resolution(&resolution.values, &resolution.sources, &settings)
        .map(|(cell, _)| cell)
}

fn held(cell: &Comparable, path: &str) -> bool {
    cell.values()
        .iter()
        // BORROW: `.as_str()` compares the path text
        .any(|value| value.path().as_str() == path)
}

/// One row of `field_paths.toml`.
struct Row {
    path: String,
    class: String,
    wildcard: bool,
}

fn rows() -> Vec<Row> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle/vectors/field_paths.toml");
    let text = std::fs::read_to_string(file).expect("field_paths.toml is committed");
    let table: toml::Table = text.parse().expect("field_paths.toml is TOML");
    table
        .get("path")
        .and_then(toml::Value::as_array)
        .expect("a [[path]] array")
        .iter()
        .map(|row| {
            let text = |key: &str| {
                row.get(key)
                    .and_then(toml::Value::as_str)
                    .expect("a text key")
                    // BORROW: `.to_owned()` makes the owned text
                    .to_owned()
            };
            Row {
                path: text("path"),
                class: text("class"),
                wildcard: row
                    .get("wildcard")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false),
            }
        })
        .collect()
}

/// The concrete paths a row stands for: a wildcard row names members chosen by hand here.
fn members(row: &Row) -> Vec<String> {
    if !row.wildcard {
        // BORROW: `.to_owned()` makes the owned path
        return vec![row.path.trim_end_matches("[]").to_owned()];
    }
    // BORROW: `.as_str()` matches the row text
    match row.path.as_str() {
        // BORROW: `.to_owned()` makes the owned path
        "mcp.*" => vec!["mcp.filesystem".to_owned()],
        "isolation.limits.*" => vec![
            // BORROW: `.to_owned()` makes the owned path
            "isolation.limits.pids".to_owned(),
            // BORROW: `.to_owned()` makes the owned path
            "isolation.limits.cpus".to_owned(),
        ],
        other => panic!("no members chosen for the wildcard row {other}"),
    }
}

/// Guards: I7, I12 — the surface classification: a cell-value path moves the cell digest and
/// is a cell value; a setting path leaves the digest alone and is a setting; a typed root is
/// filled by a derived value, moves the digest through its typed field, and takes no value below
/// it; no value that holds an absolute path enters a cell.
/// Oracle: `field_paths.toml`, hand-written from R03 section 11 and the rulings, never from
/// `experiment::paths`.
/// Bite: classify `isolation.limits.memory` as a setting in `resolve/layers.rs`.
#[tokio::test]
async fn b19_every_path_class() {
    let rows = rows();
    let count = |class: &str| rows.iter().filter(|row| row.class == class).count();
    // The rows of the file as written: a file that reads as empty must not pass.
    assert_eq!(
        (count("cell_value"), count("setting"), count("typed_root")),
        (17, 19, 5)
    );
    let roots = [
        "harness",
        "treatment",
        "inputs",
        "snapshots",
        "environments",
    ];

    for row in &rows {
        for path in members(row) {
            // BORROW: `.as_str()` lends the path text
            let at = |value: &str| [(path.as_str(), json!(value))];
            // BORROW: `.as_str()` matches the class text
            match row.class.as_str() {
                "cell_value" => {
                    let one = resolve(&at("a"), None).await;
                    let other = resolve(&at("b"), None).await;
                    let (one, other) = (
                        comparable(&one).expect("a cell"),
                        comparable(&other).expect("a cell"),
                    );
                    assert_ne!(one.digest(), other.digest(), "{path} moves the digest");
                    assert!(held(&one, &path), "{path} is a value of the cell");
                    // No value that holds an absolute path enters a cell.
                    let absolute = resolve(&at("/etc/passwd"), None).await;
                    assert!(
                        comparable(&absolute).is_err(),
                        "{path} refuses an absolute path"
                    );
                }
                "setting" => {
                    let one = resolve(&at("a"), None).await;
                    let other = resolve(&at("b"), None).await;
                    assert!(
                        // BORROW: `.as_str()` compares the path text
                        one.settings.iter().any(|s| s.path().as_str() == path),
                        "{path} resolves as a setting"
                    );
                    let (cell, twin) = (
                        comparable(&one).expect("a cell"),
                        comparable(&other).expect("a cell"),
                    );
                    assert_eq!(
                        cell.digest(),
                        twin.digest(),
                        "{path} leaves the digest alone"
                    );
                    assert!(!held(&cell, &path), "{path} is not a value of the cell");
                }
                "typed_root" => {
                    // BORROW: `.as_str()` lends the root text
                    let root = path.as_str();
                    assert!(roots.contains(&root), "{root} is one of the five roots");
                    let base = resolve(&[], None).await;
                    let flipped = resolve(&[], roots.iter().copied().find(|r| *r == root)).await;
                    let (base, flipped) = (
                        comparable(&base).expect("a cell"),
                        comparable(&flipped).expect("a cell"),
                    );
                    assert_ne!(base.digest(), flipped.digest(), "{root} moves the digest");
                    let typed = match root {
                        "harness" => base.harness().version.to_string() == "2.0.0",
                        // BORROW: `.as_str()` compares the name text
                        "treatment" => base.treatment().name.as_str() == "t-a",
                        "inputs" => base.inputs().len() == 1,
                        "snapshots" => base.snapshots().len() == 1,
                        "environments" => base.environments().len() == 1,
                        other => panic!("{other} is not a root"),
                    };
                    assert!(typed, "{root} is carried by its typed field");
                    assert!(
                        base.values()
                            .iter()
                            .all(|v| v.path().segments().next() != Some(root)),
                        "no value sits at or below {root}"
                    );
                    // A value below the root is refused, by the cell and by the surface.
                    let child = format!("{root}.child");
                    // BORROW: `.as_str()` lends the path text
                    let below = resolve(&[(child.as_str(), json!("x"))], None).await;
                    assert!(
                        comparable(&below).is_err(),
                        "{child} is refused by the cell"
                    );
                    assert!(
                        matches!(
                            Surface::from_values(&below.values),
                            Err(SurfaceError::ReservedRoot { .. })
                        ),
                        "{child} is refused by the surface"
                    );
                }
                other => panic!("unknown class {other}"),
            }
        }
    }
}

/// Guards: I20 — the budget split: `budget.per_run.*` moves the cell digest, and the caps of the
/// job (`budget.max_runs`, `max_parallel`, `max_tokens`, `max_cost_usd`, `wall_s`) never do; only
/// the per-run caps become agent flags, and the container's expiry is the wall cap plus 60 s.
/// Oracle: R03 section 10.4 formula, written out as literals below (3600 s + 60 s = 3660 s).
/// Bite: read the job cap `budget.max_cost_usd` in `to_caps` instead of `per_run.max_cost_usd`.
#[tokio::test]
async fn b20_budget_split_and_expiry() {
    for path in [
        "budget.per_run.wall_s",
        "budget.per_run.turns",
        "budget.per_run.max_cost_usd",
    ] {
        let one = comparable(&resolve(&[(path, json!(1))], None).await).expect("a cell");
        let other = comparable(&resolve(&[(path, json!(2))], None).await).expect("a cell");
        assert_ne!(one.digest(), other.digest(), "{path} moves the digest");
    }
    for path in [
        "budget.max_runs",
        "budget.max_parallel",
        "budget.max_tokens",
        "budget.max_cost_usd",
        "budget.wall_s",
    ] {
        let one = comparable(&resolve(&[(path, json!(1))], None).await).expect("a cell");
        let other = comparable(&resolve(&[(path, json!(2))], None).await).expect("a cell");
        assert_eq!(
            one.digest(),
            other.digest(),
            "{path} leaves the digest alone"
        );
    }

    let budget: Budget = serde_json::from_value(json!({
        "per_run": {"wall_s": 3600, "turns": 7, "max_cost_usd": 0.5},
        "max_runs": 3,
        "max_cost_usd": 99.0,
        "wall_s": 86400,
    }))
    .expect("a budget");
    let caps = to_caps(&budget);
    assert_eq!(
        caps.flags(),
        ["--max-turns", "7", "--max-budget-usd", "0.5"]
    );
    // R03 10.4: the supervisor's deadline is wall_s; the container expires 60 s later.
    assert_eq!(caps.expiry().map(|expiry| expiry.as_secs()), Some(3660));
}
