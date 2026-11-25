# Proved

## Context
L3 of M6.5. It runs after both drivers install the values and the preflight step refuses. It proves the feature end to end on the real harnesses, against the scripted fake model, with no RiVault run.

## Goal
One experiment shows, on both harnesses, the global and the arm's own workspace instructions reaching the model, a consumer hook acting on a tool call, the census naming both loaded files, and an invalid payload refused before the run.

## Pre-conditions
- [ ] `[cmd]` `git merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds, after L2 has merged

## Success Gates
- ⬜ `e2e_guard.md` and `reviewed/` are `done`

## Status
```mermaid
graph TD
    e2e_guard[E2E guard]:::done
    reviewed[Reviewed]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `e2e_guard.md` | 📄 Leaf Task | ✅ Done |
| `reviewed/` | 📁 Directory | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
