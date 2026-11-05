# Interfaces

## Context
Level 1 of the campaign. It builds on the L0 wire foundations, trace wire and golden oracle, and produces the execution and evaluation wire types, the `candler-harness` traits and the `resolve` skeleton: the contracts M2a and M2b consume. The level closes with the api_review verifier; `services/` is not dispatched before its findings are resolved.

## Goal
The L1 contracts land and pass a contract-fit review against their consumers before any service is built on them.

## Pre-conditions
- ✅ [cmd] The L0 leaves are landed: `for l in wire_foundations wire_trace golden_oracle; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`

## Success Gates
- ✅ [cmd] The four L1 leaves are landed: `for l in wire_execution wire_evaluation harness_traits resolve_config; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`
- ✅ [cmd] DRIFT and `cargo nextest run --workspace -E 'binary(public_surface)'` pass on `feat/m1-contracts`
- ✅ [nl] The api_review verifier reports no open finding on critical conditions 1–5, the `public_surface.rs` files (usage only) and the fit of the L0 and L1 wire types to R01, R07, the OpenCode facts, `run_config` and `eval.json`

## Status
```mermaid
graph TD
    wire_execution[Wire execution: experiment, manifest, job]:::done
    wire_evaluation[Wire evaluation: plug-ins, suites, protocol]:::done
    harness_traits[Harness traits]:::done
    resolve_config[Resolve and config]:::done
    services[Services]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `wire_execution.md` | 📄 Leaf Task | ✅ Done |
| `wire_evaluation.md` | 📄 Leaf Task | ✅ Done |
| `harness_traits.md` | 📄 Leaf Task | ✅ Done |
| `resolve_config.md` | 📄 Leaf Task | ✅ Done |
| `services/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
