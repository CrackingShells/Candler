#![cfg(test)]
//! Guards for the instruction texts and hooks of a cell: the experiment can vary them, the cell
//! digest follows them, and the misuses are refused by name when the cells are planned.
//!
//! The values go through the real resolver and `plan_runs`; the expected codes and pointers are
//! written by hand.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, ResolveInput};
use candler_core::resolve::preflight::Resolver;
use candler_core::runner::plan::{Force, PlanCtx, PlanError, PlanMode, plan_runs};
use candler_harness::driver::HarnessDriver;
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_harness_opencode::driver::OpenCodeDriver;
use candler_schemas::execution::ExperimentFile;
use candler_schemas::primitives::Digest;
use candler_schemas::refusal::Refusal;
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::{Value, json};

use crate::support;

/// The experiment text with the factors `factors` (already TOML) and no `[fixed]` value.
fn experiment_with(factors: &str) -> ExperimentFile {
    let text = format!(
        "schema = \"experiment/1\"\nreplicates = 1\n\n{factors}\n[budget.per_run]\nwall_s = 60\n\n\
         [harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"t\"\n"
    );
    ExperimentFile::from_toml_str(&text).expect("an experiment")
}

/// The digests of the cells `file` plans over the resolution of `fixed`, on `driver` when there
/// is one, with both shipped harnesses registered.
async fn planned(
    file: &ExperimentFile,
    fixed: &[(&str, Value)],
    driver: Option<Arc<dyn HarnessDriver>>,
) -> Result<BTreeSet<Digest>, PlanError> {
    let mut all = vec![
        (
            "harness",
            json!({"name": "claude-code", "version": "2.1.311"}),
        ),
        ("treatment", json!({"name": "t"})),
    ];
    all.extend(fixed.iter().map(|(path, value)| (*path, value.clone())));
    let cwd = Path::new("/work");
    let config = Arc::new(Config::new(
        cwd.to_owned(),
        None,
        UserConfig::default(),
        Some(cwd.join("state")),
    ));
    let values = all
        .iter()
        .map(|(path, value)| {
            (
                path.parse().expect("a field path"),
                JsonValue::new(value.clone()).expect("a JSON value"),
            )
        })
        .collect();
    let mut resolution = Resolver::new(config)
        .resolve(ResolveInput::new(Vec::new()).file(FileValues::new(values)))
        .await
        .expect("the fixed values resolve");
    resolution.settings.push(ResolvedValue::recorded(
        "request.treatment".parse().expect("a field path"),
        JsonValue::new(json!({"name": "t"})).expect("a JSON value"),
    ));
    let (_dir, store, lock) = support::run_plan::fresh_store();
    let ctx = PlanCtx {
        store,
        experiment_lock: lock,
        secrets_present: BTreeSet::new(),
        driver,
        harnesses: vec!["claude-code".to_owned(), "opencode".to_owned()],
    };
    let outcome = plan_runs(&ctx, file, &resolution, Force::No, PlanMode::Check)?;
    Ok(outcome.plans.iter().map(|plan| plan.cell.clone()).collect())
}

/// Guards: two levels of `instructions.workspace`, and two levels of the settings of a harness's
/// hooks, plan two cells with two digests; one level plans one.
/// Oracle: the levels are written by hand; the count of digests is the count of cells.
/// Bite: leave the values out of the cell (both levels plan one cell, one digest).
#[tokio::test]
async fn two_levels_of_instructions_or_hooks_plan_two_cells() {
    let levels = [
        ("instructions.workspace", ["\"text A\"", "\"text B\""]),
        (
            "hooks.claude_code.settings",
            ["'{\"SessionStart\": []}'", "'{\"Stop\": []}'"],
        ),
    ];
    for (path, [first, second]) in levels {
        let two = experiment_with(&format!(
            "[[factors]]\npath = \"{path}\"\nlevels = [{first}, {second}]\n"
        ));
        let cells = planned(&two, &[], None).await.expect("two levels plan");
        assert_eq!(cells.len(), 2, "{path}: two levels, two digests");
        let one = experiment_with(&format!(
            "[[factors]]\npath = \"{path}\"\nlevels = [{first}]\n"
        ));
        let single = planned(&one, &[], None).await.expect("one level plans");
        assert_eq!(single.len(), 1, "{path}: one level, one digest");
        assert!(single.is_subset(&cells), "{path}: a level keeps its digest");
    }
}

/// One misuse: the driver the cell meets, the value, and the refusal it earns.
struct Case {
    name: &'static str,
    driver: Arc<dyn HarnessDriver>,
    fixed: (&'static str, Value),
    code: &'static str,
    pointer: Option<&'static str>,
}

/// Guards: a hooks table with no entry for the cell's harness is refused as
/// `experiment.cell_value_missing` at `/fixed/hooks`, a global instruction table with none as the
/// same code at `/fixed/instructions.global`, a key that names no registered harness as
/// `experiment.cell_value_invalid`, and a hook file path with `..` as the same code at the file list.
/// Oracle: the codes and pointers are written by hand from the surface.
/// Bite: skip the reader at plan time (each cell plans).
#[tokio::test]
async fn misused_hooks_and_instructions_are_refused_by_name() {
    let file = experiment_with("");
    let opencode: Arc<dyn HarnessDriver> = Arc::new(OpenCodeDriver::new());
    let claude: Arc<dyn HarnessDriver> = Arc::new(ClaudeCodeDriver::new());
    let case = |name, driver, fixed, code, pointer| Case {
        name,
        driver,
        fixed,
        code,
        pointer,
    };
    let cases = [
        case(
            "hooks of another harness",
            Arc::clone(&opencode),
            ("hooks.claude_code.settings", json!("{}")),
            "experiment.cell_value_missing",
            Some("/fixed/hooks"),
        ),
        case(
            "global text of another harness",
            opencode,
            ("instructions.global.claude_code", json!("text")),
            "experiment.cell_value_missing",
            Some("/fixed/instructions.global"),
        ),
        case(
            "an unregistered harness key",
            Arc::clone(&claude),
            ("hooks.nosuch.settings", json!("{}")),
            "experiment.cell_value_invalid",
            Some("/hooks"),
        ),
        case(
            "a hook file path with `..`",
            claude,
            (
                "hooks.claude_code.files",
                json!([{"path": "../evil.sh", "text": "x"}]),
            ),
            "experiment.cell_value_invalid",
            Some("/hooks/claude_code/files"),
        ),
    ];
    for Case {
        name,
        driver,
        fixed,
        code,
        pointer,
    } in cases
    {
        match planned(&file, &[fixed], Some(driver)).await {
            Err(error) => {
                assert_eq!(Refusal::code(&error).as_str(), code, "{name}");
                assert_eq!(Refusal::path(&error).as_deref(), pointer, "{name}");
            }
            Ok(_) => panic!("{name}: planned"),
        }
    }
}
