# Known truth

**Goal**: Simulated experiments with planted truth go through the real `analyze`: under H0 the false-positive rate's Monte Carlo interval contains α, the power at a 0.2 difference is reported, a Simpson H0 case shows the blocked test nominal where the pooled one is not, and a two-treatment job runs end to end on fakes (G8).
**Estimate**: about 80 Sonnet calls, $3.3; no verifier (the simulations are the evidence)
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (campaign README, Gotchas); L2 is merged
- [ ] [nl] OWNED (regex): `^crates/candler-core/tests/it/guard_known_truth\.rs$|^crates/candler-core/tests/it/main\.rs$|^crates/candler-core/tests/it/support/fake_jobs\.rs$|^crates/candler-core/tests/it/support/mod\.rs$|^crates/candler-core/tests/it/guard_job_orchestrate\.rs$|^__reports__/m6a-paired-stats/leaves/known_truth_v0\.md$`. `guard_job_orchestrate.rs` only to move its fakes into `support/fake_jobs.rs`, with no row changed; if the fakes already live in `support/job_orchestrate.rs`, reuse them and skip the move (A8). In `support/mod.rs`, only the one `mod` line
**Success Gates**:
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_known_truth::/) | test(/^guard_job_orchestrate::/)'`
- ⬜ [cmd] H0, binary: 2 000 simulated experiments; the 99 % binomial interval of the rejection rate contains 0.05 (`kt_h0_binary_nominal`)
- ⬜ [cmd] Simpson H0: blocked rate nominal, pooled rate outside its interval (`kt_simpson_blocked_holds`)
- ⬜ [cmd] Weighted H0: fewer simulations (the count and the measured runtime recorded), interval contains 0.05 (`kt_h0_weighted_nominal`)
- ⬜ [cmd] The golden shape on the fakes (4 treatment levels × 3 model levels, a small replicate count) gives 6 treatment comparisons blocked by model and 3 model comparisons blocked by treatment (`kt_two_levels_end_to_end`, A8)
- ⬜ [nl] The report gives the power at a 0.2 difference with its Monte Carlo interval, and each Bite
**References**: [R03 §Known truth](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md); designing-tests science step 5

## Step 1: Simulations under H0, under effect, and Simpson
**Goal**: The battery holds its rate and its power is known.
**Implementation Logic**:
1. A seeded SplitMix64 generator draws stores in the shape `analyze` reads: two treatment levels × two model blocks, n replicates per cell, fixed per arm.
2. `kt_h0_binary_nominal`: equal rates per block, unequal between blocks; 2 000 experiments; count rejections at 0.05.
3. `kt_power_at_0_2`: B − A = 0.2 in every block; report the power with its interval (asserts only that it is above α).
4. `kt_simpson_blocked_holds`: H0 within blocks, unbalanced allocation across blocks so that pooling creates a spurious effect.
5. `kt_h0_weighted_nominal`: weighted outcomes, fewer experiments (the permutation test runs B = 10 000 inside each); measure the runtime first and set the count to stay under 60 s.
**Deliverables**: `crates/candler-core/tests/it/guard_known_truth.rs`: `simulate_store`, `kt_h0_binary_nominal`, `kt_power_at_0_2`, `kt_simpson_blocked_holds`, `kt_h0_weighted_nominal`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_known_truth::kt_(h0|power|simpson)/)'` (expected: PASS)
**Commit**: `test(analysis): guard the battery on known truth`

## Step 2: Two levels end to end
**Goal**: A two-treatment job planned, run on fakes and analysed gives the treatment comparison blocked by model.
**Implementation Logic**:
1. Move the fakes of `guard_job_orchestrate.rs` into `support/fake_jobs.rs` unchanged; its rows still pass.
2. `kt_two_levels_end_to_end` (A8, the golden shape): plan 4 treatment levels × 3 models with a small replicate count, run on the fakes with planted outcomes, analyse; 6 treatment comparisons with 3 blocks each and 3 model comparisons with 4 blocks each, the CMH battery, and the planted direction on the planted pair. The `analyze_c6_*` vector deviation from L2 is accepted: add no such vectors.
**Deliverables**: `crates/candler-core/tests/it/support/fake_jobs.rs`; `crates/candler-core/tests/it/guard_known_truth.rs`: `kt_two_levels_end_to_end`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_known_truth::/) | test(/^guard_job_orchestrate::/)'` (expected: PASS)
**Commit**: `test(jobs): run two treatment levels end to end on fakes`

## Step 3: Bites and the report
**Goal**: The rows bite, and the power is reported.
**Implementation Logic**:
1. Bites: the pooled test in `analyze` (red on `kt_simpson_`); Fisher's p instead of the mid-p (record the rate change); a constant p of 0.01 (red on `kt_h0_`). Record each.
2. Report the power at 0.2 and the weighted simulation count and runtime.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/known_truth_v0.md`: sections "Power", "Bites"
**Consistency Checks**: `grep -q '^## Power' __reports__/m6a-paired-stats/leaves/known_truth_v0.md && grep -q '^## Bites' __reports__/m6a-paired-stats/leaves/known_truth_v0.md` (expected: PASS)
**Commit**: `docs(analysis): record the power and bites of the known-truth guard`
