# Analyze output leaf report (v0)

Branch `m6a/analyze_output`, base `aa2a9ffa`.

## Commits
- `d7b939c5` test(analysis): guard blocked comparisons in analyze (rows seen red)
- `a6446b09` feat(analysis): compare declared factors blocked by the others
- `f4cb5069` feat(analysis): report blocked comparisons in analyze
- `4a4f8d2e` docs(analysis): the stop for a ruling (superseded by this report)
- `cbde6c28` fix(resolve): key the treatment verdict on request.treatment (ruling 2)
- `3f05d44f` feat(analysis): report every difference as B minus A (ruling 3, the ruled guard edits, the one-factor literal row)
- `e9f25ef0` fix(analysis): count treatment pairs; a flat bootstrap is inconclusive (verifier F1, F2)
- the report commit that carries this file

## Gates
- Targeted: `guard_analysis_report::` + `guard_stats::` 41 of 41 PASS; `guard_analyze` 13 of 13 PASS; `compar|verdict` rows PASS (72 of 72 with the report rows).
- `cargo clippy -p candler -p candler-core --all-targets -- -D warnings`: PASS. `cargo fmt --check`: PASS.
- DIGEST: numstat over oracle and vector dirs PASS (no edit); `guard_cell_digest` 9 of 9; the core digest filter 45 of 45 PASS.
- Ownership check (with the two ruled additions, `manifest/compare.rs` and `verdicts.json`): prints nothing. `fn confirm_`: none. GEN not run (no vector touched).
- Contract pages: no `docs/contract` page names the analyze columns or families; `contract_pages` passes, nothing blessed.

## Claims checked
- L1 API wired as handed over: per-run units to `blocks_of` and `unit_blocks_of`, `.keyed(&unit_block_keys(..))` for weighted designs, a seed to `test_blocked`, `holm_families` once per comparison, BCa below two units per arm surfaced per entry (`too_few_runs`).
- `level_of` read `null` for `request.treatment` (confirmed by the red treatment row); it now returns the typed reference.
- `fisher_two_sided` is no longer called from `report.rs`; kept, `fisher.toml` untouched.
- Seed: `PERMUTATION_SEED = 0xC6C7_5EED`, a fixed constant (the store holds none).

## Changed guards
Cause key: C6 comparisons, C7 per-comparison Holm, MP mid-p, W weighted battery, SGN sign B minus A, T5 spelling.

| Guard | Assertion, old → new | Cause |
|:--|:--|:--|
| core `a1_holm_family_is_one_per_role_per_call` | families: 1 family `{gate, 6}` → 6 families (3 × gate 2, 3 × quality 1) | C7 |
| | E3: `Omitted(Weighted)` → tested, `Battery::Permutation`, quality | W |
| | p: Fisher row → `midp.toml` row | MP |
| | diff, diff_lo, diff_hi: Newcombe row (A,B) → row (B,A), now `Option` | SGN |
| | p_holm: step-down over 6 gate p → over the comparison's own 2 | C7 |
| | removed `FisherRow`, `fisher_p` (dead) | MP |
| core `a2_a_comparable_pair_is_tested` | p: `fisher_p((4,5),(1,5))` 0.2063492063492064 → mid-p 0.10714285714285714 | MP |
| core `a2_a_cell_in_two_strata_is_tallied_per_stratum` | families: 1 × m 2 → 2 × m 1; p: Fisher → mid-p; `p_holm`: `2p` → `p` | C7, MP |
| core `a2_a_wrong_experiment_is_refused` | comparisons 4 → 2 (`pairs_not_one_factor` stays 2) | C6 |
| CLI `a2_a_wrong_experiment_is_refused_through_the_binary` | comparisons 4 → 2 | C6 |
| CLI `a1_the_json_equals_the_hand_counts` | families `[{gate,6}]` → 3 × `[{gate,2},{quality,1}]`; E3 `omitted: weighted` → `battery: permutation`; p Fisher → mid-p; diff rows (A,B) → (B,A); p_holm over 6 → per pair of 2 | C7, W, MP, SGN |
| CLI `a1_the_csv_tables_equal_the_hand_counts` | header 19 → 30 columns (11 appended); tested rows 6 → 9, `weighted` rows 3 → 0; p Fisher → mid-p; diff (A,B) → (B,A); summary `family.gate=6` → `family.gate=2, family.quality=1` ×3 | C6, W, MP, SGN, C7 |
| CLI `a1_a_cell_in_two_strata_is_two_arms` | families `[{gate,2}]` → `[{gate,1},{gate,1}]` | C7 |
| `verdicts.json` `t5_treatment_explains_lock_image_inputs`, `t5_equal_treatment_does_not_explain_lock_image_inputs` | `declared: ["treatment"]` → `["request.treatment"]`; expectation unchanged | T5: the old spelling is one no input can produce |
Also: test helper `Spec` gained `treatment`, `inputs`, `environments` (defaults equal the old literals); the doc of `c6_one_factor_store_keeps_todays_output` says B minus A. No file under `oracle/` or `vectors/` was edited. Rows C10 and one more declare the bare `treatment`; they still pass because a bare path still covers by prefix, but only `request.treatment` explains lock, image and inputs. No NaN sentinel existed in merged code.

## Bites
| Bite | Injected | Red row and message |
|:--|:--|:--|
| one-factor pairs kept | comparison only for one cell per side (a surrogate of the four cell pairs) | `c6_two_by_two_gives_two_blocked_comparisons`: left 0, right 2 |
| Fisher's p | `fisher_two_sided` in the one-block branch | `c6_one_factor_binary_store_keeps_todays_literals`: p 0.2063… against 0.10714…; `c6_one_factor_store_keeps_todays_output`: p 0.3498… against 0.2035… |
| one Holm family | every group keyed 0 | `c6_holm_per_comparison_and_role`: p_holm low 0.0238 against 0.0079 |
| "no difference" | `Reading::Inconclusive` prints "no difference" | `c6_reading_effect_or_inconclusive`: left "no difference", right "inconclusive" |
| level from values | the treatment branch of `level_of` off | `c6_a_treatment_level_is_its_typed_reference`: left 1, right 2 comparisons |
| bare spelling (ruling 2) | `factor == TREATMENT` in the verdict | `c6_a_treatment_store_with_new_locks_and_image_is_comparable`: Refused(["environments","inputs","isolation.image","treatment"]) |
| old sign | `newcombe_difference(a, b)` | `c6_reading_...`: diff -0.6 against 0.6; `a1_holm_...`: diff -0.3 against 0.3 |
Each file restored from a copy; the rows pass again.

## Verifier
Sonnet spec-verifier, 27 calls, VERIFY_DEVIATION with no test-gaming and no wrong comparison set (comparison set, per-run units, Holm, rulings 1 to 3 held). Findings and handling:
- F1 `pairs_not_one_factor` overcounted treatment-only pairs: fixed (`e9f25ef0`), asserted `== 2` in the treatment row.
- F2 a zero-width BCa interval read "effect": fixed, row `c6_weighted_degenerate_and_short_arms` (also covers the weighted battery and `too_few_runs`).
- F3 one-factor output differs beyond the p-value (sign, weighted entries, Holm family): by ruling 1 and 3, listed above; stale doc fixed.
- F4 `level_of` prefix-object branch: intentional (a table-level factor, verdict row C08, would read `null`); the treatment branch keeps `unwrap_or(Null)` for a serialisation failure that cannot occur.
- F5 no `analyze_c6_*.json` vector: none needed, the rows use `midp.toml`, `newcombe.toml`, `cmh.toml` rows; the numstat gate holds vacuously. Deviation from the leaf's Deliverables.
- F6 tautological `assert_ne!` removed; weighted and short-arm row added. Added afterwards: the weighted 2×2 row and a CMH reading row. Not added: a bare-spelling `t5_` refusal row (no input can produce that spelling).
- Noted, not changed: runs in blocks without both arms leave `Tested.a/b`; a run lacking a factor reads level `null`; a `StatsError` other than `NoRuns` from a test aborts the call; CSV `number()` would print `inf`/`NaN` if `or_mh` were non-finite.
Re-check of `e9f25ef0` by the same verifier: F1 CLOSED (`factors_between` maps `request.treatment` to the typed root; the treatment row asserts `pairs_not_one_factor == 2`), F2 CLOSED (Permutation reads "effect" only for `low < high` with the interval excluding 0), F6 tautology closed, F3 and F4 accepted, F5 recorded as a deviation. Residual, informational and not fixed: a flat bootstrap (for example 50 identical runs per arm, 0.1 against 0.9) reads "inconclusive" even with a tiny permutation p. The two gaps it left open are closed by rows: the CMH/ExactStrata reading by `c6_cmh_reading_follows_the_odds_ratio_interval`, and a weighted expectation in a multi-block store (keyed blocks, per-run units) by `c6_weighted_two_by_two_gives_keyed_permutation_blocks`. Those rows add coverage only and were not re-checked.

## Findings
1. Verdict fix (ruling 2) done at the source with the schema constants; no alias in `report.rs`.
2. **How `golden_blocked` could compare skill versions on imported runs.** An imported manifest's `factors` are `varying_paths` over `values`. The treatment is a typed root, never in `values`, so no imported store declares `request.treatment`, and a treatment comparison (or block) cannot arise from it. Two routes for L3: the import records the versions as distinct typed treatments and writes `request.treatment` into the manifests' factors (a change in the import, then `compare_pairs` works as is), or the version is also recorded as a value at some path (a label under `values`) which then varies and is declared. Not fixed here.
3. Signs are B minus A everywhere; CMH and exact-conditional comparisons give `diff*` as `None` (null), reading from the MH odds-ratio interval against 1.
4. `a` and `b` of a comparison are the first cell of each level in key order; levels ordered by first appearance, so a one-factor store keeps today's A and B. A comparison needs a block with cells at both levels.

5. **Behaviour change in `blocking.rs::level_of`:** a factor with no value at its exact path but values below it (a table-level factor, verdict row C08) now reads an object `{path: value}` where it read `null`. No row guards it; it is untested.
6. **Deviation from Deliverables:** the leaf names `crates/candler-core/tests/oracle/vectors/analyze_c6_*.json`; none was added. The rows take their expected values from `midp.toml`, `newcombe.toml`, `cmh.toml` and hand arithmetic, so the vectors numstat gate holds vacuously.
7. Residual from the verifier: a flat bootstrap reads "inconclusive" however small the permutation p.

## Left
Nothing for the leaf itself. Open for the coordinator: the L3 route in Finding 2.
