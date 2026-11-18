# arm_counts: leaf report

## Commits
| sha | subject |
|:----|:--------|
| 8496ad42 | test(analysis): guard the instrument-error count and the per-run units |
| f4a7f5f8 | feat(analysis): report instrument errors per arm and keep per-run units |

## Gates
- Targeted tests (candler-core `guard_stats::t3_`, `guard_analysis_report::`; candler `guard_analyze::a1_`): PASS (24/24 and 4/4)
- clippy `-p candler-core -p candler --all-targets -D warnings`, `cargo fmt --check`: PASS
- Ownership check: PASS (prints nothing)

## Claims checked
- Instrument error is an outcome-level cause (`not_measured`, cause `reason:instrument_error`), not an `InvalidReason`; `runs` counts valid runs with at least one, `outcomes` counts the outcomes. An invalid run's outcomes are in neither. The other invalid-cause maps are unchanged (asserted in `e1_`).
- No existing `a1_` oracle compares a whole arm JSON object; the new key broke none.
- `summary_csv` and every existing JSON key are untouched.

## Bites
| Bite | Result |
|:-----|:-------|
| Count instrument errors into `invalid` as well | RED on `e1_`: `left: {"gateway": 1, "instrument_error": 3}`, `right: {"gateway": 1}` |
| Keep an invalid run's unit | RED on `u1_`: `left: [(1,3,5),(2,5,5),(3,0,5),(5,4,10),(6,5,5)]`, `right: [(1,3,5),(2,5,5),(3,0,5),(5,4,10)]` |
| Order units by insertion (remove the sort) | NOT RED. `Selection::outcomes` already yields records in run-key order, so no store can read in another order; the sort is a safeguard the current selection makes redundant. `u1_` still stores its records in reverse replicate order |

Each bite was restored from a copy of `tally.rs`.

## Findings
- **Deviation from the spec:** `units` holds `(RunKey, u64, u64)`, not `f64`. The weights are `u64`, the sums are exact, and `ExpectationTally` and `ArmTally` keep `Eq`. `analyze_output` converts to `f64` where the tests need it.
- Units exist for gate and quality expectations only (the roles whose weights enter the sums); a descriptive expectation has none.
- The ordering bite cannot be made red through the public selection (see above).
- I started the first build at a 1-minute load of 28 (the load check and the build ran in one command); later builds waited for it to fall to 13.
