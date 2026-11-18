# The default statistics

`analyze` computes a small set of statistics by default. They are Candler's defaults, not the
analysis: you may analyse your results differently, and nothing here stops you. `analyze` reports
every count, per run and per expectation, so the same results can be re-analysed through your own
evaluators or your own post-processing, with the method that fits your question.

This page says what each default method answers, why Candler uses it, where it stops being valid,
and what Candler does not claim for it.

## What is fixed

- **The level.** alpha is 0.05. Intervals are 95 % intervals. The value is recorded with the
  source `core_default`; there is no option to change it yet.
- **The unit of replication is the run.** An arm is a cell: all the runs that share one cell's
  settings. The outcomes of one run share the agent's behaviour, so they are not independent of
  each other. That is why the rate pooled over a role's expectations is reported as counts (a
  numerator and a denominator) with no interval: an interval over correlated outcomes would be
  falsely narrow.
- **What counts.** A run counts in an arm's `k` and `n` only if it is valid. An invalid run is in
  neither, and is reported per arm with its reason. An end that the experimenter caused (a budget
  stop, a kill, an interruption) is never charged to an arm. A `blocked` outcome counts as a
  failure; `not_measured` and `not_applicable` outcomes are left out of `n` and reported as counts
  with their causes.

## Wilson interval

**Answers.** Given `k` passes in `n` graded runs of one arm on one expectation, which pass
probabilities are compatible with what was seen? The answer is an interval around `k / n`.

**Why Candler uses it.** The Wilson score interval keeps its coverage at the small `n` that
replicates give, and at `k = 0` or `k = n`, where the textbook Wald interval (`k/n` plus or minus
a multiple of its standard error) collapses to a single point or leaves `[0, 1]`
(Brown, Cai and DasGupta, 2001). The Clopper-Pearson interval is also valid there, but it is
conservative at small `n`: it is wider than it needs to be.

**Stops being valid.** When the runs are not independent and identically distributed draws with
one pass probability; when the outcomes are weighted so that they are not plain passes and failures
(Candler then omits the interval and gives the reason `weighted`); and when `n` is 0, where there is
nothing to interval. Its coverage is close to 95 %, not always above it: at `n = 5` and a pass
probability of 0.1 it is 91.9 %.

**Candler does not claim.** That the interval holds the true pass rate of the agent in general: it
speaks about the runs of this arm under this cell's settings. It does not claim that coverage is
never below 95 %, nor an interval for a pooled rate over several expectations.

## Newcombe difference interval

**Answers.** How much higher or lower is one arm's pass proportion than another's, with what
uncertainty? It gives the difference and an interval for it.

**Why Candler uses it.** Newcombe's hybrid score interval is built from the two arms' Wilson
limits, so it is consistent with the interval Candler shows for each arm, and it too behaves at
small `n` and at `k = 0` or `k = n`, where the Wald interval for a difference does not
(Newcombe, 1998, method 10; no continuity correction).

**Stops being valid.** Under the same conditions as the Wilson interval, for both arms: independent
runs, a fixed pass probability within each arm, plain passes and failures. It is also not defined
when either arm has no graded runs.

**Candler does not claim.** That the difference is large enough to matter, or that a difference
whose interval contains 0 is zero. The interval is reported beside the test of the next section and
is not a test itself.

## Mid-p exact test (one block)

**Answers.** If the two arms had the same pass probability, how likely is a split of passes as
uneven as the one seen, or more uneven? The p-value is the total probability of every table less
likely than the observed one, plus half the probability of the tables as likely as it, given the
margins (two-sided). With one block this is Fisher's exact test with the mid-p adjustment.

**Why Candler uses it.** It needs no large-sample condition, so it is valid at the 3 to 20 runs that
replicates give. The replicates per arm are fixed by design, so Fisher's own test, which conditions
on both margins, is conservative here; the mid-p is about as powerful as an unconditional test and
keeps its level in practice (Lydersen, Fagerland and Laake, 2009). Rejected alternatives: the
chi-square test and the z test for two proportions, whose approximations are invalid at small `n`.

**Stops being valid.** When the runs within an arm are not independent draws with one pass
probability; when the two arms were not run under comparable conditions (Candler compares cells only
inside one comparable stratum, and reports a refused stratum as refused); and when the outcomes are
weighted, where the permutation test below applies.

**Candler does not claim.** That a p-value above alpha shows the arms are equal: with a few runs
the test has little power, and a non-rejection is an absence of evidence. It does not claim that
the p-value is the probability that the null hypothesis is true, nor that a rejection is
practically significant. The mid-p can fall slightly below alpha under the null, as it is not
guaranteed to be conservative.

## Cochran-Mantel-Haenszel test (several blocks)

**Answers.** When the compared factor is tested within blocks (the model, when the question is the
skill version), does it change the pass rate with the block held fixed? The test pools each block's
evidence without letting a block with a different base rate create or hide an effect. Candler shows
the Mantel-Haenszel odds ratio of B over A with its interval, each block's own difference, and
Breslow-Day's homogeneity test as information only.

**Why Candler uses it.** Pooling the blocks into one table can reverse a real effect (Simpson's
paradox); the blocked test holds its level where the pooled one does not. No continuity correction
is applied.

**Stops being valid.** When the chi-square approximation is poor. Candler computes the
Mantel-Fleiss criterion per comparison and, when it is below 5, runs the exact conditional test (the
convolution of the blocks' hypergeometric distributions) instead.

**Candler does not claim.** That the effect is the same in every block, nor that homogeneity was
tested as a condition: per-block effects are shown, not tested.

## Permutation test and BCa interval (weighted expectations)

**Answers.** For an expectation whose outcomes carry weights, is the difference in the summed
numerators over the summed denominators larger than the labelling of runs could produce by chance?
Run labels are shuffled within blocks 10 000 times, and the p-value counts the observed labelling
once, so it is never 0 (Phipson and Smyth, 2010). The interval is a stratified bias-corrected and
accelerated bootstrap interval (Efron, 1987).

**Stops being valid.** When runs are not exchangeable within a block under no difference; the
design argues that they are. An arm with fewer than two runs gets no interval, with the reason
`too_few_runs`. The shuffle seed is fixed, so a re-run gives the same p-value.

**Candler does not claim.** An independent check of the multi-block interval beyond a twin of the
shuffle, and a reading of "no difference" from a wide interval.

## Holm adjustment

**Answers.** When several comparisons are tested at once, which of them can be called different
while the chance of calling any equal pair different stays at or below alpha? Holm's step-down
procedure adjusts the p-values for that. Candler adjusts over the expectations of one role in one
comparison.

**Why Candler uses it.** Holm controls the family-wise error rate (the chance of at least one false
rejection in the family) with no assumption of independence between the tests, which matters
because the outcomes of one run are correlated across expectations. It is never less powerful than
Bonferroni, and usually more (Holm, 1979).

**Stops being valid.** It controls the error only for the family it was given. A family that is
chosen after looking at the results, or split across several `analyze` calls, is not covered. It
inherits the validity of the p-values it adjusts.

**Candler does not claim.** An adjustment across separate `analyze` calls, across roles, or across
questions that were not in the call. It does not claim that every adjusted p-value above alpha is
a true null.

## Bonferroni (the rejected baseline)

**Answers.** The same question as Holm: it multiplies every p-value by the number of tests `m`
(capped at 1) and compares with alpha.

**Why Candler uses it.** It does not use it. It is the baseline against which Holm is chosen: it
gives the same guarantee with no assumption of independence, and Holm rejects everything Bonferroni
rejects and sometimes more, at no extra assumption.

**Stops being valid.** Where Holm does, and it is also conservative: with many tests it loses power
that Holm keeps.

**Candler does not claim.** That it is wrong; it is a sound, simple, weaker procedure.

## What Candler does not claim

- **Practical significance.** A rejection says the data are unlikely under no difference. It does not
  say the difference matters. Read the difference and its interval for size.
- **Power.** No power analysis is made: nothing says how many runs would have been enough.
- **Adjustment across calls.** Each `analyze` call is its own family.
- **Equality.** A non-rejection is not evidence of equality. Showing equivalence takes a different
  test.
- **Paired tests.** The tests here are for independent arms. Paired tests are not offered yet.
- **Comparisons outside a comparable stratum.** Comparisons exist only between cells in one
  comparable stratum. A pair the stratum check refuses is reported as refused, with its cause, and
  the rates of each of its cells are still reported.
- **Independence.** The statistics treat the runs of an arm as independent. Candler cannot check
  that for you.

## Sources

- Brown, Cai and DasGupta (2001), *Interval Estimation for a Binomial Proportion*, Statistical
  Science 16(2): [doi 10.1214/ss/1009213286](https://doi.org/10.1214/ss/1009213286). Why the Wald
  interval fails, and the alternatives.
- Newcombe (1998), *Interval estimation for the difference between independent proportions:
  comparison of eleven methods*, Statistics in Medicine 17(8): [doi
  10.1002/(SICI)1097-0258(19980430)17:8<873::AID-SIM779>3.0.CO;2-I](https://doi.org/10.1002/(SICI)1097-0258(19980430)17:8%3C873::AID-SIM779%3E3.0.CO;2-I).
- Holm (1979), *A Simple Sequentially Rejective Multiple Test Procedure*, Scandinavian Journal of
  Statistics 6(2): [JSTOR 4615733](https://www.jstor.org/stable/4615733).
- Lydersen, Fagerland and Laake (2009), *Recommended tests for association in 2x2 tables*,
  Statistics in Medicine 28(7): [doi 10.1002/sim.3531](https://doi.org/10.1002/sim.3531).
- Mantel and Haenszel (1959), J Natl Cancer Inst 22(4); Mantel and Fleiss (1980), Am J Epidemiol
  112(1); Robins, Breslow and Greenland (1986), Biometrics 42(2); Breslow and Day (1980), IARC
  Scientific Publications 32. The CMH statistic, its validity criterion, the odds-ratio variance and
  the homogeneity test.
- Phipson and Smyth (2010), Stat Appl Genet Mol Biol 9(1): article 39; Efron (1987), J Am Stat Assoc
  82(397). The permutation p-value and the BCa interval.

As entry points:

- [Binomial proportion confidence interval](https://en.wikipedia.org/wiki/Binomial_proportion_confidence_interval)
- [Fisher's exact test](https://en.wikipedia.org/wiki/Fisher%27s_exact_test), the test the mid-p adjusts
- [Holm–Bonferroni method](https://en.wikipedia.org/wiki/Holm%E2%80%93Bonferroni_method)
