# Answered

## Context
The second level of harness parity. It needs `../declaration.md`'s `AgentSetting` and `Answer`. Three leaves run in parallel, each owning its files: `mcp_render.md` (both drivers' `prepare.rs`, `candler-schemas/src/servers.rs`), `census.md` (the gateway census, `runner/evidence/census.rs`, Claude Code's `parse.rs`, the census path of the OpenCode driver) and `pages.md` (`crates/candler/tests/it/support/contract_pages/`). If `mcp_render.md` and `census.md` both need `candler-harness-opencode/src/driver.rs`, `census.md` rebases onto `mcp_render.md`.

## Goal
Each driver renders what the declaration answers, the census names servers and tools on both harnesses, and the pages are generated from the declaration.

## Pre-conditions
- [ ] `[nl]` `../declaration.md` is `done`

## Success Gates
- ⬜ `mcp_render.md`, `census.md`, `pages.md` and `recorded/` are `done`

## Status
```mermaid
graph TD
    mcp_render[MCP render]:::amendment
    census[Census]:::amendment
    pages[Generated pages]:::amendment
    recorded[Recorded]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `mcp_render.md` | 📄 Leaf Task | 🔵 Amendment |
| `census.md` | 📄 Leaf Task | 🔵 Amendment |
| `pages.md` | 📄 Leaf Task | 🔵 Amendment |
| `recorded/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
