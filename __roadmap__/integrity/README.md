# M6c Integrity

## Context
The build of candler-v1's M6c leaf (`__roadmap__/candler-v1/…/m5b/stats/surface/integrity/m6c_integrity.md`), between M6b and M7. It works from [`__reports__/diet/04-architecture_cycle_plan_v2.md`](../../__reports__/diet/04-architecture_cycle_plan_v2.md) and removes the causes behind the diet's defects: Candler state with a second producer in tests and fakes (D1), leaves cut by file and gated by counts (D3), `AGENTS.md` read as a checklist (D4), and inputs with several readers (D5). It is judged against the README purpose paragraph. Depth gives the order: `conventions.md` first; `causes/` runs P1, P2, the P4 chain and P6 in parallel; `causes/checkpoint/` holds seam review 1 and the spend checkpoint that decides P5; `snapshot/` stores the catalog snapshot; `review/` holds seam review 2; `dogfood/` closes with an outside consumer's pass. The integration branch is `feat/integrity`, cut from `origin/main` at or after `70d4c3bf`; each leaf runs in its own worktree made by `scripts/worktree.py add` from the `feat/integrity` tip and removed at merge. Every build sets `CARGO_TARGET_DIR=/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-target/<leaf>` and `RUSTC_WRAPPER=sccache` in its own environment, one dir per leaf because `scripts/worktree.py` records that worktrees sharing a target dir link each other's code, with sccache sharing compiled dependencies; at most three builds run at once, each leaf's dir is removed at its merge, and `df -h /System/Volumes/Data` is checked before each level. Docker is used locally only by the dogfooding pass, which removes what it created by name; nothing is pruned and Docker is never stopped. Two pieces of v2 wait outside the caps: P3 (the stand-in that container-e2e uses, held to the daemon by a contract guard) returns by amendment if Eliott makes `Gates` and `container-e2e` required checks on `main`; P4·S (second_sources' `resolve` failure record and the transcript fold) is recorded as open and not done.

## Goal
Each kind of state Candler stores has one producer, each consumer input has one reader, and an outside consumer's pass on the after tip finds no friction whose cause is D1 to D5.

## Pre-conditions
- [x] `[cmd]` `git merge-base --is-ancestor 70d4c3bf origin/main` succeeds (PR #57 landed)
- [x] `[nl]` The coordinator accepted plan v2 with its cut order, and Eliott's rulings are applied in it
- [x] `[cmd]` `git merge-base --is-ancestor 70d4c3bf feat/integrity` succeeds

## Success Gates
- ⬜ `conventions.md` and `causes/` are `done`
- ⬜ Every merge into `feat/integrity` that touches the paths `container-e2e` watches waited for it to be green on its pull request, and every other merge says it touched none of them
- ⬜ Spend is measured from transcripts per row, no row passed its cap without stopping, and the whole milestone, coordinator included, stays within $100

## Status
```mermaid
graph TD
    conventions[Conventions]:::done
    causes[Causes]:::inprogress
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `conventions.md` | 📄 Leaf Task | ✅ Done |
| `causes/` | 📁 Directory | 🔄 In Progress |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A1 | 2026-11-24 | seam review 1 (F3/F4) | none; one gate added to `dogfooding_pass.md` | OpenCode has no agent-to-sealed-run chain in CI; the pass's real OpenCode run is where its stored outputs are compared with what the agent wrote, before any follow-up builds that chain |
| A2 | 2026-11-24 | maintainer ruling on seam review 1 (R1) | none; fixed under seam review 1 as leaf `exec_bit` | Every stored output keeps its execute bit, whatever produced it, so an evaluation plugin can run an output; the runner records the bit as import does, the 0555 materialise path stays, and the tree digest of a new run with an executable output changes |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
