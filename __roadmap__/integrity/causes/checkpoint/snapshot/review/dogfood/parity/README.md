# Harness parity

## Context
Level 3 of M6c, opened by amendment A3 after the dogfooding pass found that a run's record names no MCP server under OpenCode and that the stored catalog always says `servers: []` (`__reports__/diet/05-findings_dogfooding_pass_v0.md`, finding (1)). The plan is `__reports__/diet/07-plan_integrity_level3_v0.md`. Claude Code is the reference harness: OpenCode is aligned to it, and a defect found on the Claude Code side is fixed in both drivers. Depth gives the order: `declaration.md` first; `answered/` renders, records and documents from the declaration; `answered/recorded/` removes the second catalog producer and proves parity; `closing/` holds seam review 3.

## Goal
Every setting that reaches the agent is answered by each registered driver from one typed declaration, and one cell run on either harness records the MCP servers and tools its agent had.

## Pre-conditions
- [x] `[cmd]` `git merge-base --is-ancestor 27d1d1dd origin/feat/integrity` succeeds (the dogfooding pass is merged)
- [x] `[nl]` Eliott approved the level-3 plan, with its rulings: the README says "per-run tool census", and `codemode` defaults to `false`

## Success Gates
- ⬜ `declaration.md` and `answered/` are `done`
- ⬜ The level's spend, measured from transcripts per row, stays within $70, and no row passed its cap without stopping
- ⬜ Every leaf report has a "What it leaves" section judging the change at the three levels (how it was developed, what it leaves in the code, how it is consumed), and seam review 3 found no second producer, second reader or passthrough left by the level

## Status
```mermaid
graph TD
    declaration[Settings declaration]:::amendment
    answered[Answered]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `declaration.md` | 📄 Leaf Task | 🔵 Amendment |
| `answered/` | 📁 Directory | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
