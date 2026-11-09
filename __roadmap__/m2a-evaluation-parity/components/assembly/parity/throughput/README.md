# Throughput: grade at machine speed

## Context
Amendment A1 (2026-11-09). golden_parity proved parity, but at 13 items/s: about 24 min per corpus pass. The profile `__reports__/m2a-evaluation-parity/throughput/00-regrade_profile_v0.md` traced the time to per-item requests, per-fork imports and `F_FULLFSYNC`, not to the Rust core. This node removes those costs in the core and helper (`regrade_throughput`) and in the HTN plug-in (`htn_preload`). `proof/` then re-proves parity at the new speed.

## Goal
Regrading the 240-run corpus runs at 100 items/s or more, with outcomes byte-identical to golden_parity's.

## Pre-conditions
- [ ] [cmd] `golden_parity` is merged on the first-parent history of `feat/m2a-evaluation-parity`
- [ ] [cmd] `htn-creator-eval@0.5.0` is tagged locally in `$R`

## Success Gates
- ⬜ [cmd] Both leaves are merged `--no-ff`: `regrade_throughput` on `feat/m2a-evaluation-parity`, `htn_preload` on `feat/candler-htn-plugin`. The local tag `htn-creator-eval@0.5.1` exists
- ⬜ [cmd] PERF12 at `--concurrency 4`: evaluate takes 12 s or less, import 1.5 s or less, and rows are equal to the reference on the four fields
- ⬜ [cmd] `proof/` gates pass

## Status
```mermaid
graph TD
    regrade_throughput[Regrade throughput in the core and helper]:::amendment
    htn_preload[HTN plug-in preloads its checker libraries]:::done
    proof[Throughput proof]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `regrade_throughput.md` | 📄 Leaf Task | 🔵 Amendment |
| `htn_preload.md` | 📄 Leaf Task | ✅ Done |
| `proof/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
