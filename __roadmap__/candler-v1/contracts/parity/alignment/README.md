# Wire alignment

## Context
Depth 3, inserted by amendment A1 of `../README.md`. After both parity milestones and before the surfaces: M2c aligns the shared `experiment/1` wire type, which both M2a and M2b parse, with the inputs, table values and treatment source the parity runs need. `surfaces/` runs after it.

## Goal
The experiment file carries everything a cell needs, so no experiment depends on `--set`.

## Pre-conditions
- [ ] `../m2a_evaluation_parity.md` and `../m2b_execution_parity.md` are `done` and merged on `main`

## Success Gates
- ⬜ `m2c_wire_alignment.md` and `surfaces/` are `done`

## Status
```mermaid
graph TD
    surfaces[Surfaces]:::planned
    m2c_wire_alignment[M2c Wire alignment]:::inprogress
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `surfaces/` | 📁 Directory | ⬜ Planned |
| `m2c_wire_alignment.md` | 📄 Leaf Task | 🔄 In Progress |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
