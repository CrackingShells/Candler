# Candler v1 — meta-roadmap

## Context
The milestone graph for implementing `__design__/architecture.md`, from the first wire schemas to a dogfoodable 0.x. Candler stays pre-1.0 for a long dogfooding period; no milestone freezes a contract. Each node is one milestone. Its detailed roadmap is not here: the milestone's own session, started by Eliott in plan mode with the milestone's reminder (list "Candler") and a link to its leaf, writes it as a separate campaign `__roadmap__/<milestone>/`.

## Reference Documents
- [R01 Architecture](../../__design__/architecture.md) — the design every milestone implements; updated together with the code when a decision changes
- [R02 Lane overlap findings](/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion/__reports__/candler_core/02-findings_lane_overlap_v0.md) — how the HTN phases interleave with the milestones (branch `feat/skill-htn-creator`)
- [R03 CLI defaults findings](/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion/__reports__/candler_core/02-findings_cli_defaults_v0.md) — today's CLI usage and its quick wins, input to M3
- [R04 Golden reference](/Users/hacker/Documents/src/External/PCfVW/agenticai4hpc26-artifact) — tag `candler-m2a-golden`, `campaign/20260806-cont-train/` (`REGRADE.md`, `PROVENANCE.json`, `expectations.jsonl`, 10,320 rows)
- [R05 waiting-on-processes eval design](/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion/.claude/skills/htn-creator-workspace/infrastructure/__reports__/waiting_on_processes_eval/01-architecture_eval_sandbox_v0.md) — the second domain, input to M4 and M8 (test plan `00-test_definition_v0.md` beside it)

## Goal
A dogfoodable 0.x, with execution and evaluation behind one API, HTN at golden parity, and a second plug-in that has exercised the contract.

## Pre-conditions
- [x] Architecture approved and committed as `__design__/architecture.md` (2026-10-28)
- [x] Golden reference frozen: tag `candler-m2a-golden` in `agenticai4hpc26-artifact`
- [x] `pro-htn-phase-6-golden` signed off by Eliott (gates M2a only; 2026-10-28)

## Success Gates
- ⬜ Every milestone leaf below is `done`
- ⬜ `__design__/architecture.md` describes what is built, and every decision it leaves open is a "Future features" item with its trigger

## Gotchas
- Depth is ordering, and the remaining work is strictly sequential: M5a, then M5b, M6a (paired statistics), then M7 (MCP server and agent skills), then M8 (the second plug-in), one directory level each. M7 needs only M3 and M6a only M2a, so the tree over-sequences them; the reminders' `after:` fields keep the exact edges, and a milestone may start early on them if Eliott says so (amendment A6); M5 is split into M5a and M5b (amendment A8).
- Ownership of shared pieces: M1 owns the `resolve` skeleton and the project config loader (plug-in directories, slot count, store root); M2a and M2b each register their preflight steps into it. In M2a `evaluate` is synchronous; M3 turns it into a job once M2b's job supervisor, slots and stray sweep exist. M3 owns the live `run` then `evaluate` flow and the `ingest`, `inspect` and `analyze` verbs. M2b owns the comparability diff and the stratum verdict that M4 and M5 rely on. M8 adds no core feature: a gap it finds goes to "Future features" in `__design__/architecture.md`, and a blocking gap gets its own core milestone, decided then.
- Each milestone leaf has the same three steps: plan (its campaign, approved by Eliott), build and land (on `feat/<milestone>`, one `--no-ff` merge on `main`), close out (knowledge-transfer report, this leaf marked `done`, reminder reconciliation proposed). The milestone's own plan decides how the work inside it is run.
- `dirtree-rdm` is `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh`; leaf paths passed to `add` need the `.md` suffix.
- Environment: Rust with local `cargo` (1.99); Python with local `uv` (Candler has no mamba env). Work in mcp-python-ingestion follows that repo: `mamba run -n forMpyIn` for its pytest, `uv run --directory .claude/skills/htn-creator-workspace/infrastructure htn-creator-eval …` for the eval front door, its own commit vocabulary.
- Nothing is pushed, in any repo, until Eliott says so. Publishing to crates.io or PyPI is Eliott's call.
- Reminder notes are append-only: changes go through the scheduling-work skill's `sw_write.py`, and only on Eliott's yes.
- A change that invalidates `__design__/architecture.md`, CONTRIBUTING or the README updates it in the same merge.

## Status
```mermaid
graph TD
    wire_schemas[Wire schemas: outcome record and catalog snapshot]:::done
    contracts[Contracts]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `wire_schemas.md` | 📄 Leaf Task | ✅ Done |
| `contracts/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A6 | 2026-11-16 | `__reports__/candler-v1-review/02-findings_amendment_a6_v0.md` | [ ] | Eliott's 2026-11-16 review: no 1.0 freeze in this campaign, the goal becomes a dogfoodable 0.x, and the order M5, M6a, M7, M8 is strict; the nodes are added in `contracts/parity/alignment/surfaces/isolation/` |
| A8 | 2026-11-16 | `__reports__/candler-v1-review/04-findings_amendment_a8_v0.md` | [ "m5b" ] | M5 is split into M5a (the leaf `m5_container_isolation.md`) and M5b (new `m5b/` in `contracts/parity/alignment/surfaces/isolation/`, wrapping `stats/`), after the M5 budget cap was reached; order M5a, M5b, M6a, M7, M8, one directory level each |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
