# Consumer path

## Context
Level 3 of M6d, after every `readers/` leaf merged. One end-to-end guard asks, from inside Candler's suite, the question htn-creator-eval's probe P0 asks: does a real agent run, graded through `candler evaluate`, show its plug-in each shell call's command and exit and each MCP call's origin? `review/` follows.

## Goal
The consumer path (run, seal, evaluate with `needs.trace`) is seen working on both real harnesses in `container-e2e`, and seen failing without the trace.

## Pre-conditions
- [ ] `[nl]` `cc_reader_exit.md`, `oc_reader_exit.md`, `serve_context.md` and `helper_context.md` are `done` and merged into `feat/run-context`

## Success Gates
- ⬜ `graded_run_sees_trace.md` and `review/` are `done`

## Status
```mermaid
graph TD
    graded_run_sees_trace[Graded run sees trace]:::inprogress
    review[Review]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `graded_run_sees_trace.md` | 📄 Leaf Task | 🔄 In Progress |
| `review/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
