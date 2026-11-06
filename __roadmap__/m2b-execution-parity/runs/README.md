# Runs

## Context
Level 1 of M2b. It needs every L0 leaf merged: the job store, comparability, the isolation leaves, resolution, the driver and the run evidence. It produces planning, one guarded run, and the `ps`, `kill` and `watch` verbs.

## Goal
Plan runs without losing data, run one cell under the container and classify its end, and inspect or stop jobs from the CLI.

## Pre-conditions
- [ ] [cmd] DEP(x) for every L0 leaf: job_store, comparability, image, container_run, container_stop, gateway, experiment_resolution, claude_code_driver, run_evidence

## Success Gates
- ⬜ [cmd] DEP(x) for run_plan, run_exec and cli_ps_kill_watch
- ⬜ [cmd] STD and LINUX pass on `feat/m2b-execution-parity` after the last merge of this level
- ⬜ [nl] The level's Grit review and the run_plan (adversarial) and run_exec (plain, ends table) verifiers report no open finding

## Status
```mermaid
graph TD
    run_plan[Run plan: keys, attempts, resume]:::planned
    run_exec[Run exec: spawn, ends, seal]:::planned
    cli_ps_kill_watch[CLI: ps, kill, watch]:::planned
    jobs[Jobs]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `run_plan.md` | 📄 Leaf Task | ⬜ Planned |
| `run_exec.md` | 📄 Leaf Task | ⬜ Planned |
| `cli_ps_kill_watch.md` | 📄 Leaf Task | ⬜ Planned |
| `jobs/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
