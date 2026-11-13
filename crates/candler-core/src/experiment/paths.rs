//! The field paths of the configuration surface, as text. A path is a cell value (hashed and
//! compared), a setting (dropped from the comparable record) or a typed root (reserved, filled by
//! derived steps). `FieldPath::from_str` turns each into a path.

/// A cell value: the model id.
pub const MODEL_ID: &str = "model.id";

/// A cell value: the model endpoint.
pub const MODEL_ENDPOINT: &str = "model.endpoint";

/// A cell value: the effort; the driver emits `--effort` only when it is set.
pub const MODEL_EFFORT: &str = "model.effort";

/// A setting: the name of the one secret that holds the token.
pub const MODEL_TOKEN_SECRET: &str = "model.token_secret";

/// A cell value: the allowed tools.
pub const TOOLS_ALLOWED: &str = "tools.allowed";

/// A cell value: the disallowed tools.
pub const TOOLS_DISALLOWED: &str = "tools.disallowed";

/// The root of the MCP server configuration, a cell value.
pub const MCP: &str = "mcp";

/// A cell value: the permission mode.
pub const PERMISSION_MODE: &str = "permission_mode";

/// A setting: the harness the request names.
pub const REQUEST_HARNESS: &str = "request.harness";

/// A setting: the treatment the request names.
pub const REQUEST_TREATMENT: &str = "request.treatment";

/// A setting: the agent's declared inputs as `{kind, digest}`, which the image step merges into
/// `inputs`.
pub const REQUEST_INPUTS: &str = "request.inputs";

/// A cell value: the prompt template.
pub const PROMPT_TEMPLATE: &str = "prompt.template";

/// A cell value: the values the template is filled with.
pub const PROMPT_VALUES: &str = "prompt.values";

/// A cell value, derived: the local image id.
pub const ISOLATION_IMAGE: &str = "isolation.image";

/// A cell value: `bridge` or the internal default.
pub const ISOLATION_NETWORK: &str = "isolation.network";

/// A cell value: the environment lock digest.
pub const ISOLATION_ENV_LOCK: &str = "isolation.env_lock";

/// The root of the container limits, a cell value.
pub const ISOLATION_LIMITS: &str = "isolation.limits";

/// A cell value: the process limit.
pub const ISOLATION_LIMITS_PIDS: &str = "isolation.limits.pids";

/// A cell value: the memory limit.
pub const ISOLATION_LIMITS_MEMORY: &str = "isolation.limits.memory";

/// A cell value: the cpu limit.
pub const ISOLATION_LIMITS_CPUS: &str = "isolation.limits.cpus";

/// A setting: the Dockerfile to build the image from.
pub const ISOLATION_BUILD_DOCKERFILE: &str = "isolation.build.dockerfile";

/// A setting: the build context directory.
pub const ISOLATION_BUILD_CONTEXT: &str = "isolation.build.context";

/// A setting: the requirement sources the env lock is compiled from, each a path relative to the
/// experiment file or `treatment:<path inside the skill>`.
pub const ISOLATION_BUILD_REQUIREMENTS: &str = "isolation.build.requirements";

/// The prefix of a requirement source read from the treatment's repository rather than a path:
/// `treatment:<path inside the skill>`.
pub const TREATMENT_SOURCE_PREFIX: &str = "treatment:";

/// A setting: the names of the files deleted from the env after the install, each matching
/// `^[A-Za-z0-9._-]+$`.
pub const ISOLATION_BUILD_PRUNE: &str = "isolation.build.prune";

/// A setting: the import packages whose distribution and version each run records from its env
/// image, each matching `^[a-z_][a-z0-9_]*$`.
pub const ISOLATION_RECORD_PACKAGES: &str = "isolation.record_packages";

/// A setting: a ready lock file used verbatim instead of compiling one.
pub const ISOLATION_BUILD_LOCK: &str = "isolation.build.lock";

/// A cell value: the wall-clock cap of one run, in seconds.
pub const BUDGET_PER_RUN_WALL_S: &str = "budget.per_run.wall_s";

/// A cell value: the turn cap of one run.
pub const BUDGET_PER_RUN_TURNS: &str = "budget.per_run.turns";

/// A cell value: the cost cap of one run.
pub const BUDGET_PER_RUN_MAX_COST_USD: &str = "budget.per_run.max_cost_usd";

/// A cell value: how the harness runs, `headless` or `session`. It has no core default: a cell
/// that does not write it runs headless.
pub const RUNNER_MODE: &str = "runner.mode";

/// A cell value: the seconds a session stays open after the agent yields with nothing pending, or
/// `none`. It is refused unless [`RUNNER_MODE`] is `session`.
pub const RUNNER_IDLE_S: &str = "runner.idle_s";

/// A typed root, derived: the harness identity.
pub const HARNESS: &str = "harness";

/// The path of the harness version below [`HARNESS`], a fact a replayed cell re-checks.
pub const HARNESS_VERSION: &str = "harness.version";

/// The path a replayed cell's secrets deviation is reported at: the names of the secrets present.
pub const SECRETS: &str = "secrets";

/// A typed root, derived: the treatment `{name, tag, commit}`.
pub const TREATMENT: &str = "treatment";

/// A typed root, derived: the digests of the server sources and the Dockerfile.
pub const INPUTS: &str = "inputs";

/// A typed root, derived: the catalog digest.
pub const SNAPSHOTS: &str = "snapshots";

/// A typed root, reserved for plug-in environments: no preflight step of the runner derives it.
pub const ENVIRONMENTS: &str = "environments";

/// A setting: the store root.
pub const STORE_ROOT: &str = "store.root";

/// A setting: the plugin directories.
pub const PLUGINS_DIRS: &str = "plugins.dirs";

/// A setting: how many runs execute at once.
pub const SLOTS_COUNT: &str = "slots.count";

/// A setting: how many evaluator processes run at once on the machine.
pub const SLOTS_EVALUATORS: &str = "slots.evaluators";

/// Reports whether `path` belongs to the configuration surface: a path named above, an operational
/// setting ([`is_setting`](crate::resolve::layers::is_setting)), a typed root or a path below one
/// (which another check refuses on its own terms), a path below `mcp` or `prompt.values`, whose
/// children are free, or an ancestor of any of these.
#[must_use]
pub fn is_known_path(path: &candler_schemas::primitives::FieldPath) -> bool {
    const LEAVES: [&str; 33] = [
        MODEL_ID,
        MODEL_ENDPOINT,
        MODEL_EFFORT,
        MODEL_TOKEN_SECRET,
        TOOLS_ALLOWED,
        TOOLS_DISALLOWED,
        PERMISSION_MODE,
        REQUEST_HARNESS,
        REQUEST_TREATMENT,
        REQUEST_INPUTS,
        PROMPT_TEMPLATE,
        ISOLATION_IMAGE,
        ISOLATION_NETWORK,
        ISOLATION_ENV_LOCK,
        ISOLATION_LIMITS_PIDS,
        ISOLATION_LIMITS_MEMORY,
        ISOLATION_LIMITS_CPUS,
        ISOLATION_BUILD_DOCKERFILE,
        ISOLATION_BUILD_CONTEXT,
        ISOLATION_BUILD_REQUIREMENTS,
        ISOLATION_BUILD_PRUNE,
        ISOLATION_BUILD_LOCK,
        ISOLATION_RECORD_PACKAGES,
        BUDGET_PER_RUN_WALL_S,
        BUDGET_PER_RUN_TURNS,
        BUDGET_PER_RUN_MAX_COST_USD,
        RUNNER_MODE,
        RUNNER_IDLE_S,
        STORE_ROOT,
        PLUGINS_DIRS,
        SLOTS_COUNT,
        ISOLATION_LIMITS,
        PROMPT_VALUES,
    ];
    const SUBTREES: [&str; 7] = [
        MCP,
        PROMPT_VALUES,
        HARNESS,
        TREATMENT,
        INPUTS,
        SNAPSHOTS,
        ENVIRONMENTS,
    ];
    let known = |text: &str| text.parse::<candler_schemas::primitives::FieldPath>().ok();
    crate::resolve::layers::is_setting(path)
        || LEAVES
            .iter()
            .filter_map(|text| known(text))
            .any(|leaf| leaf.starts_with(path))
        || SUBTREES
            .iter()
            .filter_map(|text| known(text))
            .any(|root| path.starts_with(&root) || root.starts_with(path))
}
