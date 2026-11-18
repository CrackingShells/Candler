# Execution order

**Goal**: `plan_runs` orders runs by one pure function `run_order(cells, replicates, seed)` in a new `runner/order.rs`: each replicate round holds every cell once, in an order shuffled from the experiment's digest (B3), so independence of runs is argued from the design. The old loop is removed and no copy of it remains.
**Estimate**: about 80 Sonnet calls, $3.3; no verifier (the `git grep -c` gate and the property rows are its evidence)
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (README, Gotchas); today's loop exists once: `test "$(git grep -c 'for replicate in 1..=' -- crates/candler-core/src/runner/plan.rs | cut -d: -f2)" = 1`
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/runner/(order|mod|plan)\.rs$|^crates/candler-core/tests/it/guard_run_plan\.rs$|^__reports__/m6a-paired-stats/leaves/execution_order_v0\.md$`. In `plan.rs`, only the tail of `plan_runs` (near line 985); `treatment_setting` edits `fresh_cells` (near line 369) in the same level and the hunks auto-merge. In `runner/mod.rs`, only the `mod order;` line and the `pub use self::order::{run_order, seed_of};` re-export. By supervisor ruling, `^crates/candler-core/tests/it/guard_job_orchestrate\.rs$` is also owned for the one `b34_` hunk: `b27_` and `b34_` pinned declaration order and were rewritten, `b34_` checking the in-flight pair against `run_order` on the job's seed
**Success Gates**:
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_run_plan::/) | test(/^guard_job_orchestrate::/)'`
- ⬜ [cmd] One replicate loop remains, inside `run_order`: `! git grep -q 'for replicate in 1..=' -- crates/candler-core/src/runner/plan.rs || exit 1; test "$(git grep -c 'for replicate in 1..=' -- crates/candler-core/src/runner/order.rs | cut -d: -f2)" = 1`
- ⬜ [cmd] DIGEST passes (order changes no run key or digest)
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] `Scheduler.run` runs plans in the order given (the report cites the code), so the planned order holds at execution; existing guards that pinned today's order are listed in the report with the coordinator's ruling before they change
**References**: [R03 B3](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md); `runner/plan.rs`: `plan_runs`

## Step 1: Write the b3_ property rows, seen red
**Goal**: Properties of the order, not one frozen sequence.
**Implementation Logic**:
1. `b3_each_round_holds_each_cell_once`: for several cell and replicate counts, every replicate round is a permutation of the cells and rounds come in replicate order.
2. `b3_same_seed_same_order` and `b3_other_seed_other_order` (for a design with at least 6 cells, two seeds give two orders).
3. `b3_plan_runs_uses_the_experiment_seed`: two files differing only in a digest-relevant field plan different orders; the same file planned twice plans the same order.
**Deliverables**: `crates/candler-core/tests/it/guard_run_plan.rs`: the four `b3_` rows
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_run_plan::b3_/)'` (expected: FAIL)
**Commit**: `test(runner): guard a seeded run order`

## Step 2: Add run_order
**Goal**: A pure, seeded order function.
**Implementation Logic**:
1. `pub fn run_order(cells: usize, replicates: u32, seed: u64) -> Vec<(usize, u32)>`: for each replicate in order, a Fisher–Yates shuffle of the cell indices with the scaffold's `analysis::splitmix::SplitMix64` (no second generator).
2. The seed is the first 8 bytes, little-endian, of the experiment digest `plan_runs` already has in scope; name the value in a doc comment.
**Deliverables**: `crates/candler-core/src/runner/order.rs`: `run_order`, `seed_of`; `crates/candler-core/src/runner/mod.rs`: `mod order;`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_run_plan::b3_each_/) | test(/^guard_run_plan::b3_same_/) | test(/^guard_run_plan::b3_other_/)'` (expected: PASS)
**Commit**: `feat(runner): add a seeded run order`

## Step 3: Iterate run_order in plan_runs
**Goal**: `plan_runs` has one source of order, and the old loop is gone.
**Implementation Logic**:
1. Replace the nested cell and replicate loops at the tail of `plan_runs` with one loop over `run_order(..)`; the body that builds each planned run moves unchanged.
2. Run DIGEST and the targeted test. If an existing guard pinned today's order, stop and report it to the coordinator before changing it.
**Deliverables**: `crates/candler-core/src/runner/plan.rs`: `plan_runs`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_run_plan::/) | test(/^guard_job_orchestrate::/)'` (expected: PASS)
**Commit**: `refactor(runner): plan runs in the seeded order`

## Step 4: Record the bites
**Goal**: The rows are shown to bite.
**Implementation Logic**:
1. Bites: shuffle across rounds (red on `b3_each_round_`); a constant seed (red on `b3_plan_runs_uses_`); keep the old loop beside the new one (the `git grep -c` gate goes red).
2. Record each, and cite the `Scheduler.run` code that keeps the order.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/execution_order_v0.md`: sections "Bites", "Scheduler order"
**Consistency Checks**: `grep -q '^## Bites' __reports__/m6a-paired-stats/leaves/execution_order_v0.md` (expected: PASS)
**Commit**: `docs(runner): record the bites of the run order`
