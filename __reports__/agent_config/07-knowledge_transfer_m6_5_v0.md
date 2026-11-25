# M6.5 agent configuration: knowledge transfer (v0)

Date: 2026-11-25. Branch `feat/m6.5-agent-config`, local only, not pushed.

## What landed

Instruction files (a global text per harness, a read-only workspace-root text per arm) and hooks are cell values. Candler installs them, the harness's own check judges them at job start (errors and warnings refuse the cell), and the trace's `context_start` shows what each run started with. All of G1–G13 hold on the tip ([review, round 07](07-review_m6_5_v0.md)); G13 passed live on RiVault ([round 06](06-findings_live_confirmation_v0.md)).

## Spend, measured from the transcripts

| Part | Spend |
|---|---|
| Planning and level 1 (probe, declaration, trace context) | $55.40 |
| Level 2 (two installs, preflight check, fakes and seam fixes) | $40.52 |
| Levels 3 and 4 (guard, input-mount fix, live run, review, close) | $13.77 |
| **Total** | **$109.69** |

The plan estimated about $72 in all, under a cap of $85 that Eliott raised to $125 after level 1. Levels 3 and 4 used four Sonnet agents, the costliest about $3. They worked from closed briefs that named every file and ordered frugality, iterated with check and clippy, and ran Docker and the full suite once. Levels 1 and 2 cost $95.92 against the plan's whole $72. The causes, measured in level 2, are in [round 05](05-observation_level2_out_of_scope_v0.md#process-lessons): filtered gates that let 33 tests go red, a check rule from a reading, stacked review passes, and a long coordinator thread (38 % of spend).

## Disposition of the review's findings

| # | Finding | Disposition |
|---|---|---|
| F1 | The collision pointer is always `/fixed/instructions.workspace` | Already filed as O4 (round 05) |
| F2 | OpenCode's check refuses WARN only; a plugin that fails in `setup()` might log at ERROR | Refuted by measurement: two runs of OpenCode 2.2.7 with a plugin that throws in `setup()` log `level=WARN message="failed to load plugin"`, which the check refuses |
| F3 | `HARNESS_IDS` in `candler-harness` repeats the CLI's `BUILTIN` driver list | Notice below: the harness crate cannot see the drivers, so a fix moves the list |
| F4 | No test writes to the workspace instruction file | Accepted: G9 rests on the mount's read-only flag, which the core guard checks |
| F5 | Report 06 said G1 for G5; its experiment files name a deleted treatment | Fixed in the close commit |
| F6 | Stale prose in `architecture.md` and the README status | Fixed in the close commit; the v2 architecture report keeps its old numbering, as reports are not rewritten |
| F7 | `context_start` is on the root session, not every session | Already what the contract page says (`trace-file.md`); only the historical v2 report says "every session" |

## Wins

- The trace now proves configuration: a consumer reads `context_start` and sees the exact texts, from the same gateway source on both harnesses.
- The guard found a real defect the unit tests could not see: the check did not mount declared inputs, so an OpenCode MCP server from inputs was refused.
- Rules from measurement settled two questions cheaply: the WARN-only reader (seam review) and F2 (one container run).

## Pain points

- A shared build directory ran another worktree's test binaries (round 05).
- Docker-wide container checks fail on a daemon shared with other sessions (O12).
- A job-start refusal loses its code (O11). It is the main gap a consumer meets.

## Open, for whoever picks it up

- O11 (medium): one refusal record for every job kind, so `experiment.harness_rejected_input` reaches the consumer.
- O1 (medium): an ignored OpenCode guard broken on main since 8d88cb50.
- `meet_run_context`: owed by M6d, which lands second.
- Notice F3: one list of shipped harness ids, owned where both the reader and the drivers can see it.
- Merging, pushing and the PR wait for Eliott. Then the level 3 worktree and the build slot `m6_5-target/slot-3` can be removed.

## Next cycle

- Brief closed scopes from the first dispatch: every file named, whole-suite gates, Docker once.
- One review pass per leaf, plus the milestone's closing review.
- A fresh coordinator per level once the thread is long.
