# Surfaces

## Context
Depth 4. After both parity milestones and the wire alignment (M2c): the lean CLI (M3) and the session runner with the second harness (M4), in parallel. Both need only M2b; the tree also makes them wait for M2a. Amendment A3 adds a gate to M4: no harness named in the core.

## Goal
The CLI is lean and the runner covers long-lived sessions and a second harness.

## Pre-conditions
- [ ] `../../m2a_evaluation_parity.md` and `../../m2b_execution_parity.md` are `done`
- [ ] `../m2c_wire_alignment.md` is `done`

## Success Gates
- ⬜ `m3_lean_cli.md` and `m4_session_runner.md` are `done`

## Status
```mermaid
graph TD
    m3_lean_cli[M3 Lean CLI]:::planned
    m4_session_runner[M4 Session runner and second harness]:::planned
    isolation[Isolation]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `m3_lean_cli.md` | 📄 Leaf Task | ⬜ Planned |
| `m4_session_runner.md` | 📄 Leaf Task | ⬜ Planned |
| `isolation/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A3 | 2026-11-10 | `__reports__/candler-v1-review/01-gap_analysis_harness_env_v0.md` | [ ] | The core runs Claude Code's version command and names its gateway, config and init event, so a second driver could pass without the first leaving the core; M4 gains a gate and a harness deny-list |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
