# One reader

**Goal**: Each consumer input has one reader, below every surface, that refuses what it does not understand: MCP servers and replicates first, then the rest of silent_acceptance, with no new layer (P4, cause D5).
**Estimate**: B on RiVault first, $1, cap $2; A1 Sonnet, $6, cap $12; A2 Sonnet with the HTN note, $5, cap $10. One worktree for the chain. At a cap, stop, commit what is green, and list what is left. Builds use the target dir the campaign README names, removed at the leaf's merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/integrity HEAD` succeeds
**Success Gates**:
- ⬜ An experiment with a malformed MCP server: on the base, `run` proceeds and the server is silently absent; on the tip, `resolve` refuses it with the field path, with the same record from the CLI and the op API
- ⬜ An absolute path in MCP server args is refused by `resolve`, as `run` refuses it (audit-2)
- ⬜ An invalid replicate count is refused by core's `Levels::replicates` with the same record from the CLI and the library, where on the base the library silently fell back to the file's value
- ⬜ The diff of A1 and A2 adds no trait, wrapper type or module between an input and its use; B's type is the only new type; `stdio_servers`, `Stdio` and `check_replicate_count` are gone, and a colgrep search for readers of `mcp.servers` finds one function
- ⬜ No `#[ignore]` whose reason names an OWNED set is left, and the silent_acceptance rows it hid pass
- ⬜ Each of the three commits builds and passes alone (`git rebase -x 'cargo nextest run --workspace' feat/integrity`), and B touches only `crates/candler-schemas/` with its schema drift check passing
- ⬜ The HTN migration note is in the leaf report and handed to the milestone coordinator for the HTN lane
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with the target dir the campaign README names, and `container-e2e` is green on the pull request
**References**: [Cycle plan v2 §D5, §Proof](../../../__reports__/diet/04-architecture_cycle_plan_v2.md) — the readers, the commit order and the observations; [CONTRIBUTING.md](../../../CONTRIBUTING.md) — the schemas-release gate

## Step 1: Type mcp.servers (B)
**Goal**: The experiment schema types `mcp.servers`, and a malformed entry fails validation.
**Implementation Logic**:
First check that the experiment schema can type the one path `mcp.servers` under `fixed` (keyed by dotted path through `patternProperties`) and in factor levels. If it can, type it there; if not, publish the server type as its own generated schema, and A1's reader enforces it for both. Add the typed MCP server to `candler-schemas` with its `JsonSchema`, regenerate the committed schemas, and pass the drift check. Nothing uses the type yet, so the workspace builds.
**Deliverables**: `crates/candler-schemas/` (the MCP server type; the regenerated experiment schema)
**Consistency Checks**: `git diff --name-only HEAD~1 | grep -v '^crates/candler-schemas/'` prints nothing, and the schema drift check passes (expected: PASS)
**Commit**: `feat(schemas)!: type the MCP servers an experiment declares`

## Step 2: One reader of MCP servers (A1)
**Goal**: `candler_harness::mcp::servers` is the only reader of `mcp.servers`, and `resolve` refuses a malformed entry before any run.
**Implementation Logic**:
Make `candler_harness::mcp::servers` return B's typed servers, refusing a malformed entry when it parses. OpenCode's `server()` keeps only its job, translating a typed server into OpenCode's config shape. Delete core's `stdio_servers` and `Stdio`; their caller calls `candler_harness::mcp::servers` itself and picks the stdio servers with a `match`. `resolve` calls the same function on each cell, so the refusal and the absolute-path check (audit-2) come before any run. Add the guard at the CLI and the op API for the malformed server.
**Deliverables**: `crates/candler-harness/src/mcp.rs`; OpenCode's `prepare.rs`; `crates/candler-core/src/jobs/orchestrate/servers.rs`; `resolve`'s cell check; the guards
**Consistency Checks**: the malformed-server guard passes on the tip and fails on the base (expected: PASS)
**Commit**: `feat(harness)!: read MCP servers in one place and refuse a malformed one`

## Step 3: One reader of replicates, and the rest of silent_acceptance (A2)
**Goal**: Replicates and the remaining silent inputs are refused by one reader each.
**Implementation Logic**:
Core's `Levels::replicates` loses its silent fallback to `file.replicates()`. Delete the CLI's `check_replicate_count`; the preflight calls core's reader. Apply the same pattern to the three deferred silent_acceptance cases (`runner/refusal.rs`, `jobs/refusal.rs`), `token_secret` from defaults or config, and non-string server env; un-ignore the row whose reason names OWNED. Where one reader's own code now refuses a field with its path, the three per-case codes PR #57 added fold into it only if a consumer reading the refusal still knows what to fix; otherwise they stay.
**Deliverables**: `crates/candler-core/src/runner/plan.rs`; the CLI preflight; the refusal files; the un-ignored row
**Consistency Checks**: `cargo nextest run --workspace` (expected: PASS)
**Commit**: `feat(resolve)!: read replicates in one place and refuse the silent inputs left`

## Step 4: Note, bite and verify
**Goal**: The HTN lane knows what is now refused, and the readers are shown to be the only ones.
**Implementation Logic**:
Write the migration note: which experiment files are now refused (a malformed MCP server entry, a replicate count only the CLI caught, the remaining silent-acceptance cases) and why, with the refusal each now gets, and the library break (`stdio_servers` and `Stdio` are gone). It is a note, not a compatibility shim; this repository has no external users. Bite: restore `stdio_servers`' lenient read on a scratch branch and see the malformed-server guard fail. A fresh Sonnet verifier reads the diff for new types and call hops.
**Deliverables**: `__reports__/diet/leaves/one_reader_v0.md` (§HTN migration note, §Bites, §Verifier)
**Consistency Checks**: the lenient reader restored on the tip makes the malformed-server guard fail (expected: FAIL)
**Commit**: `reports(api): close one reader per input`
