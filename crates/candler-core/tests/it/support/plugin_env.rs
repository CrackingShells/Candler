//! The route an experiment takes to a cell, for the guards that start from the `agent-env`
//! fixture: the project configuration names the fixture's plug-in directory, the experiment
//! selects its environment, and the production resolver with the plug-in environment step turns
//! both into the values of the cell.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use candler_core::resolve::config::{Config, ConfigEnv};
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::Resolver;
use candler_core::resolve::steps::plugin_environment::PluginEnvironmentStep;
use candler_schemas::primitives::FieldPath;
use candler_schemas::values::JsonValue;
use serde_json::json;

/// The environment of the fixture plug-in that an experiment selects.
pub const SELECTOR: &str = "agent-env/tiny";

/// The fixture plug-in's directory.
pub fn agent_env_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugins/agent-env")
}

/// Resolves an experiment that selects `selector` (none: an experiment without a plug-in) in
/// the project directory `project`, which holds the configuration naming the fixture plug-in.
pub async fn resolve_experiment(project: &Path, selector: Option<&str>) -> Resolution {
    std::fs::create_dir_all(project).expect("a project directory");
    std::fs::write(
        project.join("candler.toml"),
        format!(
            "[plugins]\ndirs = [{:?}]\n",
            agent_env_dir().display().to_string()
        ),
    )
    .expect("the project config");
    let config = Arc::new(Config::load(project, &ConfigEnv::default()).expect("a config"));
    let mut values = vec![
        ("model.id", json!("claude-haiku-4-5")),
        ("permission_mode", json!("bypassPermissions")),
        ("prompt.template", json!("go")),
        ("prompt.values", json!({})),
    ];
    if let Some(selector) = selector {
        values.push(("isolation.build.plugin", json!(selector)));
    }
    let layer = FileValues::new(
        values
            .into_iter()
            .map(|(path, value)| {
                (
                    path.parse::<FieldPath>().expect("a field path"),
                    JsonValue::new(value).expect("a JSON value"),
                )
            })
            .collect(),
    )
    .relative_to(project)
    .expect("absolute paths");
    Resolver::new(config)
        .register(Box::new(PluginEnvironmentStep::new().expect("a step")))
        .expect("registers")
        .resolve(ResolveInput::default().file(layer))
        .await
        .expect("the experiment resolves")
}
