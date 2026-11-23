# cull_import_eval

**Goal**: Cut the import and evaluation tests to one guard per behaviour per consumer interface, rewrite the domain-agnostic guard at the consumer interface, and remove the `late.sh` flake.
**Estimate**: about 40 Sonnet calls ($1.3 with the verifier), cap 80. At 80 calls, stop, commit what is green, and list what is left in the leaf report.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/diet HEAD` succeeds
- [ ] `[nl]` OWNED = `^crates/candler-core/tests/it/(guard_import_mini|guard_legacy_mapping|guard_package_names|guard_layering_i1|guard_oracle_[a-z_]+|guard_golden|guard_engine|guard_evaluate_job|guard_protocol_client|guard_registry|main)\.rs$|^crates/candler/tests/it/(guard_ingest|guard_oracle_ac|guard_golden_parity|guard_eval_infra|main)\.rs$|^crates/candler-core/tests/it/support/|^__reports__/diet/leaves/cull_import_eval`. No other L0 piece edits these paths; `tests/it/main.rs` mod lines are shared and the second piece to merge rebases
**Success Gates**:
- ⬜ Every deleted test is listed with its covering guard or "structure"
- ⬜ Every merged or rewritten guard was seen red once by a bite, then the bite reverted
- ⬜ `cargo nextest run -p candler-core -p candler` passes
- ⬜ `guard_protocol_client` passes 20 times in a row: `cargo nextest run -p candler-core -E 'test(/every_failure_path/)' --retries 0` in a loop
- ⬜ `cargo fmt --check` passes
- ⬜ `cargo clippy --workspace --all-targets -- -D warnings` passes
- ⬜ `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes
- ⬜ DIGEST unchanged: `cargo nextest run --workspace -E 'test(/cell_digest|context_digest|build_context/)'` passes with no vector edited
- ⬜ GOLDEN unchanged: `cargo nextest run --workspace -E 'test(/golden/)'` passes with no vector edited
- ⬜ Ownership: `git diff --name-only feat/diet...HEAD | grep -vE '^crates/candler-core/tests/it/(guard_import_mini|guard_legacy_mapping|guard_package_names|guard_layering_i1|guard_oracle_[a-z_]+|guard_golden|guard_engine|guard_evaluate_job|guard_protocol_client|guard_registry|main)\.rs$|^crates/candler/tests/it/(guard_ingest|guard_oracle_ac|guard_golden_parity|guard_eval_infra|main)\.rs$|^crates/candler-core/tests/it/support/|^__reports__/diet/leaves/cull_import_eval'` prints nothing
- ⬜ A fresh Sonnet verifier, given only this leaf and the diff, reports no promise left without a test and no gate unmet
**References**: [Diet plan §How the suite becomes the critical set](../../__reports__/diet/00-architecture_diet_plan_v0.md) — the six interfaces, the family verdicts and the critical set; [AGENTS.md](../../AGENTS.md) — checks and conventions

## Step 1: Confirm each verdict before deleting
**Goal**: Turn the sweep's verdicts for this family into checked verdicts.
**Implementation Logic**:
Work in a worktree made by `scripts/worktree.py add` from the `feat/diet` tip. Count the family's tests (`cargo nextest list` on the owned crates, filtered to the owned files). For each owned file, read its header and test docs through colgrep `search`/`expand` and confirm the verdict below. A test is kept when it observes one of the six consumer interfaces (CLI records and exit codes; the core operation API; wire formats and stored artifacts; digests and golden vectors; the plug-in contract; isolation seen from outside) and no other kept test holds the same behaviour at the same interface. A test is deleted when it pins structure, tests a test helper, or repeats a kept guard. A cull with no covering guard is kept and listed.
Verdicts from the sweep: 100 tests now, about 60 kept. Delete `guard_oracle_mini`, `guard_oracle_aggregates`, `guard_oracle_ac` (they test the oracle helper). Merge `guard_legacy_mapping` (4 survive) and `guard_package_names` (1 survives) into `guard_import_mini`. Rewrite `guard_layering_i1` (a source scan) as two rows in `guard_import_mini`: a corpus whose domain is renamed imports and grades to the same rows. `guard_ingest` keeps about 11 of 17, with i1 and i4. Fix the `late.sh` timing flake in `guard_protocol_client::every_failure_path_of_an_evaluator_is_accounted_for` by giving the test control of the clock or a window that cannot race.
**Deliverables**: `__reports__/diet/leaves/cull_import_eval_v0.md`: a table with one row per test that goes (file, test, verdict, covering guard or "structure") and the before count.
**Consistency Checks**: every deleted test in the table names a kept guard or says "structure" (expected: PASS)
**Commit**: `reports(api): record the import and evaluation cull verdicts`

## Step 2: Delete, merge and rewrite
**Goal**: Apply the checked verdicts.
**Implementation Logic**:
Delete whole files and their `mod` lines in `tests/it/main.rs`. Move the surviving rows of a merged file into its target guard, keeping their names. For each merged or rewritten guard, make one bite in the product code that the guard must catch, see it red, and revert it. Delete a `support/` helper only when no remaining test uses it. Do not touch files outside the owned set; if a covering guard lives elsewhere, cite it and leave it.
**Deliverables**: the owned test files after the cull; the leaf report's Bites section (bite, guard seen red, reverted).
**Consistency Checks**: `cargo nextest run -p candler-core -p candler` (expected: PASS)
**Commit**: `test(evaluation): cull the import and evaluation tests to guards at the consumer interface`

## Step 3: Verify
**Goal**: An agent that did not write the cull attacks it.
**Implementation Logic**:
Hand a fresh Sonnet verifier this leaf, the leaf report and `git diff feat/diet...HEAD`. Its brief: name any behaviour of the README purpose paragraph that no remaining test holds, any deleted test whose cited guard does not observe the same behaviour, and any gate unmet. Fix what it finds with a fresh agent, once; list the rest as follow-ups.
**Deliverables**: the leaf report's Verifier and Follow-ups sections.
**Consistency Checks**: the Success Gates above (expected: PASS)
**Commit**: `reports(api): close the import and evaluation cull`
