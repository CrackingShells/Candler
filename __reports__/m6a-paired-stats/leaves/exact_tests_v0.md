# exact_tests: leaf report (v0)

## Citations
Sources, and what each is cited for. **Not re-read in this run**: the brief forbids network access and
the papers are not in the repository, so the definition below is the implementer's recollection of
the sources, not a confirmed reading. This is a finding for the coordinator (see Findings).

- Lydersen S, Fagerland MW, Laake P (2009). Recommended tests for association in 2x2 tables.
  Statistics in Medicine 28:1159-1175. doi:10.1002/sim.3531. Cited for the Fisher mid-p test of
  a 2x2 table when a margin is fixed by design; the two-sided mid-p is, as recalled, the sum of
  the null probabilities of the tables less probable than the observed one plus half the
  probability of the tables as probable as it (the probability-based two-sided rule of Fisher's
  test, with half weight on ties).
- Agresti A. Categorical Data Analysis (Wiley), the section on exact inference for conditional
  independence in 2x2xK tables (the conditional distribution of the sum of the strata's first
  cells is the convolution of the strata's hypergeometrics). Section number not confirmed.

Definition implemented: with f the null pmf of the statistic (A's passes in one table; the sum of
A's passes over blocks for K tables), two-sided mid-p = sum over {x: f(x) < f(obs)} of f(x) plus
half the sum over {x: f(x) = f(obs)} of f(x). "Equal" means `|f(x) - f(obs)| <= 1e-7 * f(obs)`, the
relative tolerance `fisher_two_sided` uses (`TIE_TOLERANCE`).

## Bites
Each injected into the source, the guards run, the source restored from a copy.
- Half dropped (`less + equal` in `mid_p`, the mid-p becomes the probability-based Fisher p): both
  guards red. `midp_matches_scipy_hypergeom`: `0/0 against 0/0: got 1.0, scipy has 0.5`.
  `exact_strata_matches_convolution`: `blocks a [0, 0]/[0, 0], b [0, 4]/[0, 8]: got 1.0, the
  convolution has 0.5`.
- One block too few (the loop over blocks skips the last): `exact_strata_matches_convolution` red:
  `blocks a [0, 2]/[0, 2], b [0, 2]/[0, 8]: got 0.5, the convolution has 0.06666666666666667`.
  The mid-p guard does not use the convolution and stays green.
- Ties by exact float equality in place of the relative tolerance: both red on tie rows.
  `midp_matches_scipy_hypergeom`: `53/128 against 10/128: got 9.915556018655851e-11, scipy has
  1.6446096920638645e-10`. `exact_strata_matches_convolution`: `blocks a [1, 0]/[1, 1], b [2,
  3]/[4, 3]: got 0.7749999999999999, the convolution has 0.55`.

## Commits
| sha | subject |
|:----|:--------|
| 8201e488 | docs(stats): cite the validity of the exact tests |
| d4202745 | test(stats): add the mid-p and exact-strata vectors |
| decb3e27 | test(stats): guard the mid-p and the exact conditional test |
| 484b0320 | feat(stats): compute the mid-p and the exact conditional test |
| tip | docs(stats): record the bites of the exact tests |

## Gates
- GEN `--check`: PASS (`midp.toml: 8449 rows match`, `exact.toml: 1328 rows match`, 6 older files unchanged; about 13 s offline).
- `fisher.toml` unchanged: PASS.
- Targeted test (`guard_stats_exact::` and `guard_stats::t1_`): PASS, 10 of 10.
- Step 3 red: both guards FAIL with "the statistic is not computed".
- clippy `-p candler-core --all-targets -D warnings`: PASS; `cargo fmt --check`: PASS.
- Ownership check: PASS (prints nothing). No `fn confirm_` left.

## Claims checked
- "Stratum is in blocking.rs with a/b = (passes, runs)": true.
- "the existing helper of fisher.rs (made `pub(crate)`)": **false**, see Findings.
- "`fisher_two_sided` unchanged": true; `fisher.rs` is not edited at all.
- "mid-p = less + half the ties, ties under the tolerance of `fisher_two_sided`": implemented with `TIE_TOLERANCE` (1e-7, relative to the observed probability). The generator asserts that no table inside the tolerance is unequal in exact arithmetic (no near ties on any row).

## Findings
1. **Spec defect (minor, worked around, nothing built on it).** `fisher.rs` has no hypergeometric pmf helper: `fisher_two_sided` builds a `statrs` `Hypergeometric` inline. So `fisher.rs` is untouched. The shared code is `midp::block_pmf` (statrs, `pub(crate)`, point mass for a zero margin) and `midp::mid_p`, both used by `exact_conditional`. It uses statrs' pmf, not a log-space sum written by hand.
2. **Sources not re-read.** No network; the Lydersen et al. definition and the Agresti section are cited from recollection (see Citations). Eliott should confirm the two-sided mid-p definition, as the leaf's Step 1 asks, before the battery relies on it.
3. **Degenerate tables.** `midp_two_sided` accepts an arm with no runs or a table with no or all passes and returns 0.5 (a one-point null is its own tie), where `fisher_two_sided` returns `NoRuns` or 1. `analyze_output` must decide what to do with such a block; a one-point null carries no evidence, so 0.5 should not be reported as a p-value. In `exact_conditional` such a block is a point mass and changes nothing, as designed. Empty block slice returns `NoRuns`.
4. `exact.toml` has 1328 rows (every margin set up to 12/8/6 runs for K=2/3/4 is enumerated and checked in the generator; a sample is written) and `midp.toml` 8449 rows (0.65 MB).
5. Load average was above 12 at the first build (about 21); the build ran anyway.
