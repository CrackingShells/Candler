# Catalog snapshot

## Context
Level 2 of M6c Integrity. Of v2's level 2, only P5 is here: P3 and P4·S wait (see the campaign README). P5 builds on P1's sealing path. If the checkpoint ruled that P5 waits, `catalog_snapshot.md` is marked `blocked` with option (b) recorded, and this level closes.

## Goal
A sealed run holds the bytes of the tool catalog it ran with, or the checkpoint's decision that this waits is recorded.

## Pre-conditions
- [ ] `[nl]` `../seam_review_one.md` records the checkpoint's decision on P5

## Success Gates
- ⬜ `catalog_snapshot.md` is `done`, or `blocked` with the checkpoint's reason
- ⬜ `review/` is `done`

## Status
```mermaid
graph TD
    catalog_snapshot[Catalog snapshot]:::planned
    review[Seam review 2]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `catalog_snapshot.md` | 📄 Leaf Task | ⬜ Planned |
| `review/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
