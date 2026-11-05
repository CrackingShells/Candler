# Close-out

## Context
Level 4, the last. It runs the milestone's gates on the joined branch, re-reviews the services API against M2a and M2b, and documents the crates, fixtures and schema roots.

## Goal
The milestone's gates are recorded green and the contracts are documented for cold-start consumers.

## Pre-conditions
- [x] [cmd] The L3 leaves are landed: `for l in legacy_import seams; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`

## Success Gates
- ✅ [cmd] `gate_sweep` is landed: `for l in gate_sweep; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`
- ✅ [nl] The re-engaged api_review verifier reports no open BLOCKER or MAJOR finding, and the coordinator rules on each MINOR item

## Status
```mermaid
graph TD
    gate_sweep[Gate sweep]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `gate_sweep.md` | 📄 Leaf Task | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
