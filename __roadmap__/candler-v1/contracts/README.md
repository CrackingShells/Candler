# Contracts

## Context
Depth 1 of the meta-roadmap. Runs after `wire_schemas`, which creates the workspace and the first two wire types. M1 completes the contracts every later milestone builds on: the remaining wire types, the harness traits, the store, the process supervisor core and the legacy importer.

## Goal
Every contract and shared mechanism the two parity milestones need exists, and the golden corpus imports losslessly.

## Pre-conditions
- [ ] `wire_schemas.md` is `done` and merged on `main`

## Success Gates
- ⬜ `m1_contracts.md` is `done`

## Status
```mermaid
graph TD
    m1_contracts[M1 Contracts]:::planned
    parity[Parity]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `m1_contracts.md` | 📄 Leaf Task | ⬜ Planned |
| `parity/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
