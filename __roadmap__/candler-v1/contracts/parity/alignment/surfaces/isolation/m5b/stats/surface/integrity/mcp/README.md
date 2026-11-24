# MCP server

## Context
Depth 10. The MCP server (M7) and the agent skills that let agents use it, one level below M6c and above the second plug-in. M7 needs only M3; the tree also makes it wait for M5a, M5b, M6a, M6b and M6c. Moved here by amendment A6 of `../../../../../README.md`; amendment A11 moved it one level down, under `surface/`, and amendment A12 one more, under `diet/`, which amendment A13 renamed `integrity/`. Amendment A14 deprecated M7 pending a re-evaluation after M6c Integrity's dogfooding pass, and blocked its leaf.

## Goal
Agents drive Candler through MCP, and skills teach them to author plug-ins, drive runs and compute evals with it.

## Pre-conditions
- [ ] `../m6c_integrity.md` is `done`

## Success Gates
- ⬜ `m7_mcp_server.md` and `plugin/` are `done`

## Status
```mermaid
graph TD
    m7_mcp_server[M7 MCP server]:::blocked
    plugin[Second plug-in]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `m7_mcp_server.md` | 📄 Leaf Task | 🚫 Blocked |
| `plugin/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A6 | 2026-11-16 | `__reports__/candler-v1-review/02-findings_amendment_a6_v0.md` | [ ] | M7 moves here from `isolation/` and gains a gate: agent skills for authoring plug-ins, driving runs and computing evals through the MCP server, wanted before M8 |
| A11 | 2026-11-23 | `__reports__/m7-mcp-server/README.md` (round 02: Eliott's rulings on `01-findings_agent_consumability_v1.md`) | [ ] | M7 moves one level down, under `surface/`, after M6b; its gates are starting points that its plan refines from M6b's verdicts, and its skills are self-contained |
| A12 | 2026-11-24 | `__reports__/diet/00-architecture_diet_plan_v0.md` (asked for by the milestone coordinator, for Eliott) | [ ] | M7 moves one level down, under `diet/`, after M6c; the MCP server wraps the dieted surface |
| A14 | 2026-11-24 | `__reports__/diet/04-architecture_cycle_plan_v2.md` (milestone coordinator's ruling, after the M6c Integrity roadmap) | [ ] | M7 is deprecated as a whole and `m7_mcp_server.md` is blocked: M7, including its pre-study rulings (A11), is re-evaluated after M6c Integrity, when the P7 dogfooding pass has shown what an agent consuming Candler actually lacks. The packaging question (skills and server packaged with spawning-agent-plugins, an unpushed amendment that is dropped, not landed) is part of that re-evaluation. Left stranded, not restructured: `plugin/` (M8, whose pre-condition is M7 `done`), and with it this directory's gate and candler-v1's end |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| `m7_mcp_server.md` | -- | -- | blocked by A14: deprecated, re-evaluated after M6c Integrity's dogfooding pass |
