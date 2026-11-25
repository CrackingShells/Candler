# E2E guard

**Goal**: The real `candler run` on real Claude Code and real OpenCode, against the scripted fake model and then live on RiVault, shows from the stored trace the README's promise for instructions and hooks on both harnesses. (P0)
**Estimate**: Sonnet, $11, cap $22 (the live run included). At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use one slot of `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, and the leaf removes it when it merges. The leaf deletes the containers it starts.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds
**Success Gates**:
- ⬜ G4. From the trace: on each harness, the `context_start.system` of arm A's run holds the global text and workspace text A, and never workspace text B, and the reverse holds for arm B. On the base, the experiment is `unknown_path`.
- ⬜ G5. The scripted tool call is answered with an error carrying the hook's marker: a Claude Code PreToolUse script exits 2, and an OpenCode plugin throws in `execute.before`. On the base, there is no hook and the call succeeds.
- ⬜ G6. On real images, an invalid payload (a Claude Code hooks entry on an unknown event; an OpenCode plugin with no default export) is refused at `resolve` with `experiment.harness_rejected_input`, quoting the harness, and the valid payload passes. On the base, it is `unknown_path`.
- ⬜ G7. Each run's census lists both installed instruction files with their scope: on Claude Code the global file where the probe settled and the root `CLAUDE.md` (Project); on OpenCode the global `AGENTS.md` and the root `AGENTS.md`.
- ⬜ G9. No installed file, the root instruction file included, is among any run's collected outputs, and no run is sealed `input_leak`.
- ⬜ G11. From the trace: on each harness, the session-start hook's marker is in `context_start`, where the probe's question (g) says it lands. On the base, there is no such event.
- ⬜ G13. Live on RiVault (Reasoning-Small): one experiment on both harnesses, two arms, one replicate each, shows G4 and G11 in its stored traces, and `__reports__/agent_config/04-findings_live_confirmation_v0.md` quotes the trace excerpts.
- ⬜ The guards run in a pass that lists them as passed, not skipped (leaf template, convention 3). `docker ps -a` shows none of their containers afterwards.
**References**: [Architecture report, v2](../../../../__reports__/agent_config/02-architecture_agent_config_v2.md) — the shapes, the error model, the invariants and §Execution model; `crates/candler/tests/it/guard_cli_e2e/parity_e2e.rs` (`parity_one_installed_server_one_traced_call`, the model to follow); `assets/docker/fakemodel`

## Step 1: The guards
**Goal**: Two guards that read what a consumer reads.
**Implementation Logic**:
`guard_agent_config_e2e` runs one experiment with a global text per harness, two arms (workspace text A or B) and one blocking hook and one session-start hook per harness on both harnesses. It reads `inspect`'s members, the rendered files in the run, the stored trace's `context_start` and the stored census, as a consumer does, not the fake model's records. `guard_harness_check_refuses` runs `resolve` on real images with no model. Reuse the parity e2e's helpers; do not copy them. The check's output must list both guards as passed.
**Deliverables**: `crates/candler/tests/it/guard_cli_e2e/agent_config_e2e.rs`; the fake model's script for the blocked call
**Consistency Checks**: `cargo nextest run -p candler --run-ignored all -E 'test(agent_config) | test(harness_check)'` (expected: PASS)
**Commit**: `test(cli): prove instructions and hooks on both harnesses`

## Step 2: The live confirmation
**Goal**: The same proof on a real model.
**Implementation Logic**:
Run the guard's experiment once with `candler run` against RiVault's Reasoning-Small, which tests reachability, not scores, on both harnesses with two arms and one replicate each. Read G4 and G11 from the stored traces with the same reader the guard uses. Quote the `context_start` excerpts that show each text and the marker. No product code changes in this step.
**Deliverables**: `__reports__/agent_config/04-findings_live_confirmation_v0.md`, written with `writing-reports`, type findings
**Consistency Checks**: `test -s __reports__/agent_config/04-findings_live_confirmation_v0.md` (expected: PASS)
**Commit**: `reports(api): confirm instructions and hooks live from the trace`
