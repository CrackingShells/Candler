# Review

## Context
Level 4 of M6d, the last. An adversarial reviewer who did not build any leaf reads the whole `feat/run-context` diff against the README purpose paragraph, the codebase's idioms and waste, before the integration pull request is proposed to the maintainer.

## Goal
The whole diff is judged by someone with no stake in it, and every finding is fixed or ruled on before the pull request into `main`.

## Pre-conditions
- [ ] `[nl]` `../graded_run_sees_trace.md` is `done` and merged into `feat/run-context`

## Success Gates
- ⬜ `adversarial_review.md` is `done`

## Status
```mermaid
graph TD
    adversarial_review[Adversarial review]:::planned
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

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
