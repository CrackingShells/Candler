# guard_reuse leaf report (v0)

Type: findings. Branch `m5b/guard_reuse`, base 3be9fd3.

## 1. Commits
| sha | subject |
|:----|:--------|
| 50cb5d36 | perf(isolation): verify a destroy with one inspect of every container |
| 9ecdf4fd | test(isolation): verify a destroy on a real daemon |
| (last) | reports(isolation): record the guard_reuse leaf |

## 2. Gates (after fix round 1)
- STD (`-p candler-core`): PASS: fmt, clippy `--all-targets -D warnings`, doc `-D warnings`, nextest 293 passed.
- LINUX: PASS. SUBJ (base 3be9fd3): PASS.
- DIGEST: numstat PASS; `guard_cell_digest` (9) and the core DIGEST set (45) PASS.
- SPAWN (clean form) and GOLDEN: left to the coordinator, as ruled.
- E2E under the B4 lock: PASS, `-p candler-core -p candler`: [ 187.791s] 17 tests run: 17 passed, 437 skipped
- `guard_container_guard::` PASS; `spawn_budget_paths` PASS but the ladder bound is 12, not 8 (below); `destroy_verifies_by_one_inspect` PASS.
- OWNED check: the diff adds `isolation/docker.rs` (`ps_by_label` only, authorised by F1).
- Confirms culled: PASS.

## 3. Guards
| guard | Guards | oracle | bite and failing message |
|:------|:-------|:-------|:-------------------------|
| `spawn_budget_paths` (rows: sweep, normal exit, ladder stop, exec pre-stop, sweep of 7) | D7/A1: one list, one inspect, a stop and an rm per target | counts derived by hand (below), read from the log of a PATH-fake docker | `inspect_many` as one `inspect` per id: "sweep of 7: 23 spawns, at most 17"; `ps_by_label` inspecting per id: the same message, "sweep of 7: 23 spawns, at most 17" |
| `reuse_never_crosses_containers` | D7: facts of one container are never applied to another (two ids sharing 63 digits, one of another job) | by hand: the twin of another job answers `other_job`; only the passing twin gets stop and rm | answers matched by a 63-digit prefix: left `Refused(Mismatch)`, right `Refused(OtherJob)` (see finding 4) |
| `vanished_between_list_and_inspect` | D7: a container gone between ls and inspect is skipped | by hand: three listed, two known; stop and rm for the two only | `NotFound` mapped to a docker failure: "a vanished container is not an unknown" |
| `b11_one_inspect_before_the_verbs` (replaces `b11_inspect_before_every_verb`) | D7: one inspect, then stop, rm | ordered log `inspect, stop, rm`; a forged claim is refused with no verb | the old reuse bite does not apply any more; the log pins one inspect |
| `destroy_verifies_by_one_inspect` (e2e) | D7 on a real daemon | `docker ps -a` by label before and after; the four ids `docker run` printed | `inspect_many_answer` failing every id when docker exits 1: panic at the `NotFound` assertion for the removed id |

Spawn counts. Shape: a job of 9 containers; the run is one cell's agent container (the gateway carries no run key, so a run is the agent alone today); the other 8 are the gateway and agents of other runs. Protocol costs, fixed before measuring: a list L = 2 (one `ls`, one `inspect_many` of what it printed, since `ps_by_label` now uses one); a verification = 1 `inspect_many`; a target = stop 1 + rm 1.
| path | hand arithmetic (N = 1) | bound | measured after | before (derived or measured) |
|:-----|:------------------------|:------|:---------------|:-----------------------------|
| sweep of the run | L 2 + verify 1 + stop 1 + rm 1 | 5 | 5 | 1 ls + 9 inspects + 4 = 14 derived (job-label list) |
| normal exit (destroy hook) | hook list L 2 + last inspect 1 + sweep 5 | 8 | 8 | 25 derived (list 10 + 1 + 14) |
| ladder stop | pre-stop (list 2 + verify 1 + stop 1 = 4) + normal exit 8 | 12 | 12 | 37 derived (pre-stop 12 + 25): the audit's 37 |
| exec pre-stop | list 2 + one inspect 1 | 3 | 3 | 11 derived (1 ls + 9 inspects + 1) |
| sweep of a run of 7 (7 of 9) | L 2 + verify 1 + 2 x 7 | 17 | 17 | 38 measured on the first fixture |

G3 ("at most 8" for a stopped run in a job of 9): MET on the sweep (5) and the normal exit (8); NOT MET on the ladder stop (12). The ladder runs two hooks, each with its own list: pre-stop (list 2 + verify 1 + stop 1) and destroy (list 2 + last inspect 1 + sweep: list 2 + verify 1 + stop 1 + rm 1). The floor under this structure is 4 + 8 = 12; caching the pre-stop list in the hook would give 10. Reaching 8 needs the two hooks to share one list and one verification (a change to the hook/ladder contract, beyond A1). Stopped here as ruled; the gate is not loosened in the test, whose ladder row carries the derived 12.

Old call-order pins changed:
- `b11_inspect_before_every_verb` (inspect, stop, inspect, rm; the second inspect refusing on a changed job or creation time) is now `b11_one_inspect_before_the_verbs` (inspect, stop, rm). The two-inspect mismatch rows are deleted on purpose: the second inspect is the thing D7 removes. A forged claim now refuses before any verb.
- `b10_guard_refusals`: the positive control's docker log was two `inspect` lines; it is one `inspect_many <id>`. Its Bite text names `destroy_verified`.
- `guard_container_stop` rows needed no change.

## 4. Probes
None owned by this leaf.

## 5. Claims checked
- "37 spawns for a stopped run in a 9-container job": not reproducible as stated. The code's sweep for a run costs `ps_by_label` (1 ls + one `docker inspect` per listed container, in `docker.rs`) plus 4 per target (verify, stop, second inspect, rm). 37 = 1 + 9x4 only if the run owns all 9 and the list's own inspects are left out. Measured before: 38 for a run of 7 in a job of 9 (10 for the job-label list plus 28).
- "at most 8": unreachable for any run of more than 2 containers while a stop and an rm are one process per target (2 + 2N at the very best, and 1 + N more for the list). Built to the leaf's intent (one inspect per decision, listing by the run label), with bounds derived by hand as above.
- `ContainerGuard::verify` did not use `inspect_many`: confirmed. `pub(crate)` signatures called by `container/mod.rs` are unchanged (`container/mod.rs` has no call into the changed items; `cargo check` of the crate passes).

## 6. Findings
1. RESOLVED (F1): `DockerCli::ps_by_label` now reads the listed ids with one `inspect_many` (a container removed since the `ls` is skipped; other errors fail the list). This is the only change in `docker.rs`.
2. SPEC DEFECT, partly open: the 37 was the sweep of a run owning all 9 containers; the gate's "at most 8" holds for the sweep (5) and normal exit (8) of one run, not for the ladder (12). See the arithmetic above.
3. New `pub` API on `ContainerGuard` (they are `pub` like `destroy`, because the sweeper in another module calls them and `destroy` is `pub`): `destroy_many` and `destroy_lost_many` (one inspect for a batch). `destroy` and `destroy_lost` are the one-element case; `destroyable` and `stop` keep their signatures. No refusal rule widened: each container is refused by its own facts, and a container docker lacks is `absent`, never destroyed.
4. Bite note: with the prefix bite the twin is refused as `Mismatch` (the caller's claimed labels fail against the wrong facts) rather than `OtherJob`; the guard is double-safe, the row still turns red.
5. Behaviour: a run's sweep still reports only containers of the current job. The run-label list can return another job's container with the same run key; it is filtered out in memory by the job label, so it is never reported as refused. A recorded id that docker did not list is read through one `inspect_many` instead of one inspect each.
6. The second inspect is gone: a job that recovers while a container stops is still caught by `check_job` before `rm`.
7. A process slip, no effect on the result: a WIP `git commit` was rejected by the commit-msg hook just before I overwrote the working files with the base text for a "red against old code" run; the staged copy held the work, and I rebuilt the three sources with the same edits. The old-code run gave the measured 38.

## 7. Docker
Containers and networks: identical before and after. Images: before 14; after, one id gone and one new (7a8089051913 recorded before, fee43feefc06 after) while sibling leaves share the daemon and build the testagent image; I removed none and did not build either by name, so I cannot say which run made them. The e2e removes its own containers by exact id through the sweep (and one `docker rm --force --volumes <exact id>` of a container it made). The bite run left nothing.
e2e name: `guard_container_e2e::container_stop_e2e::destroy_verifies_by_one_inspect` (nextest: `candler-core::it guard_container_e2e::container_stop_e2e::destroy_verifies_by_one_inspect`).
