# Import

## Context
Level 3. It joins the L2 services: the legacy importer lands the golden corpus in a store, and the seams guards follow one live run across every leaf boundary.

## Goal
The 240 golden runs import equal to the oracle, and the cross-leaf seams are guarded.

## Pre-conditions
- [x] [cmd] The L2 leaves are landed: `for l in supervisor run_store claude_code_reader legacy_mapping; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`

## Success Gates
- ⬜ [cmd] Both L3 leaves are landed: `for l in legacy_import seams; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`
- ⬜ [cmd] `CANDLER_GOLDEN_CORPUS="$GOLDEN" cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_golden)'` passes
- ⬜ [nl] legacy_import's required verifier (with the legacy_mapping diff) reports no open finding, and the level's Grit review is closed

## Status
```mermaid
graph TD
    legacy_import[Legacy import]:::planned
    seams[Seams]:::planned
    closeout[Close-out]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `legacy_import.md` | 📄 Leaf Task | ⬜ Planned |
| `seams.md` | 📄 Leaf Task | ⬜ Planned |
| `closeout/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
