# Architecture diet

## Context
The build of candler-v1's M6c leaf (`__roadmap__/candler-v1/.../surface/diet/m6c_diet.md`), between M6b (surface consolidation) and M7 (MCP server). It works from the plan in [`__reports__/diet/00-architecture_diet_plan_v0.md`](../../__reports__/diet/00-architecture_diet_plan_v0.md) and is judged against the README purpose paragraph as written in `254ff4a0`. This level fixes S1 and cuts the suite family by family; `shed/` removes dead and duplicated code and audits every claim end to end; `shed/fix/` repairs what the audit finds; `shed/fix/close/` closes out. Each leaf runs in its own worktree made by `scripts/worktree.py add` from the `feat/diet` tip, and is removed at merge. At most three leaves build at once (each worktree has its own `target/`); check `df -h /System/Volumes/Data` before each wave.

## Goal
A sealed run holds the bytes it names, and the suite is about 525 guards at the consumer interfaces instead of 836 tests.

## Pre-conditions
- [ ] `[cmd]` `git -C /Users/hacker/Documents/tmp/claude-worktrees/Candler/candler-diet merge-base --is-ancestor 1002cb82 feat/diet` succeeds
- [ ] `[nl]` The coordinator (for Eliott) has approved decisions K1 to K6 of the plan

## Success Gates
- ✅ `s1_seal_blobs` merged with its guard seen red on the tip
- ✅ The seven cull leaves merged; nextest about 525 (±40), each deletion listed with its covering guard or "structure"
- ✅ `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass on the merged level
- ✅ DIGEST and GOLDEN unchanged
- ✅ Spend for the level within its caps (about $27), measured from transcripts

## Status
```mermaid
graph TD
    s1_seal_blobs[s1_seal_blobs]:::done
    cull_schemas_contracts[cull_schemas_contracts]:::planned
    cull_isolation[cull_isolation]:::done
    cull_supervision[cull_supervision]:::done
    cull_plan_store[cull_plan_store]:::planned
    cull_stats_analyze[cull_stats_analyze]:::planned
    cull_import_eval[cull_import_eval]:::planned
    cull_cli_harness[cull_cli_harness]:::done
    shed[Shed and audit]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `s1_seal_blobs.md` | 📄 Leaf Task | ✅ Done |
| `cull_schemas_contracts.md` | 📄 Leaf Task | ⬜ Planned |
| `cull_isolation.md` | 📄 Leaf Task | ✅ Done |
| `cull_supervision.md` | 📄 Leaf Task | ✅ Done |
| `cull_plan_store.md` | 📄 Leaf Task | ⬜ Planned |
| `cull_stats_analyze.md` | 📄 Leaf Task | ⬜ Planned |
| `cull_import_eval.md` | 📄 Leaf Task | ⬜ Planned |
| `cull_cli_harness.md` | 📄 Leaf Task | ✅ Done |
| `shed/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
