# Paired statistics

## Context
Depth 7. Paired statistics (M6a), split from the old M6, one level below M5b and above M7. It needs only M2a and M3's statistics; the tree also makes it wait for M5a and M5b. Its gates run on the existing HTN golden runs. Added by amendment A6 of `../../README.md`; amendment A8 moved it one level down, under `m5b/`.

## Goal
`analyze` compares paired arms with the paired tests, and the pairing model is decided and recorded in the design.

## Pre-conditions
- [ ] `../../m5_container_isolation.md` (M5a) is `done` and `../README.md`'s M5b leaves are `done`

## Success Gates
- ⬜ `m6a_paired_stats.md` and `mcp/` are `done`

## Status
```mermaid
graph TD
    m6a_paired_stats[M6a Paired statistics]:::amendment
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
| `m6a_paired_stats.md` | 📄 Leaf Task | 🔵 Amendment |
| `mcp/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A6 | 2026-11-16 | `__reports__/candler-v1-review/02-findings_amendment_a6_v0.md` | [ "m6a_paired_stats" ] | M6 splits: its paired-statistics half becomes this leaf, one level below M5 |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
