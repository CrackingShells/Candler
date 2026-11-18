# Golden design

**Goal**: The HTN golden experiment's design (3 models × 4 htn-creator versions × 20 runs) is one Candler experiment file that plans 12 cells, and `analyze` on a store of that shape gives 6 version comparisons blocked by model and 3 model comparisons blocked by version, each equal to an independent second engine (G2, A8).
**Estimate**: about 80 Sonnet calls, $3.5; no verifier (the second engine is the evidence)
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (campaign README, Gotchas); L2 is merged
- [ ] [nl] OWNED (regex): `^crates/candler/tests/it/guard_golden_design\.rs$|^crates/candler/tests/it/main\.rs$|^crates/candler/tests/oracle/golden_design/.*$|^crates/candler/tests/fixtures/golden_design/.*$|^__reports__/m6a-paired-stats/leaves/golden_design_v0\.md$`. In `main.rs`, only the one `mod` line
- [ ] [nl] It reads none of the 240 golden outcomes and needs no `CANDLER_GOLDEN_CORPUS`; the design, not the numbers, is under test (A8)
**Success Gates**:
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_golden_design::/)'`
- ⬜ [cmd] The second engine is independent and current: `! grep -nE '^(from|import) +(candler|gen_stats)' crates/candler/tests/oracle/golden_design/second_engine.py && (cd crates/candler/tests/oracle/golden_design && uv run --offline --no-project --with scipy==1.17.0 --with statsmodels==0.15.0 python second_engine.py --check)`
- ⬜ [cmd] `tests/oracle` gains only new files: `git diff --numstat $BASE_LEAF -- crates/candler/tests/oracle | awk '$2!=0{b=1}END{exit b}'`
- ⬜ [nl] The report records the planted effect size and why it has power, and each Bite
**References**: [R03 §Statistical design, §Treatment arms](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md); `m6a-l3-addendum.md` (same folder) §golden_design; A7 (table levels on `request.treatment`)

## Step 1: The plan row
**Goal**: The golden design is expressible as one experiment file.
**Implementation Logic**:
1. Start from an HTN experiment file already in the repo (`crates/candler-schemas/tests/vectors/experiment/cont_train.toml` if it fits), copied into `crates/candler/tests/fixtures/golden_design/`. It declares `model.id` at 3 levels and `request.treatment` at 4 table levels `{name = "htn-creator", tag = …}` (A7 form), 20 replicates.
2. The test builds a fixture git repo with 4 tags in a temp dir (the way the existing treatment rows do), points each level's `source` at it, and plans.
3. `gd_plan_twelve_cells`: 12 cells and 240 runs; each (model, version) pair is one cell, exactly once; each cell's treatment record is its own level's.
**Deliverables**: `crates/candler/tests/fixtures/golden_design/experiment.toml`; `crates/candler/tests/it/guard_golden_design.rs`: `gd_plan_twelve_cells`
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_golden_design::gd_plan_/)'` (expected: PASS)
**Commit**: `test(analysis): plan the golden experiment design as one file`

## Step 2: The second engine
**Goal**: An independent expected file for the golden design's comparisons.
**Implementation Logic**:
1. `second_engine.py` builds the store of Step 3 from the same seed and planted rates (written once in a small TOML both sides read, `design.toml`), builds the blocks itself (the other factor's levels), and computes per comparison and expectation: scipy `hypergeom` mid-p for one block; statsmodels `StratifiedTable` (`test_null_odds(correction=False)`, `oddsratio_pooled`, `oddsratio_pooled_confint`) and the Mantel–Fleiss criterion for two or more blocks; the exact conditional p by convolution where Mantel–Fleiss fails; the stratified permutation p and BCa interval for the weighted expectation if its draws can be matched, else the weighted rows check the battery choice, the per-block differences and the point estimate only (record which); per-block B − A; Holm per comparison and role.
2. Reuse the planned engine form: no Candler import; `uv run --offline --no-project --with scipy==1.17.0 --with statsmodels==0.15.0 python second_engine.py` writes `expected.toml`; `--check` exits non-zero on drift.
**Deliverables**: `crates/candler/tests/oracle/golden_design/{second_engine.py,design.toml,expected.toml}`: `build_store`, `blocks_of`, `midp`, `cmh`, `exact_conditional`, `holm`
**Consistency Checks**: the second-engine gate above (expected: PASS)
**Commit**: `test(analysis): add a second engine for the golden design`

## Step 3: The analysis rows
**Goal**: `analyze` on the golden shape equals the second engine and reads the planted truth.
**Implementation Logic**:
1. Build a store of the plan's shape in the test (binary expectations and one weighted expectation). Planted: one version pair with a real effect in 2 of the 3 models, one null pair; size the effect for power and record the size and its power argument in the report.
2. `gd_comparisons_shape`: 6 version comparisons with 3 blocks each and 3 model comparisons with 4 blocks each.
3. `gd_agrees_with_second_engine`: battery, p, effect, interval, per-block differences and Holm (per comparison, per role) equal `expected.toml` within 1e-9.
4. `gd_readings`: "effect" on the planted pair, "inconclusive" on the null pair.
5. `gd_level_of_reads_prefix_object`: guards `analysis/blocking.rs::level_of` reading an object under a prefix factor (two treatment levels that differ only inside the `request.treatment` object fall in different arms); unguarded since L2.
**Deliverables**: `crates/candler/tests/it/guard_golden_design.rs`: `gd_comparisons_shape`, `gd_agrees_with_second_engine`, `gd_readings`, `gd_level_of_reads_prefix_object`
**Consistency Checks**: the targeted test (expected: PASS)
**Commit**: `test(analysis): guard the golden design comparisons`

## Step 4: Bites and the report
**Goal**: The rows bite.
**Implementation Logic**:
1. Bites, each injected, seen red, restored from a copy: the pooled test instead of the blocked one; blocks taken from all factors including the compared one; continuity correction on; `level_of` reading the exact path only. Record each failing message.
2. Record the planted size and power, and which weighted quantities the engine matches.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/golden_design_v0.md`: sections "Design", "Bites"
**Consistency Checks**: `grep -q '^## Bites' __reports__/m6a-paired-stats/leaves/golden_design_v0.md` (expected: PASS)
**Commit**: `docs(analysis): record the bites of the golden design`
