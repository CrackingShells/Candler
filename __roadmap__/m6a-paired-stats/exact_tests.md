# Exact tests

**Goal**: Candler computes Fisher's two-sided mid-p for one 2×2 table and the exact conditional test of a 2×2×K table (the convolution of the blocks' hypergeometrics, also as a mid-p), each equal to scipy's `hypergeom` and to exact rational arithmetic.
**Estimate**: about 80 Sonnet calls, $3.3; no verifier (the oracle is exact-rational brute force)
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (README, Gotchas): own checkout, `$BASE_LEAF` an ancestor
- [ ] [cmd] The scaffold is merged: `test -f crates/candler-core/src/analysis/midp.rs && test -f crates/candler-core/src/analysis/exact_strata.rs && git grep -q 'pub struct Stratum' -- crates/candler-core/src/analysis/blocking.rs`
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/analysis/(midp|exact_strata)\.rs$|^crates/candler-core/src/analysis/fisher\.rs$|^crates/candler-core/tests/vectors/stats/(midp|exact)\.toml$|^crates/candler-core/tests/vectors/stats/gen_stats\.py$|^crates/candler-core/tests/it/guard_stats_exact\.rs$|^__reports__/m6a-paired-stats/leaves/exact_tests_v0\.md$`. In `gen_stats.py` only the `exact_tests` section; in `fisher.rs` only making its hypergeometric pmf `pub(crate)` for reuse, no behaviour change
**Success Gates**:
- ⬜ [cmd] GEN passes and lists `midp.toml` and `exact.toml`; `fisher.toml` is unchanged: `git diff --quiet $BASE_LEAF -- crates/candler-core/tests/vectors/stats/fisher.toml`
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_exact::/) | test(/^guard_stats::t1_/)'`
- ⬜ [cmd] Ownership check prints nothing (README, Gotchas)
- ⬜ [nl] The leaf report cites the validity sources and records each Bite
**References**: [R03 §Statistical design](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md) — design fact 3 and the battery; [R01 §Analysis statistics](../../__design__/architecture.md) — today's Fisher and its tie rule

## Step 1: Probe: cite the validity conditions
**Goal**: The mid-p definition and the exact conditional test are confirmed against their primary sources before code.
**Implementation Logic**:
1. Read Lydersen, Fagerland & Laake (2009), "Recommended tests for association in 2×2 tables", Stat Med 28:1159–1175, doi:10.1002/sim.3531, for the two-sided mid-p of Fisher's test, and a primary text for the exact conditional test of conditional independence in 2×2×K tables (for example Agresti, Categorical Data Analysis, the section on exact inference for stratified 2×2 tables). Cite the section read.
2. Confirm the definition this leaf implements: with f the null pmf of the statistic (A's passes in one table; the sum of A's passes over blocks for K tables), two-sided mid-p = Σ_{x: f(x) < f(obs)} f(x) + ½ Σ_{x: f(x) = f(obs)} f(x), "equal" under the same relative tolerance `fisher_two_sided` uses today (read it).
3. Decision it feeds: if a source defines the two-sided mid-p differently, stop and report it to the coordinator as a finding for Eliott.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/exact_tests_v0.md`: section "Citations" (sources, sections, the definition confirmed or the conflict)
**Consistency Checks**: `grep -q '10.1002/sim.3531' __reports__/m6a-paired-stats/leaves/exact_tests_v0.md` (expected: PASS)
**Commit**: `docs(stats): cite the validity of the exact tests`

## Step 2: Write the reference vectors
**Goal**: `midp.toml` and `exact.toml` hold scipy's values for a grid that includes degenerate tables.
**Implementation Logic**:
1. In the `exact_tests` section of `gen_stats.py`: `midp` rows from `scipy.stats.hypergeom` pmfs for every table with n ≤ 12 per arm and a sample of larger ones (up to 200 runs per arm); `exact` rows for K = 2 to 4 blocks, the null pmf the convolution of the blocks' hypergeometric pmfs.
2. Inside the generator, assert that a `Fraction` brute force over every table set with total n ≤ 12 gives the same p to 1e-12, so the second engine is itself checked.
3. Include zero margins (a block with no passes, or one arm with no runs) and say in a comment what each returns (a block with a zero margin contributes a point mass).
**Deliverables**: `crates/candler-core/tests/vectors/stats/gen_stats.py`: section `exact_tests`, functions `midp_rows`, `exact_rows`; `crates/candler-core/tests/vectors/stats/midp.toml`; `crates/candler-core/tests/vectors/stats/exact.toml`
**Consistency Checks**: GEN (expected: PASS)
**Commit**: `test(stats): add the mid-p and exact-strata vectors`

## Step 3: Write the guards, seen red
**Goal**: Two guards read the vectors and fail on stub implementations.
**Implementation Logic**:
1. `midp_matches_scipy_hypergeom` reads every `midp.toml` row; `exact_strata_matches_convolution` reads every `exact.toml` row; relative tolerance 1e-12. Each doc has a `Guards:` and an `Oracle:` line.
2. The signatures (C1): `analysis::midp::midp_two_sided(s: &Stratum) -> Result<f64, StatsError>`; `analysis::exact_strata::exact_conditional(strata: &[Stratum]) -> Result<f64, StatsError>`, both returning the two-sided mid-p. Stubs return `Err` so the guards go red.
**Deliverables**: `crates/candler-core/tests/it/guard_stats_exact.rs`: `midp_matches_scipy_hypergeom`, `exact_strata_matches_convolution`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_exact::/)'` (expected: FAIL)
**Commit**: `test(stats): guard the mid-p and the exact conditional test`

## Step 4: Compute the tests
**Goal**: Both guards pass, reusing the existing hypergeometric pmf.
**Implementation Logic**:
1. `midp_two_sided` sums the one-table pmf in log space with the existing helper of `fisher.rs` (made `pub(crate)`; no copy).
2. `exact_conditional` convolves the blocks' pmfs over the support of Σ a_k, then applies the same two-sided rule. Blocks with a zero margin are point masses.
3. `fisher_two_sided` stays, unchanged, until `analyze_output` swaps the call.
**Deliverables**: `crates/candler-core/src/analysis/midp.rs`: `midp_two_sided`; `crates/candler-core/src/analysis/exact_strata.rs`: `exact_conditional`, `convolve`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_exact::/) | test(/^guard_stats::t1_/)'` (expected: PASS)
**Commit**: `feat(stats): compute the mid-p and the exact conditional test`

## Step 5: Record the bites
**Goal**: Each guard is shown to catch a plausible bug.
**Implementation Logic**:
Inject, run, revert: drop the ½ (the mid-p becomes Fisher's p, the guard goes red); convolve one block too few (red); use `<=` for ties without the tolerance (red on the tie rows). Record each in the report.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/exact_tests_v0.md`: section "Bites"
**Consistency Checks**: `grep -q '^## Bites' __reports__/m6a-paired-stats/leaves/exact_tests_v0.md` (expected: PASS)
**Commit**: `docs(stats): record the bites of the exact tests`
