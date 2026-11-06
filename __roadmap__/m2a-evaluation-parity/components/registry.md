# Registry: manifests, suites, environments

**Goal**: `evaluation::registry` loads plug-in manifests and suites, checks every suite entry's kind, params and needs without spawning anything, refuses `snapshots` and `inputs` needs as unsupported in this version, and builds each plug-in environment once per lock digest as a preflight step (REG1, REG2).
**Pre-conditions**:
- [ ] [cmd] ENV, DEP(skeleton) and DEP(protocol_wire)
**Success Gates**:
- ⬜ [cmd] STD, SUBJ and SCAFFOLD pass, and OWNED passes with `^crates/candler-core/(src/evaluation/registry/|tests/(guard_registry\.rs|fixtures/registry/|public_surface\.rs)$)` with only the `registry_surface` block of `public_surface.rs` changed
- ⬜ [cmd] SPAWN, then `cargo nextest run -p candler-core -E 'binary(guard_registry)'` passes (REG1, REG2)
- ⬜ [cmd] I22: `cargo nextest run -p candler-core -E 'binary(guard_signal)'` still passes, and the registry spawns `uv` only through `supervise::Supervisor`
**References**: [R01 §Plug-ins, §Operation API (preflight steps)](../../../__design__/architecture.md) — manifest, suite, environments, preflight; [R03 §Environments, §Needs, §Scope move](../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md); `crates/candler-schemas/src/evaluation.rs` — `PluginManifest`, `SuiteEntry`, `Needs`, `KindSpec`; `crates/candler-core/src/resolve/` — `Resolver::register(Box<dyn PreflightStep>)`; [R04 REG1, REG2](../../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md)

## Step 1: Load and check manifests and suites
**Goal**: A suite entry is accepted only if its plug-in declares its kind, its params validate against the kind's params schema, and each need is one the core can meet.
**Implementation Logic**:
1. `Registry::load(manifests: &[PathBuf]) -> Result<Registry, RegistryError>` reads each `PluginManifest`, and `Registry::check_suite(&SuiteFile or the suite entries) -> Result<CheckedSuite, RegistryError>` checks every entry.
2. Params are validated with `jsonschema` (a runtime dependency from Phase 0, pinned `=0.60.2` with `resolve-http` and `resolve-file` off) against the kind's `KindSpec` params schema; the error names the entry, the kind and the JSON pointer of the first fault.
3. Needs: `artifacts` (globs under `outputs/**`) and `metadata` (named run-manifest fields such as `plan.prompt_domain_name`) are met; `snapshots` and `inputs` are refused with exactly `unsupported in this version: snapshots` or `unsupported in this version: inputs` (R03 §Scope move). An unknown need is refused.
4. No process is spawned in this step.
**Deliverables**: `crates/candler-core/src/evaluation/registry/{mod,manifest,suite,needs}.rs` (`Registry`, `CheckedSuite`, `RegistryError`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings && cargo nextest run -p candler-core -E 'test(scaffold)'` (expected: PASS)
**Commit**: `feat(registry): check suites against plug-in manifests`

## Step 2: Build plug-in environments once per lock digest
**Goal**: A preflight step materialises each plug-in's uv environment with `uv sync --frozen` keyed by the lock's digest, and a second preflight under the same digest does nothing.
**Implementation Logic**:
1. `EnvironmentStep` implements `PreflightStep`. `pub fn register_evaluation_steps(resolver: Resolver, plugins: &[PluginManifest], env_root: &Path) -> Result<Resolver, ResolveError>` adds one per plug-in through `Resolver::register`; the core never calls it itself, the `candler` crate composes it (coordinator ruling, 2026-11-06). For each plug-in it computes the sha256 of the plug-in's `uv.lock` and `pyproject.toml`, and uses `<env root>/<digest>/`; if a `ready` marker exists there, it spawns nothing.
2. Otherwise it runs `uv sync --frozen --project <plug-in dir>` with `UV_PROJECT_ENVIRONMENT=<env root>/<digest>/.venv` through `Supervisor::run_captured`, writes `ready` only after success, and leaves no half-built environment marked ready.
3. The environment is outside the store: a plug-in never writes into a store blob through it (I11).
**Deliverables**: `crates/candler-core/src/evaluation/registry/environment.rs` (`EnvironmentStep`, `EnvironmentKey`, `register_evaluation_steps`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(registry): build plug-in environments once per lock digest`

## Step 3: Guard the refusals and the environment reuse
**Goal**: REG1 and REG2 decide the registry by spec text and by the OS.
**Implementation Logic**:
1. `tests/fixtures/registry/`: one valid manifest and suite (hand-written, the synthetic plug-in's kinds), each fault a single edit applied in the test.
2. `tests/guard_registry.rs` (`#![cfg(test)]`):
   - REG1 (R8): the valid base passes (positive control); then unknown kind, params failing the schema, an unmeetable need, a `snapshots` need and an `inputs` need are each refused, the last two with the exact text of R03. Oracle: spec text. Bite: skip the `inputs` check.
   - REG2 (R6): with a stub `uv` on `PATH` that appends one line to a counter file, two preflights under the same lock digest run the stub once; the env dir's inode and mtime are unchanged by the second (`std::fs::metadata` against values taken before). Oracle: OS `stat`. Bite: rebuild on rerun.
3. Fill `registry_surface`, delete scaffold tests, record the bites.
**Deliverables**: `crates/candler-core/tests/guard_registry.rs` (REG1, REG2); `crates/candler-core/tests/fixtures/registry/`; the `registry_surface` block
**Consistency Checks**: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)' && cargo nextest run -p candler-core -E 'binary(guard_registry)'` (expected: PASS)
**Commit**: `test(registry): guard the refusals and the environment reuse`
