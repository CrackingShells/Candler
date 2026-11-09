# Components

## Context
Level L1 of M2a. It runs after `skeleton` and `protocol_wire` have landed. It builds seven independent pieces in parallel: the reference oracle, the HTN kinds, the Python helper runtime, the registry, the run context, the protocol client, and selection with rates and the generic diff. Each fills only its own module. `assembly/` composes them.

## Goal
Every L1 component is built and guarded by its R04 cornerstone tests on its own branch, and lands on `feat/m2a-evaluation-parity`, or on `feat/candler-htn-plugin` for `htn_kinds`, with one `--no-ff` merge per leaf.

## Pre-conditions
- [ ] [cmd] `git log --first-parent --format=%s feat/m2a-evaluation-parity | grep -qx 'merge(evaluation): land skeleton'`
- [ ] [cmd] `git log --first-parent --format=%s feat/m2a-evaluation-parity | grep -qx 'merge(evaluator-protocol): land protocol_wire'`
- [ ] [nl] The skeleton API review is closed with no open finding (proxy: its report in `__reports__/m2a-evaluation-parity/skeleton/`)

## Success Gates
- ⬜ [cmd] STD, DRIFT and SUBJ pass on `feat/m2a-evaluation-parity` after the last L1 merge, PYSTD passes in `$HELPER`, and HTNSTD passes on `feat/candler-htn-plugin`
- ⬜ [cmd] Every L1 leaf's `merge(<scope>): land <leaf>` is on the first-parent history of its integration branch
- ⬜ [nl] The verifiers of `reference_oracle`, `htn_kinds`, `helper_runtime`, `run_context` and `protocol_client` hold no open BLOCKER or MAJOR. The L0+L1 Grit review is closed.

## Status
```mermaid
graph TD
    reference_oracle[Reference oracle: A-C expected rows and rates]:::done
    htn_kinds[HTN kinds: adapted grader functions]:::done
    helper_runtime[Helper runtime: Python evaluator loop]:::done
    registry[Registry: manifests, suites, environments]:::done
    run_context[Run context: assembly, digest, materialiser]:::done
    protocol_client[Protocol client over supervise]:::done
    selection_rates[Selection, I4, rates and generic diff]:::done
    assembly[Assembly]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `reference_oracle.md` | 📄 Leaf Task | ✅ Done |
| `htn_kinds.md` | 📄 Leaf Task | ✅ Done |
| `helper_runtime.md` | 📄 Leaf Task | ✅ Done |
| `registry.md` | 📄 Leaf Task | ✅ Done |
| `run_context.md` | 📄 Leaf Task | ✅ Done |
| `protocol_client.md` | 📄 Leaf Task | ✅ Done |
| `selection_rates.md` | 📄 Leaf Task | ✅ Done |
| `assembly/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
