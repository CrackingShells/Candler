# one_reader (Step 1, Commit B): leaf report v0

Commit B types the MCP servers an experiment declares, in `candler-schemas`, as its own generated
schema. It touches only `crates/candler-schemas/`. Steps 2-4 (A1, A2, note) belong to the next agent
and are not started.

## First reads

- `candler_harness::mcp::servers` (`crates/candler-harness/src/mcp.rs:77`) reads `mcp.servers` as a
  map (`cell.table_at`), strips the optional `name` leaf and re-keys by rendered name
  (`by_name`, `mcp.rs:39`).
- opencode's `server()` (`crates/candler-harness-opencode/src/prepare.rs:295`) translates one entry
  into OpenCode v2's shape. The fields it reads: an optional `codemode` (default true), an optional
  `type` marker, and the two disjoint families — a launched command (`command`, `args`, `env`) or a
  remote URL (`url`, `headers`). `LOCAL_KEYS` and `REMOTE_KEYS` (`prepare.rs:276-277`) name the
  allowed keys; any other key is refused.
- The experiment schema (`crates/candler-schemas/src/execution.rs`, `RawExperimentFile`) types
  `fixed` as `BTreeMap<FieldPath, JsonValue>` (rendered through `patternProperties`) and each factor
  `level` as the generic `Level`. Every concrete value under either is the same "any JSON value
  except null".

## B: how the type is published

**Choice: publish the server type as its own generated schema** (`mcp-server.schema.json`), not in
the experiment schema.

**Why:** the experiment schema cannot type the one path `mcp.servers` in place. Under `fixed`, keys
are dotted field paths but *every* key maps to the same generic `JsonValue` schema via
`patternProperties`; there is no mechanism to give `mcp.servers` a schema different from any other
`fixed` path. A factor level is the same generic `Level`. So a server declared under `fixed` or in a
factor level can only sit under a schema that is permissive about the server object's own fields.
The typed server therefore lives at its own root, and A1's single reader applies it to both places,
as §D5's plan allows.

`candler_schemas::servers` defines `McpServer` (the full key set the drivers read, `deny_unknown_fields`)
and `McpServerKind` (the `type` markers: `stdio`, `local`, `http`, `sse`, `remote`). `command` is a
non-optional-in-schema `string`, so a server whose `command` is not text now fails this schema. It is
registered as the root `mcp-server` in `generate.rs` and committed under
`crates/candler-schemas/schema/mcp-server.schema.json`.

**Base vs tip for the observation** (`guard_mcp_server` tests): the committed experiment schema is
unchanged by B and still validates a full experiment whose `fixed."mcp.servers"` holds a server with
a non-text `command` (a number). The published `mcp-server` schema refuses that same entry. On the
base, `mcp-server.schema.json` does not exist, so the malformed entry had no typed reader at all.

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| Leaf pre-condition: `feat/integrity` is an ancestor of HEAD | PASS | `git merge-base --is-ancestor feat/integrity HEAD` succeeds |
| Base observation: experiment schema accepts a non-string `command`; no typed schema exists | PASS (observed on base) | scratch `one_reader-1` at `origin/feat/integrity`: `BASE ACCEPTS: malformed mcp.servers entry (non-string command) validates against experiment.schema.json`; `mcp-server.schema.json exists on base: False` |
| Tip observation: the typed schema refuses the non-string `command` | PASS | `guard_mcp_server::the_mcp_server_schema_refuses_a_non_text_command` (and `the_experiment_schema_cannot_type_the_mcp_servers_path`, `the_mcp_server_schema_accepts_what_a_driver_writes`) |
| Schema drift check (blessed, then without the variable) | PASS | `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas drift` → 6 passed; then `cargo nextest run -p candler-schemas -E 'test(schema_drift)'` → `committed_schemas_have_no_drift` PASS, `every_committed_schema_file_has_a_root` PASS |
| Workspace clippy | PASS | `cargo clippy --workspace --all-targets -- -D warnings` → `Finished` with no warnings |
| Workspace nextest | PASS | `cargo nextest run --workspace` → `743 tests run: 743 passed (6 slow), 32 skipped` |
| Workspace doc | PASS | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` → `Finished` |
| `cargo fmt --check` | PASS | clean after `cargo fmt` |
| Commit touches only `crates/candler-schemas/` | PASS (pending `git diff --name-only HEAD~1 \| grep -v '^crates/candler-schemas/'` prints nothing) | all changed paths are under `crates/candler-schemas/` |
| Schemas-release gate (CI) | PASS | the commit is `feat(schemas)!` and touches no other crate under `crates/` (CONTRIBUTING.md:147-182) |

## Notes

- One test, `guard_protocol_client::every_failure_path_of_an_evaluator_is_accounted_for`
  (`candler-core`), failed once during a full parallel workspace run and passed immediately on
  re-run, both on the base (scratch `one_reader-2`, three runs) and on this tip's full suite
  (743/743). It is parallel-execution contention in an orthogonal path; no other crate uses the new
  type, so B cannot regress it.
- The typed entry is deliberately a permissive-but-typed struct (every field the drivers read,
  `deny_unknown_fields`); the local/remote dichotomy and the `command`-or-`url` requirement are the
  reader's job in A1, which is the next step's work, not B's.
