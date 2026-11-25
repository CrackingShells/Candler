# OpenCode exit

**Goal**: A `shell` call in an OpenCode trace carries the exit status OpenCode reported: `exit: {kind: code, code: N}` from the tool part's `state.metadata.exit` (the rule for a nonzero exit is the one the probe recorded), and no `exit` when the part's state is an error (`aborted`, `permission.rejected`, …), when `metadata.exit` is missing, or for any tool that is not `shell` (critical path)
**Estimate**: Sonnet, $5, cap $10. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use the target dir the campaign README names; the leaf removes it at its merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/run-context HEAD` succeeds
- [ ] `[nl]` `../schema_exit_needs.md` and `../probe_shell_shapes.md` are `done`; the probe report's answers to questions (1) and (2) are read before Step 1. If the probe found that OpenCode writes a nonzero exit somewhere other than `state.metadata.exit`, the rule follows the probe and the report says so
**Success Gates**:
- ⬜ The export reader's table `opencode_export_reader_covers_trace_errors_cache_cost_and_child_sessions` gains an `exit` column, written from `jq` over the exports and never from the reader's output. Its rows cover: a completed `shell` call with exit 0 (`completed_o1`); a completed call with a nonzero exit, a stderr-only nonzero exit and a timed-out call (the probe's fixture); an aborted call (`sigint_mid_tool` or `interrupt_api`); a rejected permission (`oc_permission_ask`); a tool that is not `shell`. On the base every row with a code fails
- ⬜ Each of two bites turns the table red, and the report records the failing message: (a) `metadata.exit` read only on error states; (b) a missing `metadata` taken as exit 0
- ⬜ `export.rs` never infers a signal from a code and never reads an exit from the tool's text output
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass with the target dir the campaign README names; `container-e2e` is green on the pull request
**References**: [plan §The exit-status field, §Test plan](../../../__reports__/run-context/00-plan_run_context_v0.md); `__reports__/run-context/leaves/probe_shell_shapes_v0.md`. Code:
- `crates/candler-harness-opencode/src/export.rs`: `tool_drafts`;
- `crates/candler-harness-opencode/tests/it/guard_opencode_driver.rs`: `opencode_export_reader_covers_trace_errors_cache_cost_and_child_sessions`;
- `crates/candler-harness-opencode/tests/fixtures/streams/*/export.json`.

## Step 1: Guard on the base
**Goal**: The `exit` column exists and fails on the base.
**Implementation Logic**:
Add the `exit` column to the table and the probe fixture's rows. Write each expected value from `jq` over the fixture's `export.json` (the tool part's `state.status`, `state.metadata.exit`, `state.error`) and record the expression in the report. Run the table on the base.
**Deliverables**: `guard_opencode_driver.rs`
**Consistency Checks**: the table on the base (expected: FAIL)
**Commit**: `test(adapter-opencode): state each shell call's exit from the export`

## Step 2: Read the exit
**Goal**: The table passes.
**Implementation Logic**:
In `tool_drafts`, for a `shell` part, read the exit by the rule in the Goal and pass it to the result's constructor. Keep the rule in one place beside the existing `argv` reading.
**Deliverables**: `crates/candler-harness-opencode/src/export.rs`
**Consistency Checks**: the table on the tip (expected: PASS)
**Commit**: `feat(adapter-opencode): read a shell call's exit status`

## Step 3: Bite and verify
**Goal**: Each bite turns the table red.
**Implementation Logic**:
On a scratch branch apply bites (a) and (b) one at a time and record each failing message. Delete every `confirm_` test. A fresh Sonnet verifier gets the leaf, the report and the diff, and is asked to find a recorded `shell` part whose `exit` the reader gets wrong, or another tool that gets one.
**Deliverables**: `__reports__/run-context/leaves/oc_reader_exit_v0.md`
**Consistency Checks**: `git grep -n confirm_ -- crates python` prints nothing (expected: PASS)
**Commit**: `reports(adapter-opencode): close the shell exit status`
