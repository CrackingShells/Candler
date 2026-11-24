# Trace parity leaf, v0

An MCP call reads the same in the trace of either harness: `origin: {mcp: {server, tool}}`. The
census names no server. The OpenCode endpoint the leaf was to read the census from does not exist
in the pinned binary; the coordinator kept the gateway census (amendment A5), so each census names
its source.

Base `21d8486c` (`origin/feat/integrity` at the branch point). Tip: the tip of the
branch `leaf/l3-trace-parity`, which also merges `feat/integrity` at 29573006 (P9).

## What changed

- `ToolOrigin::Mcp` has an optional `tool` (bare tool name); `ToolOrigin::mcp_tool(server, tool)` is
  the one constructor. A trace without the field reads and writes back without it.
- Claude Code's reader fills `tool` from `mcp__<server>__<tool>`.
- `HarnessDriver::offline_reader` takes the cell's declared server names. OpenCode's export reader
  reads `<server>_<tool>` against them, longest name first; a name no declared server begins is
  `builtin` or `other`, never a guessed server. `execute` stays a built-in call with empty
  `wrapped_calls`. The core passes the names of `mcp::servers(cell)`; `Registry::offline` (ingest,
  no cell) passes none.
- `check_settings` of the OpenCode driver refuses servers where `a_` begins another name:
  `DriverError::AmbiguousServers`, code `experiment.mcp_server_prefix_ambiguous`. The contract pages are
  regenerated.
- `census.mcp_servers`, `InitReport.mcp_servers`, `InitReportError::ServersNotList` and Claude Code's
  reading of `mcp_servers` are gone; code `harness.server_list_malformed` is marked lifted.
  `SERVER_START_WAIT` no longer says the census records servers; `architecture.md` follows.
- Conformance: `Case::with_mcp_call` checks the origin of the one call a case's stream holds, and each
  driver has one such case.

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| OpenCode export of `s_t` reads `mcp{s,t}`; `other` on the base | PASS | base (`9dffe92b`): `guard_mcp_origin::one_call_has_one_origin_on_both_harnesses` FAIL, `"opencode": [{"kind":"other"}]`; tip PASS |
| Claude Code `mcp__s__t` reads the same origin | PASS | base FAIL: `{"kind":"mcp","server":"s"}` has no tool; tip PASS |
| `tool` optional; an old trace still reads | PASS | `guard_trace_wire::an_mcp_origin_without_a_tool_still_reads_and_one_with_it_round_trips`; the hand-written trace file's wrapped call has no `tool` and round-trips byte for byte |
| Stored Claude Code trace keeps the raw `system/init` line | PASS | `guard_mcp_origin::claude_code_trace_keeps_the_raw_init_line`: the trace holds `"subtype":"init"` with `"status":"connected"`; passes on the base too |
| OpenCode reader takes server names from the cell, never from spelling | PASS | `opencode_export_reader_attributes_calls_to_the_declared_servers_only`: `x_t`, `s`, `s_` are `other`; with no declared server `s_t` is `other` |
| Resolve refuses ambiguous prefixes, with a code | PASS | `opencode_refuses_servers_whose_tool_prefixes_are_ambiguous`; `contract_pages` d1 and d3 pass |
| Code mode: `execute` calls, empty `wrapped_calls` | PASS | same reader test: `execute` is `builtin`, `wrapped_calls: []` |
| `census.mcp_servers` and Claude Code's server-status reading gone; `init_tools` stays | PASS | `guard_run_evidence::b25_census_from_a_real_init` (2 values), `guard_container_record` (`census.mcp_servers` is `None`); `harness_baseline/census.json` loses the key |
| Each census names its source: Claude Code's is its `system/init`; OpenCode's is the tools its model was offered at the gateway (A5) | PASS | `CensusSource` and `census.source` stay: `guard_harness_facts::harness_census_records_what_the_source_saw` records no `census.source` for the harness's own init and `"gateway"` for the gateway's; `guard_census_gateway` holds the gateway run's `census.source`. The original endpoint does not exist: see Probe |
| Trace and census schemas regenerated; conformance checks the MCP origin for every driver | PASS | the trace schema regenerated (`drift` passes); no census schema names the field; `conformance_opencode_recorded_runs` and the Claude Code suite run `mcp_call` cases |
| A server name is letters, digits, `-` and single `_`, by `name` or by key | PASS | `guard_mcp_names::a_server_name_is_letters_digits_and_dashes_joined_by_single_underscores`: `s_`, `_s`, `a__b`, `a.b`, `my srv` refused, by name and by key; `a-b`, `a_b` accepted |
| The published schema refuses what the name rule refuses | PASS | `fix(schemas)!`: `McpServer.name` carries `SERVER_NAME_PATTERN` (`^[A-Za-z0-9-]+(?:_[A-Za-z0-9-]+)*(?![\s\S])`, the end anchor the schemas use); `guard_mcp_server::the_mcp_server_schema_refuses_what_the_reader_refuses` has the rows `a.b` and `s_`. No published schema types the server map, so the keys are checked by the reader only |
| Report's "What it leaves" | PASS | below |
| fmt, clippy `-D warnings`, nextest, doc `-D warnings` | PASS | all clean. `guard_wiring::wiring_stop_execs...` and `guard_known_truth::kt_two_levels...` each failed once in a loaded full run and passed alone (89 s and 2 s) |
| container-e2e | not run locally | runs on the PR |
| Step 3 bite | PASS | scratch worktree on the tip with `origin_of` returning `Other`: `guard_mcp_origin` FAIL (`"opencode": [{"kind":"other"}]`), the reader test and `conformance_opencode_recorded_runs` FAIL; scratch removed |
| Fresh Sonnet verifier | MISSING | the lead dispatches it |

## Probe (the stop)

The pinned image (`assets/docker/base-opencode`, `OC_VERSION=2.2.7`, built as `l3-trace-probe-img`,
run as `l3-trace-probe-1`, both removed) served `opencode serve` with a config declaring one stdio
server. Its `/openapi.json` lists 139 paths, all under `/api`; `GET /experimental/tool?provider=..&model=..`
and `/experimental/tool/ids` return the web app's HTML page, not a tool list, and no path lists tools.
`/api/mcp` answered `{"data":[]}` for the declared server; `/api/config` showed the config loaded. No
model call was made. The docs expose the tool list only to plugins as `ctx.tool.list()`. The leaf's second
check fails, so the gateway census stays (A5).

## What it leaves

- **Developed:** one probe image and container, removed; one scratch build for the bite, removed.
  Builds shared the `l3-2` target dir.
- **In the code:** one origin function per harness, one constructor, one reader of `mcp.servers`.
  The server-list census value and its error are deleted. The gateway census,
  `CensusSource` and `census.source` stay by A5. A server name is one both harnesses carry into a tool
  id unchanged, so a call splits one way only.
- **Consumed:** the experimenter reads `origin.mcp.server` and `tool` the same on either harness; a
  server's status is in Claude Code's raw init line, which the trace keeps.

## Not covered

Left by this leaf, judged at seam review 3:

- An OpenCode export ingested without its cell reads its MCP calls as `other`.
- A server the cell does not declare reads as `other` on OpenCode and as `mcp` on Claude Code: Claude
  Code's `origin_of` takes any `mcp__x__t`, OpenCode's reader only declared servers (stated on
  `ToolOrigin::Mcp`).
- `offline_reader`'s `servers` argument is unused by the Claude Code driver, and `Registry::offline`
  and the ingest path pass `&[]`. It is the shape a harness that spells calls `<server>_<tool>` needs,
  not a second producer.
- The reader of server names (`candler_harness::mcp::by_name`) is written out, not compiled from `SERVER_NAME_PATTERN`: the pattern ends with the lookahead `(?![\s\S])`, which the `regex` crate does not read, and no other runtime dependency does. The string is in the tree once, but the rule is stated twice, as that pattern and as the reader's code: a lookahead-capable regex dependency in `candler-harness`, only to state one rule once, would cost more than it saves. Two tests tie the statements together with the same names: `guard_mcp_server::the_mcp_server_schema_refuses_what_the_reader_refuses` (the schema's side, rows `a.b` and `s_`) and `guard_mcp_names::a_server_name_is_letters_digits_and_dashes_joined_by_single_underscores` (the reader's side).
- The `feat(schemas)` commit removes `ToolOrigin::mcp`, so the workspace builds again only at the
  `fix(library)` commit after it.
