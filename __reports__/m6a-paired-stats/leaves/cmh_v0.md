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

