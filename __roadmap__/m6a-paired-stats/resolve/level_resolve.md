# Level resolve

**Goal**: A file declaring two or more treatment levels resolves each distinct level once, in declared order, before planning (T3); a failing level fails the plan; each cell and `check_facts` use their own level's resolution; equal lock bytes give one image through `ImageService::ensure` (T4); a one-treatment file resolves once and plans the bytes it plans today.
**Estimate**: about 90 Sonnet calls, $3.7; verified with `level_wiring` at L2 (one verifier for the planning chain)
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (campaign README, Gotchas); `treatment_setting` is merged: `test -f __reports__/m6a-paired-stats/leaves/treatment_setting_v0.md`
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/runner/plan\.rs$|^crates/candler/src/commands/preflight\.rs$|^crates/candler-core/tests/it/guard_(run_plan|image)\.rs$|^__reports__/m6a-paired-stats/leaves/level_resolve_v0\.md$`. In `plan.rs`: `fresh_cells`, `plan_runs`, `check_facts` and a new `Levels` type; in `preflight.rs`: `check_factors`. The callers in `commands/run.rs` and the job side are `level_wiring`'s (L2): until then they pass the single resolution as the one level
**Success Gates**:
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_run_plan::/) | test(/^guard_settings_out_of_cell::/) | test(/^guard_image::/)'`
- ⬜ [cmd] One level plans the old bytes: a `t3_` row compares the planned runs of a one-treatment file with those planned at `$BASE_LEAF` (written into the row)
- ⬜ [cmd] Two levels with different lock bytes give two image ids, and two with equal lock bytes give one `ensure` call, counted by a fake `ImageService` modelled on `guard_image::b05_`: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_image::t4_/)'`
- ⬜ [cmd] DIGEST passes; ownership check prints nothing
- ⬜ [nl] The report records each Bite
**References**: [R03 T3, T4](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md); [R04 A7 findings](../../../__reports__/candler-v1-review/03-findings_amendment_a7_v0.md) — the resolve step order (treatment, plug-in env, ground truth, image, harness, catalog, session, egress)

## Step 1: Write the t3_ and t4_ rows, seen red
**Goal**: Rows that pin per-level resolution and image sharing.
**Implementation Logic**:
1. In `guard_run_plan.rs`: `t3_one_level_plans_the_old_bytes`; `t3_each_cell_uses_its_level` (two levels, each cell's planned facts come from its own level, and two levels whose `treatment` differs give two cells with different cell digests: the row moved here from `treatment_setting`, which cannot reach the cell while `fresh_cells` drops setting paths); `t3_failing_level_fails_the_plan` (the second level's resolution fails, so the plan fails and names the level); `t3_check_facts_per_level`.
2. In `guard_image.rs`: `t4_distinct_locks_two_images`, `t4_equal_locks_one_ensure`, with a counting fake `ImageService` in the shape of `b05_`'s.
3. `Guards:` and `Oracle:` lines; expected values written in the rows, never read from the new code.
**Deliverables**: `crates/candler-core/tests/it/guard_run_plan.rs`: the four `t3_` rows; `crates/candler-core/tests/it/guard_image.rs`: the two `t4_` rows
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_run_plan::t3_/) | test(/^guard_image::t4_/)'` (expected: FAIL)
**Commit**: `test(runner): guard per-level treatment resolution`

## Step 2: Plan with one resolution per level
**Goal**: `plan_runs` and `check_facts` take `Levels`, one resolution per distinct treatment level in declared order.
**Implementation Logic**:
1. `pub struct Levels` maps each distinct treatment level (in declared order) to its `Resolution`; `Levels::single(resolution)` is what a one-treatment file and today's callers pass.
2. `fresh_cells` tags each cell with its level index; `plan_runs` and `check_facts` look up the cell's own resolution; a level whose resolution failed fails the plan with the level named.
3. No resolve call is made here: resolution happens in the caller, before planning.
**Deliverables**: `crates/candler-core/src/runner/plan.rs`: `Levels`, `Levels::single`, `fresh_cells`, `plan_runs`, `check_facts`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_run_plan::/) | test(/^guard_settings_out_of_cell::/)'` (expected: PASS)
**Commit**: `feat(runner): plan each cell with its own treatment level`

## Step 3: Share images between equal locks, and check factors per level
**Goal**: T4 holds, and the CLI preflight admits a treatment factor.
**Implementation Logic**:
1. Image planning goes through `ImageService::ensure` once per distinct lock bytes; levels with equal locks reuse the id (the existing cache, not a second one).
2. `commands/preflight.rs::check_factors` admits `request.treatment` as `fresh_cells` does, and refuses every other setting as today (`guard_cli_refusal::r2_`/`r3_` unchanged).
**Deliverables**: `crates/candler-core/src/runner/plan.rs`: the image step of `plan_runs`; `crates/candler/src/commands/preflight.rs`: `check_factors`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_image::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_cli_refusal::r[23]_/)'` (expected: PASS)
**Commit**: `feat(runner): share one image between treatment levels with equal locks`

## Step 4: Bites and DIGEST
**Goal**: The rows bite, and no digest moved.
**Implementation Logic**:
1. Bites: every cell uses level 0 (red on `t3_each_cell_`); skip a failed level (red on `t3_failing_`); key the image cache by level instead of lock bytes (red on `t4_equal_locks_`).
2. Run DIGEST. Record the bites.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/level_resolve_v0.md`: section "Bites"
**Consistency Checks**: `grep -q '^## Bites' __reports__/m6a-paired-stats/leaves/level_resolve_v0.md` (expected: PASS)
**Commit**: `docs(runner): record the bites of per-level resolution`
