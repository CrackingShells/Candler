# M6c Integrity

## Context
The build of candler-v1's M6c leaf (`__roadmap__/candler-v1/…/m5b/stats/surface/integrity/m6c_integrity.md`), between M6b and M7. It works from [`__reports__/diet/04-architecture_cycle_plan_v2.md`](../../__reports__/diet/04-architecture_cycle_plan_v2.md) and removes the causes behind the diet's defects: Candler state with a second producer in tests and fakes (D1), leaves cut by file and gated by counts (D3), `AGENTS.md` read as a checklist (D4), and inputs with several readers (D5). It is judged against the README purpose paragraph. Depth gives the order: `conventions.md` first; `causes/` runs P1, P2, the P4 chain and P6 in parallel; `causes/checkpoint/` holds seam review 1 and the spend checkpoint that decides P5; `snapshot/` stores the catalog snapshot; `review/` holds seam review 2; `dogfood/` closes levels 1 and 2 with an outside consumer's pass; below it, `dogfood/parity/` (level 3, A3) states every per-harness fact in one shape that each driver must answer, installs the same MCP servers on both harnesses and records each MCP call in one trace form, with Claude Code as the reference harness. The integration branch is `feat/integrity`, cut from `origin/main` at or after `70d4c3bf`; each leaf runs in its own worktree made by `scripts/worktree.py add` from the `feat/integrity` tip and removed at merge. Every build sets `CARGO_TARGET_DIR=/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-target/<leaf>` and `RUSTC_WRAPPER=sccache` in its own environment, one dir per leaf because `scripts/worktree.py` records that worktrees sharing a target dir link each other's code, with sccache sharing compiled dependencies; at most three builds run at once, each leaf's dir is removed at its merge, and `df -h /System/Volumes/Data` is checked before each level. Docker is used locally only by the dogfooding pass and by level 3's probes and live parity runs, each of which removes what it created by name; nothing is pruned and Docker is never stopped. Two pieces of v2 wait outside the caps: P3 (the stand-in that container-e2e uses, held to the daemon by a contract guard) returns by amendment if Eliott makes `Gates` and `container-e2e` required checks on `main`; P4·S (second_sources' `resolve` failure record and the transcript fold) is recorded as open and not done.

## Goal
Each kind of state Candler stores has one producer, each consumer input has one reader, an outside consumer's pass on the after tip finds no friction whose cause is D1 to D5, and every fact about a harness is stated in one shape that each driver must answer, so one cell run on either harness installs the same MCP servers and its trace records each MCP call in the same form.

## Pre-conditions
- [x] `[cmd]` `git merge-base --is-ancestor 70d4c3bf origin/main` succeeds (PR #57 landed)
- [x] `[nl]` The coordinator accepted plan v2 with its cut order, and Eliott's rulings are applied in it
- [x] `[cmd]` `git merge-base --is-ancestor 70d4c3bf feat/integrity` succeeds

## Success Gates
- ✅ `conventions.md` and `causes/` are `done`, level 3 (`causes/checkpoint/snapshot/review/dogfood/parity/`) included
- ✅ Every merge into `feat/integrity` that touches the paths `container-e2e` watches waited for it to be green on its pull request, and every other merge says it touched none of them (19 PRs merged into `feat/integrity`, #59 to #80: 16 touched the watched paths and each had `Container e2e (Linux)` green on its head before merge; #59, #69 and #70 touched none; the 24 direct rdm/reports pushes touched none; the paths filter did not change during the campaign)
- ✅ Spend is measured from transcripts per row and no row passed its cap without stopping; levels 1 and 2, coordinator included, stay within the $127 the maintainer approved, and level 3, its lead included, within $70 (levels 1 and 2: "about $127", `__reports__/diet/06-knowledge_transfer_integrity_v0.md` §Spend; level 3: $61.78 measured at its close, same report §Level 3)

## Status
```mermaid
graph TD
    conventions[Conventions]:::done
    causes[Causes]:::done
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
| `causes/` | 📁 Directory | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A1 | 2026-11-24 | seam review 1 (F3/F4) | none; one gate added to `dogfooding_pass.md` | OpenCode has no agent-to-sealed-run chain in CI; the pass's real OpenCode run is where its stored outputs are compared with what the agent wrote, before any follow-up builds that chain |
| A2 | 2026-11-24 | maintainer ruling on seam review 1 (R1) | none; fixed under seam review 1 as leaf `exec_bit` | Every stored output keeps its execute bit, whatever produced it, so an evaluation plugin can run an output; the runner records the bit as import does, the 0555 materialise path stays, and the tree digest of a new run with an executable output changes |
| A3 | 2026-11-25 | `__reports__/diet/07-plan_integrity_level3_v0.md` (approved level-3 plan) and `05-findings_dogfooding_pass_v0.md` finding (1) | ["parity/", "declaration.md", "answered/", "mcp_render.md", "census.md", "pages.md", "recorded/", "snapshot_out.md", "parity_run.md", "closing/", "seam_review_three.md"] | The run record named no MCP server under OpenCode and the catalog snapshot came from a constant; level 3 declares the agent's settings once, has every driver answer them, makes the census the one record and proves parity on both harnesses. The goal, the done gate and the spend gate are reworded with it |
| A4 | 2026-11-25 | maintainer rulings on the A3 plan (`07-plan_integrity_level3_v1.md`) | ["trace_parity.md"]; `census.md` blocked | The per-harness lists fold into one shape (`CONFIGURATION_PATHS`'s const rows and `refusal_table!`'s exhaustive match), and a missing answer fails to compile. Which servers the agent called is for the consumer to parse from the trace, so the census keeps `census.tools` only, and `trace_parity.md` aligns both trace readers on one MCP origin instead of collecting server status. `tool-catalog-snapshot/1` stays, because an outside tool writes and reads it, and only Candler's constant producer goes. Hooks and instruction files are non-goals |
| A5 | 2026-11-25 | coordinator's ruling on the P10 stop (OpenCode 2.2.7 offers no tool-list endpoint) | none; gates reworded | P10 wanted OpenCode's census from `GET /experimental/tool`, but the pinned OpenCode 2.2.7 has no such endpoint: its HTTP API is 139 paths under `/api`, none lists tools, and `/experimental/tool` serves the web app's HTML. Its docs expose the tool list only to plugins, as `ctx.tool.list()`. Candler's gateway census for OpenCode stays: its record of the tools in the model request is exactly what the model was offered, which is the input `census.tools` comparability exists for. A plugin injected by Candler was rejected, since it adds a producer, a plugin in every cell and its upkeep, to observe something less directly. The gateway census for both harnesses was rejected too: Claude Code can hold MCP tool schemas back behind tool search, so its first model request may not list them, while its `system/init` does. The two sources measure the same input in the way each harness allows, and Claude Code is not moved onto the gateway without checking deferral at the pinned version. So `CensusSource`, `census.source`, the gateway's `census_init` and the verdict's source reading stay, and `trace_parity.md`'s gate, Goal and Step 2 say that each census names its source |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
