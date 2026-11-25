# Install on OpenCode

**Goal**: On OpenCode, the global text loads as the global `AGENTS.md`, the workspace text as the root `AGENTS.md`, and the plugins load locally, and every run's census names the instruction files that loaded. (P0)
**Estimate**: Sonnet, $6, cap $12. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use one slot of `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, and the leaf removes it when it merges. The leaf deletes the containers it starts.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds
- [ ] `[nl]` The probe report settles (c), (d) and (f)
**Success Gates**:
- ⬜ G3 (OpenCode). The *instructions*, *hooks* and *config check* rows show the OpenCode mapping: `$XDG_CONFIG_HOME/opencode/AGENTS.md`, and the workspace's root `AGENTS.md`, read-only; local plugins in `$XDG_CONFIG_HOME/opencode/plugins/`; and the probe's check or `no (…)`. On the base, no such rows exist.
- ⬜ The plan of a cell with `instructions.global.opencode`, `instructions.workspace` and `hooks.opencode` places the global `AGENTS.md` and each plugin in the home, byte for byte, lists `AGENTS.md` with the workspace text among its read-only workspace files, and writes nothing in the workspace itself. On the base, the plan has none of these.
- ⬜ The census lists each `Instructions from: <path>` block of the first request the gateway saw, read from a recorded request fixture.
**References**: [Architecture report, v1](../../../__reports__/agent_config/01-architecture_agent_config_v1.md) — the shapes, the error model, the invariants and §Execution model; `crates/candler-harness-opencode/src/prepare.rs` (`render_config`, `PreparedFile::in_home`); the OpenCode V2 evidence in the report

## Step 1: Install, census and check
**Goal**: The driver writes the values where OpenCode reads them, and a run says what loaded.
**Implementation Logic**:
Add the global file and the plugin files as home `PreparedFile`s, and the workspace text as the plan's read-only workspace file, next to `render_config`, in the same config directory. Extend the census source (the first request) with the instruction blocks. Implement `config_check` and `read_config_check` from the probe. Flip the answers to Mapped.
**Deliverables**: `crates/candler-harness-opencode/src/prepare.rs`, `driver.rs`, the census reader; plan and census guards
**Consistency Checks**: `cargo nextest run -p candler-harness-opencode` and `cargo nextest run -p candler contract_pages` (expected: PASS)
**Commit**: `feat(harness): install instructions and plugins on OpenCode`
