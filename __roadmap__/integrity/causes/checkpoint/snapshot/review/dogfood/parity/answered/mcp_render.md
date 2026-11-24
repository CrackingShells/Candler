# MCP render

**Goal**: Each driver writes its harness's own MCP configuration from the typed `McpServer`: on Claude Code `local` becomes `stdio` and `remote` becomes `http`, and nothing passes through serde. `codemode` defaults to `false` and is written into the resolved cell. Claude Code refuses `codemode = true`, giving its reason (P9).
**Estimate**: Sonnet, $6, cap $12. At the cap, stop, commit what is green, and list what is left. Builds use the target dir the campaign README names, removed at the leaf's merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor origin/feat/integrity HEAD` succeeds
- [ ] `[nl]` `../declaration.md` is `done`
- [x] `[nl]` The coordinator ruled on the digest change: filling the `codemode` default at resolve changes the cell digest of every cell that declares a server without it. Accepted: the resolved cell records what Candler installs
**Success Gates**:
- ⬜ The `.mcp.json` Claude Code gets for a server with `type = "local"`, `codemode` and `name` carries `type: "stdio"`, the name as its key, and no `codemode` or `name` key; on the base it carries all three as given
- ⬜ A Claude Code cell that sets `codemode = true` is refused through its `Codemode(true)` arm, with the reason "Claude Code offers MCP tools directly; it has no code mode". A cell that leaves it unset or sets `false` runs
- ⬜ A `remote` server reaches Claude Code as `type: "http"` with `url` and `headers`, as the Claude Code MCP documentation says (cited in the report). A field Claude Code has no place for is refused at check time, not dropped
- ⬜ OpenCode's `opencode.json` for a server with no `codemode` carries `codemode: false`; on the base it carries `true`
- ⬜ The resolved cell holds the filled default, so two cells that differ only by an explicit `codemode = false` have the same cell digest
- ⬜ A conformance check holds each driver's rendered MCP file to the keys its harness documents (cited from the harness's documentation), and fails on a passthrough key
- ⬜ The report's "What it leaves" section judges the change at the three levels
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with the target dir the campaign README names, and `container-e2e` is green on the pull request
**References**: [Level-3 plan §Design](../../../../../../../../../__reports__/diet/07-plan_integrity_level3_v1.md) — rendering from types, Eliott's `codemode` ruling; `crates/candler-harness-claude-code/src/prepare.rs` — `render_mcp`; `crates/candler-harness-opencode/src/prepare.rs` — `server`; `crates/candler-schemas/src/servers.rs` — `McpServer`, `McpServerKind`

## Step 1: Guard and probe
**Goal**: The rendering guards fail on the base.
**Implementation Logic**:
Write the guards on the rendered files, going through the drivers' `prepare` path from a cell that Candler resolved, and see them fail on the base. Read each harness's MCP configuration documentation through Context7 and cite it; do not rely on memory. Instruction files and hooks are out of scope for level 3. Probe neither, and run no `opencode debug config`: the configured side is already single-sourced by the resolved cell.
**Deliverables**: the guards in `crates/candler-harness-claude-code/tests/` and `crates/candler-harness-opencode/tests/`; the cited documentation in the report
**Consistency Checks**: the rendering guards on the base (expected: FAIL)
**Commit**: `test(library): hold each driver's MCP file to its harness's keys`

## Step 2: Render from types
**Goal**: No field of `McpServer` reaches a harness without its driver writing it.
**Implementation Logic**:
Replace `serde_json::to_value(servers)` in Claude Code's `render_mcp` with a writer that builds Claude Code's own shape: `local` written as `stdio` with `command`/`args`/`env`, `remote` as `http` with `url`/`headers`, and `name` as the key. Change the schema's `codemode` default to `false`, fill it when the cell is resolved so the resolved value carries it, and remove `unwrap_or(true)` from OpenCode's `server`. Answer `Codemode` in the declaration. Claude Code maps `false` and refuses `true`. OpenCode maps both, and its `Codemode(true)` construct says that its trace then records `execute` calls with empty `wrapped_calls`, so MCP calls made inside `execute` are not attributed and do not compare with Claude Code. Regenerate the published schema. Fix in both drivers what the Claude Code side showed: neither writes a key its harness does not document.
**Deliverables**: `crates/candler-harness-claude-code/src/prepare.rs`; `crates/candler-harness-opencode/src/prepare.rs`; `crates/candler-schemas/src/servers.rs` and its generated schema; the resolve step that fills the default
**Consistency Checks**: the rendering guards on the tip (expected: PASS)
**Commit**: `fix(library): render MCP servers from their type in both drivers`

## Step 3: Bite and verify
**Goal**: The guards catch a passthrough and a lost default.
**Implementation Logic**:
On a scratch branch, put `serde_json::to_value` back in Claude Code's renderer, then `unwrap_or(true)` back in OpenCode's, and see each guard fail. A fresh Sonnet verifier gets this leaf, the report and the diff, with the brief: find a field of `McpServer` that reaches either harness without its driver writing it.
**Deliverables**: `__reports__/diet/leaves/mcp_render_v0.md`
**Consistency Checks**: each scratch edit makes its guard fail (expected: FAIL)
**Commit**: `reports(api): close the MCP render`
