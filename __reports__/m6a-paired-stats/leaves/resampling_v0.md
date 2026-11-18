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

## Bites

Each bug was injected, the guards run, and the file restored from a copy.

| Bug injected | Red guard | Failing message |
|:--|:--|:--|
| Labels shuffled over all blocks pooled | `permutation_stays_within_blocks` (and `permutation_matches_twin`) | `p 0.6787321267873213 is not within 3 SE (0.015) of 1/2` |
| `p = b/B` | `p_counts_the_observed_once` (and `permutation_matches_twin`) | `p of the observed alone: got 0, want 0.009900990099009901` |
| Jackknife sign flipped, `d = t - mean` | `bca_matches_twin` | `one low: got 0.13463476747374484, want 0.13892183116071774` |
| `below` by plain modulo, no rejection | `splitmix_matches_twin` | `assertion left == right failed: n 4611686018427387905 draw 11` |

The first attempt at the fourth bite (`threshold > 0` as the accept test) never terminated for `n = 1`;
it was a bad injection, not a guard result, and was replaced by dropping the rejection.

Added after the first verifier round: the tie tolerance removed (`cutoff = |T|`) turns `permutation_matches_twin` red on the `tie` row.

## Verifier

Round 1 (Sonnet, spec-verifier): VERIFY_DEVIATION, no formula or sign error; F1 jackknife weights recomputed while the bootstrap holds them fixed (fixed in both languages); F2 tie tolerance unguarded (`tie` row added); F3 scipy BCa check blind to the acceleration (`skew` design plus negative controls added); F4 z0 strict count versus scipy mid-rank (documented); F5 float-degenerate jackknife (documented); F6 guard comment wrong (fixed). Re-check of the fix commit requested through SendMessage; its verdict had not arrived when the call cap forced this report, so it is OPEN: the coordinator should read the verifier's reply (agent a3e3126176bb82a4e).

## Commits

See `git log c39c26fc..HEAD`: cite (docs), SplitMix64 + twin (feat), permutation (feat), BCa (feat), fix for F1-F6. The three feat commits are split by file, so only the tip passes every gate.

## Gates

GEN PASS (`bca.toml: 7 rows match`, about 8 s); targeted test PASS (6 of 6); clippy `-D warnings` PASS; fmt PASS; ownership PASS; no `fn confirm_`.

## Claims checked

Scaffold `SplitMix64` equals the published seed-0 outputs and the twin; `below` keeps `new`, `next_u64` and `below` unchanged. `statrs` normal cdf is off by up to 1e-11, so BCa uses its own series for the cdf (ppf is fine).

## Findings

`StatsError` has no fitting variants, so bad input uses `Unreadable` and empty input `NoRuns`. BCa refuses an arm with fewer than two runs. Citations were written from knowledge (no network); check section numbers before G3. Jackknife weights are fixed at the observed values.
