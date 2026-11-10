# settings_out_of_cell: leaf report (v0)

Type: findings. Branch `m2c/settings_out_of_cell`, base `6a26fc6` (rebased after the amendment).

## Verdict
P4-F1 is fixed for every `[fixed]` setting. A setting written in the file no longer reaches `comparable.values` or the cell digest, and the resolution still holds it as a setting. Planning now also refuses a setting given as a factor (`PlanError::SettingAsFactor`). Two parts of the original spec were defective (the `request.inputs` digest row, the factor-level row); both are findings below and were amended. No expected digest moved.

## Commits
| sha | subject |
|:----|:--------|
| 129e2a7 | fix(experiment): keep settings written in the file out of the cell |
| (this) | reports(experiment): record the settings_out_of_cell leaf |

## Gates
| Gate | Result | Last relevant line |
|:-----|:-------|:-------------------|
| C1 | PASS | 2 tests run: 2 passed |
| `guard_cell_digest`, `guard_experiment_file` (schemas) | PASS | 17 passed |
| `guard_run_plan`, `guard_job_orchestrate`, `guard_import_mini` | PASS | 14 passed |
| GOLDEN (`guard_golden`, `guard_oracle_ac`) | PASS | 2 passed (64 s); 1 passed |
| STD, `-p candler-core -p candler` (nextest) | PASS | 209 run, 209 passed, 19 skipped (the ignored golden set) |
| STD clippy, fmt, doc | PASS | clippy `-D warnings` clean, `cargo fmt --check` clean, doc `-D warnings` clean |
| LINUX (`x86_64-unknown-linux-gnu`, `-p candler-core -p candler`) | PASS | Finished, no warnings |
| SPAWN (`guard_signal`) | PASS | 9 passed, after `cargo clean -p candler-core` |
| SUBJ | PASS | `cog check`: no errored commits; subjects at most 72 characters |
| OWNED | PASS | the diff lists `runner/plan.rs` and `tests/guard_settings_out_of_cell.rs` (plus this report) |
| `fn confirm_` left | none | |

## Checks before coding
- **Classification.** Every C1 path is a setting by `is_setting`: `isolation.build.requirements` and `.prune` (any `isolation.build.*`), `isolation.record_packages`, `request.inputs` and `model.token_secret`. No row was dropped for classification.
- **Settings still reach the resolution.** The route `candler run` uses puts `[fixed]` into the file layer (`request_settings`), so `Layered::resolve` sorts the path into `Resolution::settings`; `fresh_cells` only changes which paths the cell takes. C1 asserts it: the resolution holds each setting with its written value (a location reads as a path below the file's directory); the image step, through a recording `ImageService`, sees the prune list `["a.py","b.py"]` in `BuildSetting.prune` and the requirement source `req.txt` in `LockRequest`; `record_packages(&resolution)` returns `["numpy"]`.
- **One filter or two.** One chain, two predicates. `fresh_cells` still drops explicit (`--set`) paths by `overlaps` with the resolution's explicit sources; the file's cell values are now also dropped by the free function `is_setting`. The values of the resolution were never the leak (`Layered::resolve` already routes settings away from `values`). Settings that only a `--set` marks (`Layered::settings`, outside `is_setting`) reach `fresh_cells` through the explicit-overlap predicate, as before.

## Guards
Both in `crates/candler-core/tests/guard_settings_out_of_cell.rs`, planned through file layer, `ImageIdStep`, `Resolver`, `plan_runs`.

**`settings_out_of_cell_fixed_setting_never_reaches_the_cell`.** Guards: R01 §Agent environment and secrets: a setting in `[fixed]` is a setting of the run and not a value of the cell. Oracle: the hand-written table (requirements, prune, record_packages, token_secret, and `request.inputs`), the control's digest, and the control's hand-written value paths (`budget.per_run.wall_s`, `isolation.env_lock`, `isolation.image`, `model.id`, `prompt.template`).
- Bite 1, the filter removed (base behaviour): every row red. Messages seen: `requirements: \`isolation.build.requirements\` is a setting and must not be in \`comparable.values\`: [... "isolation.build.requirements" ...]` and `requirements: the cell digest differs from the control's`; the same two lines for prune, record_packages and token_secret; for `request.inputs`: `a setting in \`comparable.values\`` and `the values differ from the control's`.
- Bite 2, the filter drops all the file's cell values: `the control keeps every cell value of the file ... left: ["isolation.env_lock", "isolation.image", "prompt.template"]`.

**`settings_out_of_cell_setting_as_factor_is_refused`.** Guards: same section: an experiment that gives a setting as a factor is refused by planning with `PlanError::SettingAsFactor` naming the setting leaf, so no level is lost. Rows: the factor at `isolation.build.prune`; at its parent `isolation.build` (levels `{prune=[..]}`); at `isolation` (levels `{build={prune=[..]}}`); at `model` (levels `{token_secret="A"|"B"}`); at `budget` (levels `{max_runs=1|2}`, naming `budget.max_runs`). Controls: `budget` with levels `{per_run={turns=5|6}}` plans two cells (`budget` is a setting prefix, but the levels fill only cell values; the file's own `[budget.per_run]` is required by the schema, and a factor over its `wall_s` fails on the base too with a repeated entry, so the control fills `turns`); `model` with levels `{id="a"}` and `{id="b"}` still plans two cells. Oracle: the hand-written leaf path in the variant and message, and the levels counted by hand. Bite 1, the check disabled: red with `a setting given as a factor was planned instead of refused` (the collapse measured: one cell, no such setting in the resolution, the image step asked with an empty prune list). Bite 3, the own-path check put back for table levels: the `budget` control goes red with `plans: SettingAsFactor { path: "budget" }`. Bite 2, the check on the factor's own path only (the table levels not walked): red on the parent row, ``\`isolation.build.prune\` given as a factor was planned instead of refused: [[factors]] path = "isolation.build" ...``.

## Moved digests
None. 209 of 209 pass in `candler-core` and `candler` with no expected value edited, and `git diff --numstat` shows no change under the oracle and vector directories. No existing experiment file or test writes a setting in `[fixed]` (the repo's TOML files and the CLI fixtures write none). GOLDEN and `guard_import_mini` did not move: imports do not go through `fresh_cells`.

## Findings
1. **Spec defect (accepted, row kept): the `request.inputs` row cannot hold "digest equals the control's".** `request.inputs` carries the agent's declared inputs; the image step derives the `inputs` root from it, and `comparable.inputs` is part of the cell (D2). The row asserts that `comparable.values` has the control's value paths and no `request.inputs`, and that `comparable.inputs` is the declared list.
2. **Spec defect (accepted, fixed in the core): a setting given as a factor was lost.** The resolution is built from `[fixed]` only, so the levels collapsed into one cell and the setting never reached a step. `fresh_cells` now refuses a factor that varies a setting, before any cell is built. A value level is decided by the factor's own path; a table level only by the leaves it fills, so `budget` over `per_run` levels plans and `budget` over a `max_runs` level is refused (`Level::Fill` fills the leaves under the factor's path, so a factor at a parent table such as `isolation.build` or `model` varies them), with `PlanError::SettingAsFactor { path }`, `path` naming the setting leaf ("the setting {path} cannot be a factor; write it in [fixed]"). The schemas are unchanged.
3. **Routes that build cells from the file, all through `fresh_cells`.** `experiment_of` and `plan_runs` are its only callers. They are called by `candler run` (`commands/run.rs`: `experiment_of`, then `plan_runs` in `PlanMode::Check`) and by the orchestrator (`jobs/orchestrate/mod.rs`: `experiment_of`, then `plan_runs` in `PlanMode::Hold`, which is also what a resume runs). The orchestrator maps the error to a refusal of the job (`the experiment cannot be identified`). `--set` creates explicit values, not factors, so it has no factor route. No other subcommand (`evaluate`, `diff`, `show`, `watch`, `ps`, `kill`, `import`, `supervise`) calls `cells()` or the planner; `cells_of` in `session.rs` only reads the factor paths to label cells that `plan_runs` has already built. No route builds cells without the check.
4. No change to `is_setting`, the schemas or the shared constraints.
