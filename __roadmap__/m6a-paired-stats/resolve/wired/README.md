# Wired

## Context
Level L2 of `m6a-paired-stats`, after L1. `level_wiring` carries each cell's treatment level from the plan to the job, so a running cell mounts its own sources (T3 wiring, C3), and its verifier covers `level_resolve` too. `analyze_output` replaces today's one-factor pairs with the comparisons rule (C6) in `analyze`. They share no file. `proven/` (L3) follows.

## Goal
A two-treatment job runs each cell with its own treatment, and `analyze` reports one blocked comparison per declared factor and level pair.

## Pre-conditions
- [ ] [cmd] L1 is merged: `for l in level_resolve blocking; do test -f __reports__/m6a-paired-stats/leaves/${l}_v0.md || exit 1; done`
- [ ] [cmd] STD, DIGEST, GOLDEN, LINUX and SUBJ passed at the L1 merge (Progress table)

## Success Gates
- ⬜ [cmd] `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_job_orchestrate::/) | test(/^guard_job_spec_file::/) | test(/^guard_analysis_report::/) | test(/^guard_stats::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_cli_run::/) | test(/^guard_analyze/)'`
- ⬜ [cmd] STD, DIGEST and GOLDEN pass at the L2 merge
- ⬜ [nl] The `level_wiring` and `analyze_output` verifiers closed with no open finding

## Status
```mermaid
graph TD
    level_wiring[Level wiring]:::planned
    analyze_output[Analyze output]:::planned
    proven[Proven]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `level_wiring.md` | 📄 Leaf Task | ⬜ Planned |
| `analyze_output.md` | 📄 Leaf Task | ⬜ Planned |
| `proven/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
