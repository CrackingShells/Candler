# Assembly

## Context
Level L2 of M2a. It runs after every L1 component has landed. `engine` composes the registry, run context, protocol client and selection behind the frozen `engine::evaluate` seam. `surfaces` exposes the engine and the diff as `candler import | evaluate | show | diff`. `helper_release` makes the Python helper releasable. `htn_plugin` packages the HTN kinds as a Candler plug-in, with its environment, lock and suite. `parity/` proves the whole.

## Goal
A Candler binary can import a corpus, regrade it with a real plug-in environment, and show and diff the result. The helper and the HTN plug-in are each release-shaped, with every L2 guard of R04 in place.

## Pre-conditions
- [ ] [cmd] `for l in reference_oracle registry run_context protocol_client selection_rates; do git log --first-parent --format=%s feat/m2a-evaluation-parity | grep -qx "merge($(case $l in registry) echo registry;; protocol_client) echo evaluator-protocol;; *) echo evaluation;; esac)): land $l" || exit 1; done`
- [ ] [cmd] `git log --first-parent --format=%s feat/m2a-evaluation-parity | grep -qx 'merge(evaluator-helper): land helper_runtime'`
- [ ] [cmd] `git -C "$RW" log --first-parent --format=%s feat/candler-htn-plugin | grep -qx 'merge(htn-eval): land htn_kinds'`

## Success Gates
- ⬜ [cmd] STD, DRIFT, SUBJ, PYSTD and HTNSTD pass after the last L2 merge
- ⬜ [cmd] SPAWN, then `cargo nextest run -p candler-core -E 'binary(guard_engine)' && cargo nextest run -p candler -E 'binary(guard_cli)'` pass on the integration tip
- ⬜ [cmd] Every L2 leaf's `merge(<scope>): land <leaf>` is on the first-parent history of its integration branch
- ⬜ [nl] The `htn_plugin` adversarial verifier holds no open BLOCKER or MAJOR

## Status
```mermaid
graph TD
    engine[Evaluation engine]:::done
    surfaces[CLI surfaces: import, evaluate, show, diff]:::done
    helper_release[Helper release: models, version, CI]:::done
    htn_plugin[HTN plug-in: manifest, env, suite]:::done
    parity[Parity]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `engine.md` | 📄 Leaf Task | ✅ Done |
| `surfaces.md` | 📄 Leaf Task | ✅ Done |
| `helper_release.md` | 📄 Leaf Task | ✅ Done |
| `htn_plugin.md` | 📄 Leaf Task | ✅ Done |
| `parity/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
