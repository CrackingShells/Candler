# Seam review 2

## Context
After level 2 merges, before the dogfooding pass. The second seam review asks the same questions as the first of everything merged since it, and whether the snapshot holds at its seam.

## Goal
Nothing merged since seam review 1 reopens a cause, and the snapshot holds at its seam.

## Pre-conditions
- [ ] `[nl]` `../catalog_snapshot.md` is `done` or `blocked`, and its merge (if any) is in `feat/integrity`

## Success Gates
- ⬜ `seam_review_two.md` is `done`, with its findings fixed or recorded
- ⬜ `dogfood/` is `done`

## Status
```mermaid
graph TD
    seam_review_two[Seam review 2]:::planned
    dogfood[Dogfooding pass]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `seam_review_two.md` | 📄 Leaf Task | ⬜ Planned |
| `dogfood/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
