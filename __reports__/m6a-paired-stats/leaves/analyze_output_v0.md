# Analyze output leaf report (v0, stopped for a ruling)

Branch `m6a/analyze_output`, base `aa2a9ffa`. The leaf stopped before its bites and its verifier: four
existing `a1_` guards go red by design of C6, C7 and the mid-p, and only `a2_` rows were pre-ruled.

## Commits
- `d7b939c5` test(analysis): guard blocked comparisons in analyze (four report rows, seen red)
- `a6446b09` feat(analysis): compare declared factors blocked by the others (`report.rs`, `blocking.rs::level_of`, the pre-ruled `a2_` edits)
- `f4cb5069` feat(analysis): report blocked comparisons in analyze (`analyze.rs`, the two CLI `c6_` rows, one pre-ruled `a2_` edit)

## Gates
- `cargo clippy -p candler -p candler-core --all-targets -- -D warnings`: PASS. `cargo fmt`: applied.
- `guard_analysis_report::` and `guard_stats`: 58 of 59 pass; red: `a1_holm_family_is_one_per_role_per_call`.
- `guard_analyze`: 9 of 12 pass; red: `a1_the_json_equals_the_hand_counts`, `a1_the_csv_tables_equal_the_hand_counts`, `a1_a_cell_in_two_strata_is_two_arms`. The five `c6_` rows pass.
- Ownership check: prints nothing. No vector or oracle file touched, so the numstat gate holds. DIGEST not run.

## Claims checked
- `blocks_of`, `unit_blocks_of`, `holm_families`, `test_blocked(design, seed)`, `Blocked::keyed`, `unit_block_keys` exist in the shapes of the L1 handover and are wired as ruled.
- `request.treatment` is a typed root: `comparable().values()` never holds it, so `level_of` returned `null`. It now returns `serde_json::to_value(comparable().treatment())`.
- `fisher_two_sided` is untouched, and unused by `report.rs` now; clippy did not ask for its removal.

## Changed guards
Pre-ruled `a2_` (edited, reason in brackets):
- `guard_analysis_report::a2_a_comparable_pair_is_tested` [p is the mid-p, `midp.toml`, not Fisher's].
- `guard_analysis_report::a2_a_cell_in_two_strata_is_tallied_per_stratum` [mid-p; Holm is per comparison (C7), so two families of one test, `p_holm == p`].
- `guard_analysis_report::a2_a_wrong_experiment_is_refused` [4 pairs become 2 comparisons; `pairs_not_one_factor` stays 2].
- `guard_analyze::a2_a_wrong_experiment_is_refused_through_the_binary` [same, 4 to 2].
Not edited, red, awaiting a ruling (all `a1_`; the first failing assertion each, later ones in the same row will follow):
- core `a1_holm_family_is_one_per_role_per_call`: `families` is 1 family of 6 gate tests; now 6 families (gate 2 and quality 1 for each of the 3 comparisons); `E3` was `Omitted(Weighted)`, now tested by the permutation test; the raw p is the mid-p, not Fisher's; `p_holm` is the step-down of the comparison's own two gate p.
- CLI `a1_the_json_equals_the_hand_counts` (`families` `[{gate,6}]` becomes the six families) and `a1_a_cell_in_two_strata_is_two_arms` (`[{gate,2}]` becomes `[{gate,1},{gate,1}]`).
- CLI `a1_the_csv_tables_equal_the_hand_counts`: the comparisons header grows from 19 to 30 columns (the leaf appends after the existing ones); the three `omitted = weighted` rows become tested rows; the tested count 6 becomes 9; the p column is the mid-p.

## Bites
Not run (stopped first). To do: keep one-factor pairs (red on `c6_two_by_two_`); Fisher's p (red on `c6_one_factor_`); one Holm family across comparisons (red on `c6_holm_`); "no difference" for a spanning interval (red on `c6_reading_`); level read from `values` (red on `c6_a_treatment_level_`).

## Verifier
Not dispatched. The verifier brief plus the ruling-1 sentence is ready in the dispatch.

## Findings
1. **Verdict refuses a declared `request.treatment`.** `stratum_verdict` returns `Refused(["treatment"])` when the manifests' factors name `request.treatment`, because `declared_covers` matches `treatment.*` deviation paths and fixture C10 declares `treatment`. A treatment comparison then has levels and no tests. The row `c6_a_treatment_level_is_its_typed_reference` runs both spellings; only `treatment` yields tests. Which spelling real manifests record is unchecked: coordinator to settle (a `compare.rs` fix is outside this leaf's OWNED).
2. **Signs.** `Tested.diff/diff_lo/diff_hi` keep today's first-minus-second (A minus B) so a one-factor output only changes in p; `per_block[].diff` and `or_mh` are B over A, as ruled. A permutation interval is flipped to A minus B. CMH and exact-conditional comparisons give no interval of the difference: `NaN`, which is `null` in JSON and empty in CSV; their reading is "effect" when the MH odds-ratio interval excludes 1.
3. **Cells and levels.** `Comparison.a` and `.b` are the first cell, in key order, at each level; levels are ordered by first appearance in cell-key order, so a one-factor store keeps today's A and B. `levels` is `Some`. A comparison exists only where some block holds cells at both levels; the verdict covers every valid run of both levels.
4. **Seed.** `PERMUTATION_SEED = 0xC6C7_5EED` in `report.rs`: the store holds no seed, so a fixed constant.
5. **Weighted entries are tested** (permutation, `BCa`), no longer omitted as `weighted`. New `OmitReason::TooFewRuns` (`too_few_runs`) when an arm of a block has fewer than two units; no block with both arms is `no_runs`.
6. `level_of` also reads an object of the values below a prefix factor (a table-level factor, verdict row C08) instead of `null`, and accepts `treatment` next to `request.treatment`.
7. A degenerate mid-p table (zero margin) reports p = 0.5 as the engine gives it; its reading comes from the interval.

## Left
Rulings on the four `a1_` rows above; then edit them, run the bites, dispatch the verifier, DIGEST, the report's Bites and Verifier sections.
