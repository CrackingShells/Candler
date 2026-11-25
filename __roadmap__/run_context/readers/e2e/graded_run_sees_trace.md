# Graded run sees trace

**Goal**: A real agent run on either harness, sealed by `candler run` and graded by `candler evaluate` with a kind that declares `needs.trace` and an artifact, shows its plug-in each shell call's command and exit status and each MCP call's `{server, tool}`; without the trace, or with a reader that loses `exit`, the kind's own verdict changes (critical path)
**Estimate**: Sonnet, $10, cap $20. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use the target dir the campaign README names; the leaf removes it at its merge, and removes every image and container it built, by id.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/run-context HEAD` succeeds
- [ ] `[nl]` Every `../` leaf is `done` and merged into `feat/run-context`
- [ ] `[cmd]` `docker info` answers (never stop Docker, never prune)
**Success Gates**:
- ⬜ A new guard in `crates/candler/tests/it/guard_cli_e2e/`, on the `parity_e2e.rs` rig (both real harnesses, the production gateway, the scripted fake model), runs one cell per harness whose script writes `answer.txt` with a command that exits 0, runs `exit 3`, and calls the `parity` server's `stamp` tool. It then grades each run with a plug-in kind that declares `needs.trace = true` and `needs.artifacts = ["answer.txt"]`. The kind passes only if the trace holds the pairs `(argv, exit)` for both commands and the origin `{kind: mcp, server: parity, tool: stamp}`, and `answer.txt` is served. The oracle is the fake-model script's own commands, not Candler's output
- ⬜ The guard is `#[ignore]` like the parity guard, joins `.github/workflows/container-e2e.yml`'s expected-guard list, and is listed as PASS in a `container-e2e` run on this leaf's pull request; the report quotes the PASS line
- ⬜ Each of two bites turns the guard red, and the report records the failing message: (a) `assemble` serves no trace (the outcome is a fail or a refusal, not a pass); (b) the OpenCode reader writes no `exit` (the OpenCode row goes red while Claude Code's stays green)
- ⬜ The guard asks the question htn-creator-eval's probe P0 asks; the report says which parts of P0 it does not cover (P0's own kind and suite)
- The leaf touches `crates/**` and `.github/workflows/container-e2e.yml`; `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass, and the guard passes locally with `--run-ignored only` before the pull request
**References**: [plan §Test plan (B7)](../../../../__reports__/run-context/00-plan_run_context_v0.md). Code:
- `crates/candler/tests/it/guard_cli_e2e/parity_e2e.rs`: `parity_one_installed_server_one_traced_call`, the rig;
- `crates/candler/tests/it/guard_run_evaluate.rs`: `plugin`, a run graded by a plug-in set up beside it;
- `crates/candler/tests/it/guard_cli_e2e.rs`: the module list;
- `.github/workflows/container-e2e.yml`.

## Step 1: The guard
**Goal**: The guard runs both cells and grades them.
**Implementation Logic**:
Reuse the parity rig and its fake model; add the script turns named in the gate. Set up the plug-in beside the world as `guard_run_evaluate.rs` does, with the Python helper's typed trace reader, and a kind whose verdict is computed from the trace. Run `candler run` then `candler evaluate` through the CLI, and read the outcome record. Add the guard's name to the workflow's expected list.
**Deliverables**: a new module under `crates/candler/tests/it/guard_cli_e2e/` and its line in `guard_cli_e2e.rs`; `.github/workflows/container-e2e.yml`
**Consistency Checks**: `cargo nextest run -p candler --test it -E 'test(/^guard_cli_e2e::/)' --run-ignored only` locally (expected: PASS)
**Commit**: `test(library): grade a real run by its trace on both harnesses`

## Step 2: Bite and verify
**Goal**: Each bite turns the guard red, and CI ran it.
**Implementation Logic**:
On a scratch branch apply bites (a) and (b) one at a time, run the guard locally, and record each failing message. Remove the images and containers built, by id. Open the leaf's pull request into `feat/run-context` and wait for `container-e2e`. A fresh Sonnet verifier gets the leaf, the report, the diff and the CI log, and is asked to find a way the guard could pass while a plug-in on either harness would not see a shell call's exit.
**Deliverables**: `__reports__/run-context/leaves/graded_run_sees_trace_v0.md`
**Consistency Checks**: `git grep -n confirm_ -- crates python` prints nothing (expected: PASS)
**Commit**: `reports(library): close the graded run's trace`
