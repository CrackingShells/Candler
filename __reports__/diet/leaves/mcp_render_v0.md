# mcp_render: leaf report (v0)

Base `21d8486c` (`origin/feat/integrity`). Tip: see the pull request (the report commit follows the two code commits).
Status: done. Fresh Sonnet verifier: MISSING (dispatched by the lead after the pull request).

## Documentation cited (Context7)
- Claude Code, `/websites/code_claude`: `code.claude.com/docs/en/mcp-quickstart` and `.../agent-sdk/mcp` (`.mcp.json` entries: `type: "http"` with `url` and `headers`; `type: "stdio"` with `command` and `args`), `.../agent-sdk/typescript` (`McpStdioServerConfig`: `type?`, `command`, `args?`, `env?`; `McpSSEServerConfig`: `type: "sse"`, `url`, `headers?`). Claude Code documents no `codemode` key and no `name` key; the server's name is its key.
- OpenCode, `/websites/opencode_ai`: `opencode.ai/docs/mcp-servers`, `Local` table (`type`, `command`, `cwd`, `environment`, `disabled`, `codemode`, `timeout`, `protocol`, `pool`) and `Remote` table (`type`, `url`, `headers`, `oauth`, `disabled`, `codemode`, `timeout`, `protocol`, `pool`); `codemode` "Defaults to `true`", so Candler writes it for every server.

## Answers per transport
| Kind | Claude Code | OpenCode |
|:--|:--|:--|
| `stdio`, `local` | mapped, `type: "stdio"` with `command`, `args`, `env` | mapped, `type: "local"` (unchanged) |
| `http`, `remote` | mapped, `type: "http"` with `url`, `headers` | mapped, `type: "remote"` (unchanged) |
| `sse` | mapped, `type: "sse"` with `url`, `headers` | mapped as `remote` (unchanged; see Observations) |
| `codemode = true` | refused: "Claude Code offers MCP tools directly; it has no code mode" | mapped; the construct says its trace records `execute` calls with empty `wrapped_calls`, so MCP calls inside `execute` are not attributed and do not compare with Claude Code |
| `codemode = false` | mapped (no key) | mapped (`codemode: false`) |

## Gates
| Gate | Verdict | Evidence |
|:--|:--|:--|
| Claude Code `.mcp.json` for `local` with `codemode` and `name`: `type: "stdio"`, name as key, no `codemode`/`name` | PASS | `the_mcp_file_holds_the_keys_claude_code_documents`; FAIL on base (`e0b2da21`-equivalent run: 9/9 guards failed), PASS on tip |
| `codemode = true` refused through the `Codemode(true)` arm; unset or `false` runs | PASS | `claude_code_refuses_codemode_true` through `refuse_unanswered`; FAIL on base, PASS on tip |
| `remote` reaches Claude Code as `type: "http"` with `url`, `headers`; a field with no place is refused, not dropped | PASS | same test, server `b`. Every `McpServer` field has a place: `type`, `command`, `args`, `env`, `url`, `headers` are written, `name` is the key, `codemode` is answered (`false` maps, `true` refuses); a kind-mismatched field is refused by `mcp::read` |
| OpenCode `opencode.json` for a server with no `codemode`: `codemode: false` (base: `true`) | PASS | `opencode_a_server_name_renders_the_server_under_it` and `the_config_holds_the_keys_opencode_documents`; FAIL on base, PASS on tip |
| Two cells differing only by an explicit `codemode = false` have the same digest | PASS | `a_server_without_codemode_is_the_cell_with_codemode_off`, through `plan_runs`; FAIL on base, PASS on tip |
| Conformance check of each driver's rendered MCP file against its documented keys; fails on a passthrough key | PASS | the two `..._holds_the_keys_..._documents` tests; bite A below |
| "What it leaves" at three levels | PASS | below |
| `cargo fmt --check`, `clippy -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc` | PASS | all clean on the tip |
| `cargo nextest run --workspace` | DEVIATION | 762 run, 760 passed. `guard_supervise::a_pre_stop_command_leaves_nothing_in_its_group` and `guard_wiring::wiring_stop_execs_in_the_container_with_the_observed_id` failed under load once and passed alone (0.9 s, 88.8 s) and in an earlier full run of the five crates |
| `container-e2e` green on the pull request | MISSING | runs in CI on the pull request |
| Fresh Sonnet verifier | MISSING | dispatched by the lead |

Deviations from the brief: the guard commit is red by design (Step 1 expects FAIL on the base). The schema change is in the `fix(library)` commit, not a `schemas` commit: `McpServer::codemode` becomes `bool`, which its readers in other crates need to compile, so the type and the generated schema (`mcp-server.schema.json`, `default: false`) land with them. Driver guards build their cell with `ResolvedValue::recorded`, as their neighbours do; the digest guard goes through `plan_runs`.

## Bites (scratch worktree `integrity-scratch/mcp_render-1`, removed with its target dir)
| Edit on the tip | Guards that failed |
|:--|:--|
| A: `serde_json::to_value(server)` back in Claude Code's `render_mcp` | `the_mcp_file_holds_the_keys_claude_code_documents`, `a_server_name_renders_the_server_under_it` |
| B: `McpServer::codemode` defaulting to `true` (the OpenCode `unwrap_or(true)` equivalent now that the default is the type's) | `the_config_holds_the_keys_opencode_documents`, `opencode_a_server_name_renders_the_server_under_it`, `opencode_prepare_writes_the_provider_mcp_codemode_and_gateway_names`, `claude_code_refuses_codemode_true` |
| C: `fill_codemode` removed from `fresh_cells` | `a_server_without_codemode_is_the_cell_with_codemode_off` |
| D: `Codemode(true)` mapped in Claude Code | `claude_code_refuses_codemode_true` |

## Digest change
`fill_codemode` writes `mcp.servers.<key>.codemode = false` into each cell's values for every server that leaves it out, in `fresh_cells` (so a server declared in `[fixed]`, as a factor level, or by a plug-in environment is covered the same way). The cell digest of every cell that declares a server without `codemode` changes; a cell with no server, or a server that sets `codemode`, keeps its digest. In-repo: no recorded fixture holds a digest of such a cell.

## What it leaves
- **How it is developed.** Documentation read through Context7 before any edit; the guards were seen red on the base and green on the tip, and each fix was bitten. Three crates were built in one target dir slot; one scratch build, removed.
- **What it leaves in the code.** One writer per harness, from the type: Claude Code's `mcp_entry` and OpenCode's `server`; `serde_json::to_value(servers)` and `unwrap_or(true)` are gone. One default: the schema's `false` is the reader's default (`serde(default)`) and `fill_codemode` writes that same value into the cell; there is no `Option` and no second `unwrap_or`. One refusal site, the existing `refuse_unanswered`, now reached through Claude Code's `Codemode(true)` arm. `settings_set_by` yields `Codemode` for every declared server, so each cell with a server is asked about its mode. No new trait, wrapper or module.
- **How it is consumed.** The experimenter who writes `codemode = true` for Claude Code is refused before any run, with the reason. The `.mcp.json` Claude Code reads holds only keys it documents. The resolved cell, and so the record and the digest, say `codemode = false` where the experiment was silent.

## Observations
- OpenCode's documentation describes `remote` as a Streamable HTTP endpoint and mentions no SSE transport, yet `sse` is mapped to `remote`. Left as it was (a refusal would change compatibility); the lead decides.
- A key under `mcp.servers` given as a whole table whose name is no field-path segment gets no recorded `codemode`; the reader still gives it `false`.
- `fill_codemode` lives in `runner/plan.rs` beside the session default; the resolver steps see only experiment-wide values, so a server declared as a factor level would be missed there.
