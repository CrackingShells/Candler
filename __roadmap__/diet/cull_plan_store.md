# cull_plan_store

**Goal**: Cut the plan, resolve and run store tests to one guard per behaviour per consumer interface.
**Estimate**: about 36 Sonnet calls ($1.2 with the verifier), cap 72. At 72 calls, stop, commit what is green, and list what is left in the leaf report.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/diet HEAD` succeeds
- [ ] `[nl]` OWNED = `^crates/candler-core/tests/it/(guard_resolve[a-z_]*|guard_run_plan|guard_run_evidence|guard_store|guard_settings_out_of_cell|guard_plugin_environment|guard_tree_walk|guard_build_context|guard_frames|guard_treatment_[a-z_]+|guard_secrets_step|guard_upstream_ca|guard_seams|guard_transcript|guard_assert_clean|main)\.rs$|^__reports__/diet/leaves/cull_plan_store`. No other L0 piece edits these paths; `tests/it/main.rs` mod lines are shared and the second piece to merge rebases
**Success Gates**:
- ⬜ Every deleted test is listed with its covering guard or "structure"
- ⬜ Every merged or rewritten guard was seen red once by a bite, then the bite reverted
- ⬜ `cargo nextest run -p candler-core` passes
- ⬜ `cargo fmt --check` passes
- ⬜ `cargo clippy --workspace --all-targets -- -D warnings` passes
- ⬜ `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes
- ⬜ DIGEST unchanged: `cargo nextest run --workspace -E 'test(/cell_digest|context_digest|build_context/)'` passes with no vector edited
- ⬜ GOLDEN unchanged: `cargo nextest run --workspace -E 'test(/golden/)'` passes with no vector edited
- ⬜ Ownership: `git diff --name-only feat/diet...HEAD | grep -vE '^crates/candler-core/tests/it/(guard_resolve[a-z_]*|guard_run_plan|guard_run_evidence|guard_store|guard_settings_out_of_cell|guard_plugin_environment|guard_tree_walk|guard_build_context|guard_frames|guard_treatment_[a-z_]+|guard_secrets_step|guard_upstream_ca|guard_seams|guard_transcript|guard_assert_clean|main)\.rs$|^__reports__/diet/leaves/cull_plan_store'` prints nothing
- ⬜ A fresh Sonnet verifier, given only this leaf and the diff, reports no promise left without a test and no gate unmet
**References**: [Diet plan §How the suite becomes the critical set](../../__reports__/diet/00-architecture_diet_plan_v0.md) — the six interfaces, the family verdicts and the critical set; [AGENTS.md](../../AGENTS.md) — checks and conventions

## Step 1: Confirm each verdict before deleting
**Goal**: Turn the sweep's verdicts for this family into checked verdicts.
**Implementation Logic**:
Work in a worktree made by `scripts/worktree.py add` from the `feat/diet` tip. Count the family's tests (`cargo nextest list` on the owned crates, filtered to the owned files). For each owned file, read its header and test docs through colgrep `search`/`expand` and confirm the verdict below. A test is kept when it observes one of the six consumer interfaces (CLI records and exit codes; the core operation API; wire formats and stored artifacts; digests and golden vectors; the plug-in contract; isolation seen from outside) and no other kept test holds the same behaviour at the same interface. A test is deleted when it pins structure, tests a test helper, or repeats a kept guard. A cull with no covering guard is kept and listed.
Verdicts from the sweep: 83 tests now, about 59 kept. Delete `guard_assert_clean`. Merge `guard_tree_walk` into `guard_build_context` (2 survive). `guard_run_plan` keeps about 9 of 16; keep b26 (resume). The treatment-level (t3) rows repeated across `guard_run_plan` and `guard_settings_out_of_cell` keep one row at the op API. Leave the `PlanOutcome::summary` test to `shed_dead_dup`, which deletes it with the function.
**Deliverables**: `__reports__/diet/leaves/cull_plan_store_v0.md`: a table with one row per test that goes (file, test, verdict, covering guard or "structure") and the before count.
**Consistency Checks**: every deleted test in the table names a kept guard or says "structure" (expected: PASS)
**Commit**: `reports(api): record the plan, resolve and run store cull verdicts`

## Step 2: Delete, merge and rewrite
**Goal**: Apply the checked verdicts.
**Implementation Logic**:
Delete whole files and their `mod` lines in `tests/it/main.rs`. Move the surviving rows of a merged file into its target guard, keeping their names. For each merged or rewritten guard, make one bite in the product code that the guard must catch, see it red, and revert it. Delete a `support/` helper only when no remaining test uses it. Do not touch files outside the owned set; if a covering guard lives elsewhere, cite it and leave it.
**Deliverables**: the owned test files after the cull; the leaf report's Bites section (bite, guard seen red, reverted).
**Consistency Checks**: `cargo nextest run -p candler-core` (expected: PASS)
**Commit**: `test(runner): cull the plan, resolve and run store tests to guards at the consumer interface`

## Step 3: Verify
**Goal**: An agent that did not write the cull attacks it.
**Implementation Logic**:
Hand a fresh Sonnet verifier this leaf, the leaf report and `git diff feat/diet...HEAD`. Its brief: name any behaviour of the README purpose paragraph that no remaining test holds, any deleted test whose cited guard does not observe the same behaviour, and any gate unmet. Fix what it finds with a fresh agent, once; list the rest as follow-ups.
**Deliverables**: the leaf report's Verifier and Follow-ups sections.
**Consistency Checks**: the Success Gates above (expected: PASS)
**Commit**: `reports(api): close the plan, resolve and run store cull`
