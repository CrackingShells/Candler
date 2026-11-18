# Blocking leaf report (v0)

Branch `m6a/blocking`, base `31a103bc`.

## Probe answer

Every weighted outcome's `num` and `den` are integers on the wire: `Weight` in
`crates/candler-schemas/src/outcome.rs` is built by `Weight::new(num: u64, den: u64)` (an object of the
integers `num` and `den`), `OutcomeRecord::effective_weight` returns it, and
`ExpectationTally::units` holds `(RunKey, u64, u64)` (`analysis/tally.rs`). Units reach the permutation
engine as `f64` through `unit_blocks_of` only (a whole-number conversion, far below 2^53).

## Claims checked

- `RunView` exists (`facts`, `manifest`) but holds no outcomes; `OutcomeKind` and `Design` did not exist
  and are defined in `blocking.rs`. `UnitBlock` and `holm` exist (`UnitBlock` holds `f64` pairs).
- `Stratum`, `cmh` (`mantel_fleiss_holds`), `exact_conditional`, `midp_two_sided`,
  `stratified_permutation`, `stratified_bca` exist in the shapes the leaf assumes.

## Findings (deviations from the leaf text, intent unchanged)

1. `blocks_of` and `unit_blocks_of` take the per-run units `&[(RunKey, u64, u64)]` of the one expectation
   (the concatenation of `ExpectationTally::units()` over the arms) instead of an `&ExpectationId`:
   `RunView` carries no outcomes, so an id alone cannot select any. The caller picks the expectation by
   choosing which tally's units it passes.
2. `holm_families` takes `&mut [Vec<Tested>]` (one vector per comparison) because `Tested` does not know its
   comparison; the pure core is `holm_groups(&[(comparison, Role)], &[p])`, which the row exercises since
   `Tested` is `non_exhaustive` and cannot be built outside the crate.
3. `test_blocked(&Design, seed)` returns `Blocked` (battery, blocks, fallback, p, diff B minus A, interval,
   MH odds ratio and interval, homogeneity p, per-block effects). `Battery::ExactStrata` is only ever the
   fallback of CMH; `choose` never returns it.
4. A block whose arm has no run is left out of `blocks_of`; `test_blocked` refuses an empty arm with `NoRuns`.
   A run with several units (eval-task outcomes) contributes each unit.
5. Additive fields: `Tested { battery: Option<Battery>, blocks, fallback, per_block }` and
   `Comparison { levels: Option<(Value, Value)> }` (`Comparison.factor` already existed). Both structs are
   built by literal in `entries_of` and `compare_pairs`, which are `analyze_output`'s: the only edit there is
   one initialiser line each (`battery: None, blocks: 1, fallback: false, per_block: Vec::new()` and
   `levels: None`), no behaviour change. `battery: None` marks today's one-factor Fisher p, so it is not
   mislabelled mid-p.
6. Factor levels are read from `manifest.comparable().values()` by exact path. A factor that resolves into
   the typed root `treatment` (T1/T2) is not in `values`, so a treatment comparison needs the caller (L2) to
   say how the level is read; this leaf reads `null` for an absent path. For the coordinator.
7. The permutation seed is a parameter; `REPLICATES` = 10 000 and `LEVEL` = 0.95 are constants.
   `stratified_bca` errors (not an interval `None`) with fewer than two runs per arm, and `test_blocked`
   propagates it.

## Bites

| Bite | Injected | Red row and message |
|:-----|:---------|:--------------------|
| compared factor in the block key | `.filter(\|_\| true)` in `gather` | `blocks_exclude_the_compared_factor`, `left: []`, no block formed (the key held the compared level); `weighted_uses_permutation_and_bca` the same |
| a forking path: the test chosen from the p-value | one block takes the mid-p only when its p is below 0.05 | `one_block_uses_midp`: `left: ExactStrata, right: MidP`. A first attempt (CMH kept only when its p does not exceed the exact p) stayed green on every row, so it was replaced |
| one Holm family for gate and quality, and across comparisons | every group keyed `(0, true)` | `holm_family_is_per_comparison_and_role`: `[0.04, 0.06, 0.04, 0.06]` for the expected `[0.02, 0.04, 0.01, 0.03]` |

Each file was restored from a copy and the six rows pass again.

## Verifier

(pending)

## Commits and gates

(see the final message)
