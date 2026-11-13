//! The field paths of a cell, as text. A path is a cell value (hashed and compared), a setting
//! (dropped from the comparable record) or a typed root (reserved, filled by derived steps).
//! [`field_path!`](crate::field_path) turns each into a [`FieldPath`](crate::primitives::FieldPath)
//! checked at compile time.

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

/// Every path above is a field path; the compiler checks it.
const _: () = {
    assert!(crate::primitives::is_field_path(MODEL_ID));
    assert!(crate::primitives::is_field_path(MODEL_ENDPOINT));
    assert!(crate::primitives::is_field_path(MODEL_EFFORT));
    assert!(crate::primitives::is_field_path(MODEL_TOKEN_SECRET));
    assert!(crate::primitives::is_field_path(TOOLS_ALLOWED));
    assert!(crate::primitives::is_field_path(TOOLS_DISALLOWED));
    assert!(crate::primitives::is_field_path(MCP));
    assert!(crate::primitives::is_field_path(PERMISSION_MODE));
    assert!(crate::primitives::is_field_path(REQUEST_HARNESS));
    assert!(crate::primitives::is_field_path(REQUEST_TREATMENT));
    assert!(crate::primitives::is_field_path(REQUEST_INPUTS));
    assert!(crate::primitives::is_field_path(PROMPT_TEMPLATE));
    assert!(crate::primitives::is_field_path(PROMPT_VALUES));
    assert!(crate::primitives::is_field_path(ISOLATION_IMAGE));
    assert!(crate::primitives::is_field_path(ISOLATION_NETWORK));
    assert!(crate::primitives::is_field_path(ISOLATION_ENV_LOCK));
    assert!(crate::primitives::is_field_path(ISOLATION_LIMITS));
    assert!(crate::primitives::is_field_path(ISOLATION_LIMITS_PIDS));
    assert!(crate::primitives::is_field_path(ISOLATION_LIMITS_MEMORY));
    assert!(crate::primitives::is_field_path(ISOLATION_LIMITS_CPUS));
    assert!(crate::primitives::is_field_path(ISOLATION_BUILD_DOCKERFILE));
    assert!(crate::primitives::is_field_path(ISOLATION_BUILD_CONTEXT));
    assert!(crate::primitives::is_field_path(
        ISOLATION_BUILD_REQUIREMENTS
    ));
    assert!(crate::primitives::is_field_path(ISOLATION_BUILD_PRUNE));
    assert!(crate::primitives::is_field_path(ISOLATION_RECORD_PACKAGES));
    assert!(crate::primitives::is_field_path(ISOLATION_BUILD_LOCK));
    assert!(crate::primitives::is_field_path(BUDGET_PER_RUN_WALL_S));
    assert!(crate::primitives::is_field_path(BUDGET_PER_RUN_TURNS));
    assert!(crate::primitives::is_field_path(
        BUDGET_PER_RUN_MAX_COST_USD
    ));
    assert!(crate::primitives::is_field_path(RUNNER_MODE));
    assert!(crate::primitives::is_field_path(RUNNER_IDLE_S));
    assert!(crate::primitives::is_field_path(HARNESS));
    assert!(crate::primitives::is_field_path(HARNESS_VERSION));
    assert!(crate::primitives::is_field_path(SECRETS));
    assert!(crate::primitives::is_field_path(TREATMENT));
    assert!(crate::primitives::is_field_path(INPUTS));
    assert!(crate::primitives::is_field_path(SNAPSHOTS));
    assert!(crate::primitives::is_field_path(ENVIRONMENTS));
    assert!(crate::primitives::is_field_path(STORE_ROOT));
    assert!(crate::primitives::is_field_path(PLUGINS_DIRS));
    assert!(crate::primitives::is_field_path(SLOTS_COUNT));
    assert!(crate::primitives::is_field_path(SLOTS_EVALUATORS));
};
