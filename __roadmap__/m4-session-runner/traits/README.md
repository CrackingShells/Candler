# Traits

## Context
L1 of `m4-session-runner`. It runs once both probes have recorded what Claude Code does in a session and what OpenCode V2 does at all, so that the driver traits are designed from two harnesses rather than one. It produces the trait delta (D9), the Claude Code driver's implementation of it and the shared driver fixture suite; L2 consumes them.

## Goal
The driver traits can express everything both harnesses need, and the Claude Code driver passes the shared fixture suite.

## Pre-conditions
- [ ] [cmd] L0 merged: `for l in 'merge(adapter-opencode): land opencode_facts' 'merge(runner): land session_facts' 'merge(runner): land type_merges' 'merge(manifest): land imported_strata'; do git log --first-parent --format=%s feat/m4-session-runner | grep -qxF "$l" || echo "missing $l"; done` prints nothing
- [ ] [nl] D2's branch is settled: `session_facts` reports where the pending list comes from (the Stop payload or stream events), or Eliott ruled

## Success Gates
- ⬜ [cmd] `driver_traits.md` is `done` and its gates hold on the milestone tip
- ⬜ [cmd] STD and DIGEST pass on the milestone tip after the merge

## Status
```mermaid
graph TD
    driver_traits[Driver traits]:::done
    core[Core]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `driver_traits.md` | 📄 Leaf Task | ✅ Done |
| `core/` | 📁 Directory | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
