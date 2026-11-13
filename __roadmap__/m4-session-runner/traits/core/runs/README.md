# Runs

## Context
L3 of `m4-session-runner`. Both leaves edit the run loop that L2 reshaped, so they come after it; they share two hunks, which the coordinator merges (`runner/exec/mod.rs`, `isolation/container/mod.rs`). `session_runner` builds session mode; `opencode_wiring` runs the export command, reads it offline and stops OpenCode from inside its container.

## Goal
A session cell runs with stdin open and closes by the conservative rule, and an OpenCode run yields a trace whether it exits or is stopped.

## Pre-conditions
- [ ] [cmd] L2 merged: `for l in 'merge(isolation): land harness_out_of_core' 'merge(adapter-opencode): land opencode_driver' 'merge(adapter-opencode): land opencode_settings'; do git log --first-parent --format=%s feat/m4-session-runner | grep -qxF "$l" || echo "missing $l"; done` prints nothing

## Success Gates
- ⬜ [cmd] `session_runner.md` and `opencode_wiring.md` are `done`
- ⬜ [cmd] STD, LINUX, DIGEST and GOLDEN pass on the milestone tip; guards S4 and W1 pass in CI's `container-e2e` job and locally with Docker

## Status
```mermaid
graph TD
    session_runner[Session runner]:::done
    opencode_wiring[OpenCode wiring]:::done
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
| `session_runner.md` | 📄 Leaf Task | ✅ Done |
| `opencode_wiring.md` | 📄 Leaf Task | ✅ Done |
| `closeout/` | 📁 Directory | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
