# Skeleton: frozen seams and stubs

**Goal**: The seams every later leaf builds against compile as stubs, frozen by an API review: the core `evaluation` module tree, `engine::evaluate`, the clap `Command` enum with four stub commands, the `public_surface` anchors, and the I1 scan extended to the binary and the helper (NEUT).
**Pre-conditions**:
- [ ] [cmd] ENV, and `git log --format=%s origin/main..feat/m2a-evaluation-parity | grep -q '^build(deps): '`
- [ ] [cmd] `test -f python/candler-evaluator/src/candler_evaluator/__init__.py`
- [ ] [nl] The M2 coordinator OK'd the change to `crates/candler/src/main.rs`, which M2b also extends (proxy: the reply quoted in Amendment Log A1)
**Success Gates**:
- ⬜ [cmd] STD, SUBJ and SCAFFOLD pass, and OWNED passes with `^(crates/candler-core/src/(lib\.rs|evaluation/)|crates/candler/src/|crates/candler-core/tests/(public_surface|guard_layering_i1)\.rs$)`
- ⬜ [cmd] NEUT: `cargo nextest run -p candler-core -E 'binary(guard_layering_i1)'` passes, and fails with `rikyu` planted in a comment of `crates/candler/src/commands/mod.rs` and again in `python/candler-evaluator/src/candler_evaluator/__init__.py` (each restored with `git checkout -- <file>`; the log goes in the report)
- ⬜ [cmd] `cargo run -q -p candler -- evaluate --help >/dev/null`, and `cargo run -q -p candler -- diff --from a --to b --format json 2>"$TMPDIR/diff.err"; test $? -eq 2 && grep -qx 'candler diff: not built' "$TMPDIR/diff.err"` (a stub exits 2 with `not built` on stderr; `--format` is required, so without it clap's usage error would exit 2 before the stub runs)
- ⬜ [nl] The API review (R04 §Verification) closes with no open finding: the seams fit `engine`, `surfaces`, `selection_rates` and M2b's CLI. Its findings become behaviour in the owning leaf's guards, never signature re-checks.
**References**: [R03 §Skeleton seams](../../__reports__/m2a-evaluation-parity/00-architecture_v0.md) — the frozen types, module names and CLI flags; [R01 §CLI surface, §Evaluator protocol](../../__design__/architecture.md) — command names and request shape; [R04 NEUT](../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md) — the guard, its oracle and bite

## Step 1: Scaffold the evaluation modules and the evaluate seam
**Goal**: The core holds the module tree of M2a with every public seam declared and no behaviour.
**Implementation Logic**:
The coordinator does this leaf (R04: no implementer, an API review instead).
1. `crates/candler-core/src/lib.rs` gains `pub mod evaluation;` (alongside `frames`, `import`, `resolve`, `store`, `supervise`, `trace`).
2. `evaluation/mod.rs` declares one module per L1/L2 leaf, each a directory with a `mod.rs` holding only its doc line: `registry` (registry), `context` (run_context), `client` (protocol_client, `#[cfg(unix)]` like `supervise`), `selection` (selection_rates, with submodules `select`, `rates`, `diff`), `engine` (engine).
3. `engine/mod.rs` declares the seam R03 pins: `EvaluateRequest` (store root, plug-in manifest paths, suite reference, run selection, `NonZeroUsize` concurrency, request deadline), `EvaluateReport` (the request ids, per-status counts, outcome file paths, failed requests), `EvaluateError` (`thiserror`, `#[non_exhaustive]`, one variant `NotBuilt`; `engine` adds `EnvironmentNotReady` and the rest), and `pub async fn evaluate(request: EvaluateRequest) -> Result<EvaluateReport, EvaluateError>`, which returns `Err(EvaluateError::NotBuilt)`.
4. A `#[cfg(test)] mod scaffold` checks the stub returns `NotBuilt` (class A; Step 3 deletes it).
**Deliverables**: `crates/candler-core/src/lib.rs` (`pub mod evaluation`); `crates/candler-core/src/evaluation/mod.rs`; `crates/candler-core/src/evaluation/{registry,context,client,engine}/mod.rs`; `crates/candler-core/src/evaluation/selection/{mod,select,rates,diff}.rs`; `engine::{EvaluateRequest, EvaluateReport, EvaluateError, evaluate}`
**Consistency Checks**: `cargo check --workspace --all-targets && cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(evaluation): scaffold the evaluation modules and the evaluate seam`

## Step 2: Scaffold the four commands
**Goal**: `candler import | evaluate | show | diff` parse their frozen flags and exit 2 as stubs.
**Implementation Logic**:
1. `crates/candler/src/main.rs`: a clap derive `Cli { #[arg(long, env = "CANDLER_STORE", global = true)] store: Option<PathBuf>, #[command(subcommand)] command: Command }` and `enum Command { Import(ImportArgs), Evaluate(EvaluateArgs), Show(ShowArgs), Diff(DiffArgs) }`; `main` builds a current-thread tokio runtime and dispatches to `commands::<name>::run(&cli, args) -> ExitCode`. The enum has one variant per line and the `match` one arm per line, each verb in its own `commands/<verb>.rs`, so M2b's `Run`, `Ps`, `Kill` and `Watch` append cleanly (coordinator ruling, 2026-11-06).
2. Flags R03 pins: `import <corpus> --instrument <id@version>`; `evaluate --plugin <manifest> --suite <id> [--runs <filter>] [--concurrency N]`; `show [--rates] --format json|toml`; `diff --from <id@version> --to <id@version> [--equivalence <file>] --format json|text`.
3. `commands/mod.rs` plus `commands/{import,evaluate,show,diff}.rs`: each `run` writes `not built` to stderr and returns `ExitCode::from(2)`. Exit codes R03 pins for `surfaces`: 0 success or no change, 1 a change found by `diff`, 2 a usage or runtime error.
4. M2b appends variants to `Command` later; the enum is not `#[non_exhaustive]`, since it lives in a binary, and its variants are listed one per line so a rebase conflict is append-only.
**Deliverables**: `crates/candler/src/main.rs` (`Cli`, `Command`, `main`); `crates/candler/src/commands/{mod,import,evaluate,show,diff}.rs` (`ImportArgs`, `EvaluateArgs`, `ShowArgs`, `DiffArgs`, `run`)
**Consistency Checks**: `cargo clippy -p candler --all-targets -- -D warnings && cargo run -q -p candler -- show --help >/dev/null` (expected: PASS)
**Commit**: `feat(cli): scaffold the import, evaluate, show and diff commands`

## Step 3: Anchor the surfaces and extend the layering scan
**Goal**: Each leaf has its own `public_surface` block to fill, and the I1 scan covers the binary and the helper.
**Implementation Logic**:
1. `crates/candler-core/tests/public_surface.rs` gains empty blocks `mod registry_surface {}`, `mod run_context_surface {}`, `mod protocol_client_surface {}`, `mod selection_rates_surface {}`, `mod engine_surface {}`, each with a one-line doc naming its leaf; a leaf edits only its own block.
2. NEUT: `guard_layering_i1.rs`'s first test also scans `crates/candler/src` and `python/candler-evaluator/src` with the existing `DENY_LIST` (which already holds `htn`, `rikyu`, `tier_d` and `D-E`) through `support::assert_clean`. Its `Guards:` line names the four folders. The deny-list literal and the planted-violation self-test of `assert_clean` stay as they are.
3. Delete Step 1's `mod scaffold`. Run the bite (plant `rikyu` in each new folder, see the gate, restore with `git checkout -- <file>`) and record it in the report.
4. Hand the seams to the API reviewer (a Sonnet subagent) with R03 §Skeleton seams, `engine.md`, `surfaces.md`, `selection_rates.md` and `m2b_execution_parity.md`.
**Deliverables**: `crates/candler-core/tests/public_surface.rs` (five `mod <leaf>_surface` blocks); `crates/candler-core/tests/guard_layering_i1.rs` (scan extended); the API review in `__reports__/m2a-evaluation-parity/skeleton/`
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_layering_i1) | binary(public_surface)' && test -z "$(git grep -n 'mod scaffold' -- crates/candler-core/src/evaluation)"` (expected: PASS)
**Commit**: `test(evaluation): extend the layering scan to the binary and the helper`
