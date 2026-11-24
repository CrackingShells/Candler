# Seam review 1 and checkpoint

## Context
After level 1 merges into `feat/integrity`. A fresh reviewer reads the whole diff since the base across the seams that leaf verifiers cannot see, and the spend of level 1 is set against its caps to decide P5 before level 2 starts.

## Goal
Level 1's seams are reviewed and P5 is decided from measured spend.

## Pre-conditions
- [ ] `[cmd]` `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.3.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh ls __roadmap__/integrity/causes/` shows the four level 1 leaves `done`

## Success Gates
- ⬜ `seam_review_one.md` is `done`, with its findings fixed or recorded
- ⬜ `snapshot/` is `done`

## Status
```mermaid
graph TD
    seam_review_one[Seam review 1]:::planned
    snapshot[Catalog snapshot]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `seam_review_one.md` | 📄 Leaf Task | ⬜ Planned |
| `snapshot/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
