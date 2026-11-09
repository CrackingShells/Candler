# Jobs

## Context
Level 2 of M2b. It needs run_plan and run_exec. It produces the detached job supervisor, with its body injected (ruling D12), and the job orchestration around it: network, gateway, `max_parallel` and the job caps.

## Goal
Supervise a whole job: detach, lock, reap, map SIGTERM to `killed`, and drive the runs with the gateway up and the caps held.

## Pre-conditions
- [ ] [cmd] DEP(x) for run_plan, run_exec and cli_ps_kill_watch

## Success Gates
- ⬜ [cmd] DEP(x) for job_runtime and job_orchestrate
- ⬜ [cmd] STD and LINUX pass on `feat/m2b-execution-parity` after the last merge of this level
- ⬜ [nl] The level's Grit review and the job_runtime (adversarial) verifier report no open finding

## Status
```mermaid
graph TD
    job_runtime[Job runtime: detach, lock, reap]:::done
    job_orchestrate[Job orchestrate: gateway, caps]:::done
    cli[Run command]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `job_runtime.md` | 📄 Leaf Task | ✅ Done |
| `job_orchestrate.md` | 📄 Leaf Task | ✅ Done |
| `cli/` | 📁 Directory | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
