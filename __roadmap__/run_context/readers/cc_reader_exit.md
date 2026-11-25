# Claude Code exit

**Goal**: A `Bash` call in a Claude Code trace carries the exit status Claude Code reported: `exit: {kind: code, code: N}` when the result's content starts with `Exit code N`, `{kind: code, code: 0}` when the result is not an error and Claude Code did not move the command to the background, and no `exit` for a call its timeout moved to the background (the result's `tool_use_result` carries `backgroundTaskId` or `timedOutAfterMs`, with `is_error: false`, as the probe recorded), an interrupted call, a denial, any other error text, or any tool that is not `Bash`. The call's name comes from the call joined on `(session, span)` (critical path)
**Estimate**: Sonnet, $6, cap $12. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use the target dir the campaign README names; the leaf removes it at its merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/run-context HEAD` succeeds
- [ ] `[nl]` `../schema_exit_needs.md` and `../probe_shell_shapes.md` are `done`; the probe report's answer to question (3) (the prefix with stderr only) and to question (2) (a code on timeout) are read before Step 1
**Success Gates**:
- ⬜ `guard_reader.rs::expected_facts` reads new `exits` rows from each fixture's `expected.toml`, written from `jq` over the raw lines and never from the reader's output. The rows cover: a `Bash` call that exited 0; nonzero exits (1 and 127 in the real fixtures); a denial; a call to a tool that is not `Bash` (for example `Read`); a call in a child session; and the probe's stderr-only call (`Exit code 2` followed by the stderr text) and timed-out call (no `exit`, although `is_error` is false). On the base every `exits` row with a code fails
- ⬜ Each of four bites turns the guard red, and the report records the failing message: (a) the prefix parse removed; (b) code 0 given to every non-error result, `Read` included; (c) the call joined on span without the session; (d) the background check removed, so the probe's timed-out call reads code 0
- ⬜ Rows with a code are added only to fixtures that a real Claude Code wrote, as their provenance records (the probe's among them). A synthetic fixture made by `make_synthetic.py` gets an `exits` row only where it states an absence, since a hand-built line cannot say what Claude Code writes
- ⬜ `parse.rs` reads only the result's `is_error`, the first line of its first text content, and the presence of `backgroundTaskId` or `timedOutAfterMs` in `tool_use_result` for this; it never reads the background task's later summary text and never infers a signal from a code
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass with the target dir the campaign README names; `container-e2e` is green on the pull request
**References**: [plan §The exit-status field, §Test plan](../../../__reports__/run-context/00-plan_run_context_v0.md); `__reports__/run-context/leaves/probe_shell_shapes_v0.md`. Code:
- `crates/candler-harness-claude-code/src/parse.rs`: `block_draft` and the `tool_result` handling, `tool_call`;
- `crates/candler-harness-claude-code/tests/it/guard_reader.rs`: `expected_facts`, `load_streams`;
- `crates/candler-harness-claude-code/tests/fixtures/{streams,sessions}/*/expected.toml`.

## Step 1: Guard on the base
**Goal**: The `exits` rows exist and fail on the base.
**Implementation Logic**:
Extend `expected_facts` to compare a list of `(span, exit)` from the reader with an `exits` table in each `expected.toml`. Write each row from `jq` over the fixture's raw lines: find the `tool_use` of `Bash` and its `tool_result`, and read `is_error` and the content's first line. Record the `jq` expression in the report so the oracle can be rerun. Run the guard on the base.
**Deliverables**: `guard_reader.rs`; the `exits` rows
**Consistency Checks**: `expected_facts` on the base (expected: FAIL)
**Commit**: `test(adapter-claude-code): state each Bash call's exit from the raw lines`

## Step 2: Read the exit
**Goal**: The guard passes.
**Implementation Logic**:
When a `tool_result` block pairs with a `Bash` call of the same session, set `exit` by the rule in the Goal. The prefix is `Exit code ` followed by a decimal `i32` at the start of the first text content; anything else in an error result gives no `exit`. A denial (`tool_result_meta` non-execution), `interrupted: true`, and a result moved to the background (`backgroundTaskId` or `timedOutAfterMs` in `tool_use_result`) give no `exit` whatever the content.
**Deliverables**: `crates/candler-harness-claude-code/src/parse.rs`
**Consistency Checks**: `expected_facts` on the tip (expected: PASS)
**Commit**: `feat(adapter-claude-code): read a Bash call's exit status`

## Step 3: Bite and verify
**Goal**: Each bite turns the guard red.
**Implementation Logic**:
On a scratch branch apply bites (a), (b) and (c) one at a time and record each failing message. Delete every `confirm_` test. A fresh Sonnet verifier gets the leaf, the report and the diff, and is asked to find a recorded `Bash` result whose `exit` the reader gets wrong, or a non-`Bash` result that gets one.
**Deliverables**: `__reports__/run-context/leaves/cc_reader_exit_v0.md`
**Consistency Checks**: `git grep -n confirm_ -- crates python` prints nothing (expected: PASS)
**Commit**: `reports(adapter-claude-code): close the Bash exit status`
