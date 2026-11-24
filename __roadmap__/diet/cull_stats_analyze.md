# cull_stats_analyze

**Goal**: Cut the statistics and analysis tests to one guard per behaviour per consumer interface.
**Estimate**: about 36 Sonnet calls ($1.2 with the verifier), cap 72. At 72 calls, stop, commit what is green, and list what is left in the leaf report.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/diet HEAD` succeeds
- [ ] `[nl]` OWNED = `^crates/candler-core/tests/it/(guard_stats[a-z_]*|guard_known_truth|guard_selection|guard_analysis_[a-z_]+|guard_comparability|guard_diff|main)\.rs$|^crates/candler/tests/it/(guard_analyze[a-z_]*|guard_golden_design|main)\.rs$|^__reports__/diet/leaves/cull_stats_analyze`. No other L0 piece edits these paths; `tests/it/main.rs` mod lines are shared and the second piece to merge rebases
**Success Gates**:
- ⬜ Every deleted test is listed with its covering guard or "structure"
- ⬜ Every merged or rewritten guard was seen red once by a bite, then the bite reverted
- ⬜ `cargo nextest run -p candler-core -p candler` passes
- ⬜ `cargo fmt --check` passes
- ⬜ `cargo clippy --workspace --all-targets -- -D warnings` passes
- ⬜ `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes
- ⬜ DIGEST unchanged: `cargo nextest run --workspace -E 'test(/cell_digest|context_digest|build_context/)'` passes with no vector edited
- ⬜ GOLDEN unchanged: `cargo nextest run --workspace -E 'test(/golden/)'` passes with no vector edited
- ⬜ Ownership: `git diff --name-only feat/diet...HEAD | grep -vE '^crates/candler-core/tests/it/(guard_stats[a-z_]*|guard_known_truth|guard_selection|guard_analysis_[a-z_]+|guard_comparability|guard_diff|main)\.rs$|^crates/candler/tests/it/(guard_analyze[a-z_]*|guard_golden_design|main)\.rs$|^__reports__/diet/leaves/cull_stats_analyze'` prints nothing
- ⬜ A fresh Sonnet verifier, given only this leaf and the diff, reports no promise left without a test and no gate unmet
**References**: [Diet plan §How the suite becomes the critical set](../../__reports__/diet/00-architecture_diet_plan_v0.md) — the six interfaces, the family verdicts and the critical set; [AGENTS.md](../../AGENTS.md) — checks and conventions

## Step 1: Confirm each verdict before deleting
**Goal**: Turn the sweep's verdicts for this family into checked verdicts.
**Implementation Logic**:
Work in a worktree made by `scripts/worktree.py add` from the `feat/diet` tip. Count the family's tests (`cargo nextest list` on the owned crates, filtered to the owned files). For each owned file, read its header and test docs through colgrep `search`/`expand` and confirm the verdict below. A test is kept when it observes one of the six consumer interfaces (CLI records and exit codes; the core operation API; wire formats and stored artifacts; digests and golden vectors; the plug-in contract; isolation seen from outside) and no other kept test holds the same behaviour at the same interface. A test is deleted when it pins structure, tests a test helper, or repeats a kept guard. A cull with no covering guard is kept and listed.
Verdicts from the sweep: 106 tests now, about 67 kept. Merge `guard_selection` into `guard_analysis_report` (1 survives); `guard_analysis_report` keeps about 16 of 24 and `guard_stats` about 10 of 20. Keep the scipy and statsmodels vectors, `guard_known_truth` H0 and power, and t1 (Wilson, Fisher). The analyze hand counts repeated at the op API and the CLI keep one row at each.
**Deliverables**: `__reports__/diet/leaves/cull_stats_analyze_v0.md`: a table with one row per test that goes (file, test, verdict, covering guard or "structure") and the before count.
**Consistency Checks**: every deleted test in the table names a kept guard or says "structure" (expected: PASS)
**Commit**: `reports(api): record the statistics and analysis cull verdicts`

## Step 2: Delete, merge and rewrite
**Goal**: Apply the checked verdicts.
**Implementation Logic**:
Delete whole files and their `mod` lines in `tests/it/main.rs`. Move the surviving rows of a merged file into its target guard, keeping their names. For each merged or rewritten guard, make one bite in the product code that the guard must catch, see it red, and revert it. Delete a `support/` helper only when no remaining test uses it. Do not touch files outside the owned set; if a covering guard lives elsewhere, cite it and leave it.
**Deliverables**: the owned test files after the cull; the leaf report's Bites section (bite, guard seen red, reverted).
**Consistency Checks**: `cargo nextest run -p candler-core -p candler` (expected: PASS)
**Commit**: `test(stats): cull the statistics and analysis tests to guards at the consumer interface`

## Step 3: Verify
**Goal**: An agent that did not write the cull attacks it.
**Implementation Logic**:
Hand a fresh Sonnet verifier this leaf, the leaf report and `git diff feat/diet...HEAD`. Its brief: name any behaviour of the README purpose paragraph that no remaining test holds, any deleted test whose cited guard does not observe the same behaviour, and any gate unmet. Fix what it finds with a fresh agent, once; list the rest as follow-ups.
**Deliverables**: the leaf report's Verifier and Follow-ups sections.
**Consistency Checks**: the Success Gates above (expected: PASS)
**Commit**: `reports(api): close the statistics and analysis cull`
