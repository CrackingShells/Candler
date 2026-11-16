# MCP server

## Context
Depth 8. The MCP server (M7) and the agent skills that let agents use it, one level below M6a and above the second plug-in. M7 needs only M3; the tree also makes it wait for M5a, M5b and M6a. Moved here by amendment A6 of `../../../README.md`.

## Goal
Agents drive Candler through MCP, and skills teach them to author plug-ins, drive runs and compute evals with it.

## Pre-conditions
- [ ] `../m6a_paired_stats.md` is `done`

## Success Gates
- ⬜ `m7_mcp_server.md` and `plugin/` are `done`

## Status
```mermaid
graph TD
    m7_mcp_server[M7 MCP server]:::planned
    plugin[Second plug-in]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `m7_mcp_server.md` | 📄 Leaf Task | ⬜ Planned |
| `plugin/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A6 | 2026-11-16 | `__reports__/candler-v1-review/02-findings_amendment_a6_v0.md` | [ ] | M7 moves here from `isolation/` and gains a gate: agent skills for authoring plug-ins, driving runs and computing evals through the MCP server, wanted before M8 |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
