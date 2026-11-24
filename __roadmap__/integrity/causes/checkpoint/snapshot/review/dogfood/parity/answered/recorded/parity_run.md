# Parity run

**Goal**: One cell, run on Claude Code and on OpenCode (code mode off and on) with a real MCP server, records the server and its tools on each harness, and a container-e2e guard fails when either record does not (P13).
**Estimate**: Sonnet, $6, cap $12; the two live runs go to RiVault. At the cap, stop, commit what is green, and list what is left. Builds use the target dir the campaign README names, removed at the leaf's merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor origin/feat/integrity HEAD` succeeds
- [ ] `[nl]` `../mcp_render.md` and `../census.md` are `done`
**Success Gates**:
- ⬜ The parity guard runs in the `container-e2e` group with the real harness images, the production gateway and the scripted fake model, and passes only when each run's census names the fixture server and its tools; on the base it fails on OpenCode
- ⬜ The guard runs the same experiment file on both harnesses; the only thing that differs between the runs is the harness, and OpenCode's code-mode run differs by `codemode = true` alone
- ⬜ The fixture server is a stdio MCP server that needs nothing outside the agent base images (no python3 in the OpenCode image)
- ⬜ One live run per harness through RiVault, of the same experiment shape, shows the agent calling the server's tool and the census naming it; the runs, their store and their containers are removed by name after the record is taken
- ⬜ The container-e2e workflow's added time is measured and stated in the report
- ⬜ The report's "What it leaves" section judges the change at the three levels
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with the target dir the campaign README names, and `container-e2e` is green on the pull request
**References**: [Level-3 plan §Leaves](../../../../../../../../../../__reports__/diet/07-plan_integrity_level3_v0.md) — the parity row; `crates/candler-core/tests/it/guard_container_e2e/controls_e2e.rs` — real Claude Code behind the gateway with `assets/docker/fakemodel`; `.github/workflows/container-e2e.yml`

## Step 1: Guard on the base
**Goal**: The parity guard fails on the base for OpenCode's missing servers.
**Implementation Logic**:
Write the fixture server and the guard, following how `controls_e2e` builds its world and leaves no container behind. Script the fake model to call the fixture's tool once on each harness. Run it on the base and record the OpenCode failure.
**Deliverables**: `crates/candler-core/tests/it/guard_container_e2e/parity_e2e.rs`; the fixture server under `assets/` or the test's fixtures; the fake model's script
**Consistency Checks**: the parity guard on the base (expected: FAIL)
**Commit**: `test(runner): hold both harnesses to one census of a real MCP server`

## Step 2: Pass on the tip and prove live
**Goal**: The guard passes on the tip, and a live run shows the same.
**Implementation Logic**:
Run the guard on the tip. Then run the same experiment shape once per harness against RiVault, and record each census and the tool call from its trace; these runs prove the design, so the report compares their shape with the guard's, not their numbers. Each live run is one the work needs: no reruns for numbers.
**Deliverables**: the report's live-run record
**Consistency Checks**: the parity guard on the tip (expected: PASS)
**Commit**: `reports(api): record the live parity runs`

## Step 3: Bite and verify
**Goal**: The guard catches one harness falling out of parity.
**Implementation Logic**:
On a scratch branch, drop OpenCode's collected server status and see the guard fail. A fresh Sonnet verifier gets this leaf, the report and the diff, with the brief: find a way for the two harnesses to see different tools from one cell while the guard passes.
**Deliverables**: `__reports__/diet/leaves/parity_run_v0.md`
**Consistency Checks**: the scratch edit makes the parity guard fail (expected: FAIL)
**Commit**: `reports(api): close the parity run`
