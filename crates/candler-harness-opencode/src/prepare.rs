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

use candler_harness::driver::{DriverError, SettingRow};
use candler_harness::spec::{Cell, PreparedFile};
use candler_schemas::execution::paths::{
    BUDGET_PER_RUN_MAX_COST_USD, BUDGET_PER_RUN_TURNS, MCP, MODEL_EFFORT, MODEL_ID,
    PERMISSION_MODE, TOOLS_ALLOWED, TOOLS_DISALLOWED,
};
use candler_schemas::primitives::{FieldPath, RelPath};
use serde_json::{Map, Value, json};

/// The provider's id in the configuration and in `-m <provider>/<model>`.
pub(crate) const PROVIDER: &str = "candler";

/// The one permission mode `--auto` implements.
const ACCEPTED_MODE: &str = "bypassPermissions";

/// The baseline every `OpenCode` agent starts from (`packages/schema/src/agent.ts`, v2.2.7): the
/// ruleset `{*, *, allow}` plus an ask on `.env` reads. The configuration's top-level `permissions`
/// are appended after it and the last matching row wins, so an `allow` row restates the baseline
/// and a `deny` row, written last, overrides it.
const BASELINE: &str = "every tool call is allowed by the baseline ruleset {*, *, allow}; `--auto` \
     answers the one pending ask (`.env` reads) with once, which a headless run needs";

/// The permission modes that are refused, each with the reason. Claude Code's own documentation
/// (permission modes; permissions, read-only commands) is the source of what each mode allows.
const MODE_REFUSALS: [(&str, &str); 3] = [
    (
        "plan",
        "Claude Code's plan mode reads and runs read-only shell commands (a built-in, \
         non-configurable set its own parser decides). The OpenCode ruleset that is certainly \
         read-only (the `explore` agent's: deny everything, allow read, glob, grep, webfetch, \
         websearch, skill) denies the shell altogether, and a wildcard on command text cannot \
         express the read-only set; the stock `plan` agent is not read-only once a global rule \
         follows it (the last match wins) and its `subagent` child can edit",
    ),
    (
        "default",
        "Claude Code's default mode allows reads and a built-in, non-configurable set of read-only \
         shell commands that its own parser decides, and denies the rest in a headless run. \
         OpenCode without `--auto` also rejects what no rule allows, but its shell rows are \
         wildcards on the text of each parsed command, which approximate that set without being it",
    ),
    (
        "acceptEdits",
        "Claude Code's acceptEdits mode also allows edits and the filesystem commands mkdir, touch, \
         rm, rmdir, mv, cp and sed, inside the working directory only. OpenCode's shell resource \
         is the command's text, which knows no working directory, so \"inside it\" cannot be \
         written as a rule",
    ),
];

/// The reason any other permission mode is refused.
const OTHER_MODE: &str = "only `bypassPermissions` (`--auto`) is a mode OpenCode runs the way \
     Claude Code does; no other mode's allowed set can be written as an OpenCode ruleset";

/// The reason the turn cap is refused: what `agents.<id>.steps` does is not `--max-turns`.
const TURNS_REFUSAL: &str = "`agents.<id>.steps` makes the last request a tool-less one with an \
     injected prompt and ends the run as completed (the `oc_step_cap` fixture), where Claude Code \
     ends the run with `error_max_turns`, so the cap class would be lost and the model's last \
     turn changed";

/// The reason a lone `Edit` or `Write` in `tools.disallowed` is refused.
const EDIT_WRITE_PAIR: &str = "OpenCode's `edit` action covers the edit, write and patch tools \
     alike, so denying `Edit` or `Write` alone would deny both; list both";

/// The reason a name with no row is refused.
const NO_ACTION: &str = "a name that is not a tool OpenCode has an action for";

/// Claude Code's own effort vocabulary (`claude --help`: `--effort <level>`), the values Candler's
/// cell carries at `model.effort`. `OpenCode`'s `anthropicMessages` variant builder
/// (`packages/core/src/variant.ts`, v2.2.7) resolves the same names to adaptive thinking plus
/// `output_config.effort` for a Claude model of version 4.6 or later; a config-declared `variants`
/// entry (`ModelConfig.variants`, v2.2.7) is used as written, so the driver writes that shape
/// directly for the custom `candler` provider.
const EFFORT_LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// The model families whose id `OpenCode`'s `claudeInfo` recognises (`variant.ts:68-80`).
const FAMILIES: [&str; 5] = ["opus", "sonnet", "haiku", "fable", "mythos"];

/// Why `ultracode` is refused: Claude Code's `--effort` takes it (CLI reference: `xhigh` with its
/// ultracode workflow turned on), the Claude Code driver passes it through, and `OpenCode` has no
/// equivalent of that mode.
const ULTRACODE: &str = "Claude Code honours `ultracode` (`xhigh` effort with its ultracode mode on) \
     and OpenCode has no equivalent of that mode, so it is refused rather than run as `xhigh`";

/// Why a level outside the variants this driver writes is refused: the cell value is opaque, and the
/// Claude Code driver hands any level to `claude --effort` unchanged.
const EFFORT_LEVEL: &str = "this driver writes an `OpenCode` variant only for the levels low, medium, \
     high, xhigh and max; Claude Code receives any other level unchanged, so it is refused rather \
     than mapped to a nearby one";

/// Why an effort the model cannot take is refused.
const EFFORT_MODEL: &str = "`OpenCode` gives a model a reasoning effort only for a Claude opus, \
     sonnet, fable or mythos of version 4.6 or later (older models get a token budget, which is \
     not an effort level; haiku and ids it does not recognise get none from this driver), and \
     `xhigh` only from 4.7";

/// Candler's tool names (Claude Code's: `tools.*` of a cell) and the `OpenCode` action each tool
/// asserts. Each action is the `permission` the tool registers or asserts in `OpenCode` v2.2.7:
/// `edit`, `write` and `patch` all assert `edit` (`tool/plugin/write.ts:49`,
/// `tool/plugin/edit.ts`, `patch.ts`), the others their own name (`glob.ts`, `grep.ts`,
/// `shell.ts`, `read.ts`, `webfetch.ts`, `websearch.ts`, `subagent.ts`, `skill.ts`, `todo.ts`).
/// A name with no row (`NotebookEdit`, `MultiEdit`, any other) is refused rather than guessed.
const TOOLS: [(&str, &str); 11] = [
    ("Bash", "shell"),
    ("Edit", "edit"),
    ("Write", "edit"),
    ("Read", "read"),
    ("Glob", "glob"),
    ("Grep", "grep"),
    ("WebFetch", "webfetch"),
    ("WebSearch", "websearch"),
    ("Task", "subagent"),
    ("Skill", "skill"),
    ("TodoWrite", "todowrite"),
];

/// The value of the table's row for an MCP tool name, `mcp__<server>__<tool>`.
const MCP_ROW: &str = "mcp__<server>__<tool>";

/// The declared settings table: the rows [`settings`] answers, one per cell setting, value or
/// tool name the driver knows, mapped or refused. The driver builds its command and its files from
/// the same constants these rows read, and the guard asserts the rows and its cases are one set.
pub(crate) fn table() -> Vec<SettingRow> {
    let mut rows = vec![SettingRow::mapped(
        PERMISSION_MODE,
        ACCEPTED_MODE,
        "`--auto` on the command line",
        BASELINE,
    )];
    for (mode, why) in MODE_REFUSALS {
        rows.push(SettingRow::refused(PERMISSION_MODE, mode, why));
    }
    rows.push(SettingRow::refused(PERMISSION_MODE, "*", OTHER_MODE));
    rows.push(SettingRow::refused(
        BUDGET_PER_RUN_MAX_COST_USD,
        "*",
        "OpenCode has no cost cap",
    ));
    rows.push(SettingRow::refused(
        BUDGET_PER_RUN_TURNS,
        "*",
        TURNS_REFUSAL,
    ));
    for level in EFFORT_LEVELS {
        rows.push(SettingRow::mapped(
            MODEL_EFFORT,
            level,
            "a `variants` entry on the model, selected by `-m <model>#<level>`",
            "adaptive thinking and `output_config.effort` on every request, as recorded on the wire; only for a model the `<model>` row accepts",
        ));
    }
    rows.push(SettingRow::refused(MODEL_EFFORT, "<model>", EFFORT_MODEL));
    rows.push(SettingRow::refused(MODEL_EFFORT, "*", EFFORT_LEVEL));
    rows.push(SettingRow::refused(MODEL_EFFORT, "ultracode", ULTRACODE));
    for path in [TOOLS_ALLOWED, TOOLS_DISALLOWED] {
        let allowed = path == TOOLS_ALLOWED;
        for (name, action) in TOOLS {
            if !allowed && (name == "Edit" || name == "Write") {
                rows.push(SettingRow::refused(path, name, EDIT_WRITE_PAIR));
                continue;
            }
            rows.push(tool_row(path, name, action));
        }
        rows.push(tool_row(
            path,
            MCP_ROW,
            "<server>_<tool>, each part normalised to [A-Za-z0-9_-]",
        ));
        rows.push(SettingRow::refused(path, "*", NO_ACTION));
    }
    rows.push(SettingRow::mapped(
        TOOLS_DISALLOWED,
        "Edit+Write",
        "one `edit` deny row per name",
        "both names together: the `edit` action covers both tools",
    ));
    rows.push(SettingRow::mapped(
        TOOLS_DISALLOWED,
        "with tools.allowed",
        "the deny rows after the allow rows",
        "the last matching row wins, so a name in both lists is denied",
    ));
    rows
}

/// The row of one tool name (or the MCP pattern) in `path`, mapped to `action`.
fn tool_row(path: &str, name: &str, action: &str) -> SettingRow {
    if path == TOOLS_ALLOWED {
        SettingRow::mapped(
            path,
            name,
            &format!("a permission row {{{action}, *, allow}}"),
            "restates the baseline: a no-op, as Claude Code's `--allowedTools` only auto-approves \
             and leaves every other tool available",
        )
    } else {
        SettingRow::mapped(
            path,
            name,
            &format!("a permission row {{{action}, *, deny}}"),
            "overrides the baseline; written after the allow rows",
        )
    }
}

pub(crate) fn invalid(path: &str, detail: String) -> DriverError {
    match FieldPath::from_str(path) {
        Ok(path) => DriverError::InvalidValue { path, detail },
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

/// The model the cell names, as `OpenCode` takes it after the provider and a slash.
pub(crate) fn model_id<'c>(cell: &Cell<'c>) -> Result<&'c str, DriverError> {
    let model = cell.model()?;
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

/// `text` with every character outside `[A-Za-z0-9_-]` replaced by `_`, as `OpenCode` spells an
/// MCP server or tool in an action (`tool/runtime.ts:288-293`, `mcp.ts:21-25`, v2.2.7).
fn normalize(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The `OpenCode` action `name` maps to, or `None` when it has no row.
///
/// A name of the static table (`TOOLS`) maps directly. A direct MCP tool name, Claude Code's
/// `mcp__<server>__<tool>`, maps to `OpenCode`'s `<server>_<tool>` with each part normalised the
/// way `OpenCode` normalises it, so a server or tool name with a `.` or a space reaches the action
/// `OpenCode` asserts.
fn opencode_action(name: &str) -> Option<String> {
    if let Some((_, action)) = TOOLS.iter().find(|(claude, _)| *claude == name) {
        return Some((*action).to_owned()); // BORROW: .to_owned() makes owned text
    }
    let rest = name.strip_prefix("mcp__")?;
    let (server, tool) = rest.split_once("__")?;
    if server.is_empty() || tool.is_empty() {
        return None;
    }
    Some(format!("{}_{}", normalize(server), normalize(tool)))
}

/// The permission rows `tools.allowed` or `tools.disallowed` compiles to: one row per name, at
/// `effect`. A name with no `OpenCode` action refuses the whole list, naming the first such name,
/// rather than silently dropping it; so does a deny list that holds `Edit` or `Write` without the
/// other, since both are the one `edit` action.
///
/// In `tools.disallowed`, each name becomes a `{action, *, deny}` row, and
/// `OpenCode` v2.2.7 leaves a tool whose last matching row is a deny on `*` out of the tools it
/// sends the model (`packages/core/src/tool.ts:260`, `whollyDisabled`, `tool.ts:386-389`), as
/// Claude Code's `--disallowedTools` does; `--auto` only answers asks with once and adds no row
/// (`packages/cli/src/run/noninteractive.ts:108`), so it never overrides the deny. No request of a
/// `bypassPermissions` run with `Bash` disallowed lists `shell` (`candler-core` guard
/// `census_gateway_denied_shell_is_offered_in_no_request`). The `execute` code-mode tool the
/// request still lists cannot run a shell: its programs call only the tools of its catalog, which
/// holds the tools registered for code mode that survive the same deny filter (`tool.ts:260-268`;
/// the shell tool registers `codemode: false`, `tool/plugin/shell.ts:182`), and an interpreter
/// with no process or filesystem access (`packages/codemode/README.md`) plus `fetch`; so the deny
/// of `Bash` needs no deny of `execute`.
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
    if path == TOOLS_DISALLOWED {
        let has = |wanted: &str| names.iter().any(|name| name == wanted);
        if has("Edit") != has("Write") {
            return Err(unsupported(path, version, EDIT_WRITE_PAIR));
        }
    }
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

/// The model family and version `OpenCode`'s `claudeInfo` reads from a model id
/// (`variant.ts:68-80`, v2.2.7).
struct ClaudeInfo {
    family: &'static str,
    major: u32,
    minor: u32,
}

impl ClaudeInfo {
    /// Whether the model takes adaptive thinking and an effort: not a budget-based model (3.7,
    /// 4.0 to 4.5), not haiku, which the recorded wire does not cover.
    fn takes_effort(&self) -> bool {
        self.family != "haiku" && (self.major > 4 || (self.major == 4 && self.minor >= 6))
    }
}

/// The number at the start of `text`, its digit count and what follows.
fn leading_number(text: &str) -> Option<(u32, usize, &str)> {
    let end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (digits, rest) = text.split_at_checked(end)?;
    Some((digits.parse().ok()?, end, rest))
}

/// A version at the start of `text`: a major of one or two digits, and after a `.` or `-` a minor of one or two digits
/// (`OpenCode` also reads a date as a minor; a longer number is left out here, which makes the
/// model a manual one and so refused). Returns the major, the minor (0 when none) and the rest.
fn version_of(text: &str) -> Option<(u32, u32, &str)> {
    let (major, width, rest) = leading_number(text)?;
    if width > 2 {
        // A date such as `20241022` after a family is no major: `claude-3-5-sonnet-20241022` is
        // read by its version-first form, 3.5, which takes no effort.
        return None;
    }
    match rest.strip_prefix(['.', '-']).and_then(leading_number) {
        Some((minor, width, after)) if width <= 2 => Some((major, minor, after)),
        _ => Some((major, 0, rest)),
    }
}

/// What `OpenCode`'s `claudeInfo` reads from `id`: `<family>-<major>[-<minor>]` first, otherwise
/// `claude-<major>[-<minor>]-<family>`; `None` for an id it does not recognise.
fn claude_info(id: &str) -> Option<ClaudeInfo> {
    let id = id.to_ascii_lowercase();
    let tails = || id.char_indices().filter_map(|(at, _)| id.get(at..));
    tails()
        .find_map(|tail| {
            FAMILIES.into_iter().find_map(|family| {
                let after = tail.strip_prefix(family)?.strip_prefix('-')?;
                let (major, minor, _) = version_of(after)?;
                Some(ClaudeInfo {
                    family,
                    major,
                    minor,
                })
            })
        })
        .or_else(|| {
            tails().find_map(|tail| {
                let after = tail.strip_prefix("claude-")?;
                let (major, minor, rest) = version_of(after)?;
                let named = rest.strip_prefix('-')?;
                let family = FAMILIES
                    .into_iter()
                    .find(|family| named.starts_with(family))?;
                Some(ClaudeInfo {
                    family,
                    major,
                    minor,
                })
            })
        })
}

/// The reasoning-effort variant `model.effort` compiles to: Claude Code's own vocabulary resolved
/// to `OpenCode`'s adaptive-thinking shape (see `EFFORT_LEVELS`). A level outside the vocabulary is
/// an invalid value; a model `OpenCode` would give no such variant (see `EFFORT_MODEL`) is refused
/// here, not left to fail or be ignored at run time.
fn effort_variant(
    cell: &Cell<'_>,
    version: &semver::Version,
) -> Result<Option<(String, Value)>, DriverError> {
    let Some(value) = value_at(cell, MODEL_EFFORT)? else {
        return Ok(None);
    };
    let level = value
        .as_str()
        .ok_or_else(|| invalid(MODEL_EFFORT, "is not text".to_owned()))?; // BORROW: .to_owned() makes the owned text
    if level == "ultracode" {
        return Err(unsupported(MODEL_EFFORT, version, ULTRACODE));
    }
    if !EFFORT_LEVELS.contains(&level) {
        return Err(unsupported(MODEL_EFFORT, version, EFFORT_LEVEL));
    }
    let accepted = claude_info(model_id(cell)?).is_some_and(|info| {
        info.takes_effort() && (level != "xhigh" || info.major > 4 || info.minor >= 7)
    });
    if !accepted {
        return Err(unsupported(MODEL_EFFORT, version, EFFORT_MODEL));
    }
    Ok(Some((
        level.to_owned(), // BORROW: .to_owned() makes the owned variant id
        json!({"thinking": {"type": "adaptive", "display": "summarized"}, "effort": level}),
    )))
}

/// What the cell's settings resolve to: the pieces [`crate::driver`] and [`render_config`] need,
/// read once so the two trait methods, which the core calls independently, agree. The command line
/// is always the `--auto` one, since `bypassPermissions` is the only mode accepted.
pub(crate) struct Settings {
    /// Rows added to the configuration's top-level `permissions`: the allow rows, then the deny
    /// rows, so the last matching row is a deny.
    pub(crate) permissions: Vec<Value>,
    /// The model's one added `variants` entry (id, settings), if the cell sets an effort.
    pub(crate) variant: Option<(String, Value)>,
}

/// What the declared [`table`] says of the cell: every setting this driver can honour mapped to
/// what it does, and every other one refused naming why. `version` is the validated version the
/// refusal names.
///
/// # Errors
/// Returns [`DriverError::Unsupported`] for a setting `OpenCode` has no way to honour, and
/// [`DriverError::InvalidValue`] for a value it cannot use of a setting it does honour.
pub(crate) fn settings(
    cell: &Cell<'_>,
    version: &semver::Version,
) -> Result<Settings, DriverError> {
    let mode = cell.permission_mode()?;
    if mode != ACCEPTED_MODE {
        let why = MODE_REFUSALS
            .iter()
            .find(|(refused, _)| *refused == mode)
            .map_or(OTHER_MODE, |(_, why)| *why);
        return Err(unsupported(PERMISSION_MODE, version, why));
    }
    if value_at(cell, BUDGET_PER_RUN_MAX_COST_USD)?.is_some() {
        return Err(unsupported(
            BUDGET_PER_RUN_MAX_COST_USD,
            version,
            "OpenCode has no cost cap",
        ));
    }
    if value_at(cell, BUDGET_PER_RUN_TURNS)?.is_some() {
        return Err(unsupported(BUDGET_PER_RUN_TURNS, version, TURNS_REFUSAL));
    }
    let variant = effort_variant(cell, version)?;
    let mut permissions = tool_permission_rows(cell, TOOLS_ALLOWED, "allow", version)?;
    permissions.extend(tool_permission_rows(
        cell,
        TOOLS_DISALLOWED,
        "deny",
        version,
    )?);
    Ok(Settings {
        permissions,
        variant,
    })
}

/// The `opencode.json` of the run: the provider, the model, its effort variant, the
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
    if !settings.permissions.is_empty() {
        root.insert(
            "permissions".to_owned(), // BORROW: .to_owned() makes the owned key
            Value::Array(settings.permissions),
        );
    }
    let mut text = serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|error| invalid(MCP, error.to_string()))?; // BORROW: .to_string() gives owned text
    text.push('\n');
    let path = RelPath::from_str(&format!("{config_dir}/opencode/opencode.json"))
        .map_err(|error| invalid(MCP, error.to_string()))?; // BORROW: .to_string() gives owned text
    Ok(PreparedFile::in_home(path, text.into_bytes(), false))
}
