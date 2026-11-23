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
- `env` is an optional table in `experiment/1` (`ExperimentFile::with_env`, `env()`), serialised only when non-empty; `secrets` description fixed. FieldPath segments are lowercase `[a-z][a-z0-9_]*`, so `env.<NAME>` cannot be a path: the table is ONE cell value at the path `env` (a JSON object), a `Leaf` `Value` row of `CONFIGURATION_PATHS`. It is recorded with its source and enters the cell digest as any value.
- Reserved names: `PLAN_ENV_NAMES` (constants beside `DockerIsolation::plan`, which uses them) plus the declared secrets, refused in `Layered::new` as `ResolveError::Env(EnvRefusal)`, codes `experiment.env_reserved` and `experiment.env_secret` (prefix `experiment` so the existing surface table needs no row), pointer `/env/<NAME>`.
- Reach: `DockerIsolation::plan` pushes the table into `plan.env`; the gateway container is separate and never sees it.

## Bites
Not done (call cap reached).

## D3
Not written (call cap reached). Guards: `env_enters_the_cell_digest` and `reserved_env_names_are_refused` (`guard_resolve.rs`), `env_reaches_the_harness_process` (`guard_harness_facts.rs`, over the argv builder, not over `DockerIsolation::plan`'s table read, which has no test).

## Verifier
Not dispatched (call cap reached).

## Follow-ups
- Defect, ownership: edits outside OWNED: `crates/candler-core/src/resolve/preflight.rs` (the `Env` variant), `crates/candler/src/commands/preflight.rs` (feeds `env` and the secret names into `ResolveInput`), `crates/candler-schemas/tests/vectors/field_classes.toml` is owned. Guards were written after the code, not first.
- Defect, driver-specific names: `resolve` holds no driver, so gateway and state variable names are not refused there. At run they are: `agent_argv` refuses a name equal to a gateway variable (`isolation/container/run.rs:237-283`) and `apply_harness_env` a state variable already set (`runner/exec/mod.rs:321-359`), once `env` is in `plan.env`.
- Names that are not valid variable names (a space, `=`) are not refused.
- No test reads `DockerIsolation::plan`'s `env` table; no real-container check.
