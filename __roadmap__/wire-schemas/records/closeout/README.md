# Close-out

## Context
Level 2 of `wire-schemas`, after every `records/` leaf has merged. The coordinator updates the README and runs every milestone gate on the integrated branch before the milestone merge.

## Reference Documents
- [R01 Milestone leaf](../../../candler-v1/wire_schemas.md) — the gates that must pass

## Goal
The README says what exists, and every gate of the milestone leaf passes on `feat/wire-schemas`.

## Pre-conditions
- [ ] Every leaf of `../` is `done` and merged on `feat/wire-schemas`

## Success Gates
- ⬜ Every Success Gate of `../../../candler-v1/wire_schemas.md` passes on `feat/wire-schemas`

## Status
```mermaid
graph TD
    milestone_gates[README status and the milestone gate sweep]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `milestone_gates.md` | 📄 Leaf Task | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
