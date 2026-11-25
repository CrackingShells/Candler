# M6c Integrity

## Context
Depth 9. M6c Integrity, one level below M6b and above M7, added by amendment A12 as the architecture diet and renamed by amendment A13. The diet's first half (`__roadmap__/diet/`) fixed S1, B1 and three silent acceptances and culled the suite; it landed as PR #57. Those were instances. The rest of M6c removes their causes, as planned in `__reports__/diet/04-architecture_cycle_plan_v2.md`: Candler state with a second producer in tests and fakes (D1), milestones cut by file and gated by counts (D3), an `AGENTS.md` read as a checklist (D4), and inputs with several readers (D5). Its build is the campaign `__roadmap__/integrity/`, on `feat/integrity`.

## Goal
The README's promises of provenance, per-run tool census, grading "already on disk" and thin surfaces over one API hold by construction: each kind of stored state has one producer, each consumer input has one reader, and an outside consumer's pass on the after tip finds no friction whose cause is D1 to D5.

## Pre-conditions
- [x] `../m6b_surface_consolidation.md` is `done` (PR #56)

## Success Gates
- ⬜ `m6c_integrity.md` and `mcp/` are `done`

## Status
```mermaid
graph TD
    mcp[MCP server]:::amendment
    m6c_integrity[M6c Integrity]:::amendment
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
| `m6c_integrity.md` | 📄 Leaf Task | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A12 | 2026-11-24 | `__reports__/diet/00-architecture_diet_plan_v0.md` (asked for by the milestone coordinator, for Eliott) | [ "m6c_diet" ] | The leaf of M6c: S1 fixed at the seal, the suite cut to about 525 guards, proven dead and duplicated code shed, and a RiVault audit per README claim |
| A13 | 2026-11-24 | `__reports__/diet/04-architecture_cycle_plan_v2.md` (Eliott's rulings, through the milestone coordinator) | [ ] | M6c is renamed Integrity: `m6c_diet.md` becomes `m6c_integrity.md`, and the leaf is rewritten to remove the causes D1, D3, D4 and D5 behind the diet's defects, gated by observations, with its build in `__roadmap__/integrity/` |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
