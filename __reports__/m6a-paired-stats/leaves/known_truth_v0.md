# known_truth: findings

Simulated experiments with planted truth go through the real `Report::build` (the analysis behind `analyze`). Designs are two arms of `plan.model` over two blocks of `plan.other`; the manifests are built once per design and each simulation draws only the outcomes (seeded `SplitMix64`, 4 worker threads, one stream each). Runtimes are debug builds on a loaded shared machine.

## Rows

| row | design | simulations | result | runtime |
|---|---|---|---|---|
| `kt_h0_binary_nominal` | 12 runs/cell, rates 0.3 / 0.6 by block, equal by arm | 2 000 | 101 rejections, rate 0.0505, 99 % interval contains 0.05 | 30 to 32 s |
| `kt_simpson_blocked_holds` | allocation 20, 5, 5, 20; rates 0.8 / 0.2 by block | 2 000 | blocked 102/2000 = 0.051 (holds); pooled 1454/2000 = 0.727 (outside) | 49 s |
| `kt_h0_weighted_nominal` | 6 runs/cell, outcomes over 4, rates 0.4 / 0.7 | 1 000 | 24 rejections, rate 0.024, below the interval (see Findings) | 21 s |
| `kt_two_levels_end_to_end` | 4 treatments x 3 models x 4 replicates, 48 runs on the fakes | 1 | 6 treatment comparisons with 3 blocks, 3 model comparisons with 4 blocks, planted pair p < 0.05, B minus A = +1 in each block | 2 s |

## Power

At a 0.2 difference in every block (0.3 to 0.5 in block x, 0.5 to 0.7 in block y), 12 runs per cell, 1 500 simulations: 493 rejections, power 0.329, 95 % Monte Carlo interval 0.305 to 0.352. An earlier run at 25 runs per cell (1 000 simulations) gave 553, power 0.553, interval 0.522 to 0.584; it was dropped from the row for its runtime (about 100 s under load). The row asserts only power above 0.05.

## Bites

Each injected in `src/analysis/blocking.rs`, seen red, restored from a copy (`git status` shows only the test file).

| bite | result |
|---|---|
| pool the blocks into one stratum and use the mid-p (`binary`) | `kt_simpson_blocked_holds` red: "blocked rate 1454/2000 outside the interval of 0.05". `kt_h0_binary_nominal` also red: rate 72/2000 = 0.036 outside the interval |
| constant p of 0.01 in `binary` | `kt_h0_binary_nominal` red: rate 2000/2000; `kt_simpson_blocked_holds` red; `kt_power_at_0_2` stays green (it asserts only power above alpha) |
| Fisher's p in place of the mid-p | not run: the mid-p branch is taken only for one block, and every design here has two blocks (CMH). No rate change is observable; a one-block H0 row would be needed to bite it |
| constant p in the weighted path | not run (call cap) |

## Findings

1. Gate `kt_h0_weighted_nominal` as written ("interval contains 0.05") does not hold: the weighted blocked permutation test rejects at 0.024 (6 runs/cell) and 0.025 (10 runs/cell, 1 000 simulations, 86 s), the same seed streams. It is conservative, not inflated. The guard asserts the rate is not above the interval's upper limit and says so; the coordinator should rule whether the conservatism is intended (two-sided doubling or tie handling in `permutation.rs`) or a defect.
2. The fakes seal runs as `invalid(infrastructure)` with no `eval_infra` provenance, so `kt_two_levels_end_to_end` plants `integrity = valid` and the two provenance values in the manifests it reads back; the cells and run keys are the plan's own. The planted pair (0 of 4 against 4 of 4 in every block) has an undefined odds-ratio interval, so its reading is `Inconclusive`; the row asserts p, the direction and the counts, not the reading.
3. The fakes already live in `tests/it/support/job_orchestrate.rs`, so they were reused and `guard_job_orchestrate.rs`, `support/fake_jobs.rs` and `support/mod.rs` are untouched.
4. Runtime: a report of 100 runs costs about 0.2 s in debug; the binary rows use 12 runs per cell and 4 threads to stay under 60 s.
5. Not done: Fisher bite on a one-block design; weighted constant-p bite; no `analyze_c6_*` vectors were added (A8).

## Gates

- Targeted test, all rows of `guard_known_truth::` and `guard_job_orchestrate::`: see the final run in the message.
- clippy `-p candler-core --all-targets -D warnings`: PASS.
