# M6d Run context

## Context
The README promises that Candler's evaluation half can "grade traces and artifacts, fresh or already on disk, through pluggable evaluators that emit one outcome record format". Today an evaluator is handed only the artifacts: `context.trace` and `context.raw[]` exist on the wire, but `assemble` never fills them and silently ignores `needs.trace = true`. This campaign builds the approved plan [`__reports__/run-context/00-plan_run_context_v0.md`](../../__reports__/run-context/00-plan_run_context_v0.md): an evaluator can be handed any part a sealed run stored (trace, raw streams, manifest) the same way it is handed artifacts, and the trace records each shell call's exit status as `ToolResult.exit`, the `ExitStatus` object every supervised process already carries. htn-creator-eval is the first consumer (its probe P0 pins the release this campaign cuts); the design answers the feature's intent, not that probe. Depth gives the order: level 1 holds `schema_exit_needs.md` (the wire) and `probe_shell_shapes.md` (what real harnesses write for a failing shell call) in parallel; `readers/` (level 2) holds both trace readers, the serving in `assemble`/`materialise` and the Python helper in parallel; `readers/e2e/` (level 3) holds the end-to-end guard on real harnesses; `readers/e2e/review/` (level 4) holds the adversarial review of the whole diff. The test plan (guards with their oracle and bite, the probe, what is deliberately not tested) is the plan's §Test plan; each leaf names the guards it owns. The integration branch is `feat/run-context`, cut from `origin/main` at `8d82f63b`; each leaf runs in its own worktree made by `scripts/worktree.py add` from the `feat/run-context` tip and removed at merge. Every build sets `CARGO_TARGET_DIR=/Users/hacker/Documents/tmp/claude-worktrees/Candler/run-context-target/<leaf>` and `RUSTC_WRAPPER=sccache` in its own environment, one dir per leaf; at most three builds run at once, each leaf's dir is removed at its merge, and `df -h /System/Volumes/Data` is checked before each level. Docker is used locally by the probe and the e2e guard only, each of which removes what it built by id; nothing is pruned and Docker is never stopped. Test names that start with `confirm_` are build-step checks: each is deleted before its leaf merges, or rewritten to guard standard (`Guards: / Oracle: / Bite:`). The milestone label M6d is the coordinator's; the instruction-files campaign (M6.5) meets this one through its leaf `meet_run_context.md`: whichever lands second merges `origin/main` in (no rebase), regenerates the schemas and contract pages instead of resolving them by hand, and writes the `needs.trace` sentence about `context_start`.

## Goal
An evaluator plug-in that declares `needs.trace`, `needs.raw` or `needs.manifest` is handed a private, read-only, digest-checked copy of that part of the sealed run, or a refusal with a code; the trace records each shell call's exit status as the harness reported it on both Claude Code and OpenCode; and one real run graded through `candler evaluate` shows its plug-in each shell call's command and exit and each MCP call's `{server, tool}`.

## Pre-conditions
- [x] `[nl]` The maintainer approved plan v0 (2026-11-25), with its rulings on exit status, one release, and no blinding concept
- [x] `[cmd]` `git merge-base --is-ancestor 8d82f63b feat/run-context` succeeds
- [x] `[nl]` The maintainer approved the campaign's spend cap below before the first dispatch (2026-11-25, with the go for level 1)

## Success Gates
- ⬜ Every leaf is `done`, and each leaf report records every guard it owns seen red (on the base or under its named bite) and then green, with the failing message
- ⬜ `git grep -n confirm_ -- crates python` prints nothing on the `feat/run-context` tip
- ⬜ `container-e2e` on the integration pull request lists `graded_run_sees_trace` (or the name the e2e leaf records) as PASS, and every merge into `feat/run-context` that touched the paths that workflow watches waited for it to be green
- ⬜ The pull request into `main` is opened only after the maintainer's go; after it merges, the CI release cuts the version and the HTN Phase coordinator is told the version to pin
- ⬜ Spend is measured from transcripts per leaf, and no leaf passed its cap without stopping. Leaf estimates add to $57, caps to $114; the campaign, coordinator included, stays within the cap the maintainer approves

## Status
```mermaid
graph TD
    schema_exit_needs[Schema: exit and needs]:::done
    probe_shell_shapes[Probe: shell shapes]:::done
    readers[Readers and serving]:::inprogress
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `schema_exit_needs.md` | 📄 Leaf Task | ✅ Done |
| `probe_shell_shapes.md` | 📄 Leaf Task | ✅ Done |
| `readers/` | 📁 Directory | 🔄 In Progress |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
