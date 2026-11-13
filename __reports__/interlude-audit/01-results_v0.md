# Interlude results: what landed and what it cost

Date: 2026-11-13, 12:27–13:45 JST. Budget: $50 and 6 h. Used: about $45 and 1 h 20 min. Money is
what ran out first.

## Pull requests

| PR | Branch | Changes | Lines | CI |
|:--|:--|:--|:--|:--|
| [#24](https://github.com/CrackingShells/Candler/pull/24) | `repo/session-start-hook` | The SessionStart hook runs `scripts/worktree.py sync --quiet` | +17/−2 | merged a7cbbcf |
| [#25](https://github.com/CrackingShells/Candler/pull/25) | `ci/fast-gates` | Build cache saved from main only; `fmt` as its own job; no separate build; the title check in `pr-title.yml`; docs-only PRs skip the Rust gates; timeouts; container e2e builds 2 binaries, not 74; the answered FROM probe deleted; 1,117 lines of compile-only tests deleted | +123/−1,186 | merged 9169051; Gates 3m58s on a cold cache |
| [#26](https://github.com/CrackingShells/Candler/pull/26) | `perf/runtime-footprint` | A job's end read from the back of its log; child records deleted once spent; per-attempt gateway signals (fixes the dropped newest lines); no venv copy for control containers; FIFO semaphore before the slot files; no blocking lock on async workers; the env key covers its Dockerfile; boot id read once | +450/−193 | merged 7a8e720, after one fix: a Linux-only guard counted the records of ended children |
| [#27](https://github.com/CrackingShells/Candler/pull/27) | `perf/store-scans` | **Regraded outcomes counted twice in tallies and rates: confirmed and fixed**; one store listing per plan; the unread store index deleted; one digest constructor; one source for value paths; one trace copy; sealing off the async workers | +300/−863 | merged 2d15e33, after a rebase onto #26 (conflict on the file #25 deleted) |

All four together come to about +890/−2,240 lines.

The coordinator opened #26 and #27 with 69-character titles, over the 66 that GitHub's
" (#NN)" suffix leaves; the title check caught both. The title check counts characters, so check
`${#T}` against `72 - len(" (#NN)")` before `gh pr create`.

## Audit claims the implementers corrected

| Claim | What the code showed |
|:--|:--|
| `job.ended` is the last event | `candler kill` logs `process.stop_step` after it; the reader walks back to `job.ended` instead |
| The gateway capture should be tail-biased | Per-attempt windows through `read_since` remove the O(runs²) reads and the silent loss together |
| Skip the /proc sweep when the group is empty | A descendant that called `setsid` has already left the group; the sweep stays |
| Use `fs::copy` for artifacts | It follows a dangling link at the target, and the re-read digest is requirement R9; skipped |
| The CI title check can skip the other jobs on `edited` | A skipped Gates would show over a failed one, and `cancel-in-progress` cancels running gates; the coordinator moved the title check to its own workflow |

## Spend

| Role | Agents | Cost | Share |
|:--|--:|--:|--:|
| Auditors (Opus, read-only) | 4 | $16.4 | 37 % |
| Rust implementers (Opus) | 2 | $19.3 | 43 % |
| Their rust-conventions reviewers | 2 | $2.9 | 7 % |
| CI implementer (Sonnet) | 1 | $0.7 | 1 % |
| Coordinator | 1 | $5.0 | 11 % |
| **Total** | 10 | **$44.3** | |

## What this run teaches about running the work

1. **A loaded machine multiplies the cost of every Rust agent.** Another project's experiment
   containers kept the load at 34–81 on 12 cores. Each Rust implementer took about 56 min, mostly
   waiting on builds. Timing tests then failed under load and forced three full nextest runs.
   Before dispatching builders, check the load and wait or serialise; four build jobs per agent was
   still too many.
2. **The cheap model did the mechanical batch well and missed one interaction.** Sonnet's CI batch
   cost $0.7. Its one design flaw, skipping on `edited`, needed the coordinator's review, which
   took a few minutes. Mechanical work suits a cheap implementer plus a review; work with
   invariants still needs the stronger model.
3. **Audits pay for themselves when the implementers re-verify them.** Five audit claims were
   wrong or incomplete, and each was caught because the brief said to verify before changing.
   Keep that clause in every brief.
4. **Read-only auditors cost about $4 each, mostly their fixed prefix and the files they read.**
   Four slices covered 140k lines. Fewer, broader slices would have missed the depth that found
   the double count.

## Left for later

The backlog in [00-findings_audit_v0.md](00-findings_audit_v0.md) stands, with these additions from
the implementers:
- **Two counts and a regrade gap:** `show` without `--rates` and `inspect` still count every
  current record, and `--pending` does not regrade a run after its suite entry's params change.
- **Signals vs census:** `runner/exec/mod.rs:777` reads a signals error as "gateway did not fail",
  while census reads it as "not collected". This needs a ruling.
- **Remaining digest copies:** jobs and isolation still hand-roll digests (`controls.rs:275,373`,
  `runtime/session.rs:94`, `runtime/launch.rs:22`, `image.rs:829`). When they move to
  `Digest::of_bytes`, `import::digest_of` and its re-export go too.
- **Dead public API:** `supervise::platform::parse_proc_stat` has no user outside its module.
