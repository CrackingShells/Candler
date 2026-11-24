# Parity run

**Goal**: One cell with a real MCP server, run on Claude Code and on OpenCode with code mode off, shows two things on each harness: (a) the server is installed, and (b) the stored trace records the agent's call to the server's tool in the same normalised form. A container-e2e guard fails when either harness falls out of parity (P13).
**Estimate**: Sonnet, $6, cap $12. The two live runs go to RiVault. At the cap, stop, commit what is green, and list what is left. Builds use the target dir the campaign README names, which is removed when the leaf merges.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor origin/feat/integrity HEAD` succeeds
- [ ] `[nl]` `../mcp_render.md` and `../trace_parity.md` are `done`
**Success Gates**:
- ⬜ The parity guard runs in the `container-e2e` group with the real harness images, the production gateway and the scripted fake model. It passes only when, for each harness:
  - the fixture server is in the run's resolved `mcp.*` values and in the MCP file its driver rendered;
  - the stored trace holds the scripted call as `ToolOrigin::Mcp` with the same server and tool on both harnesses.

  On the base it fails on OpenCode (`origin: other`)
- ⬜ The guard asserts no census of servers and reads no server status
- ⬜ The guard runs the same experiment file on both harnesses, and the harness is the only thing that differs between the runs. Code mode stays at its default `false`, and the guard has no code-mode case
- ⬜ The fixture server is a stdio MCP server that needs nothing outside the agent base images (the OpenCode image has no python3)
- ⬜ One live run per harness through RiVault, of the same experiment shape, shows the agent calling the server's tool, and its stored trace attributes the call to the server. The runs, their store and their containers are removed by name after the record is taken
- ⬜ The container-e2e workflow's added time is measured and stated in the report
- ⬜ The report's "What it leaves" section judges the change at the three levels
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with the target dir the campaign README names, and `container-e2e` is green on the pull request
**References**: [Level-3 plan §Leaves](../../../../../../../../../../__reports__/diet/07-plan_integrity_level3_v1.md), the parity row. Code:
- `crates/candler-core/tests/it/guard_container_e2e/controls_e2e.rs`: real Claude Code behind the gateway, with `assets/docker/fakemodel`;
- `.github/workflows/container-e2e.yml`.

## Step 1: Guard on the base
**Goal**: The parity guard fails on the base, for OpenCode's unattributed call.
**Implementation Logic**:
Write the fixture server and the guard, following how `controls_e2e` builds its world and leaves no container behind. Script the fake model to call the fixture's tool once on each harness. Run it on the base and record the OpenCode failure.
**Deliverables**: `crates/candler-core/tests/it/guard_container_e2e/parity_e2e.rs`; the fixture server, under `assets/` or the test's fixtures; the fake model's script
**Consistency Checks**: the parity guard on the base (expected: FAIL)
**Commit**: `test(runner): hold both harnesses to one installed server and one traced call`

## Step 2: Pass on the tip and prove live
**Goal**: The guard passes on the tip, and a live run shows the same.
**Implementation Logic**:
Run the guard on the tip. Then run the same experiment shape once per harness against RiVault, and record each resolved `mcp.*` value and the call from each stored trace. These runs prove the design, so the report compares their shape with the guard's, not their numbers. Each live run must be one the work needs: no reruns for numbers.
**Deliverables**: the report's live-run record
**Consistency Checks**: the parity guard on the tip (expected: PASS)
**Commit**: `reports(api): record the live parity runs`

## Step 3: Bite and verify
**Goal**: The guard catches one harness falling out of parity.
**Implementation Logic**:
On a scratch branch, make two edits and see the guard fail on each:
- make OpenCode's reader drop the server from the origin;
- make Claude Code's renderer skip the server.

A fresh Sonnet verifier gets this leaf, the report and the diff. Its brief is to find a way for the two harnesses to install or trace one cell's server differently while the guard still passes.
**Deliverables**: `__reports__/diet/leaves/parity_run_v0.md`
**Consistency Checks**: each scratch edit makes the parity guard fail (expected: FAIL)
**Commit**: `reports(api): close the parity run`
