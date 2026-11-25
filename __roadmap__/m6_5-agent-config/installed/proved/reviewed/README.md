# Reviewed

## Context
L4 of M6.5, the last level. A read-only, adversarial review of the merged milestone, followed by the closing report.

## Goal
A reviewer who did not build the milestone finds no gate that passes for the wrong reason, and no debt left behind.

## Pre-conditions
- [ ] `[cmd]` `git merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds, after L3 has merged

## Success Gates
- ⬜ `adversarial_review.md` and `meet_run_context.md` are `done`

## Status
```mermaid
graph TD
    adversarial_review[Adversarial review]:::planned
    meet_run_context[Meet run context]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `adversarial_review.md` | 📄 Leaf Task | ⬜ Planned |
| `meet_run_context.md` | 📄 Leaf Task | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
