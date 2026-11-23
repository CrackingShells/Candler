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
    knowledge_transfer[Knowledge transfer]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `knowledge_transfer.md` | 📄 Leaf Task | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| knowledge_transfer | claude/rdm-m6b-surface-consolidation-8c47a6 | 4 | Coordinator. 6d1e17d9 ticks the gates, 18c808f3 adds the build knowledge transfer, 4c1cb8e5 adds the R01 Future row for `env` across cells, and the close commit sets every build node done. Gates on 82d28def: STD, LINUX, SUBJ, DIGEST, GOLDEN; nextest 836/836 on the rerun after the `late.sh` flake |
