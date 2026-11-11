# Harness out of core

**Goal**: `candler-core` asks the driver for every harness fact and names no harness, the binary selects the driver by `request.harness` through a registry (amendment A3, decisions D9 and D10), and no command line, census value or cell digest of a Claude Code run moves.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [x] [nl] D10 is ruled (2026-11-11): an unknown `request.harness` is refused, `feat(cli)!` with the migration in its body
- [ ] [cmd] The branch is cut from the rebase point, `main` at c356a0e (M3's scaffold, merged as 2126181, moved `foreground` and its neighbours into `commands/preflight.rs`): `git merge-base --is-ancestor c356a0e HEAD`
- [ ] [nl] Edits in `commands/import.rs` stay inside function bodies (M3 renames the file), a field added to `JobServices` is reported for the merge with M3's `evaluate_body`, and a hunk in `crates/candler-core/src/jobs/runtime/session.rs` meets M3's `observe` lines (the verified spec written to `<job dir>/spec.json`), which the milestone that merges second resolves
- [ ] [nl] OWNED: `crates/candler-core/src/isolation/{image,model}.rs`, `crates/candler-core/src/isolation/container/{run,mounts}.rs` (the variable-name hunks), `crates/candler-core/src/resolve/steps/{identity,mod}.rs`, `crates/candler-core/src/runner/evidence/census.rs`, `crates/candler-core/src/runner/exec/{mod,spawn,gather,cell}.rs` (`Observed`, the census call, `LiveHandle::observed`, the test glob, the core `SkillTree`), `crates/candler-core/src/jobs/runtime/**`, `crates/candler-core/src/evaluation/context/rule.rs` (one doc line); `crates/candler/src/{drivers.rs,commands/preflight.rs,commands/supervise.rs,commands/run.rs,commands/import.rs}`, `crates/candler/Cargo.toml`; their tests and fakes (`guard_image.rs`, `support/{image,job_orchestrate,job_runtime,cli_run}.rs`, `guard_declared_inputs.rs`, `guard_settings_out_of_cell.rs`)
**Success Gates**:
- ⬜ [cmd] H1: the image's harness version is the driver's parse of the driver's probe, and the label key is unchanged: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^harness_version_/)' && git grep -q '"candler.cc_version"' -- crates/candler-core/src/isolation/image.rs`
- ⬜ [cmd] H2: the gateway and state variables come from the driver (a second fake driver's names reach the argv), and the headless Claude Code `docker run` argv equals the one recorded from a26fe53: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^harness_env_/)'`
- ⬜ [cmd] H3: the census from the driver's `InitReport` gives `census.tools`, `census.mcp_servers` and `census.init_tools` byte-identical to a26fe53 on every Claude Code fixture: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^harness_census_/)'`
- ⬜ [cmd] H4: the repo's experiment files (`cont_train.toml`, the M2c smoke experiment, the test experiments) resolve to a `comparable` byte-identical to a26fe53's, with `harness.driver` absent: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^harness_resolution_/)'`
- ⬜ [cmd] R1: the registry selects the driver, skill tree, reader and base context by name from an unrelated cwd, and an unknown name is refused naming it: `cargo nextest run -p candler --no-tests=fail -E 'test(/^registry_/)'`
- ⬜ [cmd] The core names no harness: `! git grep -n -i -E 'claude|anthropic|opencode' -- crates/candler-core/src`, `git grep -q -E '\.version_probe\(' -- crates/candler-core/src`, `! git grep -n -E '"subtype"|"init"' -- crates/candler-core/src/runner/evidence`
- ⬜ [cmd] Only the registry names a driver in the binary: `test -z "$(git grep -l -E 'ClaudeCodeDriver|ClaudeCodeReader|ClaudeSkills' -- crates/candler/src | grep -v 'crates/candler/src/drivers.rs')"`
- ⬜ [cmd] SPAWN, STD (workspace form: the leaf spans the core and the binary), LINUX, DIGEST, GOLDEN and SUBJ pass
- ⬜ [nl] The leaf report records the bites: the core's own `--version` argv (H1 red), `ANTHROPIC_*` hardcoded or a new `CLAUDE_CONFIG_DIR` (H2 red), the core matching `system`/`init` with a reordered key (H3 red), `harness.driver` recorded (H4 red), the fallback to Claude Code (R1 red)
**References**: [R03 Gap analysis A3](../../../../__reports__/candler-v1-review/01-gap_analysis_harness_env_v0.md) — H1's evidence; [R01 §Harness drivers, §Process supervision (control containers), §Run manifest (`comparable.harness`)](../../../../__design__/architecture.md); [R09 D9, D10](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-approved-plan.md)

## Step 1: Record the baselines
**Goal**: What must not move is written down from a26fe53 before anything moves.
**Implementation Logic**:
On a checkout of a26fe53 (a scratch worktree, removed afterwards), record with the existing recording fakes: the headless `docker run` argv of a reference cell; the three `census.*` values of every Claude Code stream fixture; the `comparable` of every experiment file H4 lists. Commit them as test fixtures, each with a note naming the commit it was recorded from. Write H1–H4 against them; H1's driver-probe test and H2's second-driver test fail (red).
**Deliverables**: `crates/candler-core/tests/fixtures/harness_baseline/{argv.json,census.json,comparable/*.json}`; `crates/candler-core/tests/guard_harness_facts.rs`: `harness_version_*`, `harness_env_*`, `harness_census_*`, `harness_resolution_*`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_harness_facts)'` (expected: FAIL)
**Commit**: `test(isolation): pin what a claude code run looks like before the move`

## Step 2: Take the version from the driver
**Goal**: The control container runs the driver's probe and the driver parses it.
**Implementation Logic**:
`ImageService::cc_version` becomes a harness-neutral `harness_version` that runs the driver's `version_probe()` in the control container and calls its `parse_version`; the label constant keeps the key `candler.cc_version` under a neutral name; `HarnessIdentityStep` checks the driver's `supported()`. Remove `parse_cc_version`, the `(Claude Code)` suffix and the hardcoded argv. Error texts name the harness by its id.
**Deliverables**: `crates/candler-core/src/isolation/image.rs`: `ImageService::harness_version`, `HARNESS_VERSION_LABEL`; `crates/candler-core/src/isolation/model.rs`; `crates/candler-core/src/resolve/steps/identity.rs`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^harness_version_/)|binary(guard_image)'` (expected: PASS)
**Commit**: `refactor(isolation): read the harness version through its driver`

## Step 3: Take the variables and the census from the driver
**Goal**: The container's gateway and state variables, the census and the observed ids all come from the driver.
**Implementation Logic**:
1. `container/run.rs`: the reserved names, the base URL and token variables from `GatewayEnv` (or the provider-configuration form `driver_traits` chose).
2. `container/mounts.rs`: the writability check runs over the driver's `state_paths()`, and over HOME as today; the `CLAUDE_CONFIG_DIR` constant goes. Claude Code's paths are empty, so its argv equals the baseline.
3. `census.rs` derives the three values from `Observed.init` exactly as today; it no longer reads event keys. `LiveHandle::observed()` exposes the live reader's ids before `finish`.
4. The `.claude/**` test glob in `gather.rs` and the doc line in `rule.rs` become harness-neutral.
**Deliverables**: `crates/candler-core/src/isolation/container/{run,mounts}.rs`; `crates/candler-core/src/runner/evidence/census.rs`; `crates/candler-core/src/runner/exec/{mod,spawn,gather}.rs`: `LiveHandle::observed`; `crates/candler-core/src/evaluation/context/rule.rs`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^harness_env_/)|test(/^harness_census_/)'` (expected: PASS)
**Commit**: `refactor(isolation): take the gateway, state and census from the driver`

## Step 4: Select the driver by name
**Goal**: The binary's registry is the only place that names a driver, and the core's `SkillTree` gives way to the driver's.
**Implementation Logic**:
1. `crates/candler/src/drivers.rs`: `Registry` and `DriverEntry {driver, skills, base_context, reader}`, holding Claude Code under its `HarnessId`.
2. `foreground()` reads `request.harness` before building the image service and the resolver, and takes the base context and `supported()` from the entry; `supervise` takes the name from the job's cells; `import` takes the reader by name. `JobServices` holds the entry instead of a bare driver.
3. The core's `runner::exec::cell::SkillTree` and the binary's `ClaudeSkills` are removed in favour of `candler_harness::skills::SkillTree` and the driver crate's implementation.
4. An unknown name is refused, naming it and the registered names. That refuses input a26fe53 ran as Claude Code: the commit is breaking, and its body names the migration (name a registered harness). The coordination session approved it (2026-11-11).
**Deliverables**: `crates/candler/src/drivers.rs`: `Registry`, `DriverEntry`; `crates/candler/src/commands/{preflight,run,supervise,import}.rs`; `crates/candler-core/src/jobs/runtime/mod.rs`: `JobServices`; `crates/candler-core/src/runner/exec/cell.rs`; `crates/candler/tests/guard_registry.rs`: `registry_*`
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/^registry_/)'` (expected: PASS)
**Commit**: `feat(cli)!: select the harness driver by the experiment's name`
