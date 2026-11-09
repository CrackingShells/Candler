# Parity

## Context
Depth 2. M2a (evaluation) and M2b (execution) run in parallel after M1; each proves Candler matches today's `htn_creator_eval` on its half. They touch different crates and modules, except where both read M1's supervisor and store, which neither changes without the other's coordinator knowing. Amendment A1 adds `alignment/` after both: the wire alignment (M2c), then the surfaces. Amendment A2 adds a gate to M2c: no domain concept in the core, the binary or `assets/`.

## Goal
Candler reproduces today's grading exactly and today's runs equivalently.

## Pre-conditions
- [ ] `../m1_contracts.md` is `done` and merged on `main`

## Success Gates
- ⬜ `m2a_evaluation_parity.md` and `m2b_execution_parity.md` are `done`
- ⬜ `alignment/` is `done`

## Status
```mermaid
graph TD
    m2a_evaluation_parity[M2a Evaluation parity]:::planned
    m2b_execution_parity[M2b Execution parity]:::done
    alignment[Wire alignment]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `m2a_evaluation_parity.md` | 📄 Leaf Task | ⬜ Planned |
| `m2b_execution_parity.md` | 📄 Leaf Task | ✅ Done |
| `alignment/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A1 | 2026-11-09 | `__reports__/m2c-wire-alignment/gap_analysis_v0.md` | [ "alignment", "m2c_wire_alignment" ] | M2b's smoke found three gaps in the shared `experiment/1` wire type (no inputs, nested `[fixed]` flattened, no treatment source) that need one owner before M3 and M4 |
| A2 | 2026-11-09 | `__reports__/m2c-wire-alignment/gap_analysis_planner_v0.md` | [ ] | The core, the binary and `assets/` hold HTN-domain content (planner axis, env packages, `tier`, reference provenance names) that the I1 guard cannot see, so M2c gains a gate that moves it behind plug-in or experiment declarations and widens the guard |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
