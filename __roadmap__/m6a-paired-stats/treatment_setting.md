# Treatment setting

**Goal**: An experiment may declare `request.treatment` as a factor (T1, T2): `fresh_cells` admits that one setting, because it resolves into the hashed typed root `treatment`, and every other setting-valued factor still raises `SettingAsFactor` (I7). No digest moves.
**Estimate**: about 80 Sonnet calls, $3.3; no verifier (DIGEST and the refusal rows are its evidence)
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (README, Gotchas)
- [ ] [cmd] Which settings the refusal rows use is listed in the leaf report before any edit: `git grep -n 'request\.\|SettingAsFactor' -- crates/candler-core/tests/it/guard_settings_out_of_cell.rs crates/candler/tests/it/guard_cli_refusal.rs > /dev/null`, and the report's section "Refusal rows" names each `r2_`, `r3_` and `settings_out_of_cell` row with its setting
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/runner/plan\.rs$|^crates/candler-core/tests/it/guard_settings_out_of_cell\.rs$|^__reports__/m6a-paired-stats/leaves/treatment_setting_v0\.md$`. In `plan.rs`, only `setting_of_factor` and `fresh_cells` (near line 369); `execution_order` edits the `plan_runs` tail (near line 985) in the same level, about 600 lines away, and the two hunks auto-merge. `is_setting` (`resolve/layers.rs`) is not touched
**Success Gates**:
- ⬜ [cmd] Targeted tests: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_settings_out_of_cell::/) | test(/^guard_run_plan::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_cli_refusal::r[23]_/)'`
- ⬜ [cmd] A factor on any other `request` setting, and a factor on the `request` parent table, are still refused: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_settings_out_of_cell::t2_/)'`
- ⬜ [cmd] DIGEST passes, and a one-treatment file's `experiment_of` digest is byte-identical to `$BASE_LEAF`'s (a `t2_` row pins it)
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] The report records each Bite, and says whether `commands/preflight.rs::check_factors` (owned by `level_resolve`, not here) still refuses a treatment factor at the CLI; until `level_resolve` lands, no CLI path plans two treatment levels
**References**: [R04 A7 findings](../../__reports__/candler-v1-review/03-findings_amendment_a7_v0.md) — why a setting factor is refused today; [R03 T1, T2](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md)

## Step 1: Write the t2_ rows, seen red
**Goal**: Rows that pin the admission, the refusals that stay, and the one-treatment digest.
**Implementation Logic**:
1. In `guard_settings_out_of_cell.rs`, a `t2_` table: `request.treatment` with two levels is admitted by `fresh_cells` and yields one cell per level whose cell value at `treatment` differs; every other setting `is_setting` names, and the bare `request` parent table, still raise `SettingAsFactor`; a file with one treatment and no factor gives the same `experiment_of` digest as `$BASE_LEAF` (the expected digest is computed on `$BASE_LEAF` and written into the row, not taken from the new code).
2. `Guards:` and `Oracle:` lines on each test. The admission row is red today.
**Deliverables**: `crates/candler-core/tests/it/guard_settings_out_of_cell.rs`: `t2_treatment_is_the_one_admitted_setting`, `t2_other_settings_stay_refused`, `t2_one_treatment_digest_unchanged`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_settings_out_of_cell::t2_/)'` (expected: FAIL)
**Commit**: `test(plan): guard treatment as the one setting factor`

## Step 2: Admit the treatment factor
**Goal**: `fresh_cells` admits `request.treatment` and nothing else.
**Implementation Logic**:
1. `setting_of_factor` returns `None` for exactly `request.treatment` (a named constant beside `is_setting`'s list, not a string literal repeated), so `fresh_cells` places the level under the typed root `treatment` instead of refusing it.
2. Nothing else in `fresh_cells` changes; `plan_runs` is not touched here.
**Deliverables**: `crates/candler-core/src/runner/plan.rs`: `setting_of_factor`, `fresh_cells`, `TREATMENT_FACTOR`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_settings_out_of_cell::/) | test(/^guard_run_plan::/)'` (expected: PASS)
**Commit**: `feat(plan): admit the treatment as a declared factor`

## Step 3: Bites, DIGEST and the report
**Goal**: The rows bite, and no digest moved.
**Implementation Logic**:
1. Bites: admit every `request.*` setting (red on `t2_other_settings_stay_refused`); admit by prefix `request.treat` (red on a row with a lookalike path); put the level into the cell outside `treatment` (red on the digest row).
2. Run DIGEST and the CLI refusal rows. Record the bites, the refusal-row list and the `check_factors` answer.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/treatment_setting_v0.md`: sections "Refusal rows", "Bites", "CLI preflight"
**Consistency Checks**: `grep -q '^## Bites' __reports__/m6a-paired-stats/leaves/treatment_setting_v0.md` (expected: PASS)
**Commit**: `docs(plan): record the bites of the treatment factor`
