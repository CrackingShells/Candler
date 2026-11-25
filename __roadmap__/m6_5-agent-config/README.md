# M6.5 Agent configuration as data

## Context
A new root after M6c Integrity (#81, on `origin/main` at `8d82f63b`). It is independent of M7, which is blocked, and of M8, which lies below M7; M8's plugin may later use hooks. The plan is [the architecture report, v2](../../__reports__/agent_config/02-architecture_agent_config_v2.md), which adds to the approved plan Eliott's rulings on the HTN lane's notice (v1) and on a live, trace-read proof (v2). Every leaf runs by its §Execution model: Sonnet implementers and verifiers, a shared codebase whose debt the leaf owns, findings kept in scope or escalated, target-dir slots `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, and an $85 cap. The status is mirrored in macOS Reminders.

## Goal
A consumer gives the experiment a global instruction file per harness, each arm its own workspace instruction file and hooks, all as cell values. Candler installs them where each harness looks, the workspace file read-only at the workspace root, and refuses a cell that the harness's own check rejects.

## Pre-conditions
- [x] `[cmd]` `git merge-base --is-ancestor 80405b5c origin/main` succeeds (M6c merged)
- [x] `[nl]` Eliott approved the plan on 2026-11-25: refuse errors and warnings, take contents inline, and make this a new root
- [x] `[nl]` Eliott ruled on the HTN lane's notice on 2026-11-25: instructions come in two scopes (a global file per harness, and a workspace-root file per arm on both harnesses), and evaluator access to them is deferred

## Success Gates
- ⬜ G1–G13 of the [architecture report, v2](../../__reports__/agent_config/02-architecture_agent_config_v2.md) hold on the tip of `feat/m6.5-agent-config`, each one seen failing on `8d82f63b`
- ⬜ `probe_harness_checks.md`, `declaration.md` and `installed/` are `done`
- ⬜ The spend measured from transcripts, coordinator included, is at most $75
- ⬜ Every finding left out of scope is filed under `__reports__/agent_config/` and escalated

## Status
```mermaid
graph TD
    probe_harness_checks[Probe harness checks]:::planned
    declaration[Declaration]:::planned
    installed[Installed]:::planned
    trace_context[trace_context]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `probe_harness_checks.md` | 📄 Leaf Task | ⬜ Planned |
| `declaration.md` | 📄 Leaf Task | ⬜ Planned |
| `installed/` | 📁 Directory | ⬜ Planned |
| `trace_context.md` | 📄 Leaf Task | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
