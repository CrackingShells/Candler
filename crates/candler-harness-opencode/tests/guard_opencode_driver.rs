#![cfg(test)]
//! Guards of the `OpenCode` driver beyond the shared suite: the command line, the configuration
//! file, the state paths, the stop, the collect, the readers and the capabilities.
//!
//! Each expectation is a literal written by hand from `opencode_facts`, from the recorded fixtures,
//! or from what the real binary printed for the same file (`tests/fixtures/config`).

use std::collections::BTreeSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr as _;

use candler_harness::driver::{Disposition, DriverError, HarnessDriver};
use candler_harness::reader::FrameIn;
use candler_harness::skills::SkillTree;
use candler_harness::spec::{
    Action, Cell, CollectSpec, Observed, ProcessEnd, ProcessExit, StateKind, StopSignal, Workspace,
};
use candler_harness_opencode::driver::OpenCodeDriver;
use candler_harness_opencode::skills::OpenCodeSkills;
use candler_schemas::execution::{GitCommit, TreatmentRef};
use candler_schemas::primitives::FieldPath;
use candler_schemas::process::ExitStatus;
use candler_schemas::raw::{FrameData, RawFrame, RawRef, StreamName};
use candler_schemas::refusal::{Refusal, RefusalCode, Side};
use candler_schemas::trace::{EndedBy, Session, SessionEvent, SessionId};
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::{Value, json};

// TRAIT_OBJECT: a test returns any error its steps raise; the error is only printed
type Res<T> = Result<T, Box<dyn Error>>;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn stream_file(case: &str, file: &str) -> Res<Vec<u8>> {
    Ok(std::fs::read(
        fixtures().join("streams").join(case).join(file),
    )?)
}

/// The cell values of a run: the model and the prompt, plus `extra`.
fn values(extra: &[(&str, Value)]) -> Res<Vec<ResolvedValue>> {
    let mut all = vec![
        ("model.id", json!("claude-x")),
        ("model.endpoint", json!("https://api.example.test")),
        ("permission_mode", json!("bypassPermissions")),
        ("prompt.template", json!("Do {thing}")),
        ("prompt.values", json!({"thing": "it"})),
    ];
    // An extra value replaces the default at the same path.
    all.retain(|(path, _)| !extra.iter().any(|(over, _)| over == path));
    all.extend(extra.iter().map(|(path, value)| (*path, value.clone()))); // BORROW: .clone() copies the value
    all.into_iter()
        .map(|(path, value)| {
            Ok(ResolvedValue::recorded(
                FieldPath::from_str(path)?,
                JsonValue::new(value)?,
            ))
        })
        .collect()
}

/// The command of a cell, or the driver's refusal.
fn command_of(extra: &[(&str, Value)]) -> Res<Result<Vec<String>, DriverError>> {
    let values = values(extra)?;
    let driver = OpenCodeDriver::new()?;
    let workspace = Workspace::new(Path::new("."));
    Ok(driver
        .command(&Cell::new(&values), &workspace)
        .map(|spec| spec.argv().to_vec()))
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect() // BORROW: .to_owned() makes owned text
}

fn prepared_config(extra: &[(&str, Value)]) -> Res<(candler_harness::spec::PreparedFile, Value)> {
    let values = values(extra)?;
    let driver = OpenCodeDriver::new()?;
    let workspace = Workspace::new(Path::new("."));
    let prepared = driver.prepare(&Cell::new(&values), &workspace)?;
    let file = prepared
        .files
        .into_iter()
        .find(|file| file.path.as_str().ends_with("opencode/opencode.json"))
        .ok_or("no opencode.json")?;
    let parsed: Value = serde_json::from_slice(&file.contents)?;
    Ok((file, parsed))
}

/// The refusal of a command whose cell sets `extra`, read through its [`Refusal`] and its text.
fn unsupported_of_cell(
    extra: &[(&str, Value)],
) -> Res<(RefusalCode, Side, Option<String>, String)> {
    let error = command_of(extra)?
        .err()
        .ok_or_else(|| format!("{extra:?}: expected a refusal, got a command"))?;
    Ok((
        Refusal::code(&error),
        Refusal::side(&error),
        Refusal::path(&error),
        error.to_string(),
    ))
}

/// What one case of the settings table expects of a cell.
enum Expect {
    /// The command and the file map the setting: `permissions` is the exact top-level `permissions`
    /// of the file (`Null` for none) and `variants` the model's `variants` entry (`Null` for none,
    /// else the command line also selects it with `#<id>`).
    Maps { permissions: Value, variants: Value },
    /// The cell is refused as unsupported at `pointer`, the message holding `needle`.
    Refuses {
        pointer: &'static str,
        needle: &'static str,
    },
    /// The cell is refused as an invalid value.
    Invalid,
}

/// One case: the table row it answers, the cell values it sets, and what follows.
struct Case {
    setting: &'static str,
    value: &'static str,
    extra: Vec<(&'static str, Value)>,
    expect: Expect,
}

fn case(
    setting: &'static str,
    value: &'static str,
    extra: Vec<(&'static str, Value)>,
    expect: Expect,
) -> Case {
    Case {
        setting,
        value,
        extra,
        expect,
    }
}

/// A mapping that writes `permissions` and no variant.
fn maps(permissions: Value) -> Expect {
    Expect::Maps {
        permissions,
        variants: Value::Null,
    }
}

/// Permission rows of one effect, one per action.
fn perm_rows(effect: &str, actions: &[&str]) -> Value {
    Value::Array(
        actions
            .iter()
            .map(|action| json!({"action": action, "resource": "*", "effect": effect}))
            .collect(),
    )
}

fn refuses(pointer: &'static str, needle: &'static str) -> Expect {
    Expect::Refuses { pointer, needle }
}

/// The variants entry an effort `level` writes.
fn variants_of(level: &str) -> Value {
    json!([{"id": level, "settings": {
        "thinking": {"type": "adaptive", "display": "summarized"},
        "effort": level,
    }}])
}

/// Every case of the settings table. Each expectation is written by hand: the actions from the
/// tools' registered permissions, the reasons from Claude Code's and `OpenCode`'s own sources.
fn table_cases() -> Vec<Case> {
    let mode = "permission_mode";
    let opus = ("model.id", json!("claude-opus-4-7"));
    let mut cases = vec![
        case(mode, "bypassPermissions", vec![], maps(Value::Null)),
        case(
            mode,
            "plan",
            vec![(mode, json!("plan"))],
            refuses("/fixed/permission_mode", "read-only shell"),
        ),
        // Plan with a tool list, a subagent or an MCP tool stays refused: nothing reopens it.
        case(
            mode,
            "plan",
            vec![(mode, json!("plan")), ("tools.allowed", json!(["Edit"]))],
            refuses("/fixed/permission_mode", "read-only shell"),
        ),
        case(
            mode,
            "plan",
            vec![(mode, json!("plan")), ("tools.allowed", json!(["Task"]))],
            refuses("/fixed/permission_mode", "`subagent` child can edit"),
        ),
        case(
            mode,
            "plan",
            vec![
                (mode, json!("plan")),
                ("tools.allowed", json!(["mcp__srv__tool"])),
            ],
            refuses("/fixed/permission_mode", "read-only shell"),
        ),
        case(
            mode,
            "default",
            vec![(mode, json!("default"))],
            refuses("/fixed/permission_mode", "built-in, non-configurable set"),
        ),
        case(
            mode,
            "acceptEdits",
            vec![(mode, json!("acceptEdits"))],
            refuses("/fixed/permission_mode", "knows no working directory"),
        ),
        case(
            mode,
            "*",
            vec![(mode, json!("dontAsk"))],
            refuses("/fixed/permission_mode", "only `bypassPermissions`"),
        ),
        case(
            "budget.per_run.max_cost_usd",
            "*",
            vec![("budget.per_run.max_cost_usd", json!(1.5))],
            refuses("/budget/per_run/max_cost_usd", "no cost cap"),
        ),
        case(
            "budget.per_run.turns",
            "*",
            vec![("budget.per_run.turns", json!(5))],
            refuses("/budget/per_run/turns", "`error_max_turns`"),
        ),
        // The models and levels the effort does not map for.
        case(
            "model.effort",
            "*",
            vec![("model.effort", json!("extreme")), opus.clone()], // BORROW: .clone() copies the value
            Expect::Invalid,
        ),
    ];
    // `ultracode` is a Claude Code level the Claude Code driver passes through; OpenCode refuses it.
    cases.push(case(
        "model.effort",
        "ultracode",
        vec![("model.effort", json!("ultracode")), opus.clone()], // BORROW: .clone() copies the value
        refuses("/fixed/model.effort", "no equivalent of that mode"),
    ));
    for level in ["low", "medium", "high", "xhigh", "max"] {
        cases.push(case(
            "model.effort",
            level,
            vec![("model.effort", json!(level)), opus.clone()], // BORROW: .clone() copies the value
            Expect::Maps {
                permissions: Value::Null,
                variants: variants_of(level),
            },
        ));
    }
    // 4.6 takes `high` but not `xhigh`.
    cases.push(case(
        "model.effort",
        "high",
        vec![
            ("model.effort", json!("high")),
            ("model.id", json!("claude-opus-4-6")),
        ],
        Expect::Maps {
            permissions: Value::Null,
            variants: variants_of("high"),
        },
    ));
    for (id, level) in [
        ("claude-opus-4-6", "xhigh"),
        ("claude-3-7-sonnet", "high"),
        ("claude-opus-4-5", "high"),
        ("claude-sonnet-4-20250514", "high"),
        ("claude-opus-4-20250514", "high"),
        ("claude-sonnet-4-5-20250929", "high"),
        // Dated 3.x ids: the date is no major, these models take no adaptive thinking.
        ("claude-3-5-sonnet-20241022", "high"),
        ("claude-3-7-sonnet-20250219", "high"),
        ("claude-3-opus-20240229", "high"),
        // Haiku, even at a version that would otherwise take an effort.
        ("claude-haiku-4-5", "high"),
        ("claude-haiku-4-6", "high"),
        ("claude-haiku-5", "high"),
        ("gpt-x", "high"),
    ] {
        cases.push(case(
            "model.effort",
            "<model>",
            vec![("model.effort", json!(level)), ("model.id", json!(id))],
            refuses("/fixed/model.effort", "version 4.6 or later"),
        ));
    }
    // The tool lists: one row per name, at the action the tool asserts.
    let tools = [
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
    for (path, effect) in [("tools.allowed", "allow"), ("tools.disallowed", "deny")] {
        let pointer = if effect == "allow" {
            "/fixed/tools.allowed"
        } else {
            "/fixed/tools.disallowed"
        };
        for (name, action) in tools {
            let expect = if effect == "deny" && (name == "Edit" || name == "Write") {
                refuses(pointer, "list both")
            } else {
                maps(perm_rows(effect, &[action]))
            };
            cases.push(case(path, name, vec![(path, json!([name]))], expect));
        }
        for (name, action) in [
            ("mcp__srv__tool", "srv_tool"),
            // `OpenCode` spells every character outside `[A-Za-z0-9_-]` as `_`.
            ("mcp__my.server__a b", "my_server_a_b"),
            ("mcp__my_server__tool", "my_server_tool"),
        ] {
            cases.push(case(
                path,
                "mcp__<server>__<tool>",
                vec![(path, json!([name]))],
                maps(perm_rows(effect, &[action])),
            ));
        }
        for name in ["NotebookEdit", "MultiEdit"] {
            cases.push(case(
                path,
                "*",
                vec![(path, json!([name]))],
                refuses(pointer, "is not a tool OpenCode has an action for"),
            ));
        }
    }
    cases.push(case(
        "tools.disallowed",
        "Edit+Write",
        vec![("tools.disallowed", json!(["Write", "Edit"]))],
        maps(perm_rows("deny", &["edit", "edit"])),
    ));
    // Both lists: the deny rows come last, so the last matching row is a deny.
    cases.push(case(
        "tools.disallowed",
        "with tools.allowed",
        vec![
            ("tools.allowed", json!(["Bash", "Edit"])),
            ("tools.disallowed", json!(["Bash"])),
        ],
        maps(json!([
            {"action": "shell", "resource": "*", "effect": "allow"},
            {"action": "edit", "resource": "*", "effect": "allow"},
            {"action": "shell", "resource": "*", "effect": "deny"},
        ])),
    ));
    cases
}

/// Guards: R01 §`OpenCode` v2, §Run manifest — the declared settings table and what the driver does
/// are one set. Every row of `settings_table()` has a case below and every case answers a row
/// (dropping either side turns this red), a mapped row's case maps and a refused row's is refused,
/// at the setting's pointer in the experiment file, in a message naming the setting, the harness
/// and its version, and the same refusal comes from `prepare` as from `command`. Mapped: the run is
/// `run --standalone --format json --auto -m <provider>/<model>` with the rendered prompt after
/// `--` (a prompt that starts with `-` stays a message), and no `--agent`; an effort suffixes the
/// model with `#<level>` and writes the `variants` entry; each tool name is one permission row at
/// the action the tool asserts, allow rows before deny rows. Refused: every mode but
/// `bypassPermissions`, the cost and the turn cap, an effort the model cannot take, a tool name
/// with no action, a lone `Edit` or `Write` in the deny list.
/// Oracle: `opencode_facts` row 2 and the real binary, which refused `run … "--hello x"` as an
/// unrecognised flag and took the same text after `--`; the actions are the permissions the tools
/// register in `OpenCode` v2.2.7 (`write.ts:49`, `edit.ts`, `glob.ts`, `shell.ts`, `todo.ts`); the
/// MCP spelling is `tool/runtime.ts:288-293`; the baseline `{*, *, allow}` and last-match-wins are
/// `schema/src/agent.ts:39-50` and `permission.ts:98`; the effort shape is `variant.ts:325-339`
/// and its wire effect the `oc_effort_*` fixtures; what Claude Code's modes allow is its permission
/// modes and permissions pages (`default` reads and a built-in read-only shell set, `acceptEdits`
/// adds edits and mkdir, touch, rm, rmdir, mv, cp, sed in the working directory, `plan` reads and
/// read-only shell, `--allowedTools` only auto-approves); the turn cap is refused for what the
/// `oc_step_cap` fixture records (the last request tool-less, the run completed); the pointers are
/// written by hand from `unknown_path_pointer` and `file_field`.
#[test]
fn opencode_command_line_maps_or_refuses_every_setting() -> Res<()> {
    let plain = strings(&[
        "opencode",
        "run",
        "--standalone",
        "--format",
        "json",
        "--auto",
        "-m",
        "candler/claude-x",
        "--",
        "Do it",
    ]);
    assert_eq!(command_of(&[])??, plain);
    let dashed = command_of(&[("prompt.template", json!("--hello {thing}"))])??;
    assert_eq!(dashed.get(dashed.len() - 2).map(String::as_str), Some("--"));
    assert_eq!(dashed.last().map(String::as_str), Some("--hello it"));

    let cases = table_cases();
    let driver = OpenCodeDriver::new()?;
    let rows = driver.settings_table();
    let declared: BTreeSet<(String, String)> = rows
        .iter()
        .map(|row| (row.setting.clone(), row.value.clone())) // BORROW: .clone() copies the key out of the row
        .collect();
    let covered: BTreeSet<(String, String)> = cases
        .iter()
        .map(|c| (c.setting.to_owned(), c.value.to_owned())) // BORROW: .to_owned() makes the owned key
        .collect();
    assert_eq!(
        declared, covered,
        "the declared rows and the cases are one set"
    );
    for row in &rows {
        for c in cases
            .iter()
            .filter(|c| c.setting == row.setting && c.value == row.value)
        {
            let mapped = matches!(c.expect, Expect::Maps { .. });
            assert_eq!(
                mapped,
                row.disposition == Disposition::Mapped,
                "{} {}: the case and the row disagree",
                row.setting,
                row.value
            );
        }
    }

    for c in &cases {
        let label = format!("{} {} {:?}", c.setting, c.value, c.extra);
        match &c.expect {
            Expect::Maps {
                permissions,
                variants,
            } => {
                let argv = command_of(&c.extra)??;
                assert!(argv.contains(&"--auto".to_owned()), "{label}: {argv:?}"); // BORROW: .to_owned() makes an owned needle
                assert!(!argv.contains(&"--agent".to_owned()), "{label}: {argv:?}"); // BORROW: .to_owned() makes an owned needle
                let (_, parsed) = prepared_config(&c.extra)?;
                assert_eq!(&parsed["permissions"], permissions, "{label}");
                assert!(parsed.get("agents").is_none(), "{label}: no agents block");
                let model = c
                    .extra
                    .iter()
                    .find(|(path, _)| *path == "model.id")
                    .and_then(|(_, id)| id.as_str())
                    .unwrap_or("claude-x");
                let got = &parsed["providers"]["candler"]["models"][model]["variants"];
                if variants.is_null() {
                    assert!(got.is_null(), "{label}: {got}");
                } else {
                    let id = variants[0]["id"].as_str().ok_or("variant id")?;
                    assert!(
                        argv.contains(&format!("candler/{model}#{id}")),
                        "{label}: {argv:?}"
                    );
                    assert_eq!(got, variants, "{label}");
                }
            }
            Expect::Refuses { pointer, needle } => {
                let (code, side, at, message) = unsupported_of_cell(&c.extra)?;
                assert_eq!(code.as_str(), "experiment.setting_unsupported", "{label}");
                assert_eq!(side, Side::Experiment, "{label}");
                assert_eq!(at.as_deref(), Some(*pointer), "{label}");
                for want in [pointer, needle, &"OpenCode", &"2.2.7"] {
                    assert!(message.contains(want), "{label}: `{want}` in `{message}`");
                }
                assert!(prepared_config(&c.extra).is_err(), "{label}: prepare too");
            }
            Expect::Invalid => {
                let error = command_of(&c.extra)?
                    .err()
                    .ok_or_else(|| format!("{label}: expected a refusal"))?;
                assert!(matches!(error, DriverError::InvalidValue { .. }), "{label}");
                assert!(prepared_config(&c.extra).is_err(), "{label}: prepare too");
            }
        }
    }
    Ok(())
}

/// Guards: R01 §Agent environment and secrets, §`OpenCode` v2, I7 — the written config: the
/// provider's address and key are references to the variables the core sets, in a home-rooted
/// file inside the config directory the state paths name, and the file is the one the real binary
/// accepted; the gateway is named by two variables and nothing else of the configuration is a
/// value; `codemode` is a cell value read per server, absent written as on, `false` making the
/// server's tools direct tools; an MCP server shape the driver cannot translate is refused, not
/// guessed.
/// Oracle: `tests/fixtures/config/opencode.json`, run through `opencode debug config` in the base
/// image (`debug_config.out`: `{env:…}/v1` expanded to `http://gateway:8080/r/tag/run/key/v1`);
/// the names are the ones that fixture refers to; `opencode_facts` row 17 (omitted is on, `false`
/// is direct) and the V2 shape the real binary printed for `mcp.servers.<name>.codemode`; the
/// shapes the real binary printed (`local` with a command list, `remote` with a url) are the only
/// ones recorded.
#[test]
fn opencode_prepare_writes_the_provider_mcp_codemode_and_gateway_names() -> Res<()> {
    // The provider, the model and the config file's place.
    let (file, parsed) = prepared_config(&[(
        "mcp",
        json!({
            "loc": {"command": "uvx", "args": ["srv"], "env": {"K": "v"}, "codemode": false},
            "rem": {"type": "http", "url": "http://h/mcp", "headers": {"X": "y"}}
        }),
    )])?;
    assert_eq!(file.root, candler_harness::spec::FileRoot::Home);
    assert_eq!(file.path.as_str(), ".xdg/config/opencode/opencode.json");
    let recorded: Value = serde_json::from_slice(&std::fs::read(
        fixtures().join("config").join("opencode.json"),
    )?)?;
    assert_eq!(parsed, recorded);

    // The gateway names are the ones the config refers to.
    let gateway = OpenCodeDriver::new()?.gateway().ok_or("no gateway names")?;
    let config = String::from_utf8(std::fs::read(
        fixtures().join("config").join("opencode.json"),
    )?)?;
    assert!(config.contains(&format!("{{env:{}}}/v1", gateway.base_url)));
    assert!(config.contains(&format!("{{env:{}}}", gateway.auth_token)));
    assert_eq!(gateway.reserved, vec![gateway.base_url, gateway.auth_token]);

    // `codemode` is read from the cell, absent written as on.
    let servers = |codemode: Option<Value>| {
        let mut server = json!({"command": "uvx", "args": []});
        if let (Some(value), Some(map)) = (codemode, server.as_object_mut()) {
            map.insert("codemode".to_owned(), value); // BORROW: .to_owned() makes the owned key
        }
        json!({"s": server})
    };
    let mut written = Vec::new();
    for codemode in [None, Some(json!(true)), Some(json!(false))] {
        let (_, parsed) = prepared_config(&[("mcp", servers(codemode))])?;
        written.push(parsed["mcp"]["servers"]["s"]["codemode"].clone()); // BORROW: .clone() copies the value
    }
    assert_eq!(written, [json!(true), json!(true), json!(false)]);
    assert!(
        prepared_config(&[("mcp", servers(Some(json!("no"))))]).is_err(),
        "a codemode that is not a boolean is refused"
    );
    let (on, off) = (
        prepared_config(&[("mcp", servers(Some(json!(true))))])?.0,
        prepared_config(&[("mcp", servers(Some(json!(false))))])?.0,
    );
    assert_ne!(on.contents, off.contents, "codemode changes the treatment");

    // An MCP shape the driver cannot translate is refused, not guessed.
    let (_, parsed) = prepared_config(&[("mcp", json!({"s": {"command": "uvx"}}))])?;
    assert_eq!(parsed["mcp"]["servers"]["s"]["type"], "local");
    prepared_config(&[(
        "mcp",
        json!({"a": {"type": "stdio", "command": "uvx", "args": ["x"], "env": {"K": "v"}, "codemode": true},
               "b": {"type": "sse", "url": "http://h", "headers": {"X": "y"}, "codemode": false}}),
    )])?;
    for servers in [
        json!({"s": {"type": "websocket", "url": "ws://h"}}),
        json!({"s": {"type": "websocket", "command": "uvx"}}),
        json!({"s": {"args": ["a"]}}),
        json!({"s": {"command": "uvx", "timeout": 5}}),
        json!({"s": {"command": "uvx", "enabled": false}}),
        json!({"s": {"command": "uvx", "cwd": "/x"}}),
        json!({"s": {"command": "uvx", "tools": ["a"]}}),
        json!({"s": {"command": "uvx", "headers": {"X": "y"}}}),
        json!({"s": {"type": "http", "url": "http://h", "args": ["a"]}}),
        json!({"s": {"type": "http", "url": "http://h", "env": {"K": "v"}}}),
        json!({"s": "uvx"}),
    ] {
        assert!(prepared_config(&[("mcp", servers)]).is_err());
    }
    Ok(())
}

/// Guards: R01 §Process supervision — a run keeps its state in its own home: the four XDG
/// directories and the database file.
/// Oracle: `opencode debug paths` with `HOME`, `XDG_*` and `OPENCODE_DB` set, as printed by the
/// real binary in the base image (data, config, cache, state, `db` = the file).
#[test]
fn opencode_state_paths_are_the_recorded_layout() -> Res<()> {
    let got: Vec<(String, StateKind, String)> = OpenCodeDriver::new()?
        .state_paths()
        .into_iter()
        .map(|p| (p.env, p.kind, p.subpath.as_str().to_owned())) // BORROW: .to_owned() makes owned text
        .collect();
    let want = [
        ("XDG_DATA_HOME", StateKind::Dir, ".xdg/data"),
        ("XDG_CONFIG_HOME", StateKind::Dir, ".xdg/config"),
        ("XDG_CACHE_HOME", StateKind::Dir, ".xdg/cache"),
        ("XDG_STATE_HOME", StateKind::Dir, ".xdg/state"),
        ("OPENCODE_DB", StateKind::File, ".xdg/data/oc.db"),
    ]
    .map(|(e, k, p)| (e.to_owned(), k, p.to_owned())) // BORROW: .to_owned() makes owned text
    .to_vec();
    assert_eq!(got, want);
    Ok(())
}

fn sample_id() -> Res<SessionId> {
    Ok("ses_e32c17b4fffe2bjBVM2E3xAaLZ".parse()?)
}

/// Guards: R01 §`OpenCode` v2 — the export is collected inside the isolation, for the root session
/// the live reader saw, with `--standalone`.
/// Oracle: `opencode_facts` rows 4 and 6 and F6 (`session export --standalone <id>` prints the JSON
/// on stdout, and needs the run's `OPENCODE_DB`).
#[test]
fn opencode_collect_exports_the_root_session_inside() -> Res<()> {
    let driver = OpenCodeDriver::new()?;
    let workspace = Workspace::new(Path::new("."));
    let root = sample_id()?;
    let child: SessionId = "ses_e32c17b4fffe2bjBVM2E3xAaLZ/call1".parse()?;
    let dash: SessionId = "-sanitize".parse()?;
    let observed = Observed::new(vec![dash, child, root]);
    let specs = driver.collect(&workspace, &observed);
    assert_eq!(specs.len(), 1);
    let Some(CollectSpec::Exec { spec, name, .. }) = specs.first() else {
        return Err("the export is not collected by an Exec".into());
    };
    assert_eq!(name.as_str(), "export");
    assert_eq!(
        spec.argv(),
        strings(&[
            "opencode",
            "session",
            "export",
            "--standalone",
            "ses_e32c17b4fffe2bjBVM2E3xAaLZ"
        ])
    );
    assert_eq!(
        driver.collect(&workspace, &Observed::new(Vec::new())),
        Vec::new()
    );
    Ok(())
}

/// Guards: R01 §Harness drivers — the live reader reports the session id and the stream's last
/// word, emits no event, and finishes with the root session it saw; a stream with no session id is
/// incomplete. Oracle: the recorded stdout of `completed_o1`, `sigterm_mid_tool` and
/// `unknown_model_local` (first line, `expected.toml`) and the hand-listed classes.
#[test]
fn opencode_live_reader_reports_id_and_last_word() -> Res<()> {
    let driver = OpenCodeDriver::new()?;
    let stdout: StreamName = "stdout".parse()?;
    let feed_all = |case: &str| -> Res<(Observed, Vec<Session>, usize)> {
        let mut reader = driver.live_reader().ok_or("no live reader")?;
        let mut events = 0_usize;
        for (index, line) in stream_file(case, "stdout.jsonl")?
            .split_inclusive(|b| *b == b'\n')
            .enumerate()
        {
            let frame = RawFrame::new(None, FrameData::from_bytes(line));
            let at = RawRef::new(stdout.clone(), u64::try_from(index)?); // BORROW: .clone() copies the name
            events += reader.feed(FrameIn::new(at, None, &frame))?.len();
        }
        let observed = reader.observed();
        Ok((observed, reader.finish()?, events))
    };
    let (observed, sessions, events) = feed_all("completed_o1")?;
    assert_eq!(events, 0, "tool calls come from the export");
    assert_eq!(
        observed.session_ids,
        vec!["ses_e32c3735effeWwyJD6JU8Pywnx".parse::<SessionId>()?]
    );
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions.first().map(|s| s.ended_by().clone()),
        Some(EndedBy::Completed)
    ); // BORROW: .clone() copies the end
    let (_, sessions, _) = feed_all("sigterm_mid_tool")?;
    assert_eq!(
        sessions.first().map(|s| s.ended_by().clone()),
        Some(EndedBy::Unknown)
    ); // BORROW: .clone() copies the end
    let (observed, _, _) = feed_all("unknown_model_local")?;
    assert_eq!(
        observed.session_ids.len(),
        1,
        "the id is on the error event"
    );
    let empty = driver.live_reader().ok_or("no live reader")?;
    assert!(empty.finish().is_err());
    Ok(())
}

/// What the export reader makes of `bytes`: its events and sessions.
fn read_export(bytes: &[u8]) -> Res<(Vec<SessionEvent>, Vec<Session>)> {
    let driver = OpenCodeDriver::new()?;
    let export: StreamName = "export".parse()?;
    let mut reader = driver.offline_reader(&export).ok_or("no export reader")?;
    let frame = RawFrame::new(None, FrameData::from_bytes(bytes));
    let events = reader.feed(FrameIn::new(RawRef::new(export, 0), None, &frame))?;
    Ok((events, reader.finish()?))
}

/// The kind of each event as the trace file writes it, without the cost and the time.
fn kinds(events: &[SessionEvent]) -> Res<Vec<Value>> {
    events
        .iter()
        .map(|entry| {
            let mut kind = serde_json::to_value(&entry.event.kind)?;
            if let Some(map) = kind.as_object_mut() {
                map.remove("cost_usd");
            }
            Ok(kind)
        })
        .collect()
}

fn usage(scope: &str, input: u64, output: u64) -> Value {
    json!({"kind": "usage", "scope": scope, "input_tokens": input, "output_tokens": output,
           "cache_read_tokens": 0, "cache_creation_tokens": 0})
}

/// Guards: R01 §`OpenCode` v2, §Ends, §Run manifest (usage) — the export reader, a table over the
/// fixtures: the trace (the user and assistant messages, the tool call and its result paired by
/// the call id, per-message usage and the session's total, which holds the requests the stream
/// does not show); errors (a tool the interrupt cut short is an error result, and a run that
/// failed before any step keeps its terminal error as a notice); cache, cost and reasoning (cache
/// reads and writes in their own fields, the cost as a number, reasoning tokens kept beside the
/// usage, a reasoning block as its own event, and a tool with no recorded origin as `other`); a
/// child session (`info.parentID`) is never a root, its id its parent's, `/`, its own; a document
/// that is not an export is refused, and one that names no outcome ends `unknown`.
/// Oracle: `completed_o1/export.json`, read by hand: two assistant messages of 100/20 tokens, one
/// `shell` call that completed, `info.tokens` 300/60; `interrupt_api/export.json` (`state.status`
/// is `error`, outcome `interrupted`) and `unknown_model_local/export.json` (no assistant message,
/// `idle.error`), read by hand; a document written by hand in the recorded shape, with the numbers
/// `opencode_facts` reports for the real-model run (cost 0.00054, 4704 cached tokens); the schema's
/// rule that a child's id is its parent's id, `/`, a call id; hand-written documents.
#[test]
fn opencode_export_reader_covers_trace_errors_cache_cost_and_child_sessions() -> Res<()> {
    let (events, sessions) = read_export(&stream_file("completed_o1", "export.json")?)?;
    assert_eq!(
        kinds(&events)?,
        vec![
            json!({"kind": "user_message", "text": "Run the scripted command."}),
            json!({"kind": "assistant_message", "text": "I will run a command."}),
            json!({"kind": "tool_call", "name": "shell", "origin": {"kind": "builtin"},
                   "argv": ["echo hello-from-tool > /work/o1.txt && cat /work/o1.txt"],
                   "wrapped_calls": []}),
            json!({"kind": "tool_result", "is_error": false}),
            usage("message", 100, 20),
            json!({"kind": "assistant_message", "text": "done: the tool finished."}),
            usage("message", 100, 20),
            usage("session_total", 300, 60),
        ]
    );
    let spans: Vec<Option<String>> = events
        .iter()
        .map(|e| e.event.span.as_ref().map(|s| s.as_str().to_owned())) // BORROW: .to_owned() makes owned text
        .collect();
    assert_eq!(spans.get(2), Some(&Some("toolu_fake_oc".to_owned()))); // BORROW: .to_owned() makes owned text
    assert_eq!(
        spans.get(2),
        spans.get(3),
        "the call and its result share the id"
    );
    let seqs: Vec<u64> = events.iter().map(|e| e.event.seq).collect();
    assert_eq!(seqs, (0..8).collect::<Vec<u64>>());
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions.first().and_then(Session::model),
        Some("o1"),
        "the model as OpenCode names it"
    );

    // Errors: a tool the interrupt cut short is an error result, and a run that failed before any
    // step keeps its terminal error as a notice.
    let (events, _) = read_export(&stream_file("interrupt_api", "export.json")?)?;
    let results: Vec<Value> = kinds(&events)?
        .into_iter()
        .filter(|k| k["kind"] == "tool_result")
        .collect();
    assert_eq!(
        results,
        vec![json!({"kind": "tool_result", "is_error": true})]
    );
    let (events, sessions) = read_export(&stream_file("unknown_model_local", "export.json")?)?;
    let labels: Vec<String> = kinds(&events)?
        .iter()
        .map(|k| k["kind"].as_str().unwrap_or_default().to_owned()) // BORROW: .to_owned() makes owned text
        .collect();
    assert_eq!(labels, ["user_message", "notification", "usage"]);
    let notice = events.get(1).ok_or("no notice")?;
    assert_eq!(notice.event.ext["error"]["type"], "provider.no-route");
    assert_eq!(sessions.len(), 1);

    // Cache, cost and reasoning: tokens and cost come through as OpenCode counts them.
    let document = json!({
        "info": {"id": "ses_h", "model": {"id": "m", "providerID": "p"}, "cost": 0.00067,
                 "tokens": {"input": 900, "output": 40, "reasoning": 7,
                            "cache": {"read": 4704, "write": 11}},
                 "outcome": "succeeded", "time": {"created": 1_794_444_660_066_u64, "updated": 1_794_444_660_390_u64}},
        "messages": [
            {"id": "m1", "type": "user", "text": "go", "time": {"created": 1_794_444_660_244_u64}},
            {"id": "m2", "type": "assistant", "finish": "stop", "cost": 0.00054,
             "tokens": {"input": 300, "output": 20, "reasoning": 7, "cache": {"read": 4704, "write": 11}},
             "time": {"created": 1_794_444_660_271_u64},
             "content": [
                 {"type": "reasoning", "text": "thinking"},
                 {"type": "tool", "id": "chatcmpl-tool-ab", "name": "probe-plain_ping",
                  "state": {"status": "completed", "input": {}}}
             ]},
            {"id": "m3", "type": "idle", "outcome": "succeeded"}
        ]
    });
    let (events, _) = read_export(&serde_json::to_vec(&document)?)?;
    let usages: Vec<&SessionEvent> = events
        .iter()
        .filter(|e| {
            matches!(
                e.event.kind,
                candler_schemas::trace::EventKind::Usage { .. }
            )
        })
        .collect();
    let first = serde_json::to_value(&usages.first().ok_or("no usage")?.event.kind)?;
    assert_eq!(first["input_tokens"], 300);
    assert_eq!(first["cache_read_tokens"], 4704);
    assert_eq!(first["cache_creation_tokens"], 11);
    assert_eq!(first["cost_usd"], 0.00054);
    assert_eq!(
        usages.first().ok_or("no usage")?.event.ext["reasoning_tokens"],
        7
    );
    let total = serde_json::to_value(&usages.last().ok_or("no usage")?.event.kind)?;
    assert_eq!(
        (total["scope"].as_str(), total["input_tokens"].as_u64()),
        (Some("session_total"), Some(900))
    );
    let all = kinds(&events)?;
    assert!(all.iter().any(|k| k["kind"] == "reasoning"));
    let call = all
        .iter()
        .find(|k| k["kind"] == "tool_call")
        .ok_or("no call")?;
    assert_eq!(call["origin"], json!({"kind": "other"}));

    // A child session (`info.parentID`) is never a root.
    let child_document = json!({
        "info": {"id": "ses_child", "parentID": "ses_parent", "outcome": "succeeded"},
        "messages": [{"id": "m1", "type": "user", "text": "go"}]
    });
    let (child_events, child_sessions) = read_export(&serde_json::to_vec(&child_document)?)?;
    let child = child_sessions.first().ok_or("no session")?;
    assert_eq!(child.id().as_str(), "ses_parent/ses_child");
    assert_eq!(
        child.parent_session().map(SessionId::as_str),
        Some("ses_parent")
    );
    assert!(child_events.iter().all(|e| e.session == *child.id()));

    // A document that is not an export is refused, and one that names no outcome ends `unknown`.
    assert!(read_export(b"not json").is_err());
    assert!(read_export(br#"{"messages": []}"#).is_err());
    let (_, sessions) = read_export(br#"{"info": {"id": "ses_x"}, "messages": []}"#)?;
    assert_eq!(
        sessions.first().map(|s| s.ended_by().clone()),
        Some(EndedBy::Unknown)
    ); // BORROW: .clone() copies the end
    Ok(())
}

/// Guards: R01 §Harness drivers — the version probe and its parse.
/// Oracle: `opencode --version` printed `opencode v2.2.7` in the base image.
#[test]
fn opencode_version_is_parsed_from_the_probe() -> Res<()> {
    let driver = OpenCodeDriver::new()?;
    assert_eq!(
        driver.version_probe().argv(),
        strings(&["opencode", "--version"])
    );
    assert_eq!(
        driver.parse_version(b"opencode v2.2.7\n")?,
        semver::Version::new(2, 2, 7)
    );
    for bad in [
        &b"command not found"[..],
        b"2.2.7",
        b"opencode 2.2.7",
        b"v2.2.7",
        b"see opencode v2.2.7",
        b"opencode v2.2.8",
        b"opencode v2.3.0",
        b"opencode v1.9.0",
    ] {
        assert!(
            driver.parse_version(bad).is_err(),
            "{}",
            String::from_utf8_lossy(bad)
        );
    }
    assert!(driver.supported().matches(&semver::Version::new(2, 2, 7)));
    for other in [(2, 2, 8), (2, 3, 0), (1, 9, 0)] {
        assert!(
            !driver
                .supported()
                .matches(&semver::Version::new(other.0, other.1, other.2))
        );
    }
    Ok(())
}

/// Guards: R01 §Harness drivers (capabilities) — the driver declares no session, no census, no
/// budget and no in-harness guard, and declares that execution lives outside the child.
/// Oracle: the leaf's rule, `opencode_facts` F1 (no tool inventory), F2 (no turn or cost flag) and
/// row 9 (tools run in the `serve` child).
#[test]
fn opencode_capabilities_declare_no_session_and_outside_execution() -> Res<()> {
    let driver = OpenCodeDriver::new()?;
    let caps = driver.capabilities();
    assert!(!caps.session && !caps.census && !caps.in_harness_guard);
    assert!(!caps.budgets.turns && !caps.budgets.cost);
    assert!(caps.execution_outside_child && caps.graceful_stop);
    assert!(!caps.live_events, "the live reader emits no event");
    let values = values(&[])?;
    let workspace = Workspace::new(Path::new("."));
    assert!(driver.session(&Cell::new(&values), &workspace)?.is_none());
    assert!(driver.session_tracker().is_none());
    assert!(driver.census_stream().is_none());
    Ok(())
}

fn git(repo: &Path, args: &[&str]) -> Res<Vec<u8>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned().into());
    }
    Ok(out.stdout)
}

/// Guards: R01 §`OpenCode` v2, I7 — the treatment's skill tree is archived from the pinned commit
/// and written to the project's `.opencode/skills/<name>/`, which `OpenCode` reads, with the
/// executable bit kept and nothing of a sibling skill.
/// Oracle: `git` on a fixture repo (`git ls-tree`, `git cat-file`), and `opencode_facts` row 16
/// for the layout.
#[test]
fn opencode_skills_land_in_the_project_skill_directory() -> Res<()> {
    let repo = Path::new(env!("CARGO_TARGET_TMPDIR")).join("opencode_skills_repo");
    if repo.exists() {
        std::fs::remove_dir_all(&repo)?;
    }
    for (path, text) in [
        (
            ".claude/skills/demo/SKILL.md",
            "---\nname: demo\n---\nbody\n",
        ),
        (".claude/skills/demo/scripts/run.sh", "#!/bin/sh\n"),
        (".claude/skills/other/SKILL.md", "---\nname: other\n---\n"),
    ] {
        let full = repo.join(path);
        std::fs::create_dir_all(full.parent().ok_or("no parent")?)?;
        std::fs::write(full, text)?;
    }
    git(&repo, &["init", "-q"])?;
    git(&repo, &["add", "."])?;
    git(
        &repo,
        &[
            "update-index",
            "--chmod=+x",
            ".claude/skills/demo/scripts/run.sh",
        ],
    )?;
    git(&repo, &["commit", "-q", "-m", "skills"])?;
    let commit = String::from_utf8(git(&repo, &["rev-parse", "HEAD"])?)?;
    let mut treatment = TreatmentRef::new("demo".parse()?);
    treatment.commit = Some(GitCommit::from_str(commit.trim())?);
    let skills: &dyn SkillTree = &OpenCodeSkills; // TRAIT_OBJECT: the core holds skill trees as objects
    let command = skills.archive_command(&treatment, &repo)?;
    let archive = Command::new(command.argv().first().ok_or("no program")?)
        .args(command.argv().iter().skip(1))
        .output()?;
    assert!(archive.status.success());
    let files = skills.files(&treatment, &archive.stdout)?;
    let mut got: Vec<(String, bool, Vec<u8>)> = files
        .iter()
        .map(|f| (f.path.as_str().to_owned(), f.executable, f.contents.clone())) // BORROW: .to_owned() and .clone() copy
        .collect();
    got.sort();
    let blob = |path: &str| git(&repo, &["cat-file", "blob", &format!("HEAD:{path}")]);
    assert_eq!(
        got,
        vec![
            (
                ".opencode/skills/demo/SKILL.md".to_owned(), // BORROW: .to_owned() makes owned text
                false,
                blob(".claude/skills/demo/SKILL.md")?
            ),
            (
                ".opencode/skills/demo/scripts/run.sh".to_owned(), // BORROW: .to_owned() makes owned text
                true,
                blob(".claude/skills/demo/scripts/run.sh")?
            ),
        ]
    );
    Ok(())
}

/// Guards: I22 — the stop interrupts a real run: the script finds the `serve` child by its whole
/// argument vector (a decoy process whose arguments hold the same words is not it), reads its port
/// and password, interrupts through the API, the run ends with exit 1 and an `aborted` error, and
/// the export collected afterwards says `interrupted`. Run once against the base image, offline:
/// `CANDLER_OPENCODE_IMAGE=<image id> cargo nextest run -p candler-harness-opencode --run-ignored
/// only -E 'test(opencode_real_binary_)'`.
/// Oracle: the real `opencode` v2.2.7 binary and its export, read by the driver's readers; the
/// expectations are the recorded `interrupt_api` facts (exit 1, `aborted`, outcome `interrupted`).
#[test]
#[ignore = "needs Docker and CANDLER_OPENCODE_IMAGE, the id of the base-opencode image"]
fn opencode_real_binary_is_interrupted_by_the_stop_script() -> Res<()> {
    use std::process::Stdio;
    let image = std::env::var("CANDLER_OPENCODE_IMAGE")?;
    let driver = OpenCodeDriver::new()?;
    let values = values(&[])?;
    let cell = Cell::new(&values);
    let workspace = Workspace::new(Path::new("."));
    let run = driver.command(&cell, &workspace)?;
    let prepared = driver.prepare(&cell, &workspace)?;
    let config = String::from_utf8(prepared.files.first().ok_or("no config")?.contents.clone())?;
    let Some(Action::Exec(stop)) = driver.stop().pre_stop else {
        return Err("no pre-stop".into());
    };
    let observed_id: SessionId = "ses_placeholder".parse()?;
    let collect = match driver
        .collect(&workspace, &Observed::new(vec![observed_id]))
        .first()
    {
        Some(CollectSpec::Exec { spec, .. }) => spec.argv().to_vec(),
        _ => return Err("no collect".into()),
    };
    // The harness inside the container: the run in the background, a decoy, the stop, the export.
    let harness = r#"
import json, os, subprocess, sys, time
env = os.environ
home = env["HOME"]
for name, sub in [("XDG_DATA_HOME", ".xdg/data"), ("XDG_CONFIG_HOME", ".xdg/config"),
                  ("XDG_CACHE_HOME", ".xdg/cache"), ("XDG_STATE_HOME", ".xdg/state")]:
    os.makedirs(home + "/" + sub, exist_ok=True)
    env[name] = home + "/" + sub
env["OPENCODE_DB"] = home + "/.xdg/data/oc.db"
os.makedirs(home + "/.xdg/config/opencode", exist_ok=True)
open(home + "/.xdg/config/opencode/opencode.json", "w").write(env["CFG"])
env["CANDLER_GATEWAY_URL"] = "http://dead.invalid"
env["CANDLER_GATEWAY_TOKEN"] = "placeholder"
os.chdir("/tmp")
out = open("/tmp/out.jsonl", "w")
run = subprocess.Popen(json.loads(env["RUN"]), stdout=out)
time.sleep(6)
sid = json.loads(open("/tmp/out.jsonl").readline())["sessionID"]
decoy = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(300)", "serve", "--stdio", "--port", "0"])
fill = lambda argv: [a.replace("{session_id}", sid) for a in argv]
stop_rc = subprocess.run(fill(json.loads(env["STOP"]))).returncode
run_rc = run.wait(timeout=30)
decoy.kill()
export = subprocess.run(fill(json.loads(env["COLLECT"]))[:-1] + [sid], capture_output=True, text=True)
print(json.dumps({"stop_rc": stop_rc, "run_rc": run_rc, "stdout": open("/tmp/out.jsonl").read(),
                  "export": export.stdout, "export_rc": export.returncode}))
"#;
    let name = format!("candler-opencode-driver-test-{}", std::process::id());
    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--name",
            &name,
            "--network",
            "none",
            "--read-only",
        ])
        .args(["--cap-drop", "ALL", "--security-opt", "no-new-privileges"])
        .args([
            "--pids-limit",
            "512",
            "--memory",
            "4g",
            "--user",
            "10001:10001",
        ])
        .args(["--tmpfs", "/tmp:rw,noexec,nosuid"])
        .args(["--tmpfs", "/home/agent:rw,uid=10001,gid=10001,mode=700"])
        .args(["-e", &format!("CFG={config}")])
        .args(["-e", &format!("RUN={}", serde_json::to_string(run.argv())?)])
        .args([
            "-e",
            &format!("STOP={}", serde_json::to_string(stop.argv())?),
        ])
        .args([
            "-e",
            &format!("COLLECT={}", serde_json::to_string(&collect)?),
        ])
        .args(["--entrypoint", "python3", &image, "-I", "-c", harness])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // The stop script prints the API's answer first; the harness's own line is the last.
    let printed = String::from_utf8(output.stdout)?;
    let result: Value = serde_json::from_str(printed.lines().last().ok_or("no output")?)?;
    assert_eq!(
        result["stop_rc"], 0,
        "the stop script found the server child"
    );
    assert_eq!(result["run_rc"], 1, "an interrupt ends the run with exit 1");
    assert_eq!(result["export_rc"], 0);
    let export = result["export"].as_str().ok_or("no export")?;
    let document: Value = serde_json::from_str(export)?;
    assert_eq!(document["info"]["outcome"], "interrupted");
    let stdout = result["stdout"].as_str().ok_or("no stdout")?;
    let mut live = driver.live_reader().ok_or("no live reader")?;
    let stream: StreamName = "stdout".parse()?;
    for (index, line) in stdout.split_inclusive('\n').enumerate() {
        let frame = RawFrame::new(None, FrameData::from_bytes(line.as_bytes()));
        live.feed(FrameIn::new(
            RawRef::new(stream.clone(), u64::try_from(index)?),
            None,
            &frame,
        ))?; // BORROW: .clone() copies the name
    }
    let observed = live.observed();
    assert_eq!(
        observed.end,
        Some(candler_harness::spec::ObservedEnd::Aborted)
    );
    let (_, sessions) = read_export(export.as_bytes())?;
    assert_eq!(
        sessions.first().map(|s| s.id().clone()),
        observed.session_ids.first().cloned()
    ); // BORROW: .clone() copies the id
    let exit = ProcessExit::new(ExitStatus::Code { code: 1 }, ProcessEnd::Exited, Vec::new());
    assert_eq!(
        driver.classify_end(&exit, &sessions, &observed),
        EndedBy::Unknown
    );
    Ok(())
}

/// The sessions the live reader finishes with for a recorded stream, and the export reader's for a
/// recorded export, with the live reader's last word.
fn live_and_export(
    live_case: &str,
    export_case: &str,
) -> Res<(Vec<Session>, Vec<Session>, Observed)> {
    let driver = OpenCodeDriver::new()?;
    let stdout: StreamName = "stdout".parse()?;
    let mut live = driver.live_reader().ok_or("no live reader")?;
    for (index, line) in stream_file(live_case, "stdout.jsonl")?
        .split_inclusive(|b| *b == b'\n')
        .enumerate()
    {
        let frame = RawFrame::new(None, FrameData::from_bytes(line));
        live.feed(FrameIn::new(
            RawRef::new(stdout.clone(), u64::try_from(index)?),
            None,
            &frame,
        ))?; // BORROW: .clone() copies the name
    }
    let observed = live.observed();
    let (_, exported) = read_export(&stream_file(export_case, "export.json")?)?;
    Ok((live.finish()?, exported, observed))
}

/// Guards: R01 §Ends, `opencode_facts` row 15 — the end: the export is authoritative whatever the
/// order of the sessions the core hands over (the live reader's sessions come first or last, and
/// the run ends as the export says), standing in for the live reader's only when no export session
/// was read; an exit the stream and the export say nothing about is classified from the process
/// alone, and never as completed.
/// Oracle: live `completed_o1` (finished) with the `interrupt_api` export (outcome `interrupted`,
/// so `unknown`), and the reverse pairing, read from their `expected.toml`; `opencode_facts` row 15
/// (0 completed, 130 and 143 signals) read by hand.
#[test]
fn opencode_end_is_the_export_whatever_the_order_or_unknown_with_no_trace() -> Res<()> {
    let driver = OpenCodeDriver::new()?;
    let exit = ProcessExit::new(ExitStatus::Code { code: 1 }, ProcessEnd::Exited, Vec::new());
    for (live_case, export_case, want) in [
        ("completed_o1", "interrupt_api", EndedBy::Unknown),
        ("interrupt_api", "completed_o1", EndedBy::Completed),
        ("completed_o1", "refused_key", EndedBy::GatewayError),
    ] {
        let (live, exported, observed) = live_and_export(live_case, export_case)?;
        for both in [
            [live.clone(), exported.clone()].concat(), // BORROW: .clone() copies the sessions
            [exported.clone(), live.clone()].concat(), // BORROW: .clone() copies the sessions
        ] {
            assert_eq!(
                driver.classify_end(&exit, &both, &observed),
                want,
                "live {live_case}, export {export_case}"
            );
        }
    }
    // With no export session the live ones stand in.
    for (live_case, want) in [
        ("completed_o1", EndedBy::Completed),
        ("interrupt_api", EndedBy::Unknown),
    ] {
        let (live, _, observed) = live_and_export(live_case, live_case)?;
        assert_eq!(
            driver.classify_end(&exit, &live, &observed),
            want,
            "live only {live_case}"
        );
    }
    // An exit the stream and the export say nothing about is classified from the process alone.
    for code in [0, 1, 130, 143] {
        let no_trace = ProcessExit::new(ExitStatus::Code { code }, ProcessEnd::Exited, Vec::new());
        assert_eq!(
            driver.classify_exit(&no_trace, &[]),
            EndedBy::Unknown,
            "exit {code}"
        );
        assert_eq!(
            driver.classify_end(&no_trace, &[], &Observed::default()),
            EndedBy::Unknown,
            "exit {code}"
        );
    }
    Ok(())
}

/// Guards: R01 §Harness drivers (detect) — a sample that is not an `OpenCode` event or export is not
/// recognised, even when it names a session. Oracle: hand-written samples; the six event kinds and
/// three outcomes `opencode_facts` row 5 and 11 list.
#[test]
fn opencode_detect_recognises_only_what_it_reads() -> Res<()> {
    let driver = OpenCodeDriver::new()?;
    let live = driver.live_reader().ok_or("no live reader")?;
    let export_stream: StreamName = "export".parse()?;
    let export = driver
        .offline_reader(&export_stream)
        .ok_or("no export reader")?;
    let recorded = stream_file("completed_o1", "stdout.jsonl")?;
    assert!(live.detect(&recorded).is_some());
    assert!(
        live.detect(br#"{"type":"mystery","sessionID":"ses_x"}"#)
            .is_none()
    );
    assert!(live.detect(br#"{"type":"text"}"#).is_none());
    let document = stream_file("completed_o1", "export.json")?;
    assert!(export.detect(&document).is_some());
    assert!(
        export
            .detect(br#"{"info": {"outcome": "weird"}, "messages": []}"#)
            .is_none()
    );
    assert!(export.detect(br#"{"info": {}, "messages": []}"#).is_none());
    Ok(())
}

/// The stop script's predicate, run under `python3 -I` on sample command lines. `None` when the host
/// has no `python3`.
fn predicate(cases: &[(Vec<&str>, Vec<&str>)]) -> Res<Option<Vec<bool>>> {
    let stop = OpenCodeDriver::new()?
        .stop()
        .pre_stop
        .ok_or("no pre-stop")?;
    let Action::Exec(spec) = stop else {
        return Err("not an Exec".into());
    };
    let script = spec.argv().get(3).ok_or("no script")?;
    let driver = r#"
import json, sys
names = {"__name__": "predicate_test"}
exec(sys.argv[1], names)
cases = json.loads(sys.argv[2])
print(json.dumps([bool(names["is_server"](argv, parent)) for argv, parent in cases]))
"#;
    let out = match Command::new("python3")
        .args(["-I", "-c", driver, script, &serde_json::to_string(cases)?])
        .output()
    {
        Ok(out) => out,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(Some(serde_json::from_slice(&out.stdout)?))
}

/// Guards: I22 — the stop: the pre-stop is an `Exec` that runs inside the isolation and signals
/// TERM after it, carrying the session id as its own argument (refused to resolve for an id that
/// is not plain, or for none); and the stop script interrupts only the server child of an
/// `opencode run` client: the real child matches; a decoy holding the same words, an `sh -c` that
/// mentions them, a `timeout` wrapper, and a server whose parent is not the client do not. The
/// predicate part is skipped when the host has no `python3`.
/// Oracle: `opencode_facts` rows 9 and 10 (signal, `Exec`); the command lines `/proc` showed in the
/// base image (`opencode run ...` and `/usr/local/bin/opencode serve --stdio --port 0`), hand-listed.
#[test]
fn opencode_stop_terms_after_the_interrupt_and_matches_only_the_server_child() -> Res<()> {
    let stop = OpenCodeDriver::new()?.stop();
    assert_eq!(stop.signal, StopSignal::Term);
    let action = stop.pre_stop.ok_or("no pre-stop")?;
    assert!(
        matches!(action, Action::Exec(_)),
        "runs inside the isolation"
    );
    assert!(action.uses_session_id());
    let Some(Action::Exec(filled)) = action.resolve(Some(&sample_id()?)) else {
        return Err("the placeholder does not resolve".into());
    };
    assert!(!filled.uses_session_id());
    assert_eq!(
        filled.argv().last().map(String::as_str),
        Some("ses_e32c17b4fffe2bjBVM2E3xAaLZ")
    );
    assert!(action.resolve(None).is_none(), "no id, no interrupt");
    let unsafe_id: SessionId = "ses_1;touch$IFS/x".parse()?;
    assert!(action.resolve(Some(&unsafe_id)).is_none());

    let serve = vec!["/usr/local/bin/opencode", "serve", "--stdio", "--port", "0"];
    let run = vec![
        "opencode",
        "run",
        "--standalone",
        "--format",
        "json",
        "--",
        "hi",
    ];
    let cases = vec![
        (serve.clone(), run.clone()),
        (
            vec!["opencode", "serve", "--stdio", "--port", "0"],
            run.clone(),
        ),
        (
            vec!["python3", "-c", "x", "serve", "--stdio", "--port", "0"],
            run.clone(),
        ),
        (
            vec!["sh", "-c", "opencode serve --stdio --port 0"],
            run.clone(),
        ),
        (
            vec![
                "timeout", "60", "opencode", "serve", "--stdio", "--port", "0",
            ],
            run.clone(),
        ),
        (serve.clone(), vec!["sh", "-c", "opencode run"]),
        (
            serve.clone(),
            vec!["opencode", "serve", "--stdio", "--port", "0"],
        ),
        (serve, vec![]),
    ];
    let Some(got) = predicate(&cases)? else {
        eprintln!("skipped: no python3 on this host");
        return Ok(());
    };
    assert_eq!(got, [true, true, false, false, false, false, false, false]);
    Ok(())
}

/// One row of a fixture's committed `wire.jsonl`: a redacted, per-request projection of the fake
/// model's own `records.jsonl` (no header, no secret, no message body).
fn wire_rows(case: &str) -> Res<Vec<Value>> {
    let bytes = stream_file(case, "wire.jsonl")?;
    String::from_utf8(bytes)?
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| Ok(serde_json::from_str(line)?))
        .collect()
}

/// Guards: R01 §`OpenCode` v2, O1 wire — for each effort level the driver maps, the `variants`
/// entry it writes is the one the recorded session ran with, and every request body of that session
/// carries `thinking = adaptive/summarized` and `output_config.effort = <level>`: the C mapping of
/// `model.effort` proven on Candler's wire level by level, not from the source citation. The first
/// request of a session has no `tools`: it is the hidden `title` agent's, which runs on the same
/// model and variant, so the rule holds for it too.
/// Oracle: `tests/fixtures/streams/oc_effort_<level>/wire.jsonl`, the fake model's own recorded
/// request bodies of each session, and `config.json`, the file that session ran with
/// (`PROVENANCE.toml`); the shape is `packages/core/src/variant.ts:325-339` (v2.2.7).
#[test]
fn opencode_wire_effort_levels_reach_the_request_body_as_output_config() -> Res<()> {
    for level in ["low", "medium", "high", "xhigh", "max"] {
        let case = format!("oc_effort_{level}");
        let rows = wire_rows(&case)?;
        assert!(rows.len() >= 2, "{case}: the title request and a turn");
        for row in &rows {
            assert_eq!(
                row["thinking"],
                json!({"type": "adaptive", "display": "summarized"}),
                "{case}: {row}"
            );
            assert_eq!(
                row["output_config"],
                json!({"effort": level}),
                "{case}: {row}"
            );
        }
        let ran: Value = serde_json::from_slice(&stream_file(&case, "config.json")?)?;
        let (_, written) = prepared_config(&[
            ("model.effort", json!(level)),
            ("model.id", json!("claude-opus-4-7")),
        ])?;
        assert_eq!(
            ran["providers"]["candler"]["models"]["oc-claude-opus-4-7"]["variants"],
            written["providers"]["candler"]["models"]["claude-opus-4-7"]["variants"],
            "{case}: the driver writes the variants entry the session ran with"
        );
    }
    Ok(())
}
