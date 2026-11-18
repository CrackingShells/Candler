# Golden blocked

**Goal**: On the 240 golden runs, every blocked comparison `analyze` reports equals a second engine that imports no Candler file, and permuting version labels within model blocks gives a rejection rate near α (G2).
**Estimate**: about 80 Sonnet calls, $3.3; no verifier (the independence gate is a `[cmd]`)
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (campaign README, Gotchas); L2 is merged
- [ ] [nl] OWNED (regex): `^crates/candler/tests/it/guard_analyze_golden\.rs$|^crates/candler/tests/oracle/golden_blocked/.*$|^__reports__/m6a-paired-stats/leaves/golden_blocked_v0\.md$`
- [ ] [nl] The golden reference is htn-creator-eval 0.4.2 (the 240 runs), never `harness/evals/eval.json`; RiVault Reasoning-Small runs are not comparable
**Success Gates**:
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'test(/^guard_analyze_golden::blocked_/)'`
- ⬜ [cmd] The second engine imports no Candler file: `! grep -nE '^(from|import) +(candler|gen_stats)' crates/candler/tests/oracle/golden_blocked/second_engine.py`, and it reads only the golden store and writes only `expected.toml`
- ⬜ [cmd] `tests/oracle` gains only new files: `git diff --numstat $BASE_LEAF -- crates/candler/tests/oracle | awk '$2!=0{b=1}END{exit b}'`
- ⬜ [cmd] Permutation check: 1 000 within-model-block relabelings; the rejection rate at α = 0.05 lies in its 99 % binomial interval, about [0.033, 0.069]: `guard_analyze_golden::blocked_permutation_rate` passes
- ⬜ [nl] The report records the golden probe's answer (are eval tasks separate cells, so blocks?) and each Bite
**References**: [R03 §Statistical design, §Known truth](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md); the campaign README's golden probe and C5

## Step 1: Write the second engine
**Goal**: An independent expected file for the golden comparisons.
**Implementation Logic**:
1. `second_engine.py` reads the golden store's run outcomes (the format `guard_analyze_golden` already loads), builds the blocks of C5 itself, and computes per comparison and expectation: scipy `hypergeom` mid-p for one block; statsmodels `StratifiedTable` (`test_null_odds(correction=False)`, `oddsratio_pooled`, `oddsratio_pooled_confint`) and the Mantel–Fleiss criterion for two or more; the exact conditional p by convolution where Mantel–Fleiss fails; Holm per comparison and role.
2. Run with `uv run --offline --no-project --with scipy==1.17.0 --with statsmodels==0.15.0 python second_engine.py`; it writes `expected.toml`; `--check` exits non-zero on drift.
**Deliverables**: `crates/candler/tests/oracle/golden_blocked/second_engine.py`: `load_runs`, `blocks_of`, `midp`, `cmh`, `exact_conditional`, `holm`; `crates/candler/tests/oracle/golden_blocked/expected.toml`
**Consistency Checks**: `(cd crates/candler/tests/oracle/golden_blocked && uv run --offline --no-project --with scipy==1.17.0 --with statsmodels==0.15.0 python second_engine.py --check)` (expected: PASS)
**Commit**: `test(analysis): add a second engine for the golden comparisons`

## Step 2: Golden agreement and permutation rows
**Goal**: `analyze` equals the second engine, and the permutation rate is near α.
**Implementation Logic**:
1. `blocked_agrees_with_second_engine` (ignored by default, as the other golden rows): every comparison's battery, p, effect and interval equal `expected.toml` within 1e-9.
2. `blocked_permutation_rate`: relabel versions within each model block 1 000 times with a fixed SplitMix64 seed, run the blocked test each time, assert the rejection rate at 0.05 lies in [0.033, 0.069]. Record the measured runtime.
**Deliverables**: `crates/candler/tests/it/guard_analyze_golden.rs`: `blocked_agrees_with_second_engine`, `blocked_permutation_rate`
**Consistency Checks**: `cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'test(/^guard_analyze_golden::blocked_/)'` (expected: PASS)
**Commit**: `test(analysis): guard the golden blocked comparisons`

## Step 3: Bites and the report
**Goal**: The rows bite.
**Implementation Logic**:
1. Bites: pooled (unblocked) test in Rust (red on `blocked_agrees_`); permutation across blocks (expected: the rate leaves the interval, or record that it does not and why); continuity correction on (red). Record each.
2. Record the golden probe's answer and the permutation runtime.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/golden_blocked_v0.md`: sections "Probe", "Bites"
**Consistency Checks**: `grep -q '^## Bites' __reports__/m6a-paired-stats/leaves/golden_blocked_v0.md` (expected: PASS)
**Commit**: `docs(analysis): record the bites of the golden comparisons`
