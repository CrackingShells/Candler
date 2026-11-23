# Configuration paths

## Surface

The dotted paths an experiment file, a `--set` and a configuration file name: which are values of the experiment, which are operational settings that never reach a cell digest, and which are keys of a configuration file.

## Version

<!-- generated:version -->
<!-- versions -->
Core line: `v0.13.0`.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
| Path | Names | Class | Meaning |
|---|---|---|---|
| `model.id` | the path | experiment value | The model the harness runs. |
| `model.endpoint` | the path | experiment value | The endpoint the gateway forwards model requests to. |
| `model.effort` | the path | experiment value | The effort level the harness asks of the model. |
| `model.token_secret` | the path | setting | The name of the secret that holds the gateway's token. |
| `tools.allowed` | the path | experiment value | The tools the harness may use. |
| `tools.disallowed` | the path | experiment value | The tools the harness may not use. |
| `env` | the path | experiment value | The environment of the harness process: a table of variable name to text. |
| `mcp` | the path and every path below it | experiment value | The MCP servers of a run; the names below are free. |
| `permission_mode` | the path | experiment value | The permission mode the harness runs in. |
| `request.harness` | the path | request key | The harness the run uses. |
| `request.treatment` | the path | request key | The treatment the run applies. |
| `request.inputs` | the path | request key | The inputs the run is given. |
| `prompt.template` | the path | experiment value | The prompt template. |
| `prompt.values` | the path and every path below it | experiment value | The values the prompt template fills in; the names below are free. |
| `isolation.image` | the path | experiment value | The container image a run starts in. |
| `isolation.network` | the path | experiment value | The network mode of the container. |
| `isolation.env_lock` | the path | experiment value | The environment lock the image must match. |
| `isolation.limits` | the path | experiment value | The resource limits of the container; the limits are below. |
| `isolation.limits.pids` | the path | experiment value | The process limit of the container. |
| `isolation.limits.memory` | the path | experiment value | The memory limit of the container. |
| `isolation.limits.cpus` | the path | experiment value | The CPU limit of the container. |
| `isolation.egress` | the path | experiment value | The hosts the container may reach. |
| `isolation.record_packages` | the path | setting | Whether a run records the packages of its image. |
| `isolation.build` | every path below it | setting | Every path below it is a request to build an image. |
| `isolation.build.plugin` | the path | setting | The image build plug-in. |
| `isolation.build.dockerfile` | the path | setting | The Dockerfile of an image build. |
| `isolation.build.context` | the path | setting | The build context directory of an image build. |
| `isolation.build.requirements` | the path | setting | The requirements file of an image build. |
| `isolation.build.prune` | the path | setting | Whether an image build prunes what it leaves. |
| `isolation.build.lock` | the path | setting | The lock file of an image build. |
| `budget` | the path and every path below it | setting | A job cap: every path below it, except the per-run caps. |
| `budget.per_run` | the path and every path below it, which the enclosing family does not reach | experiment value | The per-run caps are experiment values. |
| `budget.per_run.wall_s` | the path | experiment value | The wall-clock cap of one run, in seconds. |
| `budget.per_run.turns` | the path | experiment value | The turn cap of one run. |
| `budget.per_run.max_cost_usd` | the path | experiment value | The cost cap of one run, in US dollars. |
| `replicates` | the path and every path below it | setting | How many runs each cell gets. |
| `runner.mode` | the path | experiment value | `headless` or `session`. |
| `runner.idle_s` | the path | experiment value | The idle time that ends a session, in whole seconds, or `none`. |
| `harness` | the path and every path below it | experiment value | The harness table; the names below belong to the harness. |
| `treatment` | the path and every path below it | experiment value | The treatment table. |
| `inputs` | the path and every path below it | experiment value | The inputs table. |
| `snapshots` | the path and every path below it | experiment value | The snapshots table. |
| `environments` | the path and every path below it | experiment value | The environments table. |
| `treatment:` | text that is not a field path | experiment value | The prefix of a value that names a treatment source; the rest is the path. |
| `store.root` | the path | configuration key | The store root; a relative path is read from the project root. |
| `plugins.dirs` | the path | configuration key | The directories that hold plug-ins; relative paths are read from the project root. |
| `slots.count` | the path | configuration key | How many run slots the machine offers. |
| `slots.evaluators` | the path | configuration key | How many of the slots evaluators may use. |
| `gateway.upstream_ca` | text that is not a field path | configuration key | A PEM file whose certificates the gateway trusts for its upstream. |
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
