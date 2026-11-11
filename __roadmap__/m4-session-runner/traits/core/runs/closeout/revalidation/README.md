# Revalidation

## Context
L5 of `m4-session-runner`. It measures, on the finished branch, that no stored digest moved, and confirms session mode and the OpenCode driver once each against real models. Both leaves only read the code; the live runs spend tokens and wait for Eliott.

## Goal
Zero digest moves are recorded, and the live confirmations of C1, C2, a notification-started turn and an OpenCode run are recorded.

## Pre-conditions
- [ ] [cmd] L4 merged: `for l in 'merge(runner): land session_controls' 'merge(repo): land harness_guard' 'merge(runner): land contract_docs' 'merge(design): land design_record'; do git log --first-parent --format=%s feat/m4-session-runner | grep -qxF "$l" || echo "missing $l"; done` prints nothing

## Success Gates
- ⬜ [cmd] `digest_invariance.md` and `live_confirmation.md` are `done`

## Status
```mermaid
graph TD
    digest_invariance[Digest invariance]:::planned
    live_confirmation[Live confirmation]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `digest_invariance.md` | 📄 Leaf Task | ⬜ Planned |
| `live_confirmation.md` | 📄 Leaf Task | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
