# Core

## Context
L2 of `m4-session-runner`. It consumes the traits of L1. `harness_out_of_core` moves every Claude Code fact out of `candler-core` and adds the driver registry to the binary (amendment A3); `opencode_driver` builds the second driver crate against the same traits. They share no file. L3 builds on the reshaped run loop.

## Goal
The core asks the driver for every harness fact, the binary selects drivers by name, and the OpenCode driver passes the shared fixture suite.

## Pre-conditions
- [ ] [cmd] L1 merged: `git log --first-parent --format=%s feat/m4-session-runner | grep -qxF 'merge(runner): land driver_traits'`
- [ ] [nl] The coordination session ruled on D10 (an unknown `request.harness` is refused) before `harness_out_of_core` merges

## Success Gates
- ⬜ [cmd] `harness_out_of_core.md` and `opencode_driver.md` are `done`
- ⬜ [cmd] STD, LINUX, DIGEST and GOLDEN pass on the milestone tip after the level's last merge

## Status
```mermaid
graph TD
    harness_out_of_core[Harness out of core]:::planned
    opencode_driver[OpenCode driver]:::planned
    runs[Runs]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `harness_out_of_core.md` | 📄 Leaf Task | ⬜ Planned |
| `opencode_driver.md` | 📄 Leaf Task | ⬜ Planned |
| `runs/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
