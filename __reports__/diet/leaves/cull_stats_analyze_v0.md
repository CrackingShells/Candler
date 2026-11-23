# cull_stats_analyze: leaf report v0

## Before count

Owned files (minus the ignored `guard_analyze_golden`), as listed by `cargo nextest list -p candler-core -p candler`: 102 tests (87 after). The three tests of `guard_analyze_golden` are `#[ignore]`d by design (they need `CANDLER_GOLDEN_CORPUS`); none is cited as a cover, and the file is untouched. `-p candler-core -p candler`: 560 listed before, 545 after (545 pass, 32 skipped by the profile).

## Deletions

| File | Test | Verdict | Covering guard |
|:--|:--|:--|:--|
| guard_stats | t1_z_is_the_normal_quantile | repeat | `t1_wilson_matches_the_second_engine` (the same `z` bite: 1.96 moves every limit past 1e-9) |
| guard_stats | t1_the_tie_tolerance_changes_no_vector_value | structure | tests the vector file's property; `t1_fisher_ties_follow_the_tolerance` holds the behaviour |
| guard_stats | t2_wilson_coverage_is_exact | repeat | `t1_wilson_matches_the_second_engine`; coverage is derived from the same Wilson limits |
| guard_stats | t3_blocked_counts_as_a_failure | repeat | `t3_every_rate_carries_its_counts` (`n` is pass + fail + blocked); `a1_arms_equal_the_hand_counts` |
| guard_stats | t3_experimenter_ends_are_not_charged_to_an_arm | repeat | `a1_invalid_and_experimenter_runs_are_out_of_k_and_n` (report API) |
| guard_stats | t3_roles_sum_as_rates_does | repeat | `rt1_three_hand_computed_cells` and `a1_arms_equal_the_hand_counts` (the reference implementation it compared against is held by hand sums) |
| guard_stats | t3_weights_make_an_expectation_non_binary | repeat | `a1_arms_equal_the_hand_counts` (E3 `weighted` in every arm), `c6_weighted_degenerate_and_short_arms` |
| guard_stats_blocking | blocks_exclude_the_compared_factor | repeat | `c6_two_by_two_gives_two_blocked_comparisons` |
| guard_stats_blocking | weighted_uses_permutation_and_bca | repeat | `c6_weighted_two_by_two_gives_keyed_permutation_blocks` |
| guard_stats_blocking | holm_family_is_per_comparison_and_role | repeat | `c6_holm_per_comparison_and_role` |
| guard_stats_resampling | splitmix_matches_twin | structure | the generator is internal; `permutation_matches_twin` and `bca_matches_twin` hold its outputs bit for bit through the p-values |
| guard_analysis_report | a1_holm_family_is_one_per_role_per_call | repeat | `c6_holm_per_comparison_and_role` (same six families, same step-down values) |
| guard_comparability | nuisance_set_is_the_hand_written_one | structure | pins a constant against a schema file; `class_verdict_table` holds which paths are refused or ignored |
| guard_analyze (CLI) | c6_one_factor_binary_store_keeps_todays_literals | repeat | `a1_the_json_equals_the_hand_counts` (mid-p, Newcombe difference from the vector rows) and `a2_a_comparable_pair_is_tested` (the same 4 of 5 against 1 of 5 table at the op API); the test pinned pre-mid-p history |

Helpers and imports left unused by these deletions went with them (the `store`, `manifest`, `declared` helpers and constants of `guard_stats_blocking`, `CoverageRow`, `SplitMixRow`, `alpha_beta`).

## Kept against the sweep's verdict

- `guard_selection` is not merged (2 tests stay). `sel1_...` is the only test of `I4Violation` anywhere under `crates/`; `rt1_three_hand_computed_cells` is the only hand-summed test of `rates` and the `rates_toml` form. Deleting either leaves a behaviour with no test.
- `guard_analysis_report` keeps 23 of 24 (sweep: about 16). The remaining rows each hold a distinct behaviour at the op API, and the CLI rows of `guard_analyze` repeat the hand counts, the two-class arms, the refused pair, the wrong experiment and the unsealed run once per interface, as the rule says. `u1_units_in_deterministic_order` is the only test of run-key ordering of units and stays.
- `t3_zero_weight_passes_add_nothing` is restored after the verifier: its first cover lived in the ignored, corpus-gated `guard_analyze_golden`, so no default-suite test held the zero-weight tally.
- `t3_invalid_runs_are_out_per_arm_with_their_reason` stays: the reason counted once by run key, with several records or none, is not in `a1_invalid_and_experimenter_runs_are_out_of_k_and_n`.
- `c6_one_factor_store_keeps_todays_output` stays: it is the only CLI row for factors inferred from manifests without `--experiment` with comparisons.
- `guard_stats` keeps 13 of 20 (sweep: about 10): `t2_family_wise_error_holds_under_h0` and `t2_power_is_found` are a pair (a test that never rejects passes the first alone), and `t3_excluded_statuses...`, `t3_regraded...` and `t3_every_rate...` hold behaviours no other test holds.
- The scipy and statsmodels vector rows (Wilson, Newcombe, Holm, Fisher, mid-p, exact strata, CMH), `guard_known_truth` (7), `guard_golden_design` (4) and `guard_analyze_golden` are unchanged.

## Bites

None. No guard was merged or rewritten: this cull only deletes tests, and every covering guard is an existing one that is not edited.

## Follow-ups

- If the coordinator wants `guard_selection` merged into `guard_analysis_report` as the plan says, it costs a move of about 250 lines with their helpers and one bite on `select` and `rates`; the leaf judged it not worth the risk tonight.
- `a1_the_json_equals_the_hand_counts` reads one Holm family `gate, 6` while `c6_holm_per_comparison_and_role` reads one family per comparison; both pass, so the CLI row's doc line is probably stale. Not a behaviour change; a verifier may look.

- The cover of `t3_experimenter_ends_are_not_charged_to_an_arm` pins only `killed`, not `budget_stop` or `interrupted`.
- `splitmix.toml` and `coverage.toml` have no test reader left; a later piece can delete them.

## Verifier

PASS with findings (1 medium, 3 low), then fixed. `t3_zero_weight_passes_add_nothing` was covered only by an `#[ignore]`d, corpus-gated golden test; it was restored in 0a8b97ab and the report citations were corrected.
