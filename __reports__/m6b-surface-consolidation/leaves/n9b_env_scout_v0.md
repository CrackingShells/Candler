# N9b env scout v0: harness environment from the experiment file

**Verdict: not a duplicate.** No field, setting, config key or driver output lets an experimenter set an environment variable by value for the harness process today. The decisive fact: the harness command's overlay (`CommandSpec.env_overlay`) is written only by the driver, and the Claude Code driver writes it empty (`crates/candler-harness-claude-code/src/driver.rs:227-237`). The only experiment-reachable env value is `mcp.servers.<key>.env`, which reaches MCP server processes, not the harness. Nothing maps `CLAUDE_CODE_MAX_OUTPUT_TOKENS` today: the Claude Code argv maps only effort, `--max-turns` and `--max-budget-usd` (`driver.rs:84-131`).

Status legend: **C** = confirmed in code or docs this session; **I** = inferred; **N** = not read.

## 1. Every way an env var reaches the agent container today

The agent runs in `docker create` (`isolation/container/run.rs:139-207`). Its `-e` list is built from `ContainerPlan.env` (`run.rs:139-207`, plan env loop) plus the command overlay (`run.rs:183-197` checks, `env_args` at `run.rs:108-118`).

| Source | Who sets it | Value recorded in run manifest? | In cell digest? | Evidence |
|---|---|---|---|---|
| Base plan env: `PATH`, `HOME`, `VIRTUAL_ENV`, `LANG` | Core, constants | N (not located) | N | C: `isolation/container/mod.rs:253-388` (plan literal) |
| Gateway: base URL and dummy token under the driver's names | Core, names from driver | Names only (`resolved_record` is names and kinds, `runner/exec/gather.rs:237-285`) | N | C: `run.rs:237-283` pushes both into `plan.env`; `ANTHROPIC_BASE_URL`/`ANTHROPIC_AUTH_TOKEN` at `driver.rs:30-31` |
| Driver state vars (`*_CONFIG_DIR`-style per harness) | Core, from driver `state_paths()` | N | N | C: `runner/exec/mod.rs:252-290` `apply_harness_env`; `isolation/layout.rs:16-34` `state_env` |
| Driver plain overlay (`CommandSpec.env_overlay`) | Driver only | Names and kind (`gather.rs:237-285`) | N | C: `candler-harness/src/spec.rs:157-162`; Claude Code writes none (`driver.rs:227-237`). OpenCode driver not read (N) |
| Driver `Prepared.env_overlay` ("variables to add to the child's environment") | Driver only | N | N | C: field at `spec.rs:1186-1201`; Claude Code passes `Vec::new()` (`driver.rs:227-237`). Consumer that merges it into the command not located (N) |
| Secrets (`secrets = [...]`) | Experiment names; values read from host | N | Names only (`secrets` in schema, `experiment.schema.json` ~288-289) | C: `env_pairs` refuses any secret overlay entry (`run.rs:78-106`, `SecretRefused`). So secrets do **not** reach the agent container. Host-side presence check only (`resolve/steps/secrets.rs:1-17`) |
| Gateway token secret (`model.token_secret`) | Experiment setting, host value | Never | Never (setting) | C: `resolve/layers.rs:27-58` (setting list includes `model.token_secret`, line 55) |
| Image `ENV` from the author's Dockerfile | Image build | N | N | C: `create_argv` has no clearing flag (`run.rs:139-207`). Docker CLI docs (Context7 `/docker/cli`): `-e` "overwrites variables defined in the image's Dockerfile", other variables stay. So image ENV survives unless the same name is set by `-e`. I: image ENV reaches the agent. The `run.rs:1-25` line "built from nothing" describes Candler's own list, not docker's image env |
| Plug-in fragment (`isolation.build.plugin`) | Plug-in manifest | N | Partly (build settings are settings) | C: `resolve/steps/plugin_environment.rs:1-32`; it derives build and step values, not harness env (I, from its doc) |
| MCP server env (`mcp.servers.<key>.env`) | Experiment cell value | N | Yes (cell value, `layers.rs:60-122` `mcp` subtree) | C: `candler-harness/src/mcp.rs:19-20` (`SERVERS`), `:32-70` (`by_name`). The lines cited do not show env handling; the server object is passed through as data. The env reaches the MCP server process, not the harness (I) |
| User config (`config.toml`), project config (`candler.toml`) | Operator | N | N | C: `resolve/config.rs:13-23`. Keys: project `store.root`, `plugins.dirs`; user `slots.count`, `slots.evaluators`, `gateway.upstream_ca`. No env key |
| `--set` / explicit argument | Operator | N | Depends on path | Not read in detail (N) |
| Probe containers | Core | N | N | C: `isolation/image.rs:936-943` `probe_env` (`HOME` + state vars only) |

## 2. Is there already a field that sets an env var by value for the harness?

**No.** Checked: experiment schema fields (`docs/contract/experiment-file.md:17-53`: `budget`, `factors`, `fixed`, `harness`, `inputs`, `replicates`, `secrets`, `treatment`, no env), `is_known_path` (`layers.rs:66-111`), config tables (`config.rs:13-23`), and the Claude Code argv (`driver.rs:84-131`). The `fixed` map (`execution.rs:1305-1309`) sets values by known path only. The `harness.*` subtree is harness identity (`compare.rs:1-16`), not env.

Per-harness caps that look close but are not env:
- `budget.per_run` has only `wall_s`, `turns`, `max_cost_usd` (`experiment-file.md:27-30`). No output-token cap. `budget.max_tokens` (line 26) is a job cap.
- Harness capability rows: session mode, turn cap, cost cap, stop, trace, skills layout, gateway route (`harness-capabilities.md:15-28`). No output-token row.

**Consequence:** a new feature does not duplicate an existing knob. It would be the first experiment-reachable harness env.

## 3. Boundaries a new `env` table would need

**Names refused (all of these would be checked at resolve or argv time):**

| Refused name | Why | Evidence |
|---|---|---|
| `PATH`, `HOME`, `VIRTUAL_ENV`, `LANG` | Set by the core's plan | `container/mod.rs:253-388` |
| `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_API_KEY` | Driver's gateway reserved set | `driver.rs:30-33`; `run.rs:229-235` reserved names; refused today as `ReservedName` (`run.rs:78-106`) |
| Other drivers' gateway names (e.g. `CANDLER_GATEWAY_URL` for OpenCode) | Reserved by that driver | `harness-capabilities.md:20` |
| State variables (names from `state_paths()`) | Core sets them; duplicate refused | `exec/mod.rs:252-290` |
| Names in `secrets` | Secrets never enter the overlay; a name in both lists is ambiguous | `run.rs:78-106` |
| Names repeated in `env` | Duplicate | `run.rs:183-197` `DuplicateName` |
| Names not matching `^[A-Z_][A-Z0-9_]*$` | Schema name pattern | `experiment.schema.json` ~193 (`SecretName`) |

**Comparable value or setting?** A value that changes agent behaviour must be a **comparable cell value**, not a setting. `is_setting` (`layers.rs:27-58`) excludes settings from the cell digest, so an `env` key listed as a setting would let two runs differ without differing digest. Recommendation: make `env.*` cell values (in the digest, factor-able).

**Layers:**
- Experiment file: yes.
- Plug-in fragment: possible (it already contributes values, `plugin_environment.rs:1-32`), but only with the same name refusals.
- User config: no. It would make runs of one experiment differ by machine; the config tables (`config.rs:13-23`) have no such table.
- Project config (`candler.toml`): only if the value is a cell value; I would not add it.

**Factor?** Possible if the path is in `is_known_path` (`layers.rs:66-111`); a factor on an env path is a value, so it is allowed in principle (I). Unknown paths are refused by some check not read (N).

**Secret with the same name:** refuse at resolve (name in both `env` and `secrets`). Secrets cannot reach the overlay anyway (`run.rs:78-106`), so the risk is a silent override in the plan, not a leak.

**Manifest:** record the names and the literal values for `env` (values are not secret), like the cell values. The existing `resolved_record` records names and kinds only (`gather.rs:237-285`). Whether the run manifest has an `env` field is not confirmed: I found none. The "`env: []` in a recorded manifest" note could not be located (N).

## 4. Size and files

| Step | Files | Calls (rough) |
|---|---|---|
| Schema field `env` (map name to literal) | `crates/candler-schemas/schema/experiment.schema.json`, `crates/candler-schemas/src/execution.rs` (`ExperimentFile`), `paths` constants in `execution.rs` | 3-4 |
| Schema release | `schemas@` bump (wire line `schemas@0.11.3`, `experiment-file.md:13`) | 1-2 |
| Known paths | `crates/candler-core/src/resolve/layers.rs` (`LEAVES` const 35 to 36, `is_known_path` 66-111) | 1-2 |
| Name and value checks, refusals | new step or extension of `resolve/steps/*.rs`; `docs/contract/codes.toml` rows (4-5 new codes: reserved, secret clash, duplicate, invalid name) | 4-6 |
| Plumb to argv | `crates/candler-core/src/runner/exec/mod.rs` (`apply_harness_env`-style fn), `crates/candler-core/src/isolation/container/run.rs` (`create_argv` already refuses reserved and duplicate names) | 3-4 |
| Conformance and guards | `crates/candler-harness/src/conformance.rs` (`check_command`, core-owned names), `crates/candler-core/tests/it/guard_harness_facts.rs` (refusal tests) | 2-3 |
| Generated docs | `docs/contract/experiment-file.md` fields table, `docs/contract/harness-capabilities.md` | 1-2 |

Total: about 15-25 implementation calls, plus one schema release.

**Lower-cost alternative (I):** a driver settings-table row (`settings_table()`, `driver.rs:106-138`) mapping a cell value such as `model.max_output_tokens` to the harness's own env. Claude Code's `settings.json` `env` block would carry it. `prepare::render_settings` (`crates/candler-harness-claude-code/src/prepare.rs:54-83`) writes only `permissions` today. Claude Code's settings `env` key is from memory and was **not verified** (N). That route still needs a known path, but avoids a generic env transport and its name refusals.

## Not read

- Who consumes `Prepared.env_overlay` (merge point before spawn).
- MCP `env` handling in `prepare.rs` (both drivers); the cited `mcp.rs:32-70` does not show it.
- Whether `plan.env` or the overlay enters the cell digest.
- The run manifest's fields; no `env` field located.
- The "unknown path" refusal in `is_known_path` callers.
- `docs/contract/codes.toml` contents (only its location was checked).
- OpenCode driver's env and state details.
- Claude Code `settings.json` `env` support: from memory, unverified.
- No cargo, no docker, no tests run (by brief).

## Contradictions with the "already known" list

1. The schema says secret names are "the environment variables the runs need" (`experiment.schema.json` ~288). But `env_pairs` refuses any secret in the overlay (`run.rs:78-106`), so secrets do not reach the agent container. They reach the host-side gateway (`layers.rs:55`).
2. `run.rs:1-25` says the environment is "built from nothing". Code does not clear the image env; `-e` only overrides (Docker docs). Image `ENV` survives into the agent (I).
3. `mcp.rs:32-70` does not contain MCP env handling; the span is server re-keying (`by_name`).
4. The "`env: []` in a recorded manifest" was not located; `gather.rs:237-285` is a separate record of names.
5. Confirmed as stated: `probe_env` (`image.rs:936-943`), `layers.rs` 27-58 and 60-122, `driver.rs:1-24` "data only", `apply_harness_env` (`exec/mod.rs:252-290`), `create_argv` 139-207, `reserved_names` 229-235, `agent_argv` 237-283.

## Method

colgrep search and expand only (about 25 calls), plus three direct file reads for schema and contract docs, one Context7 Docker docs query, and a `find` for `codes.toml` and the report folder.
