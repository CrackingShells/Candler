# Answered

## Context
The second level of harness parity. It needs `../declaration.md`'s `AgentSetting` and `Answer`. Three leaves run in parallel, each owning its own files: `mcp_render.md` (both drivers' `prepare.rs` MCP renderers, `candler-schemas/src/servers.rs`), `trace_parity.md` (OpenCode's `export.rs`, Claude Code's `parse.rs`, `runner/evidence/census.rs`, the trace schema, the resolve server-name step) and `pages.md` (`crates/candler/tests/it/support/contract_pages/`). `census.md` is blocked: revision 2 of A3 removed its server census, and `trace_parity.md` carries what remains. If `mcp_render.md` and `trace_parity.md` both need the OpenCode driver's `driver.rs`, `trace_parity.md` rebases onto `mcp_render.md`.

## Goal
Each driver renders what the declaration answers, both trace readers attribute an MCP call the same way, and the pages are generated from the declaration.

## Pre-conditions
- [x] `[nl]` `../declaration.md` is `done`

## Success Gates
- ⬜ `mcp_render.md`, `trace_parity.md`, `pages.md` and `recorded/` are `done`, and `census.md` stays `blocked`

## Status
```mermaid
graph TD
    mcp_render[MCP render]:::amendment
    census[Census]:::blocked
    pages[Generated pages]:::amendment
    recorded[Recorded]:::amendment
    trace_parity[Trace parity]:::amendment
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
| `census.md` | 📄 Leaf Task | 🚫 Blocked |
| `pages.md` | 📄 Leaf Task | 🔵 Amendment |
| `recorded/` | 📁 Directory | 🔵 Amendment |
| `trace_parity.md` | 📄 Leaf Task | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
