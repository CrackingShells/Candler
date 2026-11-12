# Closeout

## Context
L4 of `m4-session-runner`. Four independent leaves that need the whole of L3: the session controls (D4), the I1 harness deny-list (A3), the driver-traits contract page (A5) and the R01 record. They share no file.

## Goal
Session mode is refused for a harness version whose controls fail, the core cannot name a harness again, a driver author has a contract page, and R01 says what the code does.

## Pre-conditions
- [ ] [cmd] L3 merged: `for l in 'merge(runner): land session_runner' 'merge(runner): land opencode_wiring'; do git log --first-parent --format=%s feat/m4-session-runner | grep -qxF "$l" || echo "missing $l"; done` prints nothing

## Success Gates
- ⬜ [cmd] `session_controls.md`, `harness_guard.md`, `contract_docs.md` and `design_record.md` are `done`
- ⬜ [cmd] STD, LINUX, DIGEST and GOLDEN pass on the milestone tip

## Status
```mermaid
graph TD
    session_controls[Session controls]:::inprogress
    harness_guard[Harness guard]:::done
    contract_docs[Contract docs]:::inprogress
    design_record[Design record]:::inprogress
    revalidation[Revalidation]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `session_controls.md` | 📄 Leaf Task | 🔄 In Progress |
| `harness_guard.md` | 📄 Leaf Task | ✅ Done |
| `contract_docs.md` | 📄 Leaf Task | 🔄 In Progress |
| `design_record.md` | 📄 Leaf Task | 🔄 In Progress |
| `revalidation/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
