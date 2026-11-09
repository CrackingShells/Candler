# Surfaces: import, evaluate, show, diff

**Goal**: `candler import | evaluate | show | diff` drive the importer, the engine, the rates and the diff. They emit stable JSON whose key set is a hand-written contract, and they exit 1 exactly when `diff` finds a change, through the one exit mapping the binary shares with M2b (CLI1, CLI2, CLI3).
**Pre-conditions**:
- [ ] [cmd] ENV, DEP(skeleton) and the L1 merge of `selection_rates`
- [ ] [cmd] `git log --first-parent --format=%s feat/m2a-evaluation-parity | grep -qx 'merge(evaluator-helper): land helper_runtime'` (the synthetic plug-in for CLI2)
**Success Gates**:
- ✅ [cmd] STD, DRIFT, SUBJ and SCAFFOLD pass, and OWNED passes with `^(crates/candler/(src/(main\.rs$|commands/(mod|common|import|evaluate|show|diff)\.rs$)|tests/(guard_cli\.rs$|fixtures/cli/))|crates/candler-schemas/(schema/run-manifest\.schema\.json$|src/execution\.rs$|tests/)|crates/candler-core/(src/import/|tests/(guard_legacy_[a-z_]+\.rs$|support/legacy_[a-z_]+\.rs$)))` (widened 2026-11-09 for Steps 4 and 5)
- ✅ [cmd] SPAWN, then `cargo nextest run -p candler -E 'binary(guard_cli)'` passes (CLI1, CLI2), and `cargo nextest run -p candler -E 'binary(candler)'` passes (CLI3)
- ✅ [nl] `engine`'s integration (L2 tip) is in place before CLI2 runs a real `evaluate`. Until then CLI2 runs `import`, `show` and `diff` only, and the report says so.
**References**: [R01 §CLI surface](../../../../__design__/architecture.md); [R03 §Skeleton seams (flags, exit codes), §Generic diff (JSON keys, rates TOML)](../../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md); `crates/candler-core/src/import/` — the M1 importer API; `crates/candler-core/src/evaluation/selection/` — `DiffReport`, `Rates`; [R04 CLI1, CLI2](../../../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md)

## Step 1: Wire import and evaluate
**Goal**: `import` and `evaluate` call the core with the frozen flags.
**Implementation Logic**:
0. The exit shape, binding for the whole binary (M2 coordinator ruling, 2026-11-06):
   - `commands/mod.rs` holds only `CliError`, `Exit` and the module list.
     - `CliError` takes M2b's variants at `6a30ed1`: `Unavailable { verb }`, `Refused { reason }`, `Failed { verb, source }` and `Runtime { detail }`, all `#[non_exhaustive]`. This keeps the later rebase a no-op.
     - `pub enum Exit { Success, Changed { count: usize } }`.
     - `not_built` is removed.
   - Every verb is `async fn run(..) -> Result<Exit, CliError>`. Only `diff` builds `Exit::Changed`, and every other verb returns `Ok(Exit::Success)`.
   - A found change is never a `CliError`, because it would travel through `?` and stderr. A later non-error status becomes another `Exit` variant, never a raw `ExitCode`.
   - `main.rs` has the one mapping: `Ok(Exit::Success)` → 0, `Ok(Exit::Changed { .. })` → 1, and any `CliError` → 2, the runtime failing to start included.
   - CLI3 is one table test of that mapping in `main.rs`, with three rows. Its oracle is the diff(1) convention: 0 for the same, 1 for a difference, 2 for trouble. Bite: map `CliError` to 1.
   - `common.rs` stays the one home of the store-root lookup, the `Resolver` composition and the `EventSink`. M2b's verbs use it after their rebase.
1. `import <corpus> --instrument <id@version>` runs the M1 importer into `--store` (or `CANDLER_STORE`) and prints a one-line JSON summary (`runs`, `records`).
2. `commands/common.rs` (shared with M2b's commands): the store root (`--store`, else the configuration), the `Resolver` composition, and the binary's `EventSink`. `evaluate` uses them.
3. `evaluate --plugin <manifest> --suite <file> [--runs <glob>] [--concurrency N]` builds the `Resolver` in `common.rs`, adds the evaluation steps with `register_evaluation_steps` (passing the same `Arc<Supervisor>` the engine gets) (the composition point M2b shares: its own `register_*_steps` calls sit beside it, one per line), runs the preflight, then calls `engine::evaluate` with `registry::default_env_root`, one `Arc<Supervisor>` and a cancel receiver that `tokio::signal::ctrl_c` turns `true` (the binary receives SIGINT and never sends a signal: I22). The handler only flips the cancel watch: every stop goes out through `Supervisor`/`StopHandle`, and the handler never signals anything itself and never re-raises SIGINT to the process group (M2 coordinator condition, 2026-11-06). A test that covers Ctrl-C uses the recording fake `Signaller` of `test-support` and asserts that only groups that were spawned and re-verified were stopped, and prints `EvaluateReport` as JSON. A failed or cancelled request is a `CliError` (exit 2). On a non-Unix target `evaluate` returns `CliError::Refused` with reason `unsupported on this platform` (exit 2).
4. No new flag beyond R03's list. An extra flag is a request to the coordinator, since M2b extends the same enum.
**Deliverables**: `crates/candler/src/commands/mod.rs` (`CliError`, `Exit`); `crates/candler/src/main.rs` (the mapping and CLI3); `crates/candler/src/commands/import.rs` (`run`); `crates/candler/src/commands/evaluate.rs` (`run`)
**Consistency Checks**: `cargo clippy -p candler --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(cli): share one exit mapping and wire import and evaluate`

## Step 2: Wire show and diff
**Goal**: `show --rates` prints the rates in the reference oracle's TOML byte format, and `diff` prints the report and sets the exit code.
**Implementation Logic**:
1. `show [--rates] [--evaluator <name@version>] --format json|toml`: without `--rates`, the current records' per-status counts, per evaluator or for the one named; with `--rates` (which requires `--evaluator`), that evaluator's `Rates` per cell in R03's TOML format, keys sorted, ending in one newline, so that `diff <(candler show --rates --evaluator <name@version> --format toml) $ORACLE/rates_by_cell.toml` is a byte comparison.
2. `diff --from --to [--equivalence <file>] --format json|text`: `DiffReport` as JSON with exactly the keys of R03 §Generic diff: `compared`, `unchanged`, `excused`, `changed`, `changes`, `uncompared` (a list of `{expectation, count}`) and `equivalence` (`{sha256, rows}`, or `null` without `--equivalence`). `Ok(Exit::Changed { count: changed })` when `changed > 0`, and `Ok(Exit::Success)` otherwise. A `CliError` (exit 2) on an error, a refused equivalence file, or `compared == 0`, so that a regrade whose requests all failed never reads as "no change".
**Deliverables**: `crates/candler/src/commands/show.rs` (`run`); `crates/candler/src/commands/diff.rs` (`run`)
**Consistency Checks**: `cargo clippy -p candler --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(cli): wire candler show and diff to the core`

## Step 3: Guard the JSON contract and the end-to-end path
**Goal**: CLI1 pins the keys the `jq` gates rely on, and CLI2 runs the whole path on the mini-corpus.
**Implementation Logic**:
1. `tests/fixtures/cli/`: the expected key sets as a hand-written JSON literal.
2. `tests/guard_cli.rs` (`#![cfg(test)]`, the binary through `env!("CARGO_BIN_EXE_candler")`):
   - CLI1 (R2): on a tiny seeded store, `diff --format json` has exactly the hand-literal key set at the top level and in each `changes[]` and `uncompared[]` element and in `equivalence`. It exits 1 on one planted change, 0 on none, and 2 when the `to` version has no record. Oracle: hand-literal JSON. Bite: rename `compared`.
   - CLI2 (R1, R2): `import` the M1 mini-corpus, `evaluate` with the synthetic plug-in, then `show` and `diff`. `compared > 0`, and editing one committed outcome value in the store copy makes `diff` exit 1. Oracle: the M1 mini-corpus. Bite: `diff` always exits 0.
3. No `--help` test and no schema re-check of the JSON (dropped list). Delete scaffold tests and record bites.
**Deliverables**: `crates/candler/tests/guard_cli.rs` (CLI1, CLI2); `crates/candler/tests/fixtures/cli/`
**Consistency Checks**: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)' && cargo nextest run -p candler -E 'binary(guard_cli)'` (expected: PASS)
**Commit**: `test(cli): guard the json contract and the end-to-end path`

## Step 4: Record the rules digest in the manifest
**Goal**: an imported run's `ImportRecord` carries the sha256 of the rules file it was read with, next to its sources (amendment of 2026-11-09, the coordinator's condition on `--rules`).
**Implementation Logic**:
1. `run-manifest.schema.json`: `ImportRecord` gains an optional `rules` (`$ref Digest`, "The sha256 of the import rules file the run was read with."). The change is additive, so the ratchet holds and M1 manifests stay valid.
2. `src/execution.rs`: the matching optional field and its builder setter. Fix the schema crate's tests that build an `ImportRecord`.
3. The commit touches only `crates/candler-schemas/`. It sits first on the branch, so every commit checks.
**Deliverables**: `crates/candler-schemas/schema/run-manifest.schema.json`; `crates/candler-schemas/src/execution.rs`
**Consistency Checks**: `cargo nextest run -p candler-schemas` (expected: PASS)
**Commit**: `feat(schemas): record the import rules digest in the run manifest`

## Step 5: Write and check the rules digest on import
**Goal**: the importer writes the rules digest it is given, and a re-import of a finished run with another digest is refused, never silent.
**Implementation Logic**:
1. `ImportRules` (it is `#[non_exhaustive]`) gains an optional source digest, set through the builder. The importer writes it into every `ImportRecord.rules`.
2. A finished run whose stored `rules` differs from the given one (including present versus absent) is refused with `ImportError::Conflict`, which names `import.rules`. The existing manifest comparison already refuses it; the error must name the field. An unfinished run is replaced, as before.
3. `candler import` hashes the rules file's bytes with `candler_core::import::digest_of` and always sets the digest.
4. The commit sits second on the branch, after Step 4.
**Deliverables**: `crates/candler-core/src/import/`; `crates/candler/src/commands/import.rs`
**Consistency Checks**: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)' && cargo nextest run -p candler-core` (expected: PASS)
**Commit**: `feat(importer): record the rules digest and refuse a changed one`

