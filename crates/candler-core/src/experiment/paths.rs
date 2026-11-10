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

/// A setting: the path of the skill repository; a location, not an identity.
pub const REQUEST_TREATMENT_SOURCE: &str = "request.treatment_source";

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

/// A setting: the names of the files deleted from the env after the install, each matching
/// `^[A-Za-z0-9._-]+$`.
pub const ISOLATION_BUILD_PRUNE: &str = "isolation.build.prune";

/// A setting: a ready lock file used verbatim instead of compiling one.
pub const ISOLATION_BUILD_LOCK: &str = "isolation.build.lock";

/// A cell value: the wall-clock cap of one run, in seconds.
pub const BUDGET_PER_RUN_WALL_S: &str = "budget.per_run.wall_s";

/// A cell value: the turn cap of one run.
pub const BUDGET_PER_RUN_TURNS: &str = "budget.per_run.turns";

/// A cell value: the cost cap of one run.
pub const BUDGET_PER_RUN_MAX_COST_USD: &str = "budget.per_run.max_cost_usd";

/// A typed root, derived: the harness identity.
pub const HARNESS: &str = "harness";

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
