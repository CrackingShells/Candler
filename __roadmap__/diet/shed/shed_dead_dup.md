# shed_dead_dup

**Goal**: Remove the dead paths and duplicated machinery the diet plan proves, with no behaviour change.
**Estimate**: about 40 Sonnet calls ($1.3 with the verifier), cap 60. At 60 calls, stop, commit what is green, and list what is left in the leaf report.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/diet HEAD` succeeds, with every L0 piece merged into `feat/diet`
- [ ] `[nl]` OWNED = `^crates/candler-core/src/(runner/plan\.rs|tree\.rs|experiment/inputs\.rs|import/(raw|read)\.rs|resolve/steps/ground_truth\.rs|jobs/runtime/session\.rs|isolation/image\.rs|isolation/container/guard\.rs|digest\.rs)$|^crates/candler-core/tests/it/(guard_run_plan|guard_build_context)\.rs$|^docs/contract/codes\.toml$|^__reports__/diet/leaves/shed_dead_dup`
**Success Gates**:
- ⬜ `PlanOutcome::summary` and `PlanSummary` are gone with their only test
- ⬜ The five tree re-walks use `tree.rs Walk`; `guard_build_context` and DIGEST are unchanged
- ⬜ One sha256-to-`Digest` path and one `docker_failed` mapper remain
- ⬜ The zero-site code probe ran once and was deleted; each proven row carries `retired`, and no row is deleted
- ⬜ `cargo nextest run -p candler-core` passes
- ⬜ `cargo fmt --check` passes
- ⬜ `cargo clippy --workspace --all-targets -- -D warnings` passes
- ⬜ `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes
- ⬜ DIGEST unchanged: `cargo nextest run --workspace -E 'test(/cell_digest|context_digest|build_context/)'` passes with no vector edited
- ⬜ GOLDEN unchanged: `cargo nextest run --workspace -E 'test(/golden/)'` passes with no vector edited
- ⬜ Ownership: `git diff --name-only feat/diet...HEAD | grep -vE '^crates/candler-core/src/(runner/plan\.rs|tree\.rs|experiment/inputs\.rs|import/(raw|read)\.rs|resolve/steps/ground_truth\.rs|jobs/runtime/session\.rs|isolation/image\.rs|isolation/container/guard\.rs|digest\.rs)$|^crates/candler-core/tests/it/(guard_run_plan|guard_build_context)\.rs$|^docs/contract/codes\.toml$|^__reports__/diet/leaves/shed_dead_dup'` prints nothing
- ⬜ A fresh Sonnet verifier, given only this leaf and the diff, reports no promise left without a test and no gate unmet
**References**: [Diet plan](../../../__reports__/diet/00-architecture_diet_plan_v0.md) — §What the codebase sheds, with file and line evidence; [AGENTS.md](../../../AGENTS.md) — checks and conventions

## Step 1: Remove dead code and duplicate helpers
**Goal**: Delete what no caller uses and fold what exists twice.
**Implementation Logic**:
Re-check each item with colgrep before touching it. Delete `PlanOutcome::summary` and `PlanSummary` (`runner/plan.rs:1071-1133`) and their test (`guard_run_plan.rs:1206-1277`). Replace `jobs/runtime/session.rs:93 digest_of` with `Digest::of_bytes` and inline the `import/read.rs:220` wrapper. Keep one `docker_failed` mapper and call it from `image.rs` and `container/guard.rs`.
**Deliverables**: `runner/plan.rs`, `guard_run_plan.rs`, `jobs/runtime/session.rs`, `import/read.rs`, `isolation/image.rs`, `isolation/container/guard.rs`.
**Consistency Checks**: `cargo nextest run -p candler-core` (expected: PASS)
**Commit**: `refactor(runner): remove the unused plan summary and duplicate digest and docker mappers`

## Step 2: One tree walker
**Goal**: Every walk of a directory tree goes through `tree.rs Walk`.
**Implementation Logic**:
Move `experiment/inputs.rs:229-286`, `import/raw.rs:314-360`, `import/read.rs:400-433` and `469-483`, and `resolve/steps/ground_truth.rs:199-217` onto `Walk`. `ground_truth` builds the same path-to-digest map as `tree`, so it calls it. Keep `commands/ls.rs` (it lists only `*.toml`). If a walker differs in symlink or ordering rules, keep it and record why. Bite: change `Walk`'s ordering; see `guard_build_context` red; revert.
**Deliverables**: the five files above, `tree.rs` only if `Walk` needs a filter argument.
**Consistency Checks**: DIGEST and `guard_build_context` (expected: PASS)
**Commit**: `refactor(run-store): walk every tree through one walker`

## Step 3: Retire unemitted codes
**Goal**: Codes the catalog lists but no variant constructs are marked, not deleted.
**Implementation Logic**:
Write a throwaway test that lists `candler_core::refusal::catalog()` codes with no construction site (the eight rows of the M6b codes review, V4), run it once, record the output in the leaf report, and delete the test. Give each proven row `retired` in `docs/contract/codes.toml`; regenerate pages with the bless command only if a page guard asks.
**Deliverables**: `docs/contract/codes.toml`; the probe output in the leaf report.
**Consistency Checks**: `cargo nextest run --workspace -E 'test(/codes|refusal|contract/)'` (expected: PASS)
**Commit**: `fix(schemas): retire the codes no refusal constructs`

## Step 4: Verify
**Goal**: An agent that did not write the change attacks it.
**Implementation Logic**:
Hand a fresh Sonnet verifier this leaf, the leaf report and `git diff feat/diet...HEAD`, with the brief: find any behaviour changed by the walker or helper folding, and any code row deleted rather than retired. Fix what it finds with a fresh agent, once; list the rest as follow-ups.
**Deliverables**: `__reports__/diet/leaves/shed_dead_dup_v0.md` with First reads, Bites, Verifier and Follow-ups.
**Consistency Checks**: the Success Gates above (expected: PASS)
**Commit**: `reports(api): close shed_dead_dup`
