# Paired statistics

## Context
Depth 7. Paired statistics (M6a), split from the old M6, one level below M5b and above M7. It needs only M2a and M3's statistics; the tree also makes it wait for M5a and M5b. Its gates run on the existing HTN golden runs. Added by amendment A6 of `../../README.md`; amendment A8 moved it one level down, under `m5b/`. Amendment A7 adds treatment arms: one experiment can declare the treatment as a factor, with a treatment, env lock and image per cell.

## Goal
`analyze` compares paired arms with the paired tests, an arm can be a treatment, and the pairing model is decided and recorded in the design.

## Pre-conditions
- [x] `../../m5_container_isolation.md` (M5a) is `done` and `../README.md`'s M5b leaves are `done` (M5a: PR #36; M5b's own gates: PR #38, merge 1e6dc91; its last gate is this level)

## Success Gates
- ⬜ `m6a_paired_stats.md` and `mcp/` are `done`

## Status
```mermaid
graph TD
    m6a_paired_stats[M6a Paired statistics]:::inprogress
    mcp[MCP server]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `m6a_paired_stats.md` | 📄 Leaf Task | 🔄 In Progress |
| `mcp/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A6 | 2026-11-16 | `__reports__/candler-v1-review/02-findings_amendment_a6_v0.md` | [ "m6a_paired_stats" ] | M6 splits: its paired-statistics half becomes this leaf, one level below M5 |
| A7 | 2026-11-16 | `__reports__/candler-v1-review/03-findings_amendment_a7_v0.md` | [ ] | M6a gains treatment as a declared factor, with a treatment, env lock and image per cell, so the paired tests can compare treatments |
| A9 | 2026-11-18 | `m6a-approved-plan.md` (notes folder of session local_7f577b41), Eliott's approval of the round-4 plan | [ ] | Runs are not paired, so McNemar is dropped and "paired" becomes "blocked": blocks come from the other declared factors, Fisher becomes its mid-p, CMH gets an exact fallback, the bootstrap becomes a stratified permutation test with BCa, Holm replaces fixed-sequence |
| A10 | 2026-11-18 | `m6a-l3-addendum.md` (notes folder of session local_7f577b41), Eliott's approval of the L3 reshape | [ ] | The golden runs are the trace of a design to rerun: gate 1 drops its golden-runs clause, a new gate asks that the golden experiment plans as one file, analyses each factor blocked by the other and imports with the same factors, and gate 5 treats treatment arms like any other factor and keeps today's output only for a two-level one-factor binary experiment |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
