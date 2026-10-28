# Surfaces

## Context
Depth 3. After both parity milestones: the lean CLI (M3) and the session runner with the second harness (M4), in parallel. Both need only M2b; the tree also makes them wait for M2a.

## Goal
The CLI is lean and the runner covers long-lived sessions and a second harness.

## Pre-conditions
- [ ] `../m2a_evaluation_parity.md` and `../m2b_execution_parity.md` are `done`

## Success Gates
- ⬜ `m3_lean_cli.md` and `m4_session_runner.md` are `done`

## Status
```mermaid
graph TD
    m3_lean_cli[M3 Lean CLI]:::planned
    m4_session_runner[M4 Session runner and second harness]:::planned
    isolation[Isolation]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `m3_lean_cli.md` | 📄 Leaf Task | ⬜ Planned |
| `m4_session_runner.md` | 📄 Leaf Task | ⬜ Planned |
| `isolation/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
