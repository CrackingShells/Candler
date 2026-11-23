# experiment_env build, partial (stopped at the call cap)

Status: Step 1 reads done; no guard, schema field, wiring or refusal built. Nothing but this file is committed.

## First reads
- Additive within `experiment/1`: `RawExperimentFile` is `deny_unknown_fields` (`crates/candler-schemas/src/execution.rs:1119-1156`), but an optional field with `#[serde(default)]` and `skip_serializing_if` (as `treatment` and `inputs` already are, `execution.rs:1197-1216`) leaves every existing file valid and serialises an env-less file as today. A minor bump, no `/2`.
- `ExperimentFile` is a hand-built type: `new` takes seven positional arguments and delegates to private `build` (`execution.rs:1219-1317`); `ExperimentFile::new` has many callers, so adding `env` as a constructor argument ripples through tests and crates (the schemas commit may touch only `candler-schemas`). A builder-style setter (like `inputs: Vec::new()` set after `build`) avoids that ripple.
- Cell values: `build` makes `plan.fixed` from `fixed` with `ResolvedValue::recorded` (`execution.rs:1290-1300`); `resolve/layers.rs` resolves six layers per path, and `is_setting` (by `resolve/paths.rs` `CONFIGURATION_PATHS`) decides what never reaches a cell digest. An `env.<NAME>` path therefore needs a row in that table of a non-setting class, or `is_known_path` refuses it.
- Harness environment: `DockerIsolation::plan` builds `ContainerPlan.env` from literals `PATH`, `HOME`, `VIRTUAL_ENV`, `LANG` with values from the constants `AGENT_PATH`, `CONTAINER_HOME`, `AGENT_VENV`, `AGENT_LANG` (`isolation/container/mod.rs:259-394`); the names are literals there, so the reserved list needs name constants made there. `apply_harness_env` adds the driver's state variables (`runner/exec/mod.rs:321-359`, from `driver.state_paths()`); `agent_argv` pushes the gateway's two variable names (`driver.gateway()`) and refuses a duplicate (`isolation/container/run.rs:237-283`). The gateway is a separate container; secret values never enter `ContainerPlan.env`.
- So the reserved set is: the four literals (to become constants), `driver.gateway()` names, `driver.state_paths()` env names, and `secrets`. The last three are dynamic (driver-dependent), so the refusal at `resolve` needs the driver, which `resolve` may not hold: check before building.
- Duplicate names: not read (a repeated TOML key is a parse error by the TOML reader; unconfirmed here).
- Digest vectors: not read.

## Built
- `env` is an optional table in `experiment/1`, set after `build` (`ExperimentFile::with_env`, `env()`), serialised only when non-empty (`crates/candler-schemas/src/execution.rs`, `RawExperimentFile.env`, `ExperimentFile.env`); the `secrets` description now says its values reach the gateway only. Additive: a minor bump, no `/2`.
- `FieldPath` segments are lowercase `[a-z][a-z0-9_]*`, so `env.<NAME>` cannot be a path. The table is ONE cell value at `env`, a `Leaf` `Value` row of `CONFIGURATION_PATHS` (`resolve/paths.rs`), so it is recorded with its source and enters the cell digest like any value. `request_settings` (`crates/candler/src/commands/preflight.rs`) writes it only when the file has a table.
- Refusals: `Layered::new` calls `steps::env::check`; `ResolveError::Env(EnvRefusal)` with `Reserved`, `Invalid`, `Secret`; codes `experiment.env_reserved`, `experiment.env_name_invalid`, `experiment.env_secret` (pointer `/env/<NAME>`, `codes.toml` appended, no `remedy`). The reserved list is `PLAN_ENV_NAMES`, built from `ENV_PATH`, `ENV_HOME`, `ENV_VIRTUAL_ENV`, `ENV_LANG`, the constants `DockerIsolation::plan` now sets its variables with (`isolation/container/mod.rs`); the secrets come from `file.secrets()` through `ResolveInput::secret_names`.
- Reach: `DockerIsolation::plan` pushes each `env` entry into `plan.env`, the environment of the harness container. The gateway is another container and never sees it.

## Bites
| # | Bug made | Red guard | Reverted |
|:--|:---------|:----------|:---------|
| 1 | `env` row of the path table made a `Setting` (out of the digest) | `guard_resolve::env_enters_the_cell_digest` | yes |
| 2 | an empty `env` value written when the file has no table | `commands::preflight::tests::env_is_a_value_only_when_the_file_has_a_table`; no DIGEST test went red (see D3) | yes |
| 3 | `HOME` replaced by `PATH` in `PLAN_ENV_NAMES` | `guard_resolve::reserved_env_names_are_refused` | yes |
| 4 | the table not pushed into the plan's environment (the gateway bite: the harness plan loses it) | `guard_container_record::env_reaches_the_harness_process` | yes |

## D3
Step 2's answer: the DIGEST vectors (`guard_cell_digest`, the core guards, the oracle vectors) do NOT cover an experiment resolved from a file by the CLI: with bite 2 made, the whole `candler` suite (152 tests), `guard_cell_digest` and the core guards stayed green. The case is added as its own guard (`env_is_a_value_only_when_the_file_has_a_table`), which tests the feed, because that is where an absent table could add an `env` value.

| Test | Question | Role | Why it can fail | Oracle |
|:-----|:---------|:-----|:----------------|:-------|
| `env_enters_the_cell_digest` | does an `env` value reach the values, not the settings, and does a different value change them | guard | bite 1 | paths and text by hand; the class "Value reaches the digest" is the path table's own doc |
| `reserved_env_names_are_refused` | is each name Candler sets, each declared secret and each non-variable name refused, naming `env.<NAME>`, and a free name allowed | guard | bite 3 | the four names, a secret and three invalid names by hand |
| `env_reaches_the_harness_process` | does `DockerIsolation::plan` put the table's name and value, once, in the harness plan's environment, with no gateway variable set by the plan | guard | bite 4 | name and value by hand |
| `env_is_a_value_only_when_the_file_has_a_table` | does a file without a table add no `env` value, and one with a table add exactly that table | guard | bite 2 | table by hand |
The `Guards:` lines are in the doc comments. All four are kept.

## Verifier
Not yet run (see the final message).

## Follow-ups
- A driver or gateway name equal to an `env` entry is not refused at `resolve` (it holds no driver), but it is refused at run, not overwritten: `agent_argv` returns `DuplicateName` when a gateway variable is already in `plan.env` (`isolation/container/run.rs:237-283`), and `apply_harness_env` refuses a state variable already set (`runner/exec/mod.rs:321-359`).
- Guards were written after the code, not first.
- The gateway side has no environment of its own in the plan, so "the gateway does not get it" is covered by the plan holding no gateway variable and the table being only in `plan.env`.
- `a name already declared in secrets` is checked against `file.secrets()` only, not against `model.token_secret`.
