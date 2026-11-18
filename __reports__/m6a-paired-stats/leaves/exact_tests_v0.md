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
