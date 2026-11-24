# Collect oracle

**Goal**: Conformance checks every driver's `collect` against what the agent wrote, not against the driver's own fixture (P2, cause D1, defect B1).
**Estimate**: RiVault first, $4 with the Sonnet fallback, cap $8. If the RiVault attempt fails its gates, one Sonnet attempt follows. At the cap, stop, commit what is green, and list what is left.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/integrity HEAD` succeeds
**Success Gates**:
- ⬜ With B1's fix reverted on a scratch branch, the shared conformance case fails for OpenCode only, naming the files the agent wrote; on the base, the same revert passes conformance
- ⬜ The oracle is the set of files the scripted agent wrote; `Case::with_collect` stays only where it checks something the shared case does not
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with `CARGO_TARGET_DIR=/Users/hacker/Documents/tmp/claude-worktrees/Candler/candler-diet/target`, and `container-e2e` is green on the pull request
**References**: [Cycle plan v2 §D1, §Proof](../../../__reports__/diet/04-architecture_cycle_plan_v2.md) — B1's mechanism and the observation; `crates/candler-harness/src/conformance.rs` — `Case::with_collect`, `check_collect`

## Step 1: Add the shared case
**Goal**: One conformance case every driver runs, whose oracle comes from outside the driver.
**Implementation Logic**:
In `candler-harness`'s conformance suite, add a case where the scripted agent writes a known set of files and the check asserts that the driver's `collect` stores exactly those files. Every driver runs it. Where a driver's `Case::with_collect` fixture now checks nothing the shared case does not, remove it.
**Deliverables**: `crates/candler-harness/src/conformance.rs` (the shared case and its check); driver fixtures where `with_collect` goes
**Consistency Checks**: `cargo nextest run --workspace -E 'test(/conformance/)'` (expected: PASS)
**Commit**: `test(harness): check collect against the files the agent wrote`

## Step 2: Bite and verify
**Goal**: The case catches B1.
**Implementation Logic**:
Revert B1's fix (OpenCode's `collect` returning no file spec) on a scratch branch from the tip: the shared case fails for OpenCode only and names the files. Do the same from the base: conformance passes. Record both. A fresh Sonnet verifier gets this leaf, the report and the diff.
**Deliverables**: `__reports__/diet/leaves/collect_oracle_v0.md` (§Bites, §Verifier)
**Consistency Checks**: the B1 revert on the tip fails the shared case (expected: FAIL)
**Commit**: `reports(api): close the collect oracle`
