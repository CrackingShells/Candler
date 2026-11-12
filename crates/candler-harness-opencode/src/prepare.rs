//! The configuration file the driver asks the core to write into the agent's home, and the one
//! settings table [`settings`] reads and both [`crate::driver::OpenCodeDriver::command`] and
//! [`render_config`] build from: the two trait methods are called independently by the core, so
//! the mapping of each setting lives here once, not twice.
//!
//! The provider names the variables through which the core hands the gateway to the agent, as
//! `{env:NAME}` references that `OpenCode` expands when it loads the file, so no address and no
//! token passes through the driver. The MCP servers come from the cell's `mcp` value, with
//! `codemode` per server written out, so the run never depends on `OpenCode`'s default.

use std::str::FromStr as _;

use candler_harness::driver::DriverError;
use candler_harness::spec::{Cell, PreparedFile};
use candler_schemas::primitives::{FieldPath, RelPath};
use serde_json::{Map, Value, json};

/// The provider's id in the configuration and in `-m <provider>/<model>`.
pub(crate) const PROVIDER: &str = "candler";

const MODEL_ID: &str = "model.id";
const MCP: &str = "mcp";
/// The cell path of the permission mode.
pub(crate) const PERMISSION_MODE: &str = "permission_mode";
/// The one permission mode `--auto` implements.
const ACCEPTED_MODE: &str = "bypassPermissions";
/// The agent whose own built-in rule denies `edit` outside its directory (`plan.ts`); mapped here
/// together with an added `shell` deny, which the agent does not carry on its own.
const PLAN_MODE: &str = "plan";
/// The cell path of the reasoning effort.
const MODEL_EFFORT: &str = "model.effort";
/// The cell path of the per-run turn cap.
const BUDGET_TURNS: &str = "budget.per_run.turns";
/// The cell path of the per-run cost cap.
const BUDGET_COST: &str = "budget.per_run.max_cost_usd";
/// The cell path of the allowed-tools list.
const TOOLS_ALLOWED: &str = "tools.allowed";
/// The cell path of the disallowed-tools list.
const TOOLS_DISALLOWED: &str = "tools.disallowed";

/// Claude Code's own effort vocabulary (`claude --help`: `--effort <level>`), the values Candler's
/// cell carries at `model.effort`. `OpenCode`'s own `anthropicMessages` variant builder
/// (`packages/core/src/variant.ts`, v2.2.7) resolves the same five names to adaptive thinking for
/// a catalogued Claude model; a config-declared `variants` entry
/// (`packages/schema/src/config/provider.ts::ModelConfig.variants`, v2.2.7) is used as written,
/// bypassing that catalog lookup, so the driver can write the same shape directly for the custom
/// `candler` provider.
const EFFORT_LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// Candler's tool names (Claude Code's: `tools.*` of a cell) that `OpenCode` has a built-in action
/// for, from the census of `opencode_facts_v0.md` and the shared `edit`/`write`/`patch` grouping
/// `packages/core/src/plugin/plan.ts` shows (v2.2.7). A name with no row, and an MCP name
/// (`mcp__<server>__<tool>`) whose server segment is not unambiguous (holds its own `_`), are
/// refused rather than guessed.
const TOOL_ACTIONS: [(&str, &str); 10] = [
    ("Bash", "shell"),
    ("Edit", "edit"),
    ("Write", "write"),
    ("Read", "read"),
    ("Glob", "glob"),
    ("Grep", "grep"),
    ("WebFetch", "webfetch"),
    ("WebSearch", "websearch"),
    ("Skill", "skill"),
    ("Task", "subagent"),
];

pub(crate) fn invalid(path: &str, detail: String) -> DriverError {
    match FieldPath::from_str(path) {
        Ok(path) => DriverError::InvalidValue { path, detail },
        Err(error) => DriverError::VersionOutput {
            detail: error.to_string(), // BORROW: .to_string() gives owned text
        },
    }
}

pub(crate) fn missing(path: &str) -> DriverError {
    match FieldPath::from_str(path) {
        Ok(path) => DriverError::MissingValue { path },
        Err(error) => DriverError::VersionOutput {
            detail: error.to_string(), // BORROW: .to_string() gives owned text
        },
    }
}

/// How the harness is named in a refusal, as its users write it.
const DISPLAY_NAME: &str = "OpenCode";

/// The refusal of the setting at `path`, which `OpenCode` cannot honour at `version`.
pub(crate) fn unsupported(path: &str, version: &semver::Version, why: &str) -> DriverError {
    match FieldPath::from_str(path) {
        Ok(setting) => DriverError::unsupported(&setting, DISPLAY_NAME, version, why),
        Err(error) => DriverError::VersionOutput {
            detail: error.to_string(), // BORROW: .to_string() gives owned text
        },
    }
}

/// The value of `path` in the cell, if the cell has one.
pub(crate) fn value_at<'c>(cell: &Cell<'c>, path: &str) -> Result<Option<&'c Value>, DriverError> {
    let field = FieldPath::from_str(path).map_err(|error| invalid(path, error.to_string()))?; // BORROW: .to_string() gives owned text
    Ok(cell
        .get(&field)
        .map(candler_schemas::values::JsonValue::as_value))
}

/// The text at `path`, which the cell must have.
pub(crate) fn required_text<'c>(cell: &Cell<'c>, path: &str) -> Result<&'c str, DriverError> {
    match value_at(cell, path)? {
        None => Err(missing(path)),
        Some(value) => value
            .as_str()
            .ok_or_else(|| invalid(path, "is not text".to_owned())), // BORROW: .to_owned() makes the owned text
    }
}

/// The model the cell names, as `OpenCode` takes it after the provider and a slash.
pub(crate) fn model_id<'c>(cell: &Cell<'c>) -> Result<&'c str, DriverError> {
    let model = required_text(cell, MODEL_ID)?;
    if model.is_empty() || model.contains('#') {
        return Err(invalid(
            MODEL_ID,
            "is empty or holds `#`, which starts a variant in `-m`".to_owned(), // BORROW: .to_owned() makes the owned text
        ));
    }
    Ok(model)
}

fn strings(path: &str, what: &str, value: &Value) -> Result<Vec<String>, DriverError> {
    let bad = || invalid(path, format!("{what} is not a list of text"));
    value
        .as_array()
        .ok_or_else(bad)?
        .iter()
        .map(|item| item.as_str().map(str::to_owned).ok_or_else(bad)) // BORROW: str::to_owned makes owned text
        .collect()
}

fn text_map(path: &str, what: &str, value: &Value) -> Result<Map<String, Value>, DriverError> {
    let map = value
        .as_object()
        .ok_or_else(|| invalid(path, format!("{what} is not an object")))?;
    if map.values().any(|v| !v.is_string()) {
        return Err(invalid(
            path,
            format!("{what} holds a value that is not text"),
        ));
    }
    Ok(map.clone()) // BORROW: .clone() copies the map out of the cell
}

/// The keys a stdio server may carry, and a remote one. Any other key would be dropped, so it is
/// refused.
const LOCAL_KEYS: [&str; 5] = ["type", "command", "args", "env", "codemode"];
const REMOTE_KEYS: [&str; 4] = ["type", "url", "headers", "codemode"];

fn only_keys(at: &str, object: &Map<String, Value>, allowed: &[&str]) -> Result<(), DriverError> {
    match object.keys().find(|key| !allowed.contains(&key.as_str())) {
        // BORROW: explicit .as_str()
        Some(key) => Err(invalid(
            &format!("{at}.{key}"),
            format!(
                "is not a key OpenCode takes for this server (allowed: {})",
                allowed.join(", ")
            ),
        )),
        None => Ok(()),
    }
}

/// One server of the cell's `mcp` value, in the shape `OpenCode` V2 prints: a `local` server with a
/// command list, or a `remote` one with a url. Anything else is refused.
fn server(name: &str, entry: &Value) -> Result<Value, DriverError> {
    let at = format!("{MCP}.{name}");
    let object = entry
        .as_object()
        .ok_or_else(|| invalid(&at, "is not an object".to_owned()))?; // BORROW: .to_owned() makes the owned text
    let codemode = match object.get("codemode") {
        None => true,
        Some(Value::Bool(flag)) => *flag,
        Some(_) => {
            return Err(invalid(
                &format!("{at}.codemode"),
                "is not a boolean".to_owned(), // BORROW: .to_owned() makes the owned text
            ));
        }
    };
    let kind = object.get("type").and_then(Value::as_str);
    let mut out = Map::new();
    match (kind, object.get("command"), object.get("url")) {
        (None | Some("stdio" | "local"), Some(command), None) => {
            only_keys(&at, object, &LOCAL_KEYS)?;
            let program = command
                .as_str()
                .ok_or_else(|| invalid(&at, "`command` is not text".to_owned()))?; // BORROW: .to_owned() makes the owned text
            let mut argv = vec![program.to_owned()]; // BORROW: .to_owned() makes owned text
            if let Some(args) = object.get("args") {
                argv.extend(strings(&at, "`args`", args)?);
            }
            out.insert("type".to_owned(), json!("local")); // BORROW: .to_owned() makes the owned key
            out.insert("command".to_owned(), json!(argv)); // BORROW: .to_owned() makes the owned key
            if let Some(env) = object.get("env") {
                out.insert(
                    "environment".to_owned(), // BORROW: .to_owned() makes the owned key
                    Value::Object(text_map(&at, "`env`", env)?),
                );
            }
        }
        (Some("http" | "sse" | "remote"), None, Some(url)) => {
            only_keys(&at, object, &REMOTE_KEYS)?;
            let url = url
                .as_str()
                .ok_or_else(|| invalid(&at, "`url` is not text".to_owned()))?; // BORROW: .to_owned() makes the owned text
            out.insert("type".to_owned(), json!("remote")); // BORROW: .to_owned() makes the owned key
            out.insert("url".to_owned(), json!(url)); // BORROW: .to_owned() makes the owned key
            if let Some(headers) = object.get("headers") {
                out.insert(
                    "headers".to_owned(), // BORROW: .to_owned() makes the owned key
                    Value::Object(text_map(&at, "`headers`", headers)?),
                );
            }
        }
        _ => {
            return Err(invalid(
                &at,
                "is neither a stdio server (`command`) nor a remote one (`type` http, sse or remote, and `url`)"
                    .to_owned(), // BORROW: .to_owned() makes the owned text
            ));
        }
    }
    out.insert("codemode".to_owned(), Value::Bool(codemode)); // BORROW: .to_owned() makes the owned key
    Ok(Value::Object(out))
}

/// The `OpenCode` action `name` maps to, or `None` when it has no row.
///
/// A name of the static table (`TOOL_ACTIONS`) maps directly. A direct MCP tool name, Claude
/// Code's `mcp__<server>__<tool>` (`opencode_facts_v0.md` census; `OpenCode`'s own spelling is
/// `<server>_<tool>`), maps only when the server segment carries no `_` of its own: an
/// underscore inside the server name would make the join ambiguous to reverse (`OpenCode` does not
/// document which `_` it split on), so that case is refused rather than guessed.
fn opencode_action(name: &str) -> Option<String> {
    if let Some((_, action)) = TOOL_ACTIONS.iter().find(|(claude, _)| *claude == name) {
        return Some((*action).to_owned()); // BORROW: .to_owned() makes owned text
    }
    let rest = name.strip_prefix("mcp__")?;
    let (server, tool) = rest.split_once("__")?;
    if server.is_empty() || tool.is_empty() || server.contains('_') {
        return None;
    }
    Some(format!("{server}_{tool}"))
}

/// The permission rows `tools.allowed` or `tools.disallowed` compiles to: one row per name, at
/// `effect`. A name with no `OpenCode` action refuses the whole list, naming the first such name,
/// rather than silently dropping it.
fn tool_permission_rows(
    cell: &Cell<'_>,
    path: &str,
    effect: &str,
    version: &semver::Version,
) -> Result<Vec<Value>, DriverError> {
    let Some(value) = value_at(cell, path)? else {
        return Ok(Vec::new());
    };
    let names = strings(path, "the list", value)?;
    names
        .iter()
        .map(|name| {
            opencode_action(name).map_or_else(
                || {
                    Err(unsupported(
                        path,
                        version,
                        &format!("`{name}` is not a tool OpenCode has an action for"),
                    ))
                },
                |action| Ok(json!({"action": action, "resource": "*", "effect": effect})),
            )
        })
        .collect()
}

/// The reasoning-effort variant `model.effort` compiles to: Claude Code's own vocabulary resolved
/// to `OpenCode`'s adaptive-thinking shape (see `EFFORT_LEVELS`), or an error for any other value.
fn effort_variant(cell: &Cell<'_>) -> Result<Option<(String, Value)>, DriverError> {
    let Some(value) = value_at(cell, MODEL_EFFORT)? else {
        return Ok(None);
    };
    let level = value
        .as_str()
        .ok_or_else(|| invalid(MODEL_EFFORT, "is not text".to_owned()))?; // BORROW: .to_owned() makes the owned text
    if !EFFORT_LEVELS.contains(&level) {
        return Err(invalid(
            MODEL_EFFORT,
            format!("`{level}` is not one of {}", EFFORT_LEVELS.join(", ")),
        ));
    }
    Ok(Some((
        level.to_owned(), // BORROW: .to_owned() makes the owned variant id
        json!({"thinking": {"type": "adaptive", "display": "summarized"}, "effort": level}),
    )))
}

/// The turn cap `budget.per_run.turns` compiles to: `agents.build.steps`. A non-positive value
/// never reaches the driver (`candler_schemas::refusal_impls::ExecutionError`), so this only
/// guards the value's shape.
fn turn_cap(cell: &Cell<'_>) -> Result<Option<u64>, DriverError> {
    let Some(value) = value_at(cell, BUDGET_TURNS)? else {
        return Ok(None);
    };
    value
        .as_u64()
        .filter(|turns| *turns > 0)
        .map(Some)
        .ok_or_else(|| invalid(BUDGET_TURNS, "is not a positive integer".to_owned())) // BORROW: .to_owned() makes the owned text
}

/// What the cell's settings resolve to: the command-line pieces [`crate::driver`] needs, and the
/// configuration pieces [`render_config`] needs, read once so the two trait methods, which the
/// core calls independently, agree.
pub(crate) struct Settings {
    /// `--agent <id>`, or no flag for the default agent.
    pub(crate) agent: Option<&'static str>,
    /// Whether `--auto` is passed.
    pub(crate) auto: bool,
    /// Rows added to the configuration's top-level `permissions`, beyond the agent's own.
    pub(crate) extra_permissions: Vec<Value>,
    /// `agents.build.steps`, if the cell sets a turn cap.
    pub(crate) steps: Option<u64>,
    /// The model's one added `variants` entry (id, settings), if the cell sets an effort.
    pub(crate) variant: Option<(String, Value)>,
}

/// The settings table: every cell setting this driver can honour mapped to what it does, and
/// every other one refused naming why. `version` is the validated version the refusal names.
///
/// # Errors
/// Returns [`DriverError::Unsupported`] for a setting `OpenCode` has no way to honour, and
/// [`DriverError::InvalidValue`] for a value it cannot use of a setting it does honour.
pub(crate) fn settings(
    cell: &Cell<'_>,
    version: &semver::Version,
) -> Result<Settings, DriverError> {
    let mode = required_text(cell, PERMISSION_MODE)?;
    let (agent, auto, mut extra_permissions) = if mode == ACCEPTED_MODE {
        // `--auto` approves everything not denied.
        (None, true, Vec::new())
    } else if mode == PLAN_MODE {
        // The `plan` agent's own rule already denies `edit` outside its directory
        // (`packages/core/src/plugin/plan.ts`, v2.2.7); it adds none for `shell`, so a shell call
        // is otherwise left to ask and `--auto` would approve it. This rule closes that gap.
        (
            Some(PLAN_MODE),
            true,
            vec![json!({"action": "shell", "resource": "*", "effect": "deny"})],
        )
    } else {
        return Err(unsupported(
            PERMISSION_MODE,
            version,
            &format!(
                "`{mode}` is not a mode OpenCode runs: only `{ACCEPTED_MODE}` (`--auto`) and \
                 `{PLAN_MODE}` (`--agent plan`) are honoured"
            ),
        ));
    };
    if value_at(cell, BUDGET_COST)?.is_some() {
        return Err(unsupported(
            BUDGET_COST,
            version,
            "OpenCode has no cost cap",
        ));
    }
    let variant = effort_variant(cell)?;
    let steps = turn_cap(cell)?;
    let mut rows = tool_permission_rows(cell, TOOLS_ALLOWED, "allow", version)?;
    rows.extend(tool_permission_rows(
        cell,
        TOOLS_DISALLOWED,
        "deny",
        version,
    )?);
    // The plan agent's own added rule, if any, is last: nothing an experimenter's tool list asks
    // for can reopen what plan mode closes.
    rows.append(&mut extra_permissions);
    extra_permissions = rows;
    Ok(Settings {
        agent,
        auto,
        extra_permissions,
        steps,
        variant,
    })
}

/// The `opencode.json` of the run: the provider, the model, its effort variant, the turn cap, the
/// permission rows and the MCP servers. `config_dir` is the home-relative directory that
/// `XDG_CONFIG_HOME` names.
///
/// # Errors
/// Returns [`DriverError::Unsupported`] for a setting [`settings`] refuses, and
/// [`DriverError::InvalidValue`]/[`DriverError::MissingValue`] for the model and the `mcp` value.
pub(crate) fn render_config(
    cell: &Cell<'_>,
    config_dir: &str,
    base_url_var: &str,
    token_var: &str,
    version: &semver::Version,
) -> Result<PreparedFile, DriverError> {
    let settings = settings(cell, version)?;
    let model = model_id(cell)?;
    let mut models = Map::new();
    let model_entry = settings.variant.map_or_else(
        || json!({}),
        |(id, variant_settings)| json!({"variants": [{"id": id, "settings": variant_settings}]}),
    );
    models.insert(model.to_owned(), model_entry); // BORROW: .to_owned() makes the owned key
    let mut root = Map::new();
    root.insert(
        "providers".to_owned(), // BORROW: .to_owned() makes the owned key
        json!({ PROVIDER: {
            "package": "aisdk:@ai-sdk/anthropic",
            "settings": {
                "baseURL": format!("{{env:{base_url_var}}}/v1"),
                "apiKey": format!("{{env:{token_var}}}"),
            },
            "models": models,
        } }),
    );
    match value_at(cell, MCP)? {
        // EXPLICIT: no `mcp` value, or an empty one, writes no servers
        None => {}
        Some(Value::Object(servers)) if servers.is_empty() => {}
        Some(Value::Object(servers)) => {
            let mut translated = Map::new();
            for (name, entry) in servers {
                translated.insert(name.clone(), server(name, entry)?); // BORROW: .clone() copies the name
            }
            root.insert(
                MCP.to_owned(), // BORROW: .to_owned() makes the owned key
                json!({ "servers": translated }),
            );
        }
        Some(_) => return Err(invalid(MCP, "is not an object".to_owned())), // BORROW: .to_owned() makes the owned text
    }
    if let Some(steps) = settings.steps {
        root.insert(
            "agents".to_owned(), // BORROW: .to_owned() makes the owned key
            json!({"build": {"steps": steps}}),
        );
    }
    if !settings.extra_permissions.is_empty() {
        root.insert(
            "permissions".to_owned(), // BORROW: .to_owned() makes the owned key
            Value::Array(settings.extra_permissions),
        );
    }
    let mut text = serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|error| invalid(MCP, error.to_string()))?; // BORROW: .to_string() gives owned text
    text.push('\n');
    let path = RelPath::from_str(&format!("{config_dir}/opencode/opencode.json"))
        .map_err(|error| invalid(MCP, error.to_string()))?; // BORROW: .to_string() gives owned text
    Ok(PreparedFile::in_home(path, text.into_bytes(), false))
}
