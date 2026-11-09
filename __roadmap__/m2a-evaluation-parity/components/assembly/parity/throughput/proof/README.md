# Throughput proof

## Context
Last node of A1. It runs after `regrade_throughput` and `htn_preload` have landed, and replays golden_parity's corpus guard against `htn-creator-eval@0.5.1`. It also carries golden_parity's deferred adversarial verifier.

## Goal
GP1–GP3 pass against 0.5.1 within the throughput budget, and no verifier holds an open BLOCKER or MAJOR.

## Pre-conditions
- [ ] [cmd] `regrade_throughput` and `htn_preload` are merged, and the tag `htn-creator-eval@0.5.1` exists locally in `$R`

## Success Gates
- ⬜ [cmd] The `throughput_proof` gates pass: import 30 s or less, each regrade 89 s or less, the whole guard 5 min or less
- ⬜ [nl] The adversarial verifier holds no open BLOCKER or MAJOR; proxy: its report in the leaf report

## Status
```mermaid
graph TD
    throughput_proof[Prove parity and throughput on the corpus]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `throughput_proof.md` | 📄 Leaf Task | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
