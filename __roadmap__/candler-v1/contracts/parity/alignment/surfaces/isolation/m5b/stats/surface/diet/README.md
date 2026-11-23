# Architecture diet

## Context
Depth 9. The architecture diet (M6c), one level below M6b and above M7, added by amendment A12. After M6b consolidated the consumer surface and before an MCP server wraps it, the diet restores integrity and function: it fixes S1, cuts the suite to guards at the consumer interfaces, sheds what is proven dead or duplicated, and judges each claim of the README purpose paragraph from real runs. Its build is the campaign `__roadmap__/diet/`.

## Goal
Every claim of the README purpose paragraph is shown working end to end, or recorded as broken or not built, and the suite and code carry nothing the next agent would not want to inherit.

## Pre-conditions
- [ ] `../m6b_surface_consolidation.md` is `done` (PR #56)

## Success Gates
- ⬜ `m6c_diet.md` and `mcp/` are `done`

## Status
```mermaid
graph TD
    mcp[MCP server]:::amendment
    m6c_diet[M6c Architecture diet]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `mcp/` | 📁 Directory | 🔵 Amendment |
| `m6c_diet.md` | 📄 Leaf Task | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A12 | 2026-11-24 | `__reports__/diet/00-architecture_diet_plan_v0.md` (asked for by the milestone coordinator, for Eliott) | [ "m6c_diet" ] | The leaf of M6c: S1 fixed at the seal, the suite cut to about 525 guards, proven dead and duplicated code shed, and a RiVault audit per README claim |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
