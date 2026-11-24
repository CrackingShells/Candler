# mcp_schema: leaf report v0

`McpServer` (`crates/candler-schemas/src/servers.rs`) publishes
`crates/candler-schemas/schema/mcp-server.schema.json`. The schema accepted shapes
`candler_harness::mcp::read` refuses: `{}` (neither `command` nor `url`) and an entry with both
`command` and `url`. A published schema that accepts what its one reader refuses is a second, more
lenient reading of the same input. This leaf makes the generated schema refuse, for every shape
JSON Schema can express, what the reader refuses.

## The refusals of `mcp::read`, and whether the schema refuses them

`mcp::read` (`crates/candler-harness/src/mcp.rs:34`) deserialises the entry into `McpServer`
(serde on the type, with `deny_unknown_fields`), then rejects a shape. The reader is unchanged by
this leaf; the published schema now states each expressible refusal.

| Shape the reader refuses | Why | Schema refuses it? |
|---|---|---|
| a value that is not a table | `serde_json::from_value` fails | via the object `type` (already present) |
| an unknown key | `deny_unknown_fields` | via `additionalProperties: false` (already present) |
| neither `command` nor `url` | "has neither a `command` nor a `url`" (mcp.rs:58-63) | `oneOf` requires exactly one ✓ |
| both `command` and `url` | remote/launched detection, and "sets `url` or `headers`" (mcp.rs:52-55, 64-68) | the launched branch forbids `url`, the remote branch forbids `command` ✓ |
| a remote entry with no `url` | "is a remote server and has no `url`" (mcp.rs:49-51) | `oneOf` requires `url`, and a remote `type` without `url` matches neither branch ✓ |
| a remote entry setting `command`, `args` or `env` | "sets `command`, `args` or `env`" (mcp.rs:52-55) | the remote branch forbids all three ✓ |
| a launched entry setting `url` or `headers` | "sets `url` or `headers`" (mcp.rs:64-68) | the launched branch forbids both ✓ |
| a remote `type` (http/sse/remote) on a launched entry | the reader's family test treats it as remote | the launched branch constrains `type` to stdio/local; the remote branch forbids `command` ✓ |
| a stdio/local `type` on a remote entry | the family test treats it as launched | the remote branch constrains `type` to http/sse/remote; the launched branch forbids `url` ✓ |
| `name` that is not a plain name, or two servers rendering one `name` | `by_name` (mcp.rs:74-107), a per-map check, not one entry | cannot be stated per entry; the reader still refuses it |

The one reader's `by_name` checks (a `name` that is not a plain name, `__` in a `name`, two
servers that render one name) operate on the map of entries after re-keying, so a single-entry
schema cannot express them; that check is unchanged and still guards the read.

## Change

- `crates/candler-schemas/src/servers.rs`: `McpServer` gains a struct-level
  `#[schemars(extend("oneOf" = [...]))]` that defines the launched branch (`required: ["command"]`,
  forbidding `url`, `headers` and non-stdio/local `type`) and the remote branch
  (`required: ["url"]`, forbidding `command`, `args`, `env` and non-http/sse/remote `type`). The
  doc comment now states that exactly one of `command` and `url` is required. No serde attribute
  and no reader changed.
- `crates/candler-schemas/schema/mcp-server.schema.json`: regenerated with the `oneOf`.
- `crates/candler-schemas/tests/it/guard_mcp_server.rs`: the guard no longer asserts that
  `experiment.schema.json` accepts a malformed server (removed as instructed); the file now holds a
  table test `the_mcp_server_schema_refuses_what_the_reader_refuses` validating, against
  `mcp-server.schema.json`, ten refused shapes (including `{}` and `{"command": "x", "url":
  "http://h"}`) and the two valid entries (`{"command": "x"}` launched, `{"url": "http://h"}`
  remote), each with the pre-existing `the_mcp_server_schema_accepts_what_a_driver_writes` and
  `serde_refuses_an_unknown_key_in_a_server_entry`.
- `docs/contract/experiment-file.md`: adds one note, "`experiment.schema.json` does not check the
  entries of `mcp.servers`; `mcp-server.schema.json` checks one entry." The page's Notes are
  carried over verbatim by the contract-page generator, so the page is not regenerated.

## Gates

### G3: the four checks in RULES

| Check | Verdict | Evidence |
|---|---|---|
| `cargo fmt --check` | PASS | `FMT OK` |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | `Finished ... 52.06s`, no warnings |
| `cargo nextest run --workspace` | PASS | `749 tests run: 749 passed (6 slow), 31 skipped` |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | PASS | `Finished ... 17.72s` |

Also `cargo test -p candler --test it contract_pages`: `7 passed` (the docs-page edit does not
drift the generated pages), and `cargo nextest run -p candler-schemas`: `209 passed`.

### G2: no change outside the allowed files

`git diff --stat origin/feat/integrity` lists exactly:

```
 crates/candler-schemas/schema/mcp-server.schema.json  |  36 +++++-
 crates/candler-schemas/src/servers.rs                  |  30 ++++-
 crates/candler-schemas/tests/it/guard_mcp_server.rs    | 141 +++++++++++++--------
 docs/contract/experiment-file.md                       |   7 +-
```

`git diff --name-only origin/feat/integrity | grep -vE '^crates/candler-schemas/|^docs/contract/experiment-file.md$'`
prints nothing. `diff crates/candler-harness/src/mcp.rs` is empty; `git diff` shows no serde
attribute was changed (`#[serde(deny_unknown_fields)]` and every `#[serde(...)]` on `McpServer`
fields are untouched).

### G1: the table test fails on the base and passes on the tip

Base: scratch worktree `integrity-scratch/mcp_schema-1` at `origin/feat/integrity` with the new
`guard_mcp_server.rs` copied in, `CARGO_TARGET_DIR=.../integrity-target/mcp_schema-1`,
`cargo test -p candler-schemas --test it guard_mcp_server`:

```
running 3 tests
test guard_mcp_server::serde_refuses_an_unknown_key_in_a_server_entry ... ok
test guard_mcp_server::the_mcp_server_schema_accepts_what_a_driver_writes ... ok
test guard_mcp_server::the_mcp_server_schema_refuses_what_the_reader_refuses ... FAILED
...
thread '...' panicked at crates/candler-schemas/tests/it/guard_mcp_server.rs:94:9:
 mcp-server schema must refuse a server `neither `command` nor `url``: {}
...
test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 109 filtered out
```

Tip (this branch): `cargo nextest run -p candler-schemas -E 'test(guard_mcp_server)'`:

```
    Starting 3 tests across 2 binaries (206 tests skipped)
        PASS [   0.039s] (1/3) candler-schemas::it guard_mcp_server::serde_refuses_an_unknown_key_in_a_server_entry
        PASS [   0.039s] (2/3) candler-schemas::it guard_mcp_server::the_mcp_server_schema_accepts_what_a_driver_writes
        PASS [   0.042s] (3/3) candler-schemas::it guard_mcp_server::the_mcp_server_schema_refuses_what_the_reader_refuses
────────────
     Summary [   0.042s] 3 tests run: 3 passed, 206 skipped
```

`CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas drift`: `6 passed`, and the
bless-then-diff cycle is clean (the drift check passes without the variable too, run as part of
`cargo nextest run -p candler-schemas`).

## What is left

- The map-level `by_name` refusals remain the reader's only and are not stated in
  `mcp-server.schema.json`, because they span entries, not one entry. Nothing else is left; no
  gate is blocked.
