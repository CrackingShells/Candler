# Isolation

## Context
Depth 5. Container isolation (M5, needs M4) and the MCP server (M7, needs M3) in parallel; M5 adds injection, census, egress and plug-in environments over M2b's Docker backend. Amendment A3 adds a gate to M5: the agent environment has a declared kind, and no Python venv detail is left in the core.

## Goal
Runs can be isolated in containers, and agents can drive Candler through MCP.

## Pre-conditions
- [ ] `../m3_lean_cli.md` and `../m4_session_runner.md` are `done`

## Success Gates
- ⬜ `m5_container_isolation.md` and `m7_mcp_server.md` are `done`

## Status
```mermaid
graph TD
    m5_container_isolation[M5 Container isolation]:::planned
    m7_mcp_server[M7 MCP server]:::planned
    freeze[Freeze]:::planned
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
| `m7_mcp_server.md` | 📄 Leaf Task | ⬜ Planned |
| `freeze/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A3 | 2026-11-10 | `__reports__/candler-v1-review/01-gap_analysis_harness_env_v0.md` | [ ] | The core assumes the agent environment is a Python 3.12 venv at `/opt/env`, and the binary finds its assets in the source tree; M5 gains a gate that puts both behind a declared environment kind |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
