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
    output_contract[One output contract]:::done
    knowledge_pages[Knowledge pages]:::done
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
| `output_contract.md` | 📄 Leaf Task | ✅ Done |
| `knowledge_pages.md` | 📄 Leaf Task | ✅ Done |
| `records/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| output_contract | claude/m6b-build-output_contract | 16 | Merged f1b8b0b7. A6, A7. Every data verb prints its `candler-schemas` record as JSON by default and `--format text` renders it; refusals print `refusal/1` by default; `--error-format`, `watch --json` and `resolve --json` retire with no alias; N8 red on the tip, now a guard. C1 built and then withdrawn in new commits by Eliott's round 08 (`resolve` needed `run`'s setup and counted sealed runs as new). The step 3 commit is not green alone (accepted). Follow-ups: C1 with a read-only run-key count, `evaluate` and `resolve`-failure hand shapes, `watch --live` against `--format text`, `analyze` `text` (L2 `analysis_record`) |
| knowledge_pages | claude/m6b-build-knowledge_pages | 8 | Merged faa4511c. One `CONFIGURATION_PATHS` table feeds `is_setting`, `is_known_path`, the request keys and `check_keys`; `configuration-paths.md`, `config-file.md` and the index's exit codes are generated; `--help` prints `EXIT_CODES`. Verifier F1 (a misspelt per-run cap accepted, introduced here) fixed and guarded; F3, F4 closed. Follow-ups: F2 (value readers, the machine-wide `slots` check and `is_location` still name keys by literal; all predate L1), five path texts private in `paths.rs` (schemas crate not owned), `status` and `usage_failure` in `main.rs` still type 0/1/2 |
