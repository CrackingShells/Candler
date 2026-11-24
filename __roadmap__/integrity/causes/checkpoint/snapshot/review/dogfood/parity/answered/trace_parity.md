# Trace parity

**Goal**: An MCP call made by the agent reads the same way in the normalised trace of either harness: as `ToolOrigin::Mcp { server }`, with the server's own tool name. The census keeps `census.tools` as the comparability verdict's input, names no server, and each census names its source: Claude Code's `system/init`, and for OpenCode the tools its model was offered at the gateway. `CensusSource` and `census.source` stay and say which (P10).
**Estimate**: Sonnet, $8, cap $16. At the cap, stop, commit what is green, and list what is left. Builds use the target dir the campaign README names, which is removed when the leaf merges.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor origin/feat/integrity HEAD` succeeds
- [ ] `[nl]` `../declaration.md` is `done`
- [x] `[nl]` The coordinator ruled on the optional `tool` field of `ToolOrigin::Mcp`. It is a change to `normalised-trace/1`, and `ToolOrigin` has `deny_unknown_fields`. Accepted: `tool` holds the bare tool name, so a consumer compares one spelling across both harnesses, and it is optional so that traces stored before the change still read
**Success Gates**:
- ⬜ The OpenCode export of a call to `s_t`, in a cell that declares server `s`, reads `origin: {mcp: {server: "s", tool: "t"}}`. On the base it reads `origin: other`
- ⬜ The Claude Code trace of a call to `mcp__s__t` reads the same origin, so the two harnesses' traces of one call compare field by field
- ⬜ `tool` is optional on `ToolOrigin::Mcp`. One test parses a trace stored before the change, with no `tool` field, and it still reads
- ⬜ The stored Claude Code trace still holds the raw `system/init` line, so a consumer can recover server status by post-processing the trace, as it does for calls. If the stored trace drops that line, the status is lost for good: the lead takes this to the coordinator before the leaf lands
- ⬜ The OpenCode reader takes the server names from the cell's resolved `mcp.*` values and never guesses a server from the spelling of a name
- ⬜ Resolving refuses a set of servers whose OpenCode tool prefixes are ambiguous (for example `a` and `a_b`), beside the existing server-name check, with a code in `codes.toml`
- ⬜ Under OpenCode code mode the trace records `execute` calls with empty `wrapped_calls`. That is what OpenCode's `Codemode(true)` answer declares, and nothing parses `execute`
- ⬜ `census.mcp_servers`, and Claude Code's reading of server status from `system/init`, are gone. `census.tools` and `census.init_tools` stay, and `manifest/compare.rs`'s verdict still reads `init_tools`. `SERVER_START_WAIT`'s doc no longer says that the census records servers
- ⬜ Each census names its source: Claude Code's is its `system/init`; OpenCode's is the tools its model was offered at the gateway. `CensusSource` and `census.source` stay and say which, the gateway's `census_init` stays, and the verdict's source reading stays
- ⬜ The trace and census schemas are regenerated, and the conformance suite checks the MCP origin for every driver
- ⬜ The report's "What it leaves" section judges the change at the three levels
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with the target dir the campaign README names, and `container-e2e` is green on the pull request
**References**: [Level-3 plan §Design](../../../../../../../../../__reports__/diet/07-plan_integrity_level3_v1.md), on one trace form for an MCP call and the reduced census. Code:
- `crates/candler-schemas/src/trace.rs`: `ToolOrigin`, `WrappedCall`;
- `crates/candler-harness-opencode/src/export.rs`: `tool_drafts`;
- `crates/candler-harness-claude-code/src/parse.rs`: `tool_call`, `origin_of`, `init_report`;
- `crates/candler-core/src/runner/evidence/census.rs`: `census`, `source_label`;
- `crates/candler-core/src/isolation/gateway.rs`: `census_init`;
- `crates/candler-core/src/manifest/compare.rs`: `class_verdict`;
- `crates/candler-core/src/jobs/orchestrate/servers.rs`: `SERVER_START_WAIT`.

## Step 1: Guard on the base
**Goal**: The origin guard fails on the base for OpenCode.
**Implementation Logic**:
Write the guard as one table over both readers' fixtures. For each harness it takes the recorded output of one call to the same tool of the same declared server, and expects one origin. Extend the OpenCode export fixture with that call if it has none. Run it on the base and record the OpenCode `origin: other`. No live run is needed: the readers are pure. Check also that the stored Claude Code trace keeps the raw `system/init` line, and that the gateway's census of an OpenCode run lists the declared server's tools. If either fails, stop and tell the lead.
**Deliverables**: the guard in `crates/candler/tests/it/` (it sees both drivers); the OpenCode export fixture
**Consistency Checks**: the origin guard on the base (expected: FAIL)
**Commit**: `test(library): hold both trace readers to one MCP origin`

## Step 2: One origin, one census
**Goal**: Both readers attribute MCP calls the same way, and the census names no server.
**Implementation Logic**:
- Pass the cell's resolved server names to OpenCode's export reader. Match `<server>_<tool>` against them, longest name first, and fall back to the built-in list or `Other` only when no server matches.
- Add the optional `tool` to `ToolOrigin::Mcp`, fill it with the bare tool name in both readers, and add the test that an old trace without it still parses.
- Add the ambiguous-prefix refusal at resolve.
- Remove `census.mcp_servers`, its schema field and its producer, and reword `SERVER_START_WAIT`'s doc.
- Leave the census sources as they are: `CensusSource`, `census.source`, the gateway's `census_init` and the verdict's source reading stay. Only `census.mcp_servers` goes.

Do not add any collection of server status: which servers the agent called is for the consumer to parse from the trace.
**Deliverables**: `crates/candler-schemas/src/trace.rs` and its generated schema; `crates/candler-harness-opencode/src/export.rs` and its caller; `crates/candler-harness-claude-code/src/parse.rs`; `crates/candler-core/src/runner/evidence/census.rs` and the census schema; `crates/candler-core/src/manifest/compare.rs`; the resolve server-name step; `crates/candler-core/src/jobs/orchestrate/servers.rs`; `docs/contract/codes.toml`
**Consistency Checks**: the origin guard on the tip (expected: PASS)
**Commit**: `fix(library): attribute MCP calls the same way on both harnesses`

## Step 3: Bite and verify
**Goal**: The guard catches a reader that loses the server.
**Implementation Logic**:
On a scratch branch, make OpenCode's reader return `Other` for every MCP call again, and see the guard fail. A fresh Sonnet verifier gets this leaf, the report and the diff. Its brief is to find a call to one server's tool, on either harness, that the two traces would record differently, and any second statement of which servers a run had.
**Deliverables**: `__reports__/diet/leaves/trace_parity_v0.md`
**Consistency Checks**: the scratch edit makes the origin guard fail (expected: FAIL)
**Commit**: `reports(api): close the trace parity`
