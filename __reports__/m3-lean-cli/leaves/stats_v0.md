# stats, leaf report v0

Candler's default statistics in `candler-core/src/analysis/`, with the reader page `docs/statistics.md`. The implementer was Sonnet, and the work took 31 minutes plus a 46-minute fix round. Branch `m3/stats` has 7 commits, head 043605b, merged as `merge(analysis): land stats`. The Sonnet verifier returned PASS.

## Second engine
- scipy 1.17.0, statsmodels 0.15.0, Python 3.12.12, pinned in each vector file's `[engine]` table. `gen_stats.py` reads no Candler file, and `--check` regenerates and compares.
- Vector rows: wilson 179, fisher 593 (24 equal-arm), newcombe 576, holm 212, coverage 9, power 1.

## Decisions taken in the leaf
- **Wilson coverage floor is 0.91, and the coverage must equal scipy's exact table to 1e-9.** This deviates from D8's ≥ 0.93, which Wilson cannot meet at small n: n = 5, p = 0.1 gives exactly 0.9185, and n = 10, p = 0.1 gives 0.9298. Accepted by the coordination session on 2026-11-12, for Eliott's review.
- **Fisher's tie tolerance is 1e-7, relative.** This is R's `fisher.test` (`relErr`), where scipy uses 1e-14. statrs' hypergeometric pmf has a relative error of up to about 7e-14, so at 1e-14 exactly tied tables read as distinct (1/26 against 2/26 gave 0.6176, where scipy gives 1.0). Exact fractions over every margin of the grid find no pair of distinct probabilities with a relative gap between 1e-14 and 1e-7 (`[ties] near_ties = 0`). The verifier rechecked this independently: the smallest non-zero gap is 7.96e-4.
- `Omitted` gains `Descriptive` and `Unavailable`. `StatsError` is internal: its `[internal]` row is written by `analyze`.
- A run listed twice counts once, by its last listing. A descriptive expectation tallies its statuses, but `counts()` is `Err(Descriptive)`.
- `rand_core` 0.10: `Rng` (`RngCore` is a deprecated alias).

## Known truth (T2)
- Family-wise error over 2,000 seeded replications per scenario, bound 0.0598. The highest rate is 0.0305, at n = 30, m = 1, p = 0.5. In the correlated scenario (n = 10, m = 20) it is 0.0060.
- Power: exact 0.6353 (equal to scipy's), simulated 0.6390.
- Runtime is about 4 s in a debug build.

## Bites (each seen red, then reverted)
- **Wilson and the normal quantile:** Wald in place of Wilson; z fixed at 1.96; Wilson unclamped (n = 16, k = 16 gives 1.0000000000000002).
- **Holm:** without the step-down maximum; Holm as the identity.
- **Fisher:** twice the smaller tail; tolerance 0; tolerance 1e-14; a planted near-tie, caught by `--check`; Fisher that never rejects.
- **Newcombe:** Wald difference in place of the hybrid score.
- **Rate algebra:**
  - `blocked` left out of n;
  - `not_applicable` put in n;
  - `killed` charged to the arm;
  - invalid runs counted;
  - a zero-weight pass counted;
  - the wrong role summed;
  - an unlisted run skipped silently.
- **T2:** a continuity-corrected Wilson.

## Gates
OWNED, T1–T3, the scipy check, doc lines (19/19/19), G-digest, `guard_layering_i1`, the page loop, STD (`-p candler-core`), LINUX and SUBJ all PASS.

## Links in `docs/statistics.md`
- Verified by title or by Crossref metadata: the three Wikipedia articles, Brown–Cai–DasGupta (2001), and Newcombe (1998).
- **Not tool-verified:** Holm (1979), `https://www.jstor.org/stable/4615733`. JSTOR blocks automated fetches. Left for a by-hand check.
