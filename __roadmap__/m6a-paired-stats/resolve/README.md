# Resolve

## Context
Level L1 of `m6a-paired-stats`, after every L0 leaf is merged. `level_resolve` resolves each treatment level on the plan side (T3, T4), on top of `treatment_setting`; `blocking` builds the blocks and chooses each comparison's test from the battery, on top of the L0 engines. They share no file. `wired/` (L2) follows.

## Goal
A two-treatment plan resolves each level once and gives each cell its own level's facts and image; each comparison has its blocks and its pre-specified test.

## Pre-conditions
- [ ] [cmd] L0 is merged: `for l in exact_tests cmh resampling treatment_setting treatment_verdict arm_counts execution_order; do test -f __reports__/m6a-paired-stats/leaves/${l}_v0.md || exit 1; done`
- [ ] [cmd] STD, DIGEST and GOLDEN passed at the L0 merge (the coordinator's record in the campaign Progress table)
- [ ] [nl] The golden probe's answer (are eval tasks cells?) is recorded in the campaign Progress table

## Success Gates
- ⬜ [cmd] `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_run_plan::/) | test(/^guard_settings_out_of_cell::/) | test(/^guard_image::/) | test(/^guard_stats_blocking::/)'`
- ⬜ [cmd] STD, DIGEST, GOLDEN, LINUX and SUBJ pass at the L1 merge (campaign README, standard gates)
- ⬜ [nl] The `blocking` verifier closed with no open finding; the `level_resolve` verifier runs with `level_wiring`'s, at L2

## Status
```mermaid
graph TD
    level_resolve[Level resolve]:::planned
    blocking[Blocking]:::planned
    wired[Wired]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `level_resolve.md` | 📄 Leaf Task | ⬜ Planned |
| `blocking.md` | 📄 Leaf Task | ⬜ Planned |
| `wired/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
