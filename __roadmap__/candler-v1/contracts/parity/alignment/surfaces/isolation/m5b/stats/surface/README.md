# Surface consolidation

## Context
Depth 8. Surface consolidation (M6b), one level below M6a and above M6c (amendment A12), added by amendment A11. It turns the M7 pre-study (`__reports__/m7-mcp-server/`) into approved responsibility verdicts and lands the accepted ones, so that M7 wraps a sanitised surface.

## Goal
Candler's consumer surface holds only what is Candler's responsibility, and the accepted consumability findings have landed.

## Pre-conditions
- [x] `../m6a_paired_stats.md` is `done` (PR #52, closed in #54)

## Success Gates
- ⬜ `m6b_surface_consolidation.md` and `diet/` are `done`

## Status
```mermaid
graph TD
    m6b_surface_consolidation[M6b Surface consolidation]:::done
    diet[Architecture diet]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `m6b_surface_consolidation.md` | 📄 Leaf Task | ✅ Done |
| `diet/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A11 | 2026-11-23 | `__reports__/m7-mcp-server/README.md` (round 02: Eliott's rulings on `01-findings_agent_consumability_v1.md`) | [ "m6b_surface_consolidation" ] | The leaf of M6b: every pre-study finding gets a verdict on responsibility, blast radius, the triad at three positions and idiom fit, approved before code |
| A12 | 2026-11-24 | `__reports__/diet/00-architecture_diet_plan_v0.md` (asked for by the milestone coordinator, for Eliott) | [ "diet" ] | An architecture diet (M6c) runs after M6b and before M7; `diet/` wraps `mcp/` |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
