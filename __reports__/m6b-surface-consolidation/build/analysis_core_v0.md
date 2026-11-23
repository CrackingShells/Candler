# Analysis core build (leaf `analysis_core`, M6b)

Branch `claude/m6b-build-analysis_core`, from `083e537f`. Built: F2 (natural order of levels), C6
(decision summary), C8 (spend). R16's claim that A and B follow cell digest order was read on the
tip and is true: `compare_pairs` took the levels in the first-seen order of `place`, which walks
the cells in `CellKey` order.

## Guards changed

None. The four guards R16 names (`guard_golden_design.rs` `gd_comparisons_shape`,
`guard_analysis_report.rs` `c6_two_by_two_gives_two_blocked_comparisons`,
`guard_known_truth.rs` `kt_two_levels_end_to_end`, `guard_analyze.rs` 1252-1312) all passed
unchanged after the new order, so none was re-pinned. The ones read for both orders
(`a_is_alpha`, `same_pair`) never pinned a sign; the others see the same level as A under both
orders on their fixtures. No other test changed. The only edit to an existing test file besides the
new guard is visibility: `Spec`, its fields, `view`, `record`, `evaluator` and `store_of` in
`guard_analysis_report.rs` became `pub(super)` so the new `guard_analysis_summary.rs` reuses the
fabricated store instead of copying it.

## Bites

| Guard | Plausible bug made, seen red, reverted |
|:--|:--|
| `orientation_ignores_cell_digests` | drop the level sort (levels back in cell digest order): red, the store whose digests put beta first names `(beta, alpha)` |
| `summary_agrees_with_entries` | drop every omitted entry from the summary: red (`0 listings of one omitted entry`); flip `reject` of every decision: red |
| `spend_sums_recorded_costs_and_counts_the_rest` | count a missing cost as a counted run: red; count each unsealed run in the arm's counted runs: red |

The summary bites were run again after the fixture was reshaped (clippy `match_same_arms`); both
stayed red.

## D3

| Guard | Role | Question | Why no existing test answers it | Sustainable and scalable |
|:--|:--|:--|:--|:--|
| `orientation_ignores_cell_digests` | guard | does A and B depend on anything but the levels? | the report and known-truth tests fix one digest order and assert signs, never vary it | one two-cell fixture varying one non-factor setting, searched at run time so the opposite digest orders do not depend on a literal hash; asserts the levels and B minus A counted by hand (1/5 - 4/5), independent of level count |
| `summary_agrees_with_entries` | guard | can the headline disagree with the entries it summarises? | `guard_analyze` checks the JSON names the test and blocks; nothing derives a decision list | an in-memory store of three models by two settings and three expectations (one gate pair left out); asserts relations over every entry (one decision, same levels, role, `reject`, family size counted from the entries; every omitted entry once with its reason), no statistic is a literal |
| `spend_sums_recorded_costs_and_counts_the_rest` | guard | is the sum exactly the recorded values, never hiding a missing one? | caps tests exercise the live meter; `run_values` only builds per-run values | three sealed runs (0.25, 0.5, none) and one unsealed; the sum 0.75 and the counts 2 and 1 are written in the test; no model, no store on disk |

Each guard carries a `Guards:` line. The fixture of `summary_agrees_with_entries` is a core-side
store of the golden shape's kind (two factors, blocks, gates and a quality expectation, an arm with
nothing measured), not the CLI test's seeded store: that fixture lives in the `candler` test
binary and cannot be reached from `candler-core`.

## Verifier

One fresh Sonnet verifier (no blocking defect found), re-checked through SendMessage.

| Finding | Disposition |
|:--|:--|
| F-1 the summary guard did not assert `p_holm`, `diff`, its limits or `factor`; its family oracle repeats the formula | fixed: the guard asserts each field against the entry; the family count stays a relation counted from the entries (the same count `adjust` lists per comparison and role) |
| F-2 number, treatment and mixed-kind branches of the order have no test | follow-up: D3 asks for one fixture; the branches were reviewed by reading |
| F-3 levels that tie by value kept cell digest order (`1` and `1.0`, integers past 2^53, objects with a name) | fixed: ties break on the JSON text |
| F-4 `Summary` lists entries only, so a refused pair appears in neither list | by design (R16 C6 wording); follow-up for `analysis_record` |
| F-5 no CLI test for `summary`, `spent` or `inspect`'s sealed filter | follow-up, as above |
| F-6 report not yet committed | done in the closing commit |
| F-7 a doc line spliced to 190 columns | fixed |

## Follow-ups

- The number, treatment and mixed-kind branches of the level order are guarded only through
  strings; a row per kind belongs with the next test that touches levels.
- `Summary` omits comparisons that were refused (no entries); `Comparison.a` and `.b` still name
  the first cell of each level in key order, and decisions follow the comparisons' cell order.

- Nothing tests the new `summary` and `spent` keys of `analyze` and `inspect <job>` through the
  CLI; X1's table-driven guard in `output_contract` and `analysis_record` is where they land.
- `Summary::of` takes the comparisons; the family size is counted from the entries, which equals
  the `Family.m` that `adjust` lists per comparison and role (checked by the guard's own count).
- A job's `spent` in `inspect` covers sealed runs only (`entry.done`); the `runs` list of the same
  document still shows unsealed runs, as before.
- Spend covers every sealed run, invalid ones included (their cost was spent); the arm's tally
  covers valid runs only. `analysis_record` may want to say so on the wire.
- Step 1's commit does not compile its own `guard_analysis_summary.rs` (it names the API step 3
  adds); the commit is red by construction, as the leaf asks. A fixup commit reshaped the summary
  fixture after clippy's `match_same_arms`.
