# Declaration

**Goal**: An experiment can set `instructions.global.<harness>`, `instructions.workspace`, `hooks.claude-code` and `hooks.opencode` as fixed values or factor levels. The cell digest and `inspect` cover them, every driver answers them on the generated pages, and the misuses are refused by name. (P0)
**Estimate**: Sonnet, $8, cap $16. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use one slot of `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, and the leaf removes it when it merges. The leaf deletes the containers it starts.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds
**Success Gates**:
- ⬜ G1. `candler resolve` of an experiment with `[fixed] instructions.workspace = "…"` succeeds, and the `inspect` of a run of it shows an `instructions` member. On `8d82f63b` the resolve exits 2 with `experiment.unknown_path` at `/fixed/instructions.workspace`.
- ⬜ G2. Two levels of `instructions.workspace` plan two cells with different digests, and two levels of `hooks.claude-code.settings` do the same. On the base, both are `unknown_path`.
- ⬜ G8. `hooks.claude-code` on an OpenCode cell that has no `hooks.opencode` is refused with `experiment.cell_value_missing` at `/fixed/hooks`, and `instructions.global` with no key for the cell's harness at `/fixed/instructions.global`. A key under `hooks` or `instructions.global` that names no registered harness is refused with `experiment.cell_value_invalid` at its path. A hook file path with `..` is refused with `format.rel_path_invalid`. On the base, all three are `unknown_path`.
- ⬜ G3 (this level's half). `docs/contract/harness-capabilities.md` has the rows *instructions: global*, *instructions: workspace*, *hooks* and *config check*. Both drivers answer each with `no (…)` saying it is installed from L2, and `cargo nextest run -p candler contract_pages` passes. On the base, no such rows exist.
- ⬜ `experiment.harness_rejected_input` and `experiment.instructions_collide` are declared in `docs/contract/codes.toml` with their side (experiment) and their path. No code path raises them yet.
- ⬜ `Plan` has a field for read-only workspace files, and the trait names each driver's workspace instruction file (`CLAUDE.md`, `AGENTS.md`). Both drivers leave the field empty, so no run changes.
**References**: [Architecture report, v1](../../__reports__/agent_config/01-architecture_agent_config_v1.md) — the shapes, the error model, the invariants and §Execution model; `crates/candler-schemas/src/servers.rs` (`McpServer`, the precedent); `crates/candler-harness/src/spec.rs:651` (`AgentSetting`) and `:729` (`DriverFact`); `crates/candler-core/src/resolve/paths.rs` (`CONFIGURATION_PATHS`, `settings_set_by`); `crates/candler/src/commands/inspect.rs` (`mcp_of`); `crates/candler/tests/it/guard_inspect_mcp_config.rs`

## Step 1: Surface, schema and one reader
**Goal**: The three values exist as typed cell values, read by one reader.
**Implementation Logic**:
Add envelope types beside `McpServer` in `candler-schemas`: `Instructions {global: map<harness id, text>, workspace: text}`, `ClaudeCodeHooks {settings: object, files: map<RelPath, text>}` and `OpenCodeHooks {plugins: map<file name, text>}`, each with `deny_unknown_fields`. Their contents stay free. Publish them as their own schemas, as `mcp-server.schema.json` is. Add the `instructions` and `hooks` rows to `CONFIGURATION_PATHS`; the names below `hooks` and `instructions.global` are harness ids. Add `candler_harness::agent_config::{instructions(cell), hooks(cell, harness)}`, the twin of `candler_harness::mcp::servers`, with the refusals of G8. No second reader may exist.
**Deliverables**: `crates/candler-schemas/src/agent_config.rs` (`Instructions`, `ClaudeCodeHooks`, `OpenCodeHooks`); the published schemas; `crates/candler-core/src/resolve/paths.rs` (rows); `crates/candler-harness/src/agent_config.rs` (`instructions`, `hooks`); unit guards for G2 and G8; `crates/candler-schemas/tests/vectors/manifest/hashed_paths.toml` (the new hashed paths)
**Consistency Checks**: `cargo nextest run -p candler-schemas -p candler-harness -p candler-core` (expected: PASS)
**Commit**: `feat(api): take instructions and hooks as cell values`

## Step 2: Declaration, answers, pages and inspect
**Goal**: Every driver answers the new settings, and a run shows its values.
**Implementation Logic**:
Add `AgentSetting::Instructions(scope)` with the scopes global and workspace, `AgentSetting::Hooks` and `DriverFact::ConfigCheck`. Add the trait methods `config_check(&Plan) -> Option<CommandSpec>` and `read_config_check(output) -> CheckVerdict`, modelled on `version_probe` and `parse_version`, and the method that names the workspace instruction file. Add the `Plan` field for read-only workspace files, which L2's core leaf mounts. Both drivers answer `Refused` for now and leave the field empty, so the merged level tells the truth. Declare `experiment.harness_rejected_input` and `experiment.instructions_collide`. Regenerate the contract pages (`CANDLER_BLESS_CONTRACT=1 cargo nextest run -p candler contract_pages`) and the schemas (`CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas drift`), and add a note on `experiment-file.md` that a consumer hook may approve tool calls, which changes what `permission_mode` means. Teach `inspect` to show `instructions` and `hooks` from `/comparable/values`, as `mcp_of` does, and extend `guard_inspect_mcp_config` or add a sibling guard for G1.
**Deliverables**: `crates/candler-harness/src/spec.rs`, `driver.rs`; both drivers' `answer`/`fact`; `docs/contract/codes.toml`; the regenerated contract pages; `crates/candler/src/commands/inspect.rs`; the G1 guard
**Consistency Checks**: `cargo nextest run --workspace` and `cargo nextest run -p candler contract_pages` (expected: PASS)
**Commit**: `feat(harness): declare instructions, hooks and the harness check`
