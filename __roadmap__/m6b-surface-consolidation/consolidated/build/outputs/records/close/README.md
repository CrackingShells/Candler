# Close

## Context
Level L3 of the M6b build (`../../../README.md`), the close. It runs after every L2 leaf is merged, and the coordinator does it itself. It produces the knowledge-transfer report with the HTN lane's migration note, and leaves the machine clean.

## Goal
The build is shown green on the integration tip, recorded, cleaned up and handed to the supervising session for its PR.

## Pre-conditions
- [ ] [cmd] The three L2 leaves are merged: `for l in analysis_record docs_chores experiment_env; do test -f __reports__/m6b-surface-consolidation/build/${l}_v0.md || exit 1; done`

## Success Gates
- ⬜ [cmd] `knowledge_transfer`'s `[cmd]` gates pass
- ⬜ [nl] The supervising session has the note: the integration tip, the report, the migration note to relay at merge

## Gotchas
- **Nothing is pushed.** The supervising session owns the PR to `main` and relays the migration note to the HTN lane's coordinator at merge.
- **A red closing gate is not fixed here.** It goes back to the leaf whose change it traces to (`knowledge_transfer`, Step 1).
- **Clean up only what the build made.** Worktrees, branches, colgrep indexes, scratch roots, and Docker containers or images a leaf built, each removed by name or id. Docker is never stopped or pruned.

## Status
```mermaid
graph TD
    knowledge_transfer[Knowledge transfer]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `knowledge_transfer.md` | 📄 Leaf Task | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
