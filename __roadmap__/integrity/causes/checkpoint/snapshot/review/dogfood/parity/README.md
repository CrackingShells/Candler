# Harness parity

## Context
Level 3 of M6c, opened by amendment A3 after the dogfooding pass found that a run's record names no MCP server under OpenCode and that the stored catalog always says `servers: []` (`__reports__/diet/05-findings_dogfooding_pass_v0.md`, finding (1)). The plan is `__reports__/diet/07-plan_integrity_level3_v1.md`. Claude Code is the reference harness: OpenCode is aligned to it, and a defect found on the Claude Code side is fixed in both drivers. Depth gives the order: `declaration.md` first; `answered/` renders, traces and documents from the declaration; `answered/recorded/` removes the second catalog producer and proves parity; `closing/` holds seam review 3.

## Goal
Every fact about a harness is stated in one shape, which each registered driver must answer. One cell run on either harness installs the same MCP servers, and its stored trace records each MCP call in the same normalised form. Non-goals: hooks an experiment can set, and CLAUDE.md/AGENTS.md.

## Pre-conditions
- [x] `[cmd]` `git merge-base --is-ancestor 27d1d1dd origin/feat/integrity` succeeds (the dogfooding pass is merged)
- [x] `[nl]` Eliott approved revision 2 of the level-3 plan, with its rulings: the README says "per-run tool census"; `codemode` defaults to `false`; one shape for every per-harness list; the census keeps `census.tools` only; code mode is declared, not parsed

## Success Gates
- ⬜ `declaration.md` and `answered/` are `done`
- ⬜ The level's spend, measured from transcripts per row, stays within $70, and no row passed its cap without stopping
- ⬜ Every leaf report has a "What it leaves" section judging the change at the three levels (how it was developed, what it leaves in the code, how it is consumed), and seam review 3 found no second producer, second reader or passthrough left by the level

## Status
```mermaid
graph TD
    declaration[Settings declaration]:::done
    answered[Answered]:::inprogress
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `declaration.md` | 📄 Leaf Task | ✅ Done |
| `answered/` | 📁 Directory | 🔄 In Progress |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| `declaration.md` | `leaf/l3-declaration` | merged at `4a0dcada` (#71) | Report `__reports__/diet/leaves/declaration_v0.md`. Implementer, finisher, verifier, fix round and one design round (B2): $7.55 of the $24 cap. Inputs for seam review 3 noted. |
| `answered/mcp_render.md` | `leaf/l3-mcp-render-b` | merged at `29573006` (#74) | Report `__reports__/diet/leaves/mcp_render_v0.md`. Redone on a second branch so the schemas commit stands alone (#73 closed). Implementer and verifier: $3.46 of the $12 cap. |
| `answered/trace_parity.md` | `leaf/l3-trace-parity` | merged at `cca93ab4` (#75) | Report `__reports__/diet/leaves/trace_parity_v0.md`. Census gate reworded by A5. Adds the server-name rule and `fix(schemas)!` `SERVER_NAME_PATTERN`. Implementer, verifier and A5 sweep: $6.77 of the $16 cap. |
| `answered/pages.md` | `leaf/l3-pages` | merged at `7021d1ea` (#78) | Report `__reports__/diet/leaves/pages_v0.md`, with the RiVault trial: five OpenCode runs (`Reasoning-Medium` = DeepSeek-V4-Flash, `Reasoning-Large` = MiMo-V2.6-Pro-RL), two ending on the 32k output limit; a Sonnet review caught six defects, and its re-check one grep-dodging rename. Sonnet review: $0.68 of the $4 cap; RiVault tokens are not billed. |
| `answered/recorded/snapshot_out.md` | `leaf/l3-snapshot-out-2` | merged at `0197fbdb` (#77) | Report `__reports__/diet/leaves/snapshot_out_v0.md`. Redone on a second branch so the schemas commit stands alone (#76 closed). `comparable.snapshots` optional, written only by the importers; the evaluator protocol's `snapshots` stays until a helper that reads it as optional is released (the helper now does). Implementer (three rounds) and verifier: $9.56 of the $16 cap. |
| `answered/recorded/parity_run.md` | `leaf/l3-parity-run` | merged at `92c0f3a3` (#79) | Report `__reports__/diet/leaves/parity_run_v0.md`. Guard `parity_e2e::parity_one_installed_server_one_traced_call` on both real images, the production gateway and the fake model; it adds about 28 s to container-e2e. Live runs on `RiVault/Reasoning-Medium` (DeepSeek-V4-Flash): both traces show `{mcp, parity, stamp}`. The leaf's "no python3 in the OpenCode image" was stale. Implementer and verifier: $3.95 of the $12 cap. |
