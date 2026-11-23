# Outputs and pages

## Context
Level L1 of the M6b build (`../README.md`). It runs after every L0 leaf is merged: `output_contract` needs `analysis_core`'s keys and `preflight_checks`' plan summary; `knowledge_pages` needs `codes_inner`'s Notes. It produces one output contract for every data verb and the generated configuration pages; `records/` builds on both.

## Goal
Every data verb prints its record as JSON by default with one `--format` option, and the configuration paths, configuration keys and exit codes each have one owner that the pages render.

## Pre-conditions
- [ ] [cmd] The four L0 leaves are merged: `for l in analysis_core codes_inner preflight_checks server_check; do test -f __reports__/m6b-surface-consolidation/build/${l}_v0.md || exit 1; done`
- [ ] [cmd] The disk has room: `df -h /System/Volumes/Data` shows at least 40 GiB available

## Success Gates
- ⬜ [cmd] Both leaves' `[cmd]` gates pass on the integration tip after both merges
- ⬜ [cmd] STD, DIGEST, LINUX, SUBJ and GOLDEN pass at this level's merge (build README, Execution model: L1 runs all five)
- ⬜ [nl] Both verifiers closed with no open finding; the coordinator ruled any DIGEST failure of a CLI output fixture with `output_contract`'s rows cited, recorded in the Progress table

## Gotchas
- **The race in this level.** `output_contract` and `knowledge_pages` both edit `crates/candler/src/main.rs`, `commands/mod.rs` and `commands/refusal.rs`. Merge `output_contract` first; `knowledge_pages` rebases onto it and touches only `EXIT_CODES`, `after_help` and the `config.no_store` row in those files. Both may regenerate `docs/contract/` pages: the second rebases and regenerates, never hand-merges a generated file.
- **`output_contract` is the large leaf** (cap 102 calls). If it reaches its cap, it commits what is green and lists what is left; the coordinator decides with the supervisor whether the rest waits or gets a second leaf, before `records/` starts, since `analysis_record` builds on its option.
- **Compatibility.** `output_contract` flips defaults on both streams: the supervisor's conditions are in its Success Gates. Any compatibility finding goes to the supervising session before the merge.

## Status
```mermaid
graph TD
    output_contract[One output contract]:::inprogress
    knowledge_pages[Knowledge pages]:::inprogress
    records[Records, docs and env]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `output_contract.md` | 📄 Leaf Task | 🔄 In Progress |
| `knowledge_pages.md` | 📄 Leaf Task | 🔄 In Progress |
| `records/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
