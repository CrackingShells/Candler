//! The configuration file the driver asks the core to write into the agent's home, and the one
//! reading of the cell's settings, [`settings`], that the plan's command line and
//! [`render_config`] both build from, so the mapping of each setting lives here once.
//!
//! The provider names the variables through which the core hands the gateway to the agent, as
//! `{env:NAME}` references that `OpenCode` expands when it loads the file, so no address and no
//! token passes through the driver. The MCP servers come from the cell's `mcp.servers.*` leaves, with
//! `codemode` per server written out, so the run never depends on `OpenCode`'s default.

use std::str::FromStr as _;

use candler_harness::driver::DriverError;
use candler_harness::spec::{
    AgentSetting, Answer, Cell, EffortLevel, PermissionMode, PreparedFile,
};
use candler_schemas::execution::paths::{
    MCP, MODEL_EFFORT, MODEL_ID, TOOLS_ALLOWED, TOOLS_DISALLOWED,
};
use candler_schemas::primitives::{FieldPath, RelPath};
use candler_schemas::servers::{McpServer, McpServerKind};
use serde_json::{Map, Value, json};

/// The provider's id in the configuration and in `-m <provider>/<model>`.
pub(crate) const PROVIDER: &str = "candler";

/// The baseline every `OpenCode` agent starts from (`packages/schema/src/agent.ts`, v2.2.7): the
/// ruleset `{*, *, allow}` plus an ask on `.env` reads. The configuration's top-level `permissions`
/// are appended after it and the last matching row wins, so an `allow` row restates the baseline
/// and a `deny` row, written last, overrides it.
const BASELINE: &str = "`--auto` on the command line: every tool call is allowed by the baseline ruleset {*, *, allow}; `--auto` \
     answers the one pending ask (`.env` reads) with once, which a headless run needs";

/// Why the `plan` permission mode is refused. Claude Code's own documentation (permission modes;
/// permissions, read-only commands) is the source of what each mode allows.
const PLAN_MODE: &str = "Claude Code's plan mode reads and runs read-only shell commands (a built-in, \
         non-configurable set its own parser decides). The OpenCode ruleset that is certainly \
         read-only (the `explore` agent's: deny everything, allow read, glob, grep, webfetch, \
         websearch, skill) denies the shell altogether, and a wildcard on command text cannot \
         express the read-only set; the stock `plan` agent is not read-only once a global rule \
         follows it (the last match wins) and its `subagent` child can edit";

/// Why the `default` permission mode is refused. Claude Code's own documentation (permission modes;
/// permissions, read-only commands) is the source of what each mode allows.
const DEFAULT_MODE: &str = "Claude Code's default mode allows reads and a built-in, non-configurable set of read-only \
         shell commands that its own parser decides, and denies the rest in a headless run. \
         OpenCode without `--auto` also rejects what no rule allows, but its shell rows are \
         wildcards on the text of each parsed command, which approximate that set without being it";

/// Why the `acceptEdits` permission mode is refused. Claude Code's own documentation (permission modes;
/// permissions, read-only commands) is the source of what each mode allows.
const ACCEPT_EDITS_MODE: &str = "Claude Code's acceptEdits mode also allows edits and the filesystem commands mkdir, touch, \
         rm, rmdir, mv, cp and sed, inside the working directory only. OpenCode's shell resource \
         is the command's text, which knows no working directory, so \"inside it\" cannot be \
         written as a rule";

/// Why the `auto` permission mode is refused. Claude Code's own documentation (permission modes)
/// is the source of what the mode does.
const AUTO_MODE: &str = "Claude Code's auto mode has a model classifier review each action, such as a shell \
         command or a network request, before it runs. OpenCode has no classifier, and a ruleset \
         decides by the action's text alone";

/// Why the `dontAsk` permission mode is refused. Claude Code's own documentation (permission modes;
/// permissions, read-only commands) is the source of what the mode allows.
const DONT_ASK_MODE: &str = "Claude Code's dontAsk mode denies every call that would prompt, so it allows the allow \
         rules plus reads and the built-in, non-configurable set of read-only shell commands its own \
         parser decides. OpenCode's shell rows are wildcards on the text of each parsed command, \
         which approximate that set without being it";

/// The reason the turn cap is refused: what `agents.<id>.steps` does is not `--max-turns`.
const TURNS_REFUSAL: &str = "`agents.<id>.steps` makes the last request a tool-less one with an \
     injected prompt and ends the run as completed (the `oc_step_cap` fixture), where Claude Code \
     ends the run with `error_max_turns`, so the cap class would be lost and the model's last \
     turn changed";

/// The reason a lone `Edit` or `Write` in `tools.disallowed` is refused.
const EDIT_WRITE_PAIR: &str = "OpenCode's `edit` action covers the edit, write and patch tools \
     alike, so denying `Edit` or `Write` alone would deny both; list both";

/// The model families whose id `OpenCode`'s `claudeInfo` recognises (`variant.ts:68-80`).
const FAMILIES: [&str; 5] = ["opus", "sonnet", "haiku", "fable", "mythos"];

/// Why `ultracode` is refused.
const ULTRACODE: &str = "`ultracode` is `xhigh` effort with an ultracode mode on, and OpenCode has \
     no equivalent of that mode, so it is refused rather than run as `xhigh`";

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

/// How `OpenCode` meets one key. The driver builds its command and its files from the same
/// constants these answers read.
#[must_use]
pub(crate) const fn answer(setting: AgentSetting) -> Answer {
    match setting {
        AgentSetting::ModelId => Answer::Mapped {
            construct: "`-m candler/<model>` on the command line",
        },
        AgentSetting::ModelEndpoint => Answer::Candler {
            how: "the gateway forwards model requests to the endpoint",
        },
        AgentSetting::Effort(EffortLevel::Ultracode) => Answer::Refused { reason: ULTRACODE },
        AgentSetting::Effort(
            EffortLevel::Low
            | EffortLevel::Medium
            | EffortLevel::High
            | EffortLevel::Xhigh
            | EffortLevel::Max,
        ) => Answer::Mapped {
            construct: "a `variants` entry on the model, selected by `-m <model>#<level>`, for a Claude \
                 opus, sonnet, fable or mythos of 4.6 or later (`xhigh` from 4.7); refused for any \
                 other model",
        },
        AgentSetting::PermissionMode(PermissionMode::BypassPermissions) => Answer::Mapped {
            construct: BASELINE,
        },
        AgentSetting::PermissionMode(PermissionMode::Plan) => Answer::Refused { reason: PLAN_MODE },
        AgentSetting::PermissionMode(PermissionMode::Default) => Answer::Refused {
            reason: DEFAULT_MODE,
        },
        AgentSetting::PermissionMode(PermissionMode::AcceptEdits) => Answer::Refused {
            reason: ACCEPT_EDITS_MODE,
        },
        AgentSetting::PermissionMode(PermissionMode::Auto) => Answer::Refused { reason: AUTO_MODE },
        AgentSetting::PermissionMode(PermissionMode::DontAsk) => Answer::Refused {
            reason: DONT_ASK_MODE,
        },
        AgentSetting::ToolsAllowed => Answer::Mapped {
            construct: "a permission row {action, *, allow} per name, a no-op over the baseline",
        },
        AgentSetting::ToolsDisallowed => Answer::Mapped {
            construct: "a permission row {action, *, deny} per name, written after the allow rows",
        },
        AgentSetting::Env => Answer::Candler {
            how: "the core sets the cell's variables on the agent's process",
        },
        AgentSetting::Inputs => Answer::Candler {
            how: "the core places the inputs in the workspace",
        },
        AgentSetting::Skills => Answer::Mapped {
            construct: "the project skill directory",
        },
        AgentSetting::McpTransport(McpServerKind::Stdio | McpServerKind::Local) => Answer::Mapped {
            construct: "an `mcp` entry of type `local`",
        },
        AgentSetting::McpTransport(McpServerKind::Sse) => Answer::Refused {
            reason: "OpenCode documents no SSE transport; its `remote` is Streamable HTTP",
        },
        AgentSetting::McpTransport(McpServerKind::Http | McpServerKind::Remote) => Answer::Mapped {
            construct: "an `mcp` entry of type `remote`",
        },
        AgentSetting::Codemode(true) => Answer::Mapped {
            construct: "the `codemode` key of the `mcp` entry, `true`: the server's tools are reached through `execute`, whose calls the trace records with empty `wrapped_calls`, so MCP calls made inside `execute` are not attributed and do not compare with Claude Code",
        },
        AgentSetting::Codemode(false) => Answer::Mapped {
            construct: "the `codemode` key of the `mcp` entry, `false`: the server's tools are direct tools",
        },
        AgentSetting::McpName => Answer::Mapped {
            construct: "the key of the `mcp` entry",
        },
        AgentSetting::WallBudget => Answer::Candler {
            how: "the core ends the run at the wall budget",
        },
        AgentSetting::TurnBudget => Answer::Refused {
            reason: TURNS_REFUSAL,
        },
        AgentSetting::CostBudget => Answer::Refused {
            reason: "OpenCode has no cost cap",
        },
        AgentSetting::SessionMode => Answer::Refused {
            reason: "OpenCode runs no session: its trace comes from the export after the run",
        },
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
pub(crate) const DISPLAY_NAME: &str = "OpenCode";

/// The refusal of the setting at `path`, which `OpenCode` cannot honour.
pub(crate) fn unsupported(path: &str, why: &str) -> DriverError {
    match FieldPath::from_str(path) {
        Ok(setting) => DriverError::unsupported(&setting, DISPLAY_NAME, why),
        Err(error) => DriverError::VersionOutput {
            detail: error.to_string(), // BORROW: .to_string() gives owned text
        },
    }
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

/// One server of the cell's `mcp` value, in the shape `OpenCode` V2 prints: a `local` server with a
/// command list, or a `remote` one with a url.
fn server(server: &McpServer) -> Value {
    let mut out = Map::new();
    match candler_harness::mcp::kind(server) {
        McpServerKind::Stdio | McpServerKind::Local => {
            if let Some(command) = &server.command {
                let mut argv = vec![command.clone()]; // BORROW: .clone() copies the program into the list
                argv.extend(server.args.iter().flatten().cloned()); // BORROW: .cloned() copies the arguments
                out.insert("type".to_owned(), json!("local")); // BORROW: .to_owned() makes the owned key
                out.insert("command".to_owned(), json!(argv)); // BORROW: .to_owned() makes the owned key
                if let Some(env) = &server.env {
                    out.insert("environment".to_owned(), json!(env)); // BORROW: .to_owned() makes the owned key
                }
            }
        }
        // `sse` is refused by `answer` before a file is written.
        McpServerKind::Http | McpServerKind::Sse | McpServerKind::Remote => {
            if let Some(url) = &server.url {
                out.insert("type".to_owned(), json!("remote")); // BORROW: .to_owned() makes the owned key
                out.insert("url".to_owned(), json!(url)); // BORROW: .to_owned() makes the owned key
                if let Some(headers) = &server.headers {
                    out.insert("headers".to_owned(), json!(headers)); // BORROW: .to_owned() makes the owned key
                }
            }
        }
    }
    out.insert("codemode".to_owned(), Value::Bool(server.codemode)); // BORROW: .to_owned() makes the owned key
    Value::Object(out)
}

/// `text` with every character outside `[A-Za-z0-9_-]` replaced by `_`, as `OpenCode` spells an
/// MCP server or tool in an action (`tool/runtime.ts:288-293`, `mcp.ts:21-25`, v2.2.7).
///
/// This mirrors `OpenCode`'s spelling, so it keeps case and `-`; it is not one of the name
/// grammars `candler_schemas::primitives` owns, and must not be folded into them.
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
) -> Result<Vec<Value>, DriverError> {
    let Some(value) = cell
        .value_str(path)
        .map_err(|error| invalid(path, error.to_string()))?
    else {
        return Ok(Vec::new());
    };
    let names = strings(path, "the list", value)?;
    if path == TOOLS_DISALLOWED {
        let has = |wanted: &str| names.iter().any(|name| name == wanted);
        if has("Edit") != has("Write") {
            return Err(unsupported(path, EDIT_WRITE_PAIR));
        }
    }
    names
        .iter()
        .map(|name| {
            opencode_action(name).map_or_else(
                || {
                    Err(unsupported(
                        path,
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
/// to `OpenCode`'s adaptive-thinking shape: `OpenCode`'s `anthropicMessages` variant builder
/// (`packages/core/src/variant.ts`, v2.2.7) resolves the same names to adaptive thinking plus
/// `output_config.effort`, and a config-declared `variants` entry is used as written. A model
/// `OpenCode` would give no such variant (see `EFFORT_MODEL`) is refused here, not left to fail
/// or be ignored at run time.
fn effort_variant(cell: &Cell<'_>) -> Result<Option<(String, Value)>, DriverError> {
    let Some(level) = cell.effort()? else {
        return Ok(None);
    };
    let accepted = claude_info(model_id(cell)?).is_some_and(|info| {
        info.takes_effort() && (level != EffortLevel::Xhigh || info.major > 4 || info.minor >= 7)
    });
    if !accepted {
        return Err(unsupported(MODEL_EFFORT, EFFORT_MODEL));
    }
    let text = level.as_str();
    Ok(Some((
        text.to_owned(), // BORROW: .to_owned() makes the owned variant id
        json!({"thinking": {"type": "adaptive", "display": "summarized"}, "effort": text}),
    )))
}

/// What the cell's settings resolve to: the pieces the command line and [`render_config`] need,
/// read once per plan. The command line is always the `--auto` one, since `bypassPermissions` is
/// the only mode accepted.
pub(crate) struct Settings {
    /// Rows added to the configuration's top-level `permissions`: the allow rows, then the deny
    /// rows, so the last matching row is a deny.
    pub(crate) permissions: Vec<Value>,
    /// The model's one added `variants` entry (id, settings), if the cell sets an effort.
    pub(crate) variant: Option<(String, Value)>,
}

/// What the cell's settings resolve to once [`answer`] has accepted them: the values the driver
/// cannot take (an effort level for a model, a tool name) are refused here.
///
/// # Errors
/// Returns [`DriverError::Unsupported`] for a setting `OpenCode` has no way to honour, and
/// [`DriverError::InvalidValue`] for a value it cannot use of a setting it does honour.
pub(crate) fn settings(cell: &Cell<'_>) -> Result<Settings, DriverError> {
    let variant = effort_variant(cell)?;
    let mut permissions = tool_permission_rows(cell, TOOLS_ALLOWED, "allow")?;
    permissions.extend(tool_permission_rows(cell, TOOLS_DISALLOWED, "deny")?);
    Ok(Settings {
        permissions,
        variant,
    })
}

/// The `opencode.json` of the run: the provider, the model, the effort variant and the
/// permission rows of `settings`, and the MCP servers. `config_dir` is the home-relative directory
/// that `XDG_CONFIG_HOME` names.
///
/// # Errors
/// Returns [`DriverError::InvalidValue`]/[`DriverError::MissingValue`] for the model and the `mcp`
/// value.
pub(crate) fn render_config(
    cell: &Cell<'_>,
    settings: Settings,
    config_dir: &str,
    base_url_var: &str,
    token_var: &str,
) -> Result<PreparedFile, DriverError> {
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
    if let Some(servers) = candler_harness::mcp::servers(cell)? {
        let mut translated = Map::new();
        for (name, entry) in &servers {
            translated.insert(name.clone(), server(entry)); // BORROW: .clone() copies the name
        }
        root.insert(
            MCP.to_owned(), // BORROW: .to_owned() makes the owned key
            json!({ "servers": translated }),
        );
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
