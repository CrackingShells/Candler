# Recorded

## Context
The third level of harness parity. It needs the census reduced to tools by `../trace_parity.md` and the typed rendering of `../mcp_render.md`. `snapshot_out.md` removes the constant catalog producer and makes the census the one record; `parity_run.md` runs one cell on both harnesses with a real MCP server. They own disjoint files: `snapshot_out.md` the catalog step, the evidence provider, `comparable`, `inspect`, the oracle vectors, the fake docker, README and `AGENTS.md`; `parity_run.md` a new container-e2e guard and its fixture.

## Goal
The census is the only record of the tools a run had, and a parity guard holds both harnesses to it.

## Pre-conditions
- [x] `[nl]` `../mcp_render.md` and `../trace_parity.md` are `done` (`../census.md` stays `blocked`)

## Success Gates
- ⬜ `snapshot_out.md`, `parity_run.md` and `closing/` are `done`

## Status
```mermaid
graph TD
    snapshot_out[Snapshot out]:::done
    parity_run[Parity run]:::inprogress
    closing[Closing]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `snapshot_out.md` | 📄 Leaf Task | ✅ Done |
| `parity_run.md` | 📄 Leaf Task | 🔄 In Progress |
| `closing/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
