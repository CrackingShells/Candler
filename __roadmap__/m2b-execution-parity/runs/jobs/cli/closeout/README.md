# Closeout

## Context
Level 4 of M2b, the last. It needs `candler run`. It proves the supervision gates on real Docker (B36) and runs the equivalence smoke through RiVault, which checks every feature_map row.

## Goal
Prove that nothing outlives a killed supervisor, and that the container runner keeps every feature of sandbox contract v5.

## Pre-conditions
- [ ] [cmd] DEP(cli_run)
- [ ] [cmd] `docker version --format '{{.Server.Version}}'` prints a version

## Success Gates
- ⬜ [cmd] DEP(x) for supervision_gates and equivalence_smoke
- ⬜ [cmd] The e2e suite passes on macOS, and the Linux half is recorded from the `container-e2e` CI run (never as passed before it runs)
- ⬜ [nl] The supervision_gates (narrowed) verifier reports no open finding, and the smoke report records every feature_map row

## Status
```mermaid
graph TD
    supervision_gates[Supervision gates]:::planned
    equivalence_smoke[Equivalence smoke]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `supervision_gates.md` | 📄 Leaf Task | ⬜ Planned |
| `equivalence_smoke.md` | 📄 Leaf Task | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
