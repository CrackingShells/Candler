# E2E guard

**Goal**: The real `candler run` on real Claude Code and real OpenCode, against the scripted fake model, shows the README's promise for instructions and hooks on both harnesses. (P0)
**Estimate**: Sonnet, $8, cap $16. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use one slot of `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, and the leaf removes it when it merges. The leaf deletes the containers it starts.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds
**Success Gates**:
- ⬜ G4. On each harness, the fake model's recorded requests for arm A hold the global text and workspace text A, and never workspace text B, and the reverse holds for arm B. On the base, the experiment is `unknown_path`.
- ⬜ G5. The scripted tool call is answered with an error carrying the hook's marker: a Claude Code PreToolUse script exits 2, and an OpenCode plugin throws in `execute.before`. On the base, there is no hook and the call succeeds.
- ⬜ G6. On real images, an invalid payload (a Claude Code hooks entry on an unknown event; an OpenCode plugin with no default export) is refused at `resolve` with `experiment.harness_rejected_input`, quoting the harness, and the valid payload passes. On the base, it is `unknown_path`.
- ⬜ G7. Each run's census lists both installed instruction files with their scope: on Claude Code the global file where the probe settled and the root `CLAUDE.md` (Project); on OpenCode the global `AGENTS.md` and the root `AGENTS.md`.
- ⬜ G9. No installed file, the root instruction file included, is among any run's collected outputs, and no run is sealed `input_leak`.
- ⬜ The guards run in a pass that lists them as passed, not skipped (leaf template, convention 3). `docker ps -a` shows none of their containers afterwards.
**References**: [Architecture report, v1](../../../../__reports__/agent_config/01-architecture_agent_config_v1.md) — the shapes, the error model, the invariants and §Execution model; `crates/candler/tests/it/guard_cli_e2e/parity_e2e.rs` (`parity_one_installed_server_one_traced_call`, the model to follow); `assets/docker/fakemodel`

## Step 1: The guards
**Goal**: Two guards that read what a consumer reads.
**Implementation Logic**:
`guard_agent_config_e2e` runs one experiment with a global text per harness, two arms (workspace text A or B) and one hook per harness on both harnesses. It reads `inspect`'s members, the rendered files in the run, the fake model's records and the stored census. `guard_harness_check_refuses` runs `resolve` on real images with no model. Reuse the parity e2e's helpers; do not copy them. The check's output must list both guards as passed.
**Deliverables**: `crates/candler/tests/it/guard_cli_e2e/agent_config_e2e.rs`; the fake model's script for the blocked call
**Consistency Checks**: `cargo nextest run -p candler --run-ignored all -E 'test(agent_config) | test(harness_check)'` (expected: PASS)
**Commit**: `test(cli): prove instructions and hooks on both harnesses`
