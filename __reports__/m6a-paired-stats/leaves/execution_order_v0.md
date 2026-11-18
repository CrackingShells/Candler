# execution_order (leaf report, v0)

Findings type. `plan_runs` orders runs by `run_order(cells, replicates, seed)` in `runner/order.rs`;
the seed is the first 8 bytes, little-endian, of the experiment digest.

## Commits
| sha | subject |
|:----|:--------|
| 189b7795 | test(runner): guard a seeded run order |
| 12bf20e7 | feat(runner): add a seeded run order |
| 01ab489e | refactor(runner): plan runs in the seeded order |
| 918d8b52 | docs(runner): record the bites of the run order |
| (tip) | fix(runner): pin the in-flight pair and the seed contract |

## Gates
- Targeted test (`guard_run_plan::` and `guard_job_orchestrate::`): PASS, 9 passed.
- Replicate loop: PASS in substance. `plan.rs` has none, `order.rs` has one. The gate as written
  fails on the zero: `git grep -c` prints nothing when there is no match, so `test "" = 0` is false.
  Checked with `git grep -c ... plan.rs; echo $?` (empty output, exit 1) and `order.rs` (count 1).
- DIGEST: PASS (numstat awk exit 0; `guard_cell_digest` 9 passed; the core guards 45 passed).
- `cargo clippy -p candler-core --all-targets -- -D warnings`: PASS. `cargo fmt --check`: PASS.
- Ownership: prints nothing, with the extension for `guard_job_orchestrate.rs` the coordinator ruled.

## Claims checked
- Today's loop existed once in `plan.rs`: yes, removed. One loop remains, in `run_order`.
- `PlanOutcome.sealed` is now in run order. No stored record or digest input is written from it
  (DIGEST passes; no consumer of `.sealed` was found outside tests and the struct).
- `run_order` and `seed_of` are re-exported from `runner` (`pub use`); `mod order;` is private.
  The ruling named only `run_order`; `seed_of` is re-exported too, with a doc stating the reproducibility contract, because `b3_plan_runs_uses_the_experiment_seed`
  checks its byte order.

## Existing guards that pinned today's order (exception under the coordinator's ruling)
- `guard_run_plan::b27_changed_fact_is_refused_and_the_cell_is_pinned`: zipped plans with the model
  list by position (three places). It now finds each plan by its cell digest. It still checks the
  refused fact and the pinned cell.
- `guard_job_orchestrate::b34_cap_stops_live_runs_only`: expected m3 and m4 in flight. It now
  computes the expected pair with `run_order` on the job's own seed (`seed_of` of the experiment
  digest the sealed runs store), keeps the unsealed cells, takes the first `max_parallel` and
  asserts the ended runs equal them. `run_order` is the fixture; the `b3_` rows pin it. Every later
  check is unchanged.

## Bites
Each injected from a copy, seen red, restored from the copy (`git status` clean after).
| bite | guard | message |
|:-----|:------|:--------|
| shuffle the whole sequence across rounds | `b3_each_round_holds_each_cell_once` | `seed 0, 2 cells: the round of replicate 2 holds only it` |
| constant seed `0` in `plan_runs` | `b3_plan_runs_uses_the_experiment_seed` | `another digest plans another order` (both sides the same 16-run list) |
| declaration order restored in `plan_runs` | `b34_cap_stops_live_runs_only` | `only the first `max_parallel` runs of the planned order ended` (ended pair differs from expected) |
| old loop kept beside the new one | the `git grep -c` gate | `plan.rs` count 1, gate expects none |
The step 1 rows were red first for the right reason: one error, `unresolved import ... runner::order`.
After step 2, the plan row alone was red (`another digest plans another order`).

## Scheduler order
`Scheduler::run` (`crates/candler-core/src/jobs/orchestrate/schedule.rs`) takes `plans: Vec<RunPlan>`
into a `VecDeque` (`pending`), and each pass of its loop pops `pending.pop_front()` and spawns it.
Runs start in the planned order; up to `budget.max_parallel` overlap, so completion order may differ.

## Findings
- Gate defect in the `git grep -c` pre/post form described under Gates.
- A cell digest does not depend on the other cells, which the plan test uses to map plans to cells.
