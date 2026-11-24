# Configuration paths

## Surface

The dotted paths an experiment file, a `--set` and a configuration file name: which are values of the experiment, which are operational settings that never reach a cell digest, and which are keys of a configuration file.

## Version

<!-- generated:version -->
<!-- versions -->
Core line: `v0.15.0`.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
| Path | Names | Class | Meaning | claude-code | opencode |
|---|---|---|---|---|---|
| `model.id` | the path | experiment value | The model the harness runs. | `--model` on the command line | `-m candler/<model>` on the command line |
| `model.endpoint` | the path | experiment value | The endpoint the gateway forwards model requests to. | Candler: the gateway forwards model requests to the endpoint | Candler: the gateway forwards model requests to the endpoint |
| `model.effort` | the path | experiment value | The effort level the harness asks of the model. | `--effort <level>` on the command line | a `variants` entry on the model, selected by `-m <model>#<level>`, for a Claude opus, sonnet, fable or mythos of 4.6 or later (`xhigh` from 4.7); refused for any other model |
| `model.token_secret` | the path | setting | The name of the secret that holds the gateway's token. | Candler only | Candler only |
| `tools.allowed` | the path | experiment value | The tools the harness may use. | `permissions.allow` of `.claude/settings.json` | a permission row {action, *, allow} per name, a no-op over the baseline |
| `tools.disallowed` | the path | experiment value | The tools the harness may not use. | `permissions.deny` of `.claude/settings.json` | a permission row {action, *, deny} per name, written after the allow rows |
| `env` | the path | experiment value | The environment of the harness process: a table of variable name to text. | Candler: the core sets the cell's variables on the agent's process | Candler: the core sets the cell's variables on the agent's process |
| `mcp` | the path and every path below it | experiment value | The MCP servers of a run; the names below are free. | an entry of `mcpServers` in `.mcp.json`, of type `stdio`; no (Claude Code offers MCP tools directly; it has no code mode); the key of the server's `.mcp.json` entry | an `mcp` entry of type `local`; the `codemode` key of the `mcp` entry, `true`: the server's tools are reached through `execute`, whose calls the trace records with empty `wrapped_calls`, so MCP calls made inside `execute` are not attributed and do not compare with Claude Code; the key of the `mcp` entry |
| `permission_mode` | the path | experiment value | The permission mode the harness runs in. | `--permission-mode <mode>` on the command line | no (Claude Code's default mode allows reads and a built-in, non-configurable set of read-only shell commands that its own parser decides, and denies the rest in a headless run. OpenCode without `--auto` also rejects what no rule allows, but its shell rows are wildcards on the text of each parsed command, which approximate that set without being it) |
| `request.harness` | the path | request key | The harness the run uses. | Candler only | Candler only |
| `request.treatment` | the path | request key | The treatment the run applies. | Candler only | Candler only |
| `request.inputs` | the path | request key | The inputs the run is given. | Candler only | Candler only |
| `prompt.template` | the path | experiment value | The prompt template. | Candler only | Candler only |
| `prompt.values` | the path and every path below it | experiment value | The values the prompt template fills in; the names below are free. | Candler only | Candler only |
| `isolation.image` | the path | experiment value | The container image a run starts in. | Candler only | Candler only |
| `isolation.network` | the path | experiment value | The network mode of the container. | Candler only | Candler only |
| `isolation.env_lock` | the path | experiment value | The environment lock the image must match. | Candler only | Candler only |
| `isolation.limits` | the path | experiment value | The resource limits of the container; the limits are below. | Candler only | Candler only |
| `isolation.limits.pids` | the path | experiment value | The process limit of the container. | Candler only | Candler only |
| `isolation.limits.memory` | the path | experiment value | The memory limit of the container. | Candler only | Candler only |
| `isolation.limits.cpus` | the path | experiment value | The CPU limit of the container. | Candler only | Candler only |
| `isolation.egress` | the path | experiment value | The hosts the container may reach. | Candler only | Candler only |
| `isolation.record_packages` | the path | setting | Whether a run records the packages of its image. | Candler only | Candler only |
| `isolation.build` | every path below it | setting | Every path below it is a request to build an image. | Candler only | Candler only |
| `isolation.build.plugin` | the path | setting | The image build plug-in. | Candler only | Candler only |
| `isolation.build.dockerfile` | the path | setting | The Dockerfile of an image build. | Candler only | Candler only |
| `isolation.build.context` | the path | setting | The build context directory of an image build. | Candler only | Candler only |
| `isolation.build.requirements` | the path | setting | The requirements file of an image build. | Candler only | Candler only |
| `isolation.build.prune` | the path | setting | Whether an image build prunes what it leaves. | Candler only | Candler only |
| `isolation.build.lock` | the path | setting | The lock file of an image build. | Candler only | Candler only |
| `budget` | the path and every path below it | setting | A job cap: every path below it, except the per-run caps. | Candler only | Candler only |
| `budget.per_run` | the path and every path below it, which the enclosing family does not reach | experiment value | The per-run caps are experiment values. | Candler only | Candler only |
| `budget.per_run.wall_s` | the path | experiment value | The wall-clock cap of one run, in seconds. | Candler: the core ends the run at the wall budget | Candler: the core ends the run at the wall budget |
| `budget.per_run.turns` | the path | experiment value | The turn cap of one run. | `--max-turns` on the command line | no (`agents.<id>.steps` makes the last request a tool-less one with an injected prompt and ends the run as completed (the `oc_step_cap` fixture), where Claude Code ends the run with `error_max_turns`, so the cap class would be lost and the model's last turn changed) |
| `budget.per_run.max_cost_usd` | the path | experiment value | The cost cap of one run, in US dollars. | `--max-budget-usd` on the command line | no (OpenCode has no cost cap) |
| `replicates` | the path and every path below it | setting | How many runs each cell gets. | Candler only | Candler only |
| `runner.mode` | the path | experiment value | `headless` or `session`. | stream-json over an open stdin, one turn per message | no (OpenCode runs no session: its trace comes from the export after the run) |
| `runner.idle_s` | the path | experiment value | The idle time that ends a session, in whole seconds, or `none`. | Candler only | Candler only |
| `harness` | the path and every path below it | experiment value | The harness table; the names below belong to the harness. | Candler only | Candler only |
| `treatment` | the path and every path below it | experiment value | The treatment table. | Candler only | Candler only |
| `inputs` | the path and every path below it | experiment value | The inputs table. | Candler: the core places the inputs in the workspace | Candler: the core places the inputs in the workspace |
| `snapshots` | the path and every path below it | experiment value | The snapshots table. | Candler only | Candler only |
| `environments` | the path and every path below it | experiment value | The environments table. | Candler only | Candler only |
| `treatment:` | text that is not a field path | experiment value | The prefix of a value that names a treatment source; the rest is the path. | Candler only | Candler only |
| `store.root` | the path | configuration key | The store root; a relative path is read from the project root. | Candler only | Candler only |
| `plugins.dirs` | the path | configuration key | The directories that hold plug-ins; relative paths are read from the project root. | Candler only | Candler only |
| `slots.count` | the path | configuration key | How many run slots the machine offers. | Candler only | Candler only |
| `slots.evaluators` | the path | configuration key | How many of the slots evaluators may use. | Candler only | Candler only |
| `gateway.upstream_ca` | text that is not a field path | configuration key | A PEM file whose certificates the gateway trusts for its upstream. | Candler only | Candler only |
<!-- /generated:fields -->

## Refusals

<!-- generated:refusals -->
| Code | When | Path at fault | Side to fix |
|---|---|---|---|
| `experiment.unknown_path` | A dotted path under `[fixed]`, in a factor or in `--set` is none that the experiment surface has, such as a misspelt `model.idd`. | The key under `/fixed`, the `path` of the factor, or the key of the level's table that holds the path; none for `--set`. | experiment |
| `experiment.unknown_request_key` | `--set` names a key under `request` that is none of the request settings, such as a misspelt `request.harnes`. | - | experiment |
| `config.retired_setting` | `--set` names `request.treatment_source`, which `[treatment] source` in the experiment file replaced. | - | config |
<!-- /generated:refusals -->

## Notes
