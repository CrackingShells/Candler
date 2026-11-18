# Resampling

**Goal**: Candler computes a stratified permutation test and a stratified BCa interval for the difference in weighted pass rate (Σnum/Σden), from a seeded SplitMix64 that a Python twin reproduces to 1e-12, and each agrees with an independent engine within its Monte Carlo error.
**Estimate**: about 110 Sonnet calls, $4.5, plus its verifier (about 40 calls, $1.6)
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions; the scaffold is merged: `test -f crates/candler-core/src/analysis/permutation.rs && test -f crates/candler-core/src/analysis/bca.rs && test -f crates/candler-core/src/analysis/splitmix.rs`
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/analysis/(permutation|bca)\.rs$|^crates/candler-core/src/analysis/splitmix\.rs$|^crates/candler-core/tests/vectors/stats/(splitmix|permutation|bca)\.toml$|^crates/candler-core/tests/vectors/stats/gen_stats\.py$|^crates/candler-core/tests/it/guard_stats_resampling\.rs$|^__reports__/m6a-paired-stats/leaves/resampling_v0\.md$`; in `gen_stats.py` only the `resampling` section
**Success Gates**:
- ⬜ [cmd] GEN passes: the twin's values equal Rust's to 1e-12, and the generator's own Monte Carlo agreement assertions pass, so `--check` stays deterministic
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_resampling::/)'`
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] Permutations stay within blocks (a guard row with blocks of different rates shows it), and the p-value is (b+1)/(B+1)
- ⬜ [nl] The verifier closed with no open finding
**References**: [R03 §The battery](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md) — the weighted row and the second engines

## Step 1: Probe: cite the validity conditions
**Goal**: The permutation p-value, the stratified statistic and BCa are confirmed against their sources.
**Implementation Logic**:
1. Read Phipson & Smyth (2010, Stat Appl Genet Mol Biol 9:39) for p = (b+1)/(B+1), a primary text on stratified (restricted) permutation tests for exchangeability within blocks, and Efron (1987, JASA 82:171) for BCa (bias correction z₀ and the jackknife acceleration a). Cite the sections read.
2. Confirm the statistic: T = Σ_k w_k (r_Bk − r_Ak), r the block's Σnum/Σden for one arm, w_k = n_k / N (runs in the block over all runs); two-sided, a permuted T* counts when |T*| ≥ |T_obs| − 1e-12·max(1, |T_obs|).
3. Decision it feeds: a conflict goes to the coordinator.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/resampling_v0.md`: section "Citations"
**Consistency Checks**: `grep -q 'Phipson' __reports__/m6a-paired-stats/leaves/resampling_v0.md && grep -q 'Efron' __reports__/m6a-paired-stats/leaves/resampling_v0.md` (expected: PASS)
**Commit**: `docs(stats): cite the validity of the resampling tests`

## Step 2: The generator and its twin
**Goal**: The scaffold's SplitMix64 equals its Python twin, bit for bit, and gains a within-block shuffle.
**Implementation Logic**:
1. The scaffold wrote `SplitMix64::new(seed)`, `next_u64` and `below(n)` by rejection; this step checks them against the twin (the same rule in both languages), fixes them if the twin disagrees with the published algorithm, and adds `shuffle_within(&mut [usize])` (Fisher–Yates).
2. In the `resampling` section of `gen_stats.py`, the twin writes `splitmix.toml` (the first 1 000 outputs for 5 seeds, and `below` draws for awkward n), and the guard `splitmix_matches_twin` reads it exactly.
**Deliverables**: `crates/candler-core/src/analysis/splitmix.rs`: `SplitMix64`, `next_u64`, `below`, `shuffle_within`; `gen_stats.py`: section `resampling`, `SplitMix64` twin; `crates/candler-core/tests/vectors/stats/splitmix.toml`; `crates/candler-core/tests/it/guard_stats_resampling.rs`: `splitmix_matches_twin`
**Consistency Checks**: GEN `&& cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_resampling::splitmix_/)'` (expected: PASS)
**Commit**: `feat(stats): add a seeded SplitMix64 with a Python twin`

## Step 3: The stratified permutation test
**Goal**: The permutation p-value equals the twin's to 1e-12 and an exact enumeration within its Monte Carlo error.
**Implementation Logic**:
1. Signature (C1): `analysis::permutation::stratified_permutation(blocks: &[UnitBlock], b: u32, seed: u64) -> Result<PermResult, StatsError>`, `UnitBlock { a: Vec<(f64, f64)>, b: Vec<(f64, f64)> }` (per-run num, den), `PermResult { statistic: f64, p: f64, extreme: u32 }`. Labels are shuffled within each block only.
2. The twin computes the same values to 1e-12 (`permutation.toml`). Inside the generator, for tiny designs, the exact p over every within-block relabeling must lie within 3 Monte Carlo SE of the B = 10 000 estimate, and for one block scipy's `permutation_test(permutation_type="independent")` must too; the generator exits 1 if not.
3. Guards: `permutation_matches_twin`, `permutation_stays_within_blocks` (blocks of very different rates under H0 within blocks: a pooled shuffle would reject, the stratified one does not), `p_counts_the_observed_once`.
**Deliverables**: `crates/candler-core/src/analysis/permutation.rs`: `stratified_permutation`, `UnitBlock`, `PermResult`; `crates/candler-core/tests/vectors/stats/permutation.toml`; `guard_stats_resampling.rs`: the three guards
**Consistency Checks**: GEN `&& cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_resampling::/)'` (expected: PASS)
**Commit**: `feat(stats): add the stratified permutation test`

## Step 4: The stratified BCa interval
**Goal**: The BCa interval equals the twin's to 1e-12 and scipy's within Monte Carlo error.
**Implementation Logic**:
1. Signature (C1): `analysis::bca::stratified_bca(blocks: &[UnitBlock], b: u32, level: f64, seed: u64) -> Result<(f64, f64), StatsError>`: resample runs with replacement within each arm within each block; z₀ from the share of replicates below T_obs; a from the leave-one-run-out jackknife over all runs.
2. The twin writes `bca.toml`. Inside the generator, for one block, `scipy.stats.bootstrap(method="BCa")` endpoints over 20 seeds give a Monte Carlo SE, and the twin's endpoints must lie within 3 SE; exit 1 if not.
3. Guard `bca_matches_twin`; a degenerate block (all runs equal) gives a defined result written in the module doc.
**Deliverables**: `crates/candler-core/src/analysis/bca.rs`: `stratified_bca`, `jackknife_acceleration`; `crates/candler-core/tests/vectors/stats/bca.toml`; `guard_stats_resampling.rs`: `bca_matches_twin`, `bca_degenerate_block`
**Consistency Checks**: GEN `&& cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_resampling::/)'` (expected: PASS)
**Commit**: `feat(stats): add the stratified BCa interval`

## Step 5: Bites and verifier
**Goal**: The guards bite, and a fresh verifier finds nothing open.
**Implementation Logic**:
1. Bites: shuffle across blocks (red on `permutation_stays_within_blocks`); p = b/B (red); flip the jackknife sign in a (red on `bca_matches_twin`); a biased `below` by modulo (red on `splitmix_matches_twin`). Record each.
2. Dispatch the verifier (Sonnet, fresh agent) with the brief below verbatim plus the branch name; fix, re-check through SendMessage, report once.
3. **Verifier brief:** "You verify the leaf `__roadmap__/m6a-paired-stats/resampling.md` on branch <branch>. Read the leaf, `analysis/{splitmix,permutation,bca}.rs`, the `resampling` section of `gen_stats.py`, its TOML files and `guard_stats_resampling.rs`. Do not read the implementer's report until you have your own findings. Attack: Rust and the twin sharing one misreading of the method (check the twin against Phipson & Smyth and Efron yourself, not against Rust); a permutation that crosses blocks; the (b+1)/(B+1) count and the tie tolerance; the BCa z₀ and the jackknife sign; whether the Monte Carlo agreement assertions in the generator can fail at all; nondeterminism in `--check`; a guard that would pass with a wrong implementation. Report each finding with file, line and a failing input. Change nothing."
**Deliverables**: `__reports__/m6a-paired-stats/leaves/resampling_v0.md`: sections "Bites", "Verifier"
**Consistency Checks**: `grep -q '^## Bites' __reports__/m6a-paired-stats/leaves/resampling_v0.md && grep -q '^## Verifier' __reports__/m6a-paired-stats/leaves/resampling_v0.md` (expected: PASS)
**Commit**: `docs(stats): record the bites and the verifier of the resampling tests`
