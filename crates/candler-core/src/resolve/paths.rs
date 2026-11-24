//! The configuration path vocabulary: every path an experiment, a `--set` or a configuration file
//! names, with what it is, in one table that the resolver, the request-key check, the typed
//! configuration readers and the contract pages all read.

use candler_harness::driver::{DriverError, HarnessDriver};
use candler_harness::mcp;
use candler_harness::spec::{AgentSetting, Answer, Cell, EffortLevel, PermissionMode};
use candler_schemas::execution::paths::{
    BUDGET_PER_RUN_MAX_COST_USD, BUDGET_PER_RUN_TURNS, BUDGET_PER_RUN_WALL_S, ENV, ENVIRONMENTS,
    HARNESS, INPUTS, ISOLATION_BUILD_CONTEXT, ISOLATION_BUILD_DOCKERFILE, ISOLATION_BUILD_LOCK,
    ISOLATION_BUILD_PLUGIN, ISOLATION_BUILD_PRUNE, ISOLATION_BUILD_REQUIREMENTS, ISOLATION_EGRESS,
    ISOLATION_ENV_LOCK, ISOLATION_IMAGE, ISOLATION_LIMITS, ISOLATION_LIMITS_CPUS,
    ISOLATION_LIMITS_MEMORY, ISOLATION_LIMITS_PIDS, ISOLATION_NETWORK, ISOLATION_RECORD_PACKAGES,
    MCP, MODEL_EFFORT, MODEL_ENDPOINT, MODEL_ID, MODEL_TOKEN_SECRET, PERMISSION_MODE, PLUGINS_DIRS,
    PROMPT_TEMPLATE, PROMPT_VALUES, REQUEST_HARNESS, REQUEST_INPUTS, REQUEST_TREATMENT,
    RUNNER_IDLE_S, RUNNER_MODE, SLOTS_COUNT, SLOTS_EVALUATORS, SNAPSHOTS, STORE_ROOT,
    TOOLS_ALLOWED, TOOLS_DISALLOWED, TREATMENT, TREATMENT_SOURCE_PREFIX,
};
use candler_schemas::primitives::FieldPath;
use candler_schemas::servers::McpServerKind;

const BUDGET: &str = "budget";
const BUDGET_PER_RUN: &str = "budget.per_run";
const REPLICATES: &str = "replicates";
const ISOLATION_BUILD: &str = "isolation.build";
const GATEWAY_UPSTREAM_CA: &str = "gateway.upstream_ca";

/// How a row names paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// The one path.
    Leaf,
    /// The path and every path below it.
    Subtree,
    /// Every path strictly below it.
    Below,
    /// The path and everything below it, which no enclosing row of a family reaches.
    Carveout,
    /// Text that is not a field path: a form of a value, or a key of a configuration file only.
    NotAPath,
}

/// What a path is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Class {
    /// A value of the experiment; it reaches the cell digest.
    Value,
    /// An operational setting; it never reaches a cell digest.
    Setting,
    /// A setting that `--set` may name under `request`.
    Request,
    /// A key of a configuration file; as a path it is an operational setting.
    ConfigKey,
}

impl Class {
    /// Whether a path of this class is an operational setting.
    #[must_use]
    pub const fn is_setting(self) -> bool {
        !matches!(self, Self::Value)
    }

    /// The word the contract page prints.
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::Value => "experiment value",
            Self::Setting => "setting",
            Self::Request => "request key",
            Self::ConfigKey => "configuration key",
        }
    }
}

/// The configuration file a key belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigFile {
    /// No file: the path is written in an experiment or given to `--set`.
    None,
    /// `candler.toml`, found at or above the working directory.
    Project,
    /// The user configuration, `config.toml` under the candler directory of the configuration home.
    User,
}

/// Whether a path reaches the agent.
#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::exhaustive_enums,
    reason = "EXHAUSTIVE: a row states one or the other; a reader must handle both."
)]
pub enum Reach {
    /// The path is read by the harness's driver, as the keys listed (one value of each key the
    /// path carries).
    Agent(&'static [AgentSetting]),
    /// Candler reads the path itself; no driver sees it.
    CandlerOnly,
}

/// The agent settings no configuration path reaches, each with where it comes from.
pub const NO_PATH: [(AgentSetting, &str); 1] = [(
    AgentSetting::Skills,
    "the skills are the treatment's skill tree, not a configuration path",
)];

/// One row of [`CONFIGURATION_PATHS`].
#[derive(Debug, Clone, Copy)]
pub struct PathRow {
    /// The path text.
    pub path: &'static str,
    /// Which paths the row names.
    pub kind: Kind,
    /// What the path is.
    pub class: Class,
    /// Whether the path reaches the agent, and as which keys.
    pub reach: Reach,
    /// The configuration file the key belongs to, for a configuration key.
    pub file: ConfigFile,
    /// The one line the contract page prints.
    pub meaning: &'static str,
}

const fn row(
    path: &'static str,
    kind: Kind,
    class: Class,
    reach: Reach,
    meaning: &'static str,
) -> PathRow {
    PathRow {
        path,
        kind,
        class,
        reach,
        file: ConfigFile::None,
        meaning,
    }
}

const fn key(path: &'static str, kind: Kind, file: ConfigFile, meaning: &'static str) -> PathRow {
    PathRow {
        path,
        kind,
        class: Class::ConfigKey,
        reach: Reach::CandlerOnly,
        file,
        meaning,
    }
}

/// Every path the configuration surface names.
pub const CONFIGURATION_PATHS: [PathRow; 49] = [
    row(
        MODEL_ID,
        Kind::Leaf,
        Class::Value,
        Reach::Agent(&[AgentSetting::ModelId]),
        "The model the harness runs.",
    ),
    row(
        MODEL_ENDPOINT,
        Kind::Leaf,
        Class::Value,
        Reach::Agent(&[AgentSetting::ModelEndpoint]),
        "The endpoint the gateway forwards model requests to.",
    ),
    row(
        MODEL_EFFORT,
        Kind::Leaf,
        Class::Value,
        Reach::Agent(&[AgentSetting::Effort(EffortLevel::Low)]),
        "The effort level the harness asks of the model.",
    ),
    row(
        MODEL_TOKEN_SECRET,
        Kind::Leaf,
        Class::Setting,
        Reach::CandlerOnly,
        "The name of the secret that holds the gateway's token.",
    ),
    row(
        TOOLS_ALLOWED,
        Kind::Leaf,
        Class::Value,
        Reach::Agent(&[AgentSetting::ToolsAllowed]),
        "The tools the harness may use.",
    ),
    row(
        TOOLS_DISALLOWED,
        Kind::Leaf,
        Class::Value,
        Reach::Agent(&[AgentSetting::ToolsDisallowed]),
        "The tools the harness may not use.",
    ),
    row(
        ENV,
        Kind::Leaf,
        Class::Value,
        Reach::Agent(&[AgentSetting::Env]),
        "The environment of the harness process: a table of variable name to text.",
    ),
    row(
        MCP,
        Kind::Subtree,
        Class::Value,
        Reach::Agent(&[
            AgentSetting::McpTransport(McpServerKind::Stdio),
            AgentSetting::Codemode(true),
            AgentSetting::McpName,
        ]),
        "The MCP servers of a run; the names below are free.",
    ),
    row(
        PERMISSION_MODE,
        Kind::Leaf,
        Class::Value,
        Reach::Agent(&[AgentSetting::PermissionMode(PermissionMode::Default)]),
        "The permission mode the harness runs in.",
    ),
    row(
        REQUEST_HARNESS,
        Kind::Leaf,
        Class::Request,
        Reach::CandlerOnly,
        "The harness the run uses.",
    ),
    row(
        REQUEST_TREATMENT,
        Kind::Leaf,
        Class::Request,
        Reach::CandlerOnly,
        "The treatment the run applies.",
    ),
    row(
        REQUEST_INPUTS,
        Kind::Leaf,
        Class::Request,
        Reach::CandlerOnly,
        "The inputs the run is given.",
    ),
    row(
        PROMPT_TEMPLATE,
        Kind::Leaf,
        Class::Value,
        Reach::CandlerOnly,
        "The prompt template.",
    ),
    row(
        PROMPT_VALUES,
        Kind::Subtree,
        Class::Value,
        Reach::CandlerOnly,
        "The values the prompt template fills in; the names below are free.",
    ),
    row(
        ISOLATION_IMAGE,
        Kind::Leaf,
        Class::Value,
        Reach::CandlerOnly,
        "The container image a run starts in.",
    ),
    row(
        ISOLATION_NETWORK,
        Kind::Leaf,
        Class::Value,
        Reach::CandlerOnly,
        "The network mode of the container.",
    ),
    row(
        ISOLATION_ENV_LOCK,
        Kind::Leaf,
        Class::Value,
        Reach::CandlerOnly,
        "The environment lock the image must match.",
    ),
    row(
        ISOLATION_LIMITS,
        Kind::Leaf,
        Class::Value,
        Reach::CandlerOnly,
        "The resource limits of the container; the limits are below.",
    ),
    row(
        ISOLATION_LIMITS_PIDS,
        Kind::Leaf,
        Class::Value,
        Reach::CandlerOnly,
        "The process limit of the container.",
    ),
    row(
        ISOLATION_LIMITS_MEMORY,
        Kind::Leaf,
        Class::Value,
        Reach::CandlerOnly,
        "The memory limit of the container.",
    ),
    row(
        ISOLATION_LIMITS_CPUS,
        Kind::Leaf,
        Class::Value,
        Reach::CandlerOnly,
        "The CPU limit of the container.",
    ),
    row(
        ISOLATION_EGRESS,
        Kind::Leaf,
        Class::Value,
        Reach::CandlerOnly,
        "The hosts the container may reach.",
    ),
    row(
        ISOLATION_RECORD_PACKAGES,
        Kind::Leaf,
        Class::Setting,
        Reach::CandlerOnly,
        "Whether a run records the packages of its image.",
    ),
    row(
        ISOLATION_BUILD,
        Kind::Below,
        Class::Setting,
        Reach::CandlerOnly,
        "Every path below it is a request to build an image.",
    ),
    row(
        ISOLATION_BUILD_PLUGIN,
        Kind::Leaf,
        Class::Setting,
        Reach::CandlerOnly,
        "The image build plug-in.",
    ),
    row(
        ISOLATION_BUILD_DOCKERFILE,
        Kind::Leaf,
        Class::Setting,
        Reach::CandlerOnly,
        "The Dockerfile of an image build.",
    ),
    row(
        ISOLATION_BUILD_CONTEXT,
        Kind::Leaf,
        Class::Setting,
        Reach::CandlerOnly,
        "The build context directory of an image build.",
    ),
    row(
        ISOLATION_BUILD_REQUIREMENTS,
        Kind::Leaf,
        Class::Setting,
        Reach::CandlerOnly,
        "The requirements file of an image build.",
    ),
    row(
        ISOLATION_BUILD_PRUNE,
        Kind::Leaf,
        Class::Setting,
        Reach::CandlerOnly,
        "Whether an image build prunes what it leaves.",
    ),
    row(
        ISOLATION_BUILD_LOCK,
        Kind::Leaf,
        Class::Setting,
        Reach::CandlerOnly,
        "The lock file of an image build.",
    ),
    row(
        BUDGET,
        Kind::Subtree,
        Class::Setting,
        Reach::CandlerOnly,
        "A job cap: every path below it, except the per-run caps.",
    ),
    row(
        BUDGET_PER_RUN,
        Kind::Carveout,
        Class::Value,
        Reach::CandlerOnly,
        "The per-run caps are experiment values.",
    ),
    row(
        BUDGET_PER_RUN_WALL_S,
        Kind::Leaf,
        Class::Value,
        Reach::Agent(&[AgentSetting::WallBudget]),
        "The wall-clock cap of one run, in seconds.",
    ),
    row(
        BUDGET_PER_RUN_TURNS,
        Kind::Leaf,
        Class::Value,
        Reach::Agent(&[AgentSetting::TurnBudget]),
        "The turn cap of one run.",
    ),
    row(
        BUDGET_PER_RUN_MAX_COST_USD,
        Kind::Leaf,
        Class::Value,
        Reach::Agent(&[AgentSetting::CostBudget]),
        "The cost cap of one run, in US dollars.",
    ),
    row(
        REPLICATES,
        Kind::Subtree,
        Class::Setting,
        Reach::CandlerOnly,
        "How many runs each cell gets.",
    ),
    row(
        RUNNER_MODE,
        Kind::Leaf,
        Class::Value,
        Reach::Agent(&[AgentSetting::SessionMode]),
        "`headless` or `session`.",
    ),
    row(
        RUNNER_IDLE_S,
        Kind::Leaf,
        Class::Value,
        Reach::CandlerOnly,
        "The idle time that ends a session, in whole seconds, or `none`.",
    ),
    row(
        HARNESS,
        Kind::Subtree,
        Class::Value,
        Reach::CandlerOnly,
        "The harness table; the names below belong to the harness.",
    ),
    row(
        TREATMENT,
        Kind::Subtree,
        Class::Value,
        Reach::CandlerOnly,
        "The treatment table.",
    ),
    row(
        INPUTS,
        Kind::Subtree,
        Class::Value,
        Reach::Agent(&[AgentSetting::Inputs]),
        "The inputs table.",
    ),
    row(
        SNAPSHOTS,
        Kind::Subtree,
        Class::Value,
        Reach::CandlerOnly,
        "The snapshots table.",
    ),
    row(
        ENVIRONMENTS,
        Kind::Subtree,
        Class::Value,
        Reach::CandlerOnly,
        "The environments table.",
    ),
    row(
        TREATMENT_SOURCE_PREFIX,
        Kind::NotAPath,
        Class::Value,
        Reach::CandlerOnly,
        "The prefix of a value that names a treatment source; the rest is the path.",
    ),
    key(
        STORE_ROOT,
        Kind::Leaf,
        ConfigFile::Project,
        "The store root; a relative path is read from the project root.",
    ),
    key(
        PLUGINS_DIRS,
        Kind::Leaf,
        ConfigFile::Project,
        "The directories that hold plug-ins; relative paths are read from the project root.",
    ),
    key(
        SLOTS_COUNT,
        Kind::Leaf,
        ConfigFile::User,
        "How many run slots the machine offers.",
    ),
    key(
        SLOTS_EVALUATORS,
        Kind::Leaf,
        ConfigFile::User,
        "How many of the slots evaluators may use.",
    ),
    key(
        GATEWAY_UPSTREAM_CA,
        Kind::NotAPath,
        ConfigFile::User,
        "A PEM file whose certificates the gateway trusts for its upstream.",
    ),
];

/// The paths of the request class, in table order; `N` is how many the table has.
///
/// # Panics
/// Fails to compile, when used in a constant, if the table has not `N` request keys.
#[must_use]
#[expect(
    clippy::indexing_slicing,
    reason = "an index past `N` is the compile-time failure that says the count is wrong"
)]
pub const fn request_keys<const N: usize>() -> [&'static str; N] {
    let mut out = [""; N];
    let mut found = 0;
    let mut rest: &[PathRow] = &CONFIGURATION_PATHS;
    while let [row, tail @ ..] = rest {
        if matches!(row.class, Class::Request) {
            out[found] = row.path;
            found += 1;
        }
        rest = tail;
    }
    assert!(found == N, "the table has another number of request keys");
    out
}

/// The table's field path of a row; `None` for text that is not one.
fn field_path(row: &PathRow) -> Option<FieldPath> {
    if matches!(row.kind, Kind::NotAPath) {
        return None;
    }
    row.path.parse().ok()
}

/// Reports whether the row names `path`.
fn covers(kind: Kind, root: &FieldPath, path: &FieldPath) -> bool {
    match kind {
        Kind::Leaf => path.as_str() == root.as_str(),
        Kind::Subtree | Kind::Carveout => path.starts_with(root),
        Kind::Below => path.starts_with(root) && path.as_str() != root.as_str(),
        Kind::NotAPath => false,
    }
}

/// A text no variant of `T` names, refused at `path` with the accepted spellings.
fn unnamed(path: &FieldPath, text: &str, accepted: &[&str]) -> DriverError {
    DriverError::InvalidValue {
        path: path.clone(), // BORROW: the error owns the path
        detail: format!(
            "is `{text}`; the accepted texts are {}",
            accepted.join(", ")
        ),
    }
}

/// The effort level the cell holds at `path`, refusing text that names no level. The cell keeps
/// the text; [`Cell::effort`] maps the text checked here.
fn effort_at(cell: &Cell<'_>, path: &FieldPath) -> Result<Option<EffortLevel>, DriverError> {
    cell.text(path)?
        .map(|text| {
            EffortLevel::from_text(text)
                .ok_or_else(|| unnamed(path, text, &EffortLevel::ALL.map(EffortLevel::as_str)))
        })
        .transpose()
}

/// The permission mode the cell holds at `path`, refusing text that names no mode. The cell keeps
/// the text; [`Cell::permission_mode`] maps the text checked here. A cell that names none runs in Claude Code's own mode when no flag is
/// given, [`PermissionMode::Default`], and the driver is asked about that mode like any other.
fn mode_at(cell: &Cell<'_>, path: &FieldPath) -> Result<PermissionMode, DriverError> {
    cell.text(path)?
        .map_or(Ok(PermissionMode::Default), |text| {
            PermissionMode::from_text(text).ok_or_else(|| {
                unnamed(path, text, &PermissionMode::ALL.map(PermissionMode::as_str))
            })
        })
}

/// The settings `cell` sets, each with the path of the row that reaches it and with the value the
/// cell gives it, in the order of [`CONFIGURATION_PATHS`]. The rows are the only place a path
/// meets a setting. The skills and the session mode are not read here: the treatment holds the
/// first, and the plan's session refuses the second.
///
/// # Errors
/// [`DriverError::InvalidValue`] when the effort or the permission mode is text no level or mode
/// names, or the value is not text, and the error of the reader of `mcp.servers`.
pub fn settings_set_by(cell: &Cell<'_>) -> Result<Vec<(AgentSetting, FieldPath)>, DriverError> {
    let mut set: Vec<(AgentSetting, FieldPath)> = Vec::new();
    for row in &CONFIGURATION_PATHS {
        let Reach::Agent(families) = row.reach else {
            continue;
        };
        let Some(path) = field_path(row) else {
            continue;
        };
        if cell.below(&path).is_empty() && row.path != PERMISSION_MODE {
            continue;
        }
        let mut found: Vec<AgentSetting> = Vec::new();
        for family in families {
            match family {
                AgentSetting::Effort(_) => {
                    found.extend(effort_at(cell, &path)?.map(AgentSetting::Effort));
                }
                AgentSetting::PermissionMode(_) => {
                    found.push(AgentSetting::PermissionMode(mode_at(cell, &path)?));
                }
                // The three keys of a server are read together, from the transport's entry.
                AgentSetting::McpTransport(_) => {
                    for server in mcp::servers(cell)?.unwrap_or_default().into_values() {
                        found.push(AgentSetting::McpTransport(mcp::kind(&server)));
                        found.extend(server.codemode.map(AgentSetting::Codemode));
                        if server.name.is_some() {
                            found.push(AgentSetting::McpName);
                        }
                    }
                }
                AgentSetting::Codemode(_)
                | AgentSetting::McpName
                | AgentSetting::Skills
                | AgentSetting::SessionMode => {}
                AgentSetting::ModelId
                | AgentSetting::ModelEndpoint
                | AgentSetting::ToolsAllowed
                | AgentSetting::ToolsDisallowed
                | AgentSetting::Env
                | AgentSetting::Inputs
                | AgentSetting::WallBudget
                | AgentSetting::TurnBudget
                | AgentSetting::CostBudget => found.push(*family),
            }
        }
        for setting in found {
            if !set.iter().any(|(held, _)| *held == setting) {
                set.push((setting, path.clone())); // BORROW: each setting keeps its row's path
            }
        }
    }
    Ok(set)
}

/// Refuses a cell that sets something the driver answers [`Answer::Refused`], with the
/// [`DriverError::Unsupported`] that names the setting, the harness and the answer's reason. The
/// one place a refused setting is refused, before the driver plans.
///
/// # Errors
/// [`DriverError::Unsupported`] for the first set setting the driver refuses, and the error of
/// [`settings_set_by`] when a value of the cell cannot be read.
pub fn refuse_unanswered(
    driver: &dyn HarnessDriver, // TRAIT_OBJECT: the check reads any driver
    cell: &Cell<'_>,
) -> Result<(), DriverError> {
    for (setting, path) in settings_set_by(cell)? {
        if let Answer::Refused { reason } = driver.answer(setting) {
            return Err(DriverError::unsupported(
                &path,
                driver.id().to_string().as_str(), // BORROW: .to_string() gives owned text
                reason,
            ));
        }
    }
    Ok(())
}

/// Reports whether the row with the most segments that names `path` is of a setting class.
#[must_use]
pub fn names_a_setting(path: &FieldPath) -> bool {
    let mut best: Option<(usize, bool)> = None;
    for row in &CONFIGURATION_PATHS {
        let Some(root) = field_path(row) else {
            continue;
        };
        if !covers(row.kind, &root, path) {
            continue;
        }
        let depth = root.segments().count();
        if best.is_none_or(|(deepest, _)| depth > deepest) {
            best = Some((depth, row.class.is_setting()));
        }
    }
    best.is_some_and(|(_, setting)| setting)
}

/// Reports whether a row reaches `path`: names it, lies below it, or is an ancestor of a path
/// it names.
#[must_use]
pub fn reaches(path: &FieldPath) -> bool {
    CONFIGURATION_PATHS.iter().any(|row| {
        let Some(root) = field_path(row) else {
            return false;
        };
        match row.kind {
            Kind::Leaf => root.starts_with(path),
            Kind::Subtree | Kind::Below => {
                (path.starts_with(&root) && !row.class.is_setting()) || root.starts_with(path)
            }
            Kind::Carveout | Kind::NotAPath => false,
        }
    })
}

/// The tables and keys the rows give to `file`, as `(table, key)` pairs.
pub fn config_keys(file: ConfigFile) -> impl Iterator<Item = (&'static str, &'static str)> {
    CONFIGURATION_PATHS
        .iter()
        .filter(move |row| row.file == file)
        .filter_map(|row| row.path.split_once('.'))
}

#[cfg(test)]
mod tests {
    use crate::resolve::layers::is_known_path;

    /// Guards: a path below the per-run caps that no row names is not a known path, though the
    /// `budget` family encloses it.
    /// Oracle: the per-run caps are the three rows `budget.per_run.*`; `budget.per_run.wall` is
    /// a misspelling of one.
    #[test]
    fn a_misspelt_per_run_cap_is_not_known() {
        let known = |text: &str| is_known_path(&text.parse().expect("a field path"));
        assert!(known("budget.per_run.wall_s"));
        assert!(known("budget.per_run"));
        assert!(known("budget.total.usd"));
        assert!(!known("budget.per_run.wall"));
    }
}
