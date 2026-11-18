# Proven

## Context
Level L3 of `m6a-paired-stats`, the last one. It proves the battery built at L0 to L2: `golden_blocked` against a second engine on the 240 golden runs (htn-creator-eval 0.4.2), `known_truth` by simulation through the real `analyze`, and `design_record` writes the design and its citations into `__design__/architecture.md`. The three share no file.

## Goal
The blocked battery is shown correct on real data and on known truth, and its design is recorded with every validity source cited.

## Pre-conditions
- [ ] [cmd] L2 is merged: `for l in level_wiring analyze_output; do git log --format=%s | grep -q "^merge(.*): land $l" || exit 1; done`
- [ ] [cmd] STD, DIGEST and GOLDEN passed at the L2 merge (Progress table)

## Success Gates
- ⬜ [cmd] G2: `cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'test(/^guard_analyze_golden::blocked_/)'`
- ⬜ [cmd] G8: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_known_truth::/)'`
- ⬜ [cmd] G3: the citation loop of the campaign README passes
- ⬜ [cmd] STD, DIGEST, GOLDEN, LINUX and SUBJ pass at the L3 merge

## Status
```mermaid
graph TD
    golden_blocked[Golden blocked]:::planned
    known_truth[Known truth]:::planned
    design_record[Design record]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `golden_blocked.md` | 📄 Leaf Task | ⬜ Planned |
| `known_truth.md` | 📄 Leaf Task | ⬜ Planned |
| `design_record.md` | 📄 Leaf Task | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
