# Parity

## Context
Depth 2. M2a (evaluation) and M2b (execution) run in parallel after M1; each proves Candler matches today's `htn_creator_eval` on its half. They touch different crates and modules, except where both read M1's supervisor and store, which neither changes without the other's coordinator knowing.

## Goal
Candler reproduces today's grading exactly and today's runs equivalently.

## Pre-conditions
- [ ] `../m1_contracts.md` is `done` and merged on `main`

## Success Gates
- ⬜ `m2a_evaluation_parity.md` and `m2b_execution_parity.md` are `done`

## Status
```mermaid
graph TD
    m2a_evaluation_parity[M2a Evaluation parity]:::planned
    m2b_execution_parity[M2b Execution parity]:::planned
    surfaces[Surfaces]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `m2a_evaluation_parity.md` | 📄 Leaf Task | ⬜ Planned |
| `m2b_execution_parity.md` | 📄 Leaf Task | ⬜ Planned |
| `surfaces/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
