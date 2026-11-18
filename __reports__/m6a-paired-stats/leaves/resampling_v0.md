# Resampling: leaf report v0

## Citations

No network is allowed to this leaf, so the sources below are cited from the implementer's knowledge
of them, not re-read; the coordinator or Eliott should check the section numbers against the
papers before the milestone cites them in the architecture document (gate G3).

- **Phipson & Smyth (2010), Stat Appl Genet Mol Biol 9:39, "Permutation P-values should never be
  zero".** Sampling B random permutations, the valid p-value is (b + 1) / (B + 1), b being the
  number of sampled permutations at least as extreme as the observed statistic (the
  Introduction and the section on random permutations; the exact-p correction is the later
  section). The observed labelling is itself one of the permutations, so it is counted once
  among the B + 1 and p is never 0. The leaf computes exactly this.
- **Exchangeability within blocks (restricted permutation).** Under H0 (no difference in any
  block) the labels of the runs of one block are exchangeable, so the reference set is the
  product of the within-block relabellings, which is what Candler shuffles. Primary text for
  restricted permutation tests: Good, *Permutation, Parametric and Bootstrap Tests of
  Hypotheses*, 3rd ed., Springer 2005, ch. 3 (stratified and blocked designs); Pesarin,
  *Multivariate Permutation Tests*, Wiley 2001, ch. 2 (synchronized permutations within
  strata). Exchangeability is a design fact here (isolated containers, randomized order), not
  something the data prove.
- **Efron (1987), JASA 82:171, "Better bootstrap confidence intervals".** The BCa endpoint for
  confidence level 1 - 2 alpha is the bootstrap quantile at
  Phi(z0 + (z0 + z_alpha) / (1 - a (z0 + z_alpha))) (section 2, eq. 2.3), z0 = Phi^-1 of the
  share of replicates below the estimate (the bias correction) and a the acceleration, the
  jackknife form a = sum(d^3) / (6 (sum d^2)^(3/2)), d = mean - theta_(i) (section 6, eq. 6.6).
  The leaf's sign, d = mean - theta_(i), is Efron's; flipping it flips the interval's skew
  correction.

Statistic as built (T1 of the leaf): T = sum_k w_k (r_Bk - r_Ak), w_k = n_k / N, two-sided, a
permuted T* counts when |T*| >= |T| - 1e-12 max(1, |T|). Nothing here contradicts the battery.
