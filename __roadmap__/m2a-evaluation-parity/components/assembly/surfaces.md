# Surfaces: import, evaluate, show, diff

**Goal**: `candler import | evaluate | show | diff` drive the importer, the engine, the rates and the diff. They emit stable JSON whose key set is a hand-written contract, and they exit 1 exactly when `diff` finds a change (CLI1, CLI2).
**Pre-conditions**:
- [ ] [cmd] ENV, DEP(skeleton) and the L1 merge of `selection_rates`
- [ ] [cmd] `git log --first-parent --format=%s feat/m2a-evaluation-parity | grep -qx 'merge(evaluator-helper): land helper_runtime'` (the synthetic plug-in for CLI2)
**Success Gates**:
- ⬜ [cmd] STD, SUBJ and SCAFFOLD pass, and OWNED passes with `^crates/candler/(src/commands/(import|evaluate|show|diff)\.rs|tests/(guard_cli\.rs|fixtures/cli/))$`
- ⬜ [cmd] SPAWN, then `cargo nextest run -p candler -E 'binary(guard_cli)'` passes (CLI1, CLI2)
- ⬜ [nl] `engine`'s integration (L2 tip) is in place before CLI2 runs a real `evaluate`. Until then CLI2 runs `import`, `show` and `diff` only, and the report says so.
**References**: [R01 §CLI surface](../../../../__design__/architecture.md); [R03 §Skeleton seams (flags, exit codes), §Generic diff (JSON keys, rates TOML)](../../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md); `crates/candler-core/src/import/` — the M1 importer API; `crates/candler-core/src/evaluation/selection/` — `DiffReport`, `Rates`; [R04 CLI1, CLI2](../../../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md)

## Step 1: Wire import and evaluate
**Goal**: `import` and `evaluate` call the core with the frozen flags.
**Implementation Logic**:
1. `import <corpus> --instrument <id@version>` runs the M1 importer into `--store` (or `CANDLER_STORE`) and prints a one-line JSON summary (`runs`, `records`).
2. `evaluate --plugin <manifest> --suite <id> [--runs <filter>] [--concurrency N]` calls `engine::evaluate` and prints `EvaluateReport` as JSON. A failed request exits 2.
3. No new flag beyond R03's list. An extra flag is a request to the coordinator, since M2b extends the same enum.
**Deliverables**: `crates/candler/src/commands/import.rs` (`run`); `crates/candler/src/commands/evaluate.rs` (`run`)
**Consistency Checks**: `cargo clippy -p candler --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(cli): wire candler import and evaluate to the core`

## Step 2: Wire show and diff
**Goal**: `show --rates` prints the rates in the reference oracle's TOML byte format, and `diff` prints the report and sets the exit code.
**Implementation Logic**:
1. `show [--rates] --format json|toml`: without `--rates`, the current records' per-status counts; with `--rates`, `Rates` per cell in R03's TOML format, keys sorted, ending in one newline, so that `diff <(candler show --rates --format toml) $ORACLE/rates_by_cell.toml` is a byte comparison.
2. `diff --from --to [--equivalence <file>] --format json|text`: `DiffReport` as JSON with exactly the keys `compared`, `changed`, `changes`, `uncompared` (a list of `{expectation, count}`) and `equivalences_applied`. Exit 1 when `changed > 0`, 0 otherwise, 2 on an error.
**Deliverables**: `crates/candler/src/commands/show.rs` (`run`); `crates/candler/src/commands/diff.rs` (`run`)
**Consistency Checks**: `cargo clippy -p candler --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(cli): wire candler show and diff to the core`

## Step 3: Guard the JSON contract and the end-to-end path
**Goal**: CLI1 pins the keys the `jq` gates rely on, and CLI2 runs the whole path on the mini-corpus.
**Implementation Logic**:
1. `tests/fixtures/cli/`: the expected key sets as a hand-written JSON literal.
2. `tests/guard_cli.rs` (`#![cfg(test)]`, the binary through `env!("CARGO_BIN_EXE_candler")`):
   - CLI1 (R2): on a tiny seeded store, `diff --format json` has exactly the hand-literal key set at the top level and in each `changes[]` and `uncompared[]` element. It exits 1 on one planted change and 0 on none. Oracle: hand-literal JSON. Bite: rename `compared`.
   - CLI2 (R1, R2): `import` the M1 mini-corpus, `evaluate` with the synthetic plug-in, then `show` and `diff`. `compared > 0`, and editing one committed outcome value in the store copy makes `diff` exit 1. Oracle: the M1 mini-corpus. Bite: `diff` always exits 0.
3. No `--help` test and no schema re-check of the JSON (dropped list). Delete scaffold tests and record bites.
**Deliverables**: `crates/candler/tests/guard_cli.rs` (CLI1, CLI2); `crates/candler/tests/fixtures/cli/`
**Consistency Checks**: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)' && cargo nextest run -p candler -E 'binary(guard_cli)'` (expected: PASS)
**Commit**: `test(cli): guard the json contract and the end-to-end path`
