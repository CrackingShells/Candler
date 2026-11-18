# known_truth: findings

Simulated experiments with planted truth go through the real `Report::build` (the analysis behind `analyze`). Designs are two arms of `plan.model` over two blocks of `plan.other`; the manifests are built once per design and each simulation draws only the outcomes (seeded `SplitMix64`, 4 worker threads, one stream each). Runtimes are debug builds on a loaded shared machine.

## Rows

| row | design | simulations | result | runtime |
|---|---|---|---|---|
| `kt_h0_binary_nominal` | 12 runs/cell, rates 0.3 / 0.6 by block, equal by arm | 2 000 | 101 rejections, rate 0.0505, 99 % interval contains 0.05 | 30 to 32 s |
| `kt_simpson_blocked_holds` | allocation 20, 5, 5, 20; rates 0.8 / 0.2 by block | 2 000 | blocked 102/2000 = 0.051 (holds); pooled 1454/2000 = 0.727 (outside) | 49 s |
| `kt_h0_weighted_nominal` | 6 runs/cell, outcomes over 4 (5 weight values), rates 0.4 / 0.7 | 1 000 | 24 rejections, rate 0.024, below the interval [0.037, 0.063]: RED, two-sided as gated (see Findings) | 21 s |
| `kt_h0_weighted_continuous` | 6 runs/cell, weights on a grid of 1 000, upper limits 0.4 / 0.7 | 1 000 | 59 rejections, rate 0.059, interval contains 0.05 | 22 s |
| `kt_h0_one_block_nominal` | one block, 12 runs/arm, both arms 0.3 | 4 000 | 179 rejections, rate 0.0447, interval contains 0.05 | 8 s |
| `kt_two_levels_end_to_end` | 4 treatments x 3 models x 4 replicates, 48 runs on the fakes | 1 | 6 treatment comparisons with 3 blocks, 3 model comparisons with 4 blocks, planted pair p < 0.05, B minus A = +1 in each block | 2 s |

## Power

At a 0.2 difference in every block (0.3 to 0.5 in block x, 0.5 to 0.7 in block y), 12 runs per cell, 1 500 simulations: 493 rejections, power 0.329, 95 % Monte Carlo interval 0.305 to 0.352. An earlier run at 25 runs per cell (1 000 simulations) gave 553, power 0.553, interval 0.522 to 0.584; it was dropped from the row for its runtime (about 100 s under load). The row asserts only power above 0.05.

## Bites

Each injected in `src/analysis/blocking.rs`, seen red, restored from a copy (`git status` shows only the test file).

| bite | result |
|---|---|
| pool the blocks into one stratum and use the mid-p (`binary`) | `kt_simpson_blocked_holds` red: "blocked rate 1454/2000 outside the interval of 0.05". `kt_h0_binary_nominal` also red: rate 72/2000 = 0.036 outside the interval |
| constant p of 0.01 in `binary` | `kt_h0_binary_nominal` red: rate 2000/2000; `kt_simpson_blocked_holds` red; `kt_power_at_0_2` stays green (it asserts only power above alpha) |
| Fisher's p in place of the mid-p on the one-block path (`binary`, `fisher_two_sided` for `midp_two_sided`) | `kt_h0_one_block_nominal` red: 74/4000 = 0.0185 against 179/4000 = 0.0447 with the mid-p; "rejection rate 0.0185 outside the 99 % interval of 0.05". Restored from a copy; `git status` shows only the test file |
| constant p in the weighted path | not run (call cap); `kt_h0_weighted_nominal` is red already, so the bite would be read on `kt_h0_weighted_continuous` |

## Findings

1. **Weighted known truth (ruling A14): the row stays two-sided and is red; no defect found in `src/`.** The row counts the raw permutation p of the one weighted quality expectation (`Tested::p`, from `stratified_permutation` through `blocking::weighted`), never `p_holm` or `reject`. Ruled out: (i) Holm over two expectations (the simulated store holds one weighted expectation, `E3`, and no other quality one; and the row reads `p`, not `p_holm`); (ii) the "effect" reading (the BCa interval plays no part in the count). The rate is still about alpha/2. Evidence (1 000 simulations, same seed stream, p of `Tested::p`):

   | weights | runs/cell | p <= 0.05 | p < 0.05 | distinct p (1e-6) | histogram of p, 10 bins of 0.1 |
   |---|---|---|---|---|---|
   | 5 values (0, 1/4 .. 1) | 6 | 24 (0.024) | 24 | 703 | 60 85 93 84 77 70 100 56 116 259 |
   | 5 values (0, 1/4 .. 1) | 10 | 25 (0.025) | 25 | 775 | 60 86 86 83 94 95 96 92 127 181 |
   | grid of 1 000 | 6 | 56 (0.056) | 56 | 955 | 93 102 112 96 105 102 109 84 98 99 |
   | grid of 1 000 | 10 | 45 (0.045) | 45 | 953 | 95 109 105 93 94 96 101 109 91 107 |

   The simulation draws 5 distinct weight values (outcome 0 to 4 over 4). With near-continuous weights the same code is flat and nominal (0.056 and 0.045 in the probe, 0.059 in the committed row with integer draws; all inside [0.037, 0.063]), so the code is not at fault: with 5 values the statistic ties heavily, the `|stat| >= |observed|` count takes every tied permutation, and the p piles into the top bin (259 against 100 expected) while the low bins thin out. That is the conservatism of an exact permutation test on tied data, not a bug. The row `kt_h0_weighted_nominal` therefore cannot pass at these weights, and the gate (interval contains 0.05) cannot be met by this design. For the coordinator: either rule that the gate holds on a design without ties (`kt_h0_weighted_continuous` now guards that and is green), or rule a design with more weight values. The previous one-sided relaxation is reverted; no gate wording changed.
2. The fakes seal runs as `invalid(infrastructure)` because the fake run leaves no container (`final_inspect` in `tests/it/support/job_orchestrate.rs` returns none), and `classify_end` in `src/runner/exec/ends.rs` reads a missing container inspect as an infrastructure failure when the end is not the arm's own: the recorded item missing is the container's final inspect, so this points at the fakes, not at the runner (the runner is right to refuse to read an exit it never saw). `kt_two_levels_end_to_end` plants `integrity = valid` and the three provenance values in the manifests it reads back; the cells and run keys are the plan's own. The planted pair (0 of 4 against 4 of 4 in every block) has an undefined odds-ratio interval, so its reading is `Inconclusive`; the row asserts p, the direction and the counts, not the reading.
3. The fakes already live in `tests/it/support/job_orchestrate.rs`, so they were reused and `guard_job_orchestrate.rs`, `support/fake_jobs.rs` and `support/mod.rs` are untouched.
4. Runtime: a report of 100 runs costs about 0.2 s in debug; the binary rows use 12 runs per cell and 4 threads to stay under 60 s.
5. Not done: weighted constant-p bite; no `analyze_c6_*` vectors were added (A8).

## Gates

- Targeted test, all rows of `guard_known_truth::` and `guard_job_orchestrate::` (12 tests, `--no-fail-fast`): FAIL, 11 passed, 1 failed: `kt_h0_weighted_nominal` red by design (24/1000 = 0.024 outside [0.037, 0.063], see Findings 1). `kt_h0_weighted_continuous` rerun alone after the last edit: PASS (59/1000, 22 s).
- `cargo fmt --check`: PASS (exit 0).
- ownership check (`git diff --name-only afd32f0b..HEAD | grep -vE ...`): prints nothing.
- clippy `-p candler-core --all-targets -D warnings`: PASS (`Finished` after the last edit).
