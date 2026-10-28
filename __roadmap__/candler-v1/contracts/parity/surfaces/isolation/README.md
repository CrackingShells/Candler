# Isolation

## Context
Depth 4. Container isolation (M5, needs M4) and the MCP server (M7, needs M3) in parallel; Docker enters the project here.

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

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
