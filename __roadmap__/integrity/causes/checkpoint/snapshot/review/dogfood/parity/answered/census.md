# Census

**Goal**: The census records, on both harnesses, each MCP server the agent had with its status and its tools, from what the harness itself reports (P10).
**Estimate**: Sonnet, $12, cap $24; the lead takes the probe fork of Step 1. At the cap, stop, commit what is green, and list what is left. Builds use the target dir the campaign README names, removed at the leaf's merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor origin/feat/integrity HEAD` succeeds
- [ ] `[nl]` `../declaration.md` is `done`
**Success Gates**:
- ⬜ The census of an OpenCode run whose cell declares one server records `mcp_servers: [{name, status, tools}]` naming that server and its tools; on the base the gateway census has no servers
- ⬜ The census of a Claude Code run records the same shape from `system/init`, so the two harnesses' records of one cell compare field by field
- ⬜ Under OpenCode code mode the record names the server and its tools from the source Step 1 chose, and says which source that was
- ⬜ Resolving refuses two servers whose names collide once normalised to the gateway's tool-name alphabet
- ⬜ The census evidence schema is regenerated, and the conformance suite checks the shape for every driver with a census
- ⬜ The report's "What it leaves" section judges the change at the three levels
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with the target dir the campaign README names, and `container-e2e` is green on the pull request
**References**: [Level-3 plan §Design](../../../../../../../../../__reports__/diet/07-plan_integrity_level3_v0.md) — one record; `crates/candler-core/src/runner/evidence/census.rs` — `census`; `crates/candler-core/src/runner/gateway.rs` — `census_init`; `crates/candler-harness/src/spec.rs` — `CensusSource`, `InitReport`; `crates/candler-harness-claude-code/src/parse.rs` — `init_report`

## Step 1: Guard and probe the code-mode source
**Goal**: The census guard fails on the base, and the source of tool names under code mode is chosen by observation.
**Implementation Logic**:
Write the guard on a run from the campaign's sealed-run producer with one declared server, and see it fail on the base. Then probe, in the OpenCode base image with a container named for this leaf and removed by name, what lists the tools of an MCP server during a session under code mode: first a session API of the run's own OpenCode child; then the description of the `execute` tool the gateway sees; then an MCP `tools/list` that Candler runs inside the isolation. The first that works is the source; record each one tried and what it returned. Names and status come from `mcp.list` against the run's OpenCode child (`CensusSource::Collected`); outside code mode, tools are the gateway's `<server>_<tool>` names matched against the configured names. If none of the three works under code mode, stop and send the lead the record: the gate's wording is then the lead's call with the coordinator, not the implementer's.
**Deliverables**: the census guard in `crates/candler-core/tests/it/`; the probe's record in the report
**Consistency Checks**: the census guard on the base (expected: FAIL)
**Commit**: `test(runner): require the census to name the agent's MCP servers`

## Step 2: Record servers on both harnesses
**Goal**: One census shape, filled by each driver's own source.
**Implementation Logic**:
Give `InitReport` the servers with status and tools; Claude Code fills them from `system/init` and its `mcp__<server>__<tool>` names, OpenCode from the sources Step 1 chose. Replace the `None` that `census_init` passes. Add the collision refusal at resolve, beside the server-name check that exists. The census stays one producer: no step copies it into a second record.
**Deliverables**: `crates/candler-harness/src/spec.rs`; `crates/candler-core/src/runner/{gateway.rs,evidence/census.rs}`; `crates/candler-harness-claude-code/src/parse.rs`; the census path of `crates/candler-harness-opencode/src/driver.rs`; the regenerated census schema
**Consistency Checks**: the census guard on the tip (expected: PASS)
**Commit**: `feat(runner): record the MCP servers and tools of every census`

## Step 3: Bite and verify
**Goal**: The guard catches a census without servers.
**Implementation Logic**:
On a scratch branch, put `None` back in `census_init` and see the guard fail. A fresh Sonnet verifier gets this leaf, the report and the diff, with the brief: find a run on either harness whose agent had a server that its census does not name.
**Deliverables**: `__reports__/diet/leaves/census_v0.md`
**Consistency Checks**: the scratch edit makes the census guard fail (expected: FAIL)
**Commit**: `reports(api): close the census`
