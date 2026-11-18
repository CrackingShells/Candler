# CMH leaf report (v0)

Leaf `cmh` of `m6a-paired-stats`, branch `m6a/cmh`.

## Citations

No network is available to this leaf, so the four sources were not re-read in this session. Each
claim below is stated from the published formula and then checked numerically: the generator
`gen_stats.py` recomputes the statistic, the odds ratio, the interval and the homogeneity test
with statsmodels' `StratifiedTable`, and the guards hold Candler to those values at 1e-12. A
reader with library access should confirm the section numbers marked "as cited".

| Quantity | Source (as cited) | What the leaf uses |
|:--|:--|:--|
| CMH statistic | Mantel & Haenszel (1959), J Natl Cancer Inst 22:719, the summary chi-square; the form without the 1/2 correction is the uncorrected variant the battery names | `(sum(a_k - E a_k))^2 / sum Var a_k`, 1 df, hypergeometric mean and variance per block, variance `n1 n2 m1 m2 / (n^2 (n-1))` |
| MH odds ratio | Mantel & Haenszel (1959), the pooled estimator | `sum(a d / n) / sum(b c / n)` |
| Variance of log OR_MH | Robins, Breslow & Greenland (1986), Biometrics 42:311, the variance estimator consistent in both sparse-data and large-strata limits | `sum P R / (2 R^2) + sum (P S + Q R) / (2 R S) + sum Q S / (2 S^2)`, with `P = (a+d)/n`, `Q = (b+c)/n`, `R_k = a d / n`, `S_k = b c / n` |
| Validity criterion | Mantel & Fleiss (1980), Am J Epidemiol 112:129 | the chi-square approximation is adequate when `min(sum E - sum L, sum U - sum E) >= 5`, with `L_k = max(0, m1 - n2)` and `U_k = min(n1, m1)` the bounds of `a_k` given the margins |
| Homogeneity | Breslow & Day (1980), Statistical Methods in Cancer Research, vol. I, the test of a common odds ratio | `sum (a_k - e_k)^2 / Var_k` at the MH odds ratio, `K - 1` df; reported, never a gate. Tarone's adjustment is not applied (statsmodels `adjust=False`) |

Confirmed against the battery: no ½ correction, 1 df, the criterion at 5 with the minimum of the
two margins to the bounds, and homogeneity as information only. No source contradicts the
battery, so nothing stops the leaf.

## Zero-cell convention

Written in the module doc of `cmh.rs` and repeated here.

- No shift: a zero cell stays zero (statsmodels `shift_zeros=False`).
- A block with a zero margin (no runs in an arm, no passes, no fails) is dropped from every
  quantity, Breslow–Day's degrees of freedom included.
- No informative block: statistic, p, odds ratio, interval and homogeneity are `None`; the
  Mantel–Fleiss quantity is 0 and does not hold. statsmodels gives `NaN` for the statistic.
- `Σ S = 0`: `or_mh` is `None` (statsmodels `inf`). `Σ R = 0` with `Σ S > 0`: `or_mh` is 0 and has
  no interval (statsmodels 0 and a `NaN` interval).
- Homogeneity needs two informative blocks and a positive finite odds ratio. At an odds ratio of
  exactly 1 statsmodels divides by zero; Candler's statistic is the sum over blocks of
  `(a - E)^2 / v`, which is 0 when every block is independent.
- The odds ratio is the odds of passing in B over the odds in A (the estimand is B − A).

## Commits

| sha | subject |
|:--|:--|
| ea805b1a | docs(stats): cite the validity of the CMH test |
| 7ebd77d2 | test(stats): add the CMH vectors |
| a54d46ab | test(stats): guard the CMH test |
| 5e559d74 | feat(stats): compute the CMH test and the MH odds ratio |
| 777c2a1c | fix(stats): hold the Mantel-Fleiss tie and guard the level and OR 1 |

## Bites

Each injection was made on a copy of `cmh.rs`, run against the five guards, and the file restored
from the copy.

| Injection | Red guard and message |
|:--|:--|
| ½ continuity correction on the statistic | `cmh_statistic_matches_statsmodels`: "one block, Mantel-Fleiss exactly five: statistic is 1.71, the second engine has 3.0399999999999996" |
| Woolf's variance instead of RBG | `mh_odds_ratio_and_rbg_interval_match_statsmodels`: "zero cells in two blocks: or_lo is 0.0, the second engine has 0.48574327609977075" |
| zero-margin block kept in the sums | `degenerate_blocks_follow_the_convention`: "all blocks degenerate: statistic"; `homogeneity_matches_statsmodels`: "zero margin: no passes in a block: homogeneity_p is NaN" |
| criterion compared to 3 | `mantel_fleiss_matches_hand_count`: "one block, Mantel-Fleiss four: verdict" |
| (verifier F1) tie slack removed | `mantel_fleiss_matches_hand_count`: "Mantel-Fleiss exactly five over four blocks: verdict" |
| (verifier F2) level ignored (always 0.95) | `mh_odds_ratio_and_rbg_interval_match_statsmodels`: "random K=3 #0 at alpha 0.1: or_lo is 1.9968, the second engine has 2.1329" |
| (verifier F3) Breslow–Day root form altered | `homogeneity_matches_statsmodels`: "zero cells in two blocks: homogeneity_p is 1.0, the second engine has 3.32e-5" |

## Gates

- GEN (`gen_stats.py --check`): PASS, `cmh.toml: 33 rows match`, the six other files unchanged.
- Targeted test `guard_stats_cmh::`: PASS, 5 passed.
- `cargo clippy -p candler-core --all-targets -- -D warnings`: PASS.
- `cargo fmt --check`: PASS.
- Ownership check: PASS, prints nothing.
- No `fn confirm_` in `crates/`: PASS.

## Claims checked

- "statsmodels' `StratifiedTable` supplies every value": yes, except the homogeneity p-value
  (see Findings) and the Mantel–Fleiss quantity (exact rationals from the margins).
- "statsmodels agrees wherever it returns a finite value": asserted in the generator on every
  block of at least two runs, for the statistic and the odds ratio.
- "`statrs` is a dependency": yes; no new crate.

## Findings

1. **Tolerance.** `statrs` computes the χ² survival function with an absolute error near 1e-16,
   so a p-value below 1e-4 cannot meet a pure relative 1e-12 (seen at p = 4.6e-5, relative
   1.1e-12). The guards use relative 1e-12 plus absolute 1e-15, as `gen_stats.py` does.
2. **statsmodels' homogeneity p-value** is `1 - cdf`, which loses the tail. The vector holds
   scipy's `chi2.sf` of statsmodels' statistic, asserted within 1e-12 of statsmodels' own p.
3. **statsmodels divides by `1 - OR`** in the Breslow–Day test, so it returns `NaN` at an odds
   ratio of exactly 1; the vector row for that case is the hand-derived value (statistic 0).
4. **Orientation.** The leaf does not say which arm is the numerator. The odds ratio is B over A,
   matching the estimand B − A; `analyze_output` should show it with that label.
5. **Counts near 2^53.** Each stratum is checked at 2^53, so a block total can reach 2^54 and lose
   exactness in a float; not reachable with Candler's run counts.
6. **Mantel–Fleiss tie.** The criterion is compared with a 1e-9 slack so that a quantity of
   exactly 5 does not read 4.999999999999998 after float summation.

## Verifier

One fresh Sonnet `spec-verifier`, 16 calls, then a re-check through SendMessage.

First pass: degenerate blocks, the RBG variance, the hypergeometric variance, the Mantel–Fleiss
bounds, the vector provenance and the ownership check all passed. Findings, all fixed in
`777c2a1c`:

- F1 the Mantel–Fleiss verdict flipped on an exact 5 over four blocks (float sum 4.999999999999998):
  a 1e-9 slack and a four-block exact-5 row.
- F2 a hard-coded 95 % level passed every guard: rows now carry `level`, with rows at 0.90 and 0.99.
- F3 the Breslow–Day branch at an odds ratio of exactly 1 was unguarded: a hand-derived row
  (statistic 0, p 1; statsmodels returns `NaN` there).
- F4 Breslow–Day cancellation near an odds ratio of 1 at counts near 2^53: stable root form.
- Notes accepted: F5 (the statistic does not discriminate a kept zero-margin block; the
  homogeneity degrees of freedom do), F6 (float exactness near 2^53, Findings 5), F7 (the
  "all degenerate" NaN assertion needs a block of two runs).

Re-check verdict: VERIFY_COMPLETE, no open finding. The verifier did not read this report.
