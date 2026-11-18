# Level resolve

**Goal**: A file declaring two or more treatment levels resolves each distinct level once, in declared order, before planning (T3); a failing level fails the plan; each cell and `check_facts` use their own level's resolution; a file with a level left without a resolution fails the plan naming it; a one-treatment file resolves once and plans the bytes it plans today.
**Estimate**: about 90 Sonnet calls, $3.7; verified with `level_wiring` at L2 (one verifier for the planning chain). Amended by A5: T4 moved to `level_wiring`
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (campaign README, Gotchas); `treatment_setting` is merged: `test -f __reports__/m6a-paired-stats/leaves/treatment_setting_v0.md`
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/runner/plan\.rs$|^crates/candler/src/commands/preflight\.rs$|^crates/candler-core/tests/it/guard_run_plan\.rs$|^__reports__/m6a-paired-stats/leaves/level_resolve_v0\.md$`. In `plan.rs`: `fresh_cells`, `plan_runs`, `check_facts` and a new `Levels` type; in `preflight.rs`: `check_factors`. The callers in `commands/run.rs` and the job side are `level_wiring`'s (L2): until then they pass the single resolution unchanged, through `From<&Resolution>`
**Success Gates**:
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_run_plan::/) | test(/^guard_settings_out_of_cell::/)'`
- ⬜ [cmd] One level plans the old bytes: a `t3_` row compares the planned runs of a one-treatment file with those planned at `$BASE_LEAF` (written into the row)
- ⬜ [cmd] Two levels whose `treatment` differs give two cells with two cell digests, and a level without a resolution fails the plan: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_run_plan::t3_/)'`
- ⬜ [cmd] DIGEST passes; ownership check prints nothing
- ⬜ [nl] The report records each Bite
**References**: [R03 T3, T4](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md); [R04 A7 findings](../../../__reports__/candler-v1-review/03-findings_amendment_a7_v0.md) — the resolve step order (treatment, plug-in env, ground truth, image, harness, catalog, session, egress)

## Step 1: Write the t3_ rows, seen red
**Goal**: Rows that pin per-level resolution.
**Implementation Logic**:
1. In `guard_run_plan.rs`: `t3_one_level_plans_the_old_bytes`; `t3_each_cell_uses_its_level` (two levels, each cell's planned facts come from its own level, and two levels whose `treatment` differs give two cells with different cell digests: the row moved here from `treatment_setting`, which cannot reach the cell while `fresh_cells` drops setting paths); `t3_unresolved_level_fails_the_plan` (a level without a resolution, whether one resolution for two levels or two for three, fails the plan and names the level; a failed resolve never reaches the plan as a `Resolution`, so this row also covers a failing level); `t3_check_facts_per_level`.
2. `Guards:` and `Oracle:` lines; expected values written in the rows, never read from the new code.
**Deliverables**: `crates/candler-core/tests/it/guard_run_plan.rs`: the four `t3_` rows
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_run_plan::t3_/)'` (expected: FAIL)
**Commit**: `test(runner): guard per-level treatment resolution`

## Step 2: Plan with one resolution per level
**Goal**: `plan_runs` and `check_facts` take `Levels`, one resolution per distinct treatment level in declared order.
**Implementation Logic**:
1. `pub struct Levels<'a>` borrows one `Resolution` per distinct treatment level (in declared order); `impl<'a> From<&'a Resolution> for Levels<'a>` is the single level, and `plan_runs` takes `levels: impl Into<Levels<'a>>`, so no existing caller or guard changes.
2. `fresh_cells` tags each cell with its level index; `plan_runs` and `check_facts` use the cell's own resolution; a level without a resolution fails the plan with the level named, through an existing error variant that already has a code.
3. No resolve call is made here: resolution happens in the caller, before planning.
**Deliverables**: `crates/candler-core/src/runner/plan.rs`: `Levels`, `Levels::single`, `fresh_cells`, `plan_runs`, `check_facts`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_run_plan::/) | test(/^guard_settings_out_of_cell::/)'` (expected: PASS)
**Commit**: `feat(runner): plan each cell with its own treatment level`

## Step 3: Check factors in the CLI preflight
**Goal**: The CLI preflight admits a treatment factor (T4 moved to `level_wiring`, A5).
**Implementation Logic**:
1. `commands/preflight.rs::check_factors` admits `request.treatment` as `fresh_cells` does, and refuses every other setting as today (`guard_cli_refusal::r2_`/`r3_` unchanged).
**Deliverables**: `crates/candler/src/commands/preflight.rs`: `check_factors`
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_cli_refusal::r[23]_/)'` (expected: PASS)
**Commit**: `feat(runner): admit a treatment factor in the CLI preflight`

## Step 4: Bites and DIGEST
**Goal**: The rows bite, and no digest moved.
**Implementation Logic**:
1. Bites: every cell uses level 0 (red on `t3_each_cell_`); fall back to the first resolution for a level without one (red on `t3_unresolved_`).
2. Run DIGEST. Record the bites.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/level_resolve_v0.md`: section "Bites"
**Consistency Checks**: `grep -q '^## Bites' __reports__/m6a-paired-stats/leaves/level_resolve_v0.md` (expected: PASS)
**Commit**: `docs(runner): record the bites of per-level resolution`
