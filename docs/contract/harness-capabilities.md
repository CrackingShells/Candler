# Harness capabilities

## Surface

What a run can expect of each registered harness: what the harness does itself, what Candler supplies in its place, and what resolving refuses.

## Version

<!-- generated:version -->
<!-- versions -->
Core line: `v0.15.0`; the `candler-harness` crate has the same version.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
| Capability | `claude-code` | `opencode` |
|---|---|---|
| model id | `--model` on the command line | `-m candler/<model>` on the command line |
| model endpoint | Candler: the gateway forwards model requests to the endpoint | Candler: the gateway forwards model requests to the endpoint |
| model effort: low | `--effort <level>` on the command line | a `variants` entry on the model, selected by `-m <model>#<level>`, for a Claude opus, sonnet, fable or mythos of 4.6 or later (`xhigh` from 4.7); refused for any other model |
| model effort: medium | `--effort <level>` on the command line | a `variants` entry on the model, selected by `-m <model>#<level>`, for a Claude opus, sonnet, fable or mythos of 4.6 or later (`xhigh` from 4.7); refused for any other model |
| model effort: high | `--effort <level>` on the command line | a `variants` entry on the model, selected by `-m <model>#<level>`, for a Claude opus, sonnet, fable or mythos of 4.6 or later (`xhigh` from 4.7); refused for any other model |
| model effort: xhigh | `--effort <level>` on the command line | a `variants` entry on the model, selected by `-m <model>#<level>`, for a Claude opus, sonnet, fable or mythos of 4.6 or later (`xhigh` from 4.7); refused for any other model |
| model effort: max | `--effort <level>` on the command line | a `variants` entry on the model, selected by `-m <model>#<level>`, for a Claude opus, sonnet, fable or mythos of 4.6 or later (`xhigh` from 4.7); refused for any other model |
| model effort: ultracode | `--effort ultracode` on the command line, which Claude Code reads as `xhigh` with its ultracode mode on | no (`ultracode` is `xhigh` effort with an ultracode mode on, and OpenCode has no equivalent of that mode, so it is refused rather than run as `xhigh`) |
| permission mode: default | `--permission-mode <mode>` on the command line | no (Claude Code's default mode allows reads and a built-in, non-configurable set of read-only shell commands that its own parser decides, and denies the rest in a headless run. OpenCode without `--auto` also rejects what no rule allows, but its shell rows are wildcards on the text of each parsed command, which approximate that set without being it) |
| permission mode: acceptEdits | `--permission-mode <mode>` on the command line | no (Claude Code's acceptEdits mode also allows edits and the filesystem commands mkdir, touch, rm, rmdir, mv, cp and sed, inside the working directory only. OpenCode's shell resource is the command's text, which knows no working directory, so "inside it" cannot be written as a rule) |
| permission mode: plan | `--permission-mode <mode>` on the command line | no (Claude Code's plan mode reads and runs read-only shell commands (a built-in, non-configurable set its own parser decides). The OpenCode ruleset that is certainly read-only (the `explore` agent's: deny everything, allow read, glob, grep, webfetch, websearch, skill) denies the shell altogether, and a wildcard on command text cannot express the read-only set; the stock `plan` agent is not read-only once a global rule follows it (the last match wins) and its `subagent` child can edit) |
| permission mode: bypassPermissions | `--permission-mode <mode>` on the command line | `--auto` on the command line: every tool call is allowed by the baseline ruleset {*, *, allow}; `--auto` answers the one pending ask (`.env` reads) with once, which a headless run needs |
| permission mode: auto | `--permission-mode <mode>` on the command line | no (Claude Code's auto mode has a model classifier review each action, such as a shell command or a network request, before it runs. OpenCode has no classifier, and a ruleset decides by the action's text alone) |
| permission mode: dontAsk | `--permission-mode <mode>` on the command line | no (Claude Code's dontAsk mode denies every call that would prompt, so it allows the allow rules plus reads and the built-in, non-configurable set of read-only shell commands its own parser decides. OpenCode's shell rows are wildcards on the text of each parsed command, which approximate that set without being it) |
| tools allowed | `permissions.allow` of `.claude/settings.json` | a permission row {action, *, allow} per name, a no-op over the baseline |
| tools disallowed | `permissions.deny` of `.claude/settings.json` | a permission row {action, *, deny} per name, written after the allow rows |
| env | Candler: the core sets the cell's variables on the agent's process | Candler: the core sets the cell's variables on the agent's process |
| inputs | Candler: the core places the inputs in the workspace | Candler: the core places the inputs in the workspace |
| skills | the project skill directory | the project skill directory |
| MCP transport: stdio | an entry of `mcpServers` in `.mcp.json`, of type `stdio` | an `mcp` entry of type `local` |
| MCP transport: local | an entry of `mcpServers` in `.mcp.json`, of type `stdio` | an `mcp` entry of type `local` |
| MCP transport: http | an entry of `mcpServers` in `.mcp.json`, of type `http` | an `mcp` entry of type `remote` |
| MCP transport: sse | an entry of `mcpServers` in `.mcp.json`, of type `sse` | no (OpenCode documents no SSE transport; its `remote` is Streamable HTTP) |
| MCP transport: remote | an entry of `mcpServers` in `.mcp.json`, of type `http` | an `mcp` entry of type `remote` |
| MCP codemode: off | no key: Claude Code offers the server's tools directly | the `codemode` key of the `mcp` entry, `false`: the server's tools are direct tools |
| MCP codemode: on | no (Claude Code offers MCP tools directly; it has no code mode) | the `codemode` key of the `mcp` entry, `true`: the server's tools are reached through `execute`, whose calls the trace records with empty `wrapped_calls`, so MCP calls made inside `execute` are not attributed and do not compare with Claude Code |
| MCP server name | the key of the server's `.mcp.json` entry | the key of the `mcp` entry |
| wall cap | Candler: the core ends the run at the wall budget | Candler: the core ends the run at the wall budget |
| turn cap | `--max-turns` on the command line | no (`agents.<id>.steps` makes the last request a tool-less one with an injected prompt and ends the run as completed (the `oc_step_cap` fixture), where Claude Code ends the run with `error_max_turns`, so the cap class would be lost and the model's last turn changed) |
| cost cap | `--max-budget-usd` on the command line | no (OpenCode has no cost cap) |
| session mode | stream-json over an open stdin, one turn per message | no (OpenCode runs no session: its trace comes from the export after the run) |
| gateway route | Candler: gateway, address in `ANTHROPIC_BASE_URL` | Candler: gateway, address in `CANDLER_GATEWAY_URL` |
| authoritative trace | yes | Candler: collects `export` after the run |
| graceful stop | SIGTERM ends the sessions cleanly | an interrupt request to the `serve` child, then SIGTERM to the client |
| live events | stream-json events on stdout while the run lasts | no (the live reader emits no event: the trace comes from the export) |
| in-harness guard | no (the driver configures no Claude Code hook) | no (the driver configures no OpenCode guard) |
| execution outside the child | no (tools run in the child process) | tools run in the `serve` child, which the stop interrupts first |
| census source | Claude Code's `system/init` event lists the tools | Candler: the tools the gateway offered the model in its first request |
| permission mode when none is set | `--permission-mode default` on the command line, which in a headless run denies every tool call that needs approval unless an allow rule or a hook approves it | Candler: the run is under `--auto` and the baseline ruleset {*, *, allow}, so every tool is allowed: OpenCode has no headless ask mode |
<!-- /generated:fields -->

## Refusals

<!-- generated:refusals -->
| Code | When | Path at fault | Side to fix |
|---|---|---|---|
| `experiment.session_refused` | A cell's session settings are refused: `runner.mode` is not `headless` or `session`, `runner.idle_s` is not whole seconds or `none`, `runner.idle_s` is set on a cell that is not a session, or the harness's driver cannot run a session. | - | experiment |
| `experiment.setting_unsupported` | The experiment sets a setting that the harness has no way to honour, such as a turn cap for a harness with no turn limit; the run is refused rather than the setting dropped. | The setting in the experiment file: a field under `/budget`, or the key of `[fixed]` that holds it, such as `/fixed/model.effort`. | experiment |
<!-- /generated:refusals -->

## Notes

- `no (…)` is a refusal: `experiment.session_refused` while resolving, `experiment.setting_unsupported` when the cells are planned, before a job starts. [checked: v@0.15.0]
- In session mode Candler also counts turns across the session and ends the run when they exceed `budget.per_run.turns`. [checked: v@0.7.0]
- The settings a driver maps or refuses, such as permission modes, effort levels and tool names, are the answers its driver gives for each `AgentSetting`. [checked: v@0.7.0]
