# Isolation

## Context
Depth 5, and the rest of the campaign: one milestone per directory level, strictly in this order. Container isolation is split by amendment A8: M5a (needs M4) is here as `m5_container_isolation.md`, M5b (egress proxy, I8 guard, guard reuse) is in `m5b/`; paired statistics (M6a) is in `m5b/stats/`, the MCP server and agent skills (M7) in `m5b/stats/mcp/`, the second plug-in (M8) in `m5b/stats/mcp/plugin/`. M5 as a whole is lean: an egress allowlist, plug-in-declared agent environments over the existing `isolation.build.*`, the guard that ground truth never enters the agent container (I8), and one Docker client with one control-container builder. Amendment A3 added an environment-kind gate to M5; amendment A6 supersedes it and removes outcome injection, the whole-container census and the macOS host backend.

## Goal
Runs can be isolated in containers with a closed network, and the plug-in environments and the Docker plumbing are built once.

## Pre-conditions
- [ ] `../m3_lean_cli.md` and `../m4_session_runner.md` are `done`

## Success Gates
- ⬜ `m5_container_isolation.md` is `done`
- ⬜ `m5b/` is `done`

## Status
```mermaid
graph TD
    m5_container_isolation[M5a Container isolation]:::done
    m5b[M5b Egress proxy and ground-truth guard]:::inprogress
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `m5_container_isolation.md` | 📄 Leaf Task | ✅ Done |
| `m5b/` | 📁 Directory | 🔄 In Progress |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A3 | 2026-11-10 | `__reports__/candler-v1-review/01-gap_analysis_harness_env_v0.md` | [ ] | The core assumes the agent environment is a Python 3.12 venv at `/opt/env`, and the binary finds its assets in the source tree; M5 gains a gate that puts both behind a declared environment kind |
| A6 | 2026-11-16 | `__reports__/candler-v1-review/02-findings_amendment_a6_v0.md` | [ "stats", "m6a_paired_stats", "mcp", "plugin", "m8_second_plugin" ] | M5 becomes lean and its A3 environment-kind gate is superseded (the assets-path item was already done: `crates/candler/src` embeds its assets and reads no `CARGO_MANIFEST_DIR`); M6 splits into M6a (paired statistics, one level below M5) and M8 (the second plug-in, renamed from `m6_second_plugin_freeze.md`, with no freeze); M7 moves one level below M6a; `freeze/` is removed with its gates, including A5's contract-page check against 1.0.0 |
| A8 | 2026-11-16 | `__reports__/candler-v1-review/04-findings_amendment_a8_v0.md` | [ "m5b" ] | M5 is split after its $60 cap was reached at about $56 with the rest forecast at $57-67 more: M5a (this directory's `m5_container_isolation.md`) keeps the Docker client, the egress cell value (refused until the proxy lands), `docker create` and plug-in environments; M5b (new `m5b/`, one level below M5a) wraps `stats/` and takes the guard reuse (A1), the egress proxy, the I8 refusal and e2e, and the live smoke; the order is M5a, M5b, M6a, M7, M8 |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| `m5_container_isolation.md` | `feat/m5-container-isolation` | merge 3e654ef (PR #36) | M5a landed; close report in the PR #36 merge message and `__reports__/m5-container-isolation/leaves/`; the egress allowlist, I8 and A1 gates moved to `m5b/` by A8 |
