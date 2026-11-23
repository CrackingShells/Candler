# Records, docs and env

## Context
Level L2 of the M6b build (`../../README.md`). It runs after `outputs/` is merged: `analysis_record` cuts the analysis record once, after `analysis_core` and `output_contract`; `experiment_env` and `docs_chores` follow `knowledge_pages`. It produces the analysis record and a run's transcript, the experiment `env` table, and the docs that state the ruled design.

## Goal
`analyze` prints `analysis-report/1` with the ruled names, an experiment can set its harness's environment, and R01, `docs/statistics.md` and `README.md` say what Candler is now.

## Pre-conditions
- [ ] [cmd] Both L1 leaves are merged: `for l in output_contract knowledge_pages; do test -f __reports__/m6b-surface-consolidation/build/${l}_v0.md || exit 1; done`
- [x] [nl] Eliott fixed the new word for the comparability sense of "stratum": "comparability class", key `comparability_class` (2026-11-23, round 06), written into `analysis_record`'s pre-condition
- [ ] [cmd] The disk has room: `df -h /System/Volumes/Data` shows at least 40 GiB available

## Success Gates
- ⬜ [cmd] The three leaves' `[cmd]` gates pass on the integration tip after all three merges
- ⬜ [cmd] STD and DIGEST pass at this level's merge
- ⬜ [nl] Both verifiers (`analysis_record`, `experiment_env`) closed with no open finding; the coordinator read `docs_chores`' diff at integration (it has no verifier)

## Gotchas
- **The race in this level.** `analysis_record` and `experiment_env` both edit `crates/candler-schemas/src/lib.rs`, `generate.rs`, `docs/contract/codes.toml` (append-only) and the generated pages. Merge `analysis_record` first; `experiment_env` rebases, appends its rows after the other's, and regenerates schemas and pages. Neither hand-merges a generated file.
- **One word, two leaves.** `analysis_record` renames the comparability sense in code and in the record; `docs_chores` renames it in R01 and `docs/statistics.md`. Both use the word in `analysis_record`'s pre-condition. `blocking.rs`'s `Stratum` (a block) keeps its name, and refusal codes keep theirs.
- **R01 is shared in time, not in this level.** `output_contract` (L1) edited R01 §CLI surface; `docs_chores` edits the rest of R01, and the line numbers it cites are as of `a0df4d48`.
- **Two `schemas@` changes.** `analysis_record` adds a record and `experiment_env` adds an optional field; each lands in a commit that touches `crates/candler-schemas/` only. CI cuts the release after the supervisor's PR merges.

## Status
```mermaid
graph TD
    analysis_record[Analysis record]:::done
    docs_chores[Docs chores]:::done
    experiment_env[Experiment env table]:::done
    close[Close]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `analysis_record.md` | 📄 Leaf Task | ✅ Done |
| `docs_chores.md` | 📄 Leaf Task | ✅ Done |
| `experiment_env.md` | 📄 Leaf Task | ✅ Done |
| `close/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| docs_chores | claude/m6b-build-docs_chores | 8 | Merged 096d8959. Seven leaf commits plus the coordinator's 0f76e1a2 (the comparability class across R01). R01 gains the responsibility record and the ten Future rows; README is the store window; X8 no longer promises paired tests. Follow-ups: the quick start's `resolve` step has no offline fixture (accepted), R01's `interval_reading` named in §Readings only |
| analysis_record | claude/m6b-build-analysis_record | 8 | Merged 40aaef40. A10. `analyze` prints `analysis-report/1`, text and CSV render it; `stratum` (comparability) is `comparability_class`, `reading` is `interval_reading`; `inspect <run> --transcript`. Verifier closed after one re-check. Follow-ups: no CLI test of `--transcript`, no text-content guard, `build.rs:66` wording, `text_line` degrades silently |
| experiment_env | claude/m6b-build-experiment_env | 14 | Merged 82d28def. A8, A9, A11. `[env]` reaches the harness and enters the cell digest; `resolve` refuses reserved, secret, invalid names and `env` outside the table (`experiment.env_source`). Verifier closed. Follow-ups: driver and gateway names, varying `env` across cells (Future), `probe_env`'s literal `HOME`, `secret_names` default |
