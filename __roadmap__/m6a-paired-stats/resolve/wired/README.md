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
    level_wiring[Level wiring]:::done
    analyze_output[Analyze output]:::done
    proven[Proven]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `level_wiring.md` | 📄 Leaf Task | ✅ Done |
| `analyze_output.md` | 📄 Leaf Task | ✅ Done |
| `proven/` | 📁 Directory | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| `level_wiring.md` | m6a/level_wiring | 10 | merged 637ef2d3. Probe: the treatment commit IS in the cell digest (R01 holds; the L1 handover was wrong). Equal levels refused naming both (t3_equal_levels_are_refused, existing PlanError::Cell/Document code). Table-level syntax on request.treatment (A7); two levels plan and run through the binary (t3_cli_*). T4 rows green on base (bite unreachable). record_packages per level unguarded (PackageReading non_exhaustive). Verifier closed, 3 rounds |
| `analyze_output.md` | m6a/analyze_output | 9 | merged 57cffaa2. a1_ and a2_ guards changed under A6, listed in the report and merge body; t5_ fixtures respelled request.treatment. Treatment keyed by its typed reference. PERMUTATION_SEED fixed. No analyze_c6_ vector files (rows use midp/newcombe/cmh). level_of reads an object under a prefix factor, unguarded. Verifier closed. L2 close at 57cffaa2: STD, DIGEST, GOLDEN, SUBJ and the level gate pass |
