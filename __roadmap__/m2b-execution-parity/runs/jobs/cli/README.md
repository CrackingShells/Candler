# Run command

## Context
Level 3 of M2b. It needs job_runtime and job_orchestrate, because `candler run` composes `supervise_entry` with `orchestrate` (ruling D12). It produces the user-facing `candler run` and the resolver composition.

## Goal
Ship `candler run`: resolve and preflight at the terminal, hand the spec to a detached supervisor, return at once.

## Pre-conditions
- [ ] [cmd] DEP(x) for job_runtime and job_orchestrate

## Success Gates
- ⬜ [cmd] DEP(x) for cli_run
- ⬜ [cmd] STD and LINUX pass on `feat/m2b-execution-parity` after the merge
- ⬜ [nl] The level's Grit review reports no open finding

## Status
```mermaid
graph TD
    cli_run[CLI: run]:::done
    closeout[Closeout]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `cli_run.md` | 📄 Leaf Task | ✅ Done |
| `closeout/` | 📁 Directory | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
