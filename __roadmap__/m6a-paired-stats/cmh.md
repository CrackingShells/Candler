# CMH

**Goal**: Candler computes the Cochran–Mantel–Haenszel test without continuity correction, the Mantel–Haenszel odds ratio with its Robins–Breslow–Greenland interval, the Mantel–Fleiss criterion and the homogeneity test, each equal to statsmodels' `StratifiedTable`, with a defined result on degenerate blocks.
**Estimate**: about 110 Sonnet calls, $4.5, plus its verifier (about 40 calls, $1.6)
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (README, Gotchas); the scaffold is merged: `test -f crates/candler-core/src/analysis/cmh.rs`
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/analysis/cmh\.rs$|^crates/candler-core/tests/vectors/stats/cmh\.toml$|^crates/candler-core/tests/vectors/stats/gen_stats\.py$|^crates/candler-core/tests/it/guard_stats_cmh\.rs$|^__reports__/m6a-paired-stats/leaves/cmh_v0\.md$`; in `gen_stats.py` only the `cmh` section
**Success Gates**:
- ⬜ [cmd] GEN passes and lists `cmh.toml`, whose rows include degenerate blocks
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_cmh::/)'`
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] The zero-cell convention is written in `cmh.rs`'s module doc and in the report, and matches statsmodels wherever statsmodels defines a value
- ⬜ [nl] The verifier closed with no open finding
**References**: [R03 §The battery](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md) — the ≥2-block row and its fallback

## Step 1: Probe: cite the validity conditions
**Goal**: The statistic, the interval and the criterion are confirmed against their primary sources.
**Implementation Logic**:
1. Read Mantel & Haenszel (1959, JNCI 22:719), Mantel & Fleiss (1980, Am J Epidemiol 112:129) for the criterion, Robins, Breslow & Greenland (1986, Biometrics 42:311) for the variance of log OR_MH, and Breslow & Day (1980) for homogeneity. Cite the sections read.
2. Confirm: CMH χ² = (Σ(a_k − E a_k))² / Σ Var a_k, no ½ correction, 1 df; Mantel–Fleiss holds when both min(Σ upper_k − Σ E_k, Σ E_k − Σ lower_k) ≥ 5, upper and lower the bounds of a_k given the margins; homogeneity is reported, never a gate.
3. Decision it feeds: a conflict with the battery stops the leaf and goes to the coordinator.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/cmh_v0.md`: section "Citations"
**Consistency Checks**: `grep -q 'Mantel' __reports__/m6a-paired-stats/leaves/cmh_v0.md && grep -q 'Robins' __reports__/m6a-paired-stats/leaves/cmh_v0.md` (expected: PASS)
**Commit**: `docs(stats): cite the validity of the CMH test`

## Step 2: Write the reference vectors
**Goal**: `cmh.toml` holds statsmodels' values, degenerate blocks included.
**Implementation Logic**:
1. In the `cmh` section of `gen_stats.py`, use `statsmodels.stats.contingency_tables.StratifiedTable`: `test_null_odds(correction=False)`, `oddsratio_pooled`, `oddsratio_pooled_confint(alpha=0.05)`, `test_equal_odds()`; compute the Mantel–Fleiss quantity by hand from the margins.
2. Rows: K = 2 to 6 blocks with 5 to 200 runs per arm; blocks with a zero cell; a block with a zero margin (dropped: no information); all blocks degenerate (statistic undefined); Σ R = 0 or Σ S = 0 (OR undefined or infinite).
3. For each degenerate row write what Candler returns (`None` for an undefined OR or interval, the statistic over informative blocks only), and assert in the generator that statsmodels agrees wherever it returns a finite value.
**Deliverables**: `crates/candler-core/tests/vectors/stats/gen_stats.py`: section `cmh`, `cmh_rows`; `crates/candler-core/tests/vectors/stats/cmh.toml`
**Consistency Checks**: GEN (expected: PASS)
**Commit**: `test(stats): add the CMH vectors`

## Step 3: Write the guards, seen red
**Goal**: One guard per quantity reads the vectors and fails on a stub.
**Implementation Logic**:
1. `cmh_statistic_matches_statsmodels`, `mh_odds_ratio_and_rbg_interval_match_statsmodels`, `mantel_fleiss_matches_hand_count`, `homogeneity_matches_statsmodels`, `degenerate_blocks_follow_the_convention`; relative tolerance 1e-12; `Guards:` and `Oracle:` lines.
2. The signature (C1): `analysis::cmh::cmh(strata: &[Stratum], level: f64) -> Result<CmhResult, StatsError>`, with `CmhResult { statistic: Option<f64>, p: Option<f64>, or_mh: Option<f64>, or_ci: Option<(f64, f64)>, mantel_fleiss: f64, mantel_fleiss_holds: bool, homogeneity_p: Option<f64> }`.
**Deliverables**: `crates/candler-core/tests/it/guard_stats_cmh.rs`: the five guards above
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_cmh::/)'` (expected: FAIL)
**Commit**: `test(stats): guard the CMH test`

## Step 4: Compute the test
**Goal**: The guards pass.
**Implementation Logic**:
Implement `cmh` and its helpers (`expected_and_variance`, `rbg_variance`, `breslow_day`), the p-value from the χ²₁ survival function already used in the crate (or `statrs` if it is already a dependency; no new crate without a coordinator ruling). Write the zero-cell convention in the module doc.
**Deliverables**: `crates/candler-core/src/analysis/cmh.rs`: `cmh`, `CmhResult`, `expected_and_variance`, `rbg_variance`, `breslow_day`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_cmh::/)'` (expected: PASS)
**Commit**: `feat(stats): compute the CMH test and the MH odds ratio`

## Step 5: Bites and verifier
**Goal**: The guards are shown to bite, and a fresh verifier finds nothing open.
**Implementation Logic**:
1. Bites: add the ½ continuity correction (red); use Woolf's variance instead of RBG (red); keep a zero-margin block in the sum (red on the degenerate rows); compare the criterion to 3 instead of 5 (red). Record each.
2. Dispatch the verifier (Sonnet, fresh agent) with the brief below verbatim plus the branch name; fix, re-check through SendMessage, report once.
3. **Verifier brief:** "You verify the leaf `__roadmap__/m6a-paired-stats/cmh.md` on branch <branch>. Read the leaf, `crates/candler-core/src/analysis/cmh.rs`, the `cmh` section of `gen_stats.py`, `cmh.toml` and `guard_stats_cmh.rs`. Do not read the implementer's report until you have your own findings. Attack: degenerate blocks (a zero cell, a zero margin, all blocks degenerate, Σ R or Σ S zero) and whether Rust, the generator and statsmodels agree on each; the zero-cell convention as documented against what the code does; the Mantel–Fleiss bounds and the threshold of 5; the RBG variance terms; whether any vector row was computed from Candler's own code rather than statsmodels; whether a guard could pass with a wrong implementation. Report each finding with file, line and a failing input. Change nothing."
**Deliverables**: `__reports__/m6a-paired-stats/leaves/cmh_v0.md`: sections "Bites", "Verifier"
**Consistency Checks**: `grep -q '^## Bites' __reports__/m6a-paired-stats/leaves/cmh_v0.md && grep -q '^## Verifier' __reports__/m6a-paired-stats/leaves/cmh_v0.md` (expected: PASS)
**Commit**: `docs(stats): record the bites and the verifier of the CMH test`
