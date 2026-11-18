# level_resolve: per-level planning (A5 scope)

Scope after the A5 ruling: Steps 1 (the `t3_` rows), 2, 3.2 (`check_factors`) and 4. T4 (`ImageService::ensure`
sharing, the `t4_` rows, `guard_image.rs`) moved to `level_wiring`.

## Commits
| Sha | Subject |
|:----|:--------|
| dc826bd6 | test(runner): guard per-level treatment resolution |
| d45813d8 | feat(runner): plan each cell with its own treatment level |
| 027f8cb4 | feat(runner): admit a treatment factor in the CLI preflight |

## Design
`Levels<'a>` holds one `&Resolution` per distinct `request.treatment` level, in declared order. `From<&Resolution>`
is the single level; `From<&[Resolution]>` is the multi-level form. `plan_runs` takes `impl Into<Levels<'a>>`;
`experiment_of`, `check_facts` and every existing caller keep their signatures. A cell's level is the position of its
`request.treatment` value among the factor's distinct values; a cell with no treatment factor uses level 0. Each cell is
built from, and re-checked (`check_facts`) against, its own level's resolution.

## Gates
| Gate | Result |
|:-----|:-------|
| `guard_run_plan::` + `guard_settings_out_of_cell::` | PASS, 16 tests run: 16 passed |
| `guard_cli_refusal::r[23]_` | PASS, 15 tests run: 15 passed |
| DIGEST | PASS (numstat clean; `guard_cell_digest` 9 passed; core digest filter 45 passed) |
| clippy `-p candler-core -p candler --all-targets -D warnings` | PASS |
| fmt | PASS |
| `t3_each_cell_uses_its_level` (two different cell digests) | PASS |

## Claims checked
- `check_factors` reaches `fresh_cells` through `experiment_of(file, &Resolution::default())`: confirmed. It refuses only
  `SettingAsFactor`; a two-level file now gets the unresolved-level error from the empty resolution, which
  `check_factors` ignores on purpose (commented).
- The treatment `commit` is not part of the cell digest (two stored cells differing only in commit share one digest and
  the planner reports them as ambiguous). The level rows therefore differ in the treatment `tag`.

## Bites
| Bite | Red on | Message |
|:-----|:-------|:--------|
| every cell uses level 0 | `t3_each_cell_uses_its_level`, `t3_check_facts_per_level` | `assertion left == right failed` (planned digests are one, not two); `the second cell` (no fresh cell of level 2) |
| skip an unresolved level (fall back to the first resolution) | `t3_unresolved_level_fails_the_plan` | `one resolution: ()` (the plan succeeded) |
| red-first | the rows using `Levels` were red by compile error before Step 2 (the type did not exist); `t3_one_level_plans_the_old_bytes` holds literals taken from `31a103bc` code and was green at both ends | |

The image-cache bite is not in this leaf (T4 moved to `level_wiring`).

## Findings
1. `PlanError` has no variant for a missing level, and a new variant needs arms in `runner/refusal.rs` (not owned) and a
   code. The unresolved level reuses `PlanError::Cell { ExecutionError::Document { detail, path: "/factors" } }`, whose
   code already exists. Its message names the level (`the treatment level "other" has no resolution`). Not tested at the CLI.
2. `t3_failing_level_fails_the_plan` is not a separate row: a failing resolution cannot reach the plan as a `&Resolution`,
   so `t3_unresolved_level_fails_the_plan` (one resolution, and two of three) is the row.
3. `t3_check_facts_per_level` exercises `check_facts` directly (a stored cell has no deviation under its own level and a
   `treatment` deviation under another). That `plan_runs` passes each cell's own resolution is guarded by the cells holding
   their own level in `t3_each_cell_uses_its_level`.
4. Interim for `level_wiring`: `commands/run.rs` still passes one resolution, so a two-level file now fails at the plan
   with the unresolved-level error where it used to plan one collapsed cell. `experiment_of` (public) behaves the same.

## Left
T4 (image sharing, `t4_` rows) and the callers of `plan_runs` with several resolutions: `level_wiring` (L2).
