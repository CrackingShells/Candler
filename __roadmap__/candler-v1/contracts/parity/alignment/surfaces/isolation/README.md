# Isolation

## Context
Depth 5, and the rest of the campaign: one milestone per directory level, strictly in this order. Container isolation (M5, needs M4) is here; paired statistics (M6a) is in `stats/`, the MCP server and agent skills (M7) in `stats/mcp/`, the second plug-in (M8) in `stats/mcp/plugin/`. M5 is lean: an egress allowlist, plug-in-declared agent environments over the existing `isolation.build.*`, the guard that ground truth never enters the agent container (I8), and one Docker client with one control-container builder. Amendment A3 added an environment-kind gate to M5; amendment A6 supersedes it and removes outcome injection, the whole-container census and the macOS host backend.

## Goal
Runs can be isolated in containers with a closed network, and the plug-in environments and the Docker plumbing are built once.

## Pre-conditions
- [ ] `../m3_lean_cli.md` and `../m4_session_runner.md` are `done`

## Success Gates
- ⬜ `m5_container_isolation.md` is `done`
- ⬜ `stats/` is `done`

## Status
```mermaid
graph TD
    m5_container_isolation[M5 Container isolation]:::planned
    stats[Paired statistics]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `m5_container_isolation.md` | 📄 Leaf Task | ⬜ Planned |
| `stats/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A3 | 2026-11-10 | `__reports__/candler-v1-review/01-gap_analysis_harness_env_v0.md` | [ ] | The core assumes the agent environment is a Python 3.12 venv at `/opt/env`, and the binary finds its assets in the source tree; M5 gains a gate that puts both behind a declared environment kind |
| A6 | 2026-11-16 | `__reports__/candler-v1-review/02-findings_amendment_a6_v0.md` | [ "stats", "m6a_paired_stats", "mcp", "plugin", "m8_second_plugin" ] | M5 becomes lean and its A3 environment-kind gate is superseded (the assets-path item was already done: `crates/candler/src` embeds its assets and reads no `CARGO_MANIFEST_DIR`); M6 splits into M6a (paired statistics, one level below M5) and M8 (the second plug-in, renamed from `m6_second_plugin_freeze.md`, with no freeze); M7 moves one level below M6a; `freeze/` is removed with its gates, including A5's contract-page check against 1.0.0 |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
