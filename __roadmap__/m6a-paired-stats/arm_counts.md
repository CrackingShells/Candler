# Arm counts

**Goal**: Each arm reports `instrument_error {runs, outcomes}` with its counts of the other invalid causes unchanged (E1), and `arm_counts` exposes each run's `(num, den)` per expectation in a deterministic order, the units the permutation and BCa tests need (C4).
**Estimate**: about 70 Sonnet calls, $2.9; no verifier
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (README, Gotchas)
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/analysis/(tally|report)\.rs$|^crates/candler/src/commands/analyze\.rs$|^crates/candler-core/tests/it/guard_analysis_report\.rs$|^__reports__/m6a-paired-stats/leaves/arm_counts_v0\.md$`. In `report.rs`, only `ArmReport` and the code that builds it; in `analyze.rs`, only `arm_json`. `summary_csv` and every existing JSON key are unchanged. `analyze_output` (L2) edits these files later, in sequence
**Success Gates**:
- ⬜ [cmd] Targeted tests: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats::t3_/) | test(/^guard_analysis_report::a1_/) | test(/^guard_analysis_report::e1_/) | test(/^guard_analysis_report::u1_/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_analyze::a1_/)'`
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] The `e1_` oracle is a hand count written in the test, not a value read from the code; the report records each Bite. If an existing `a1_` oracle compares whole JSON objects and the new key breaks it, the implementer stops and asks the coordinator before changing it
**References**: [R03 E1, per-run units](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md); `analysis/tally.rs`: `ArmTally`, `ExpectationTally`

## Step 1: Write the e1_ and u1_ rows, seen red
**Goal**: Rows that pin the new count and the units.
**Implementation Logic**:
1. `e1_instrument_error_counted_per_arm`: a store of runs with known causes (instrument error in some runs, other invalid causes in others, valid runs), its expected `{runs, outcomes}` counted by hand in the test; the other invalid-cause counts equal today's.
2. `u1_units_in_deterministic_order`: for a weighted expectation, `units()` yields one `(num, den)` per valid run, ordered by run key; the same store read twice gives the same order; Σnum and Σden equal the tally's sums.
**Deliverables**: `crates/candler-core/tests/it/guard_analysis_report.rs`: `e1_instrument_error_counted_per_arm`, `u1_units_in_deterministic_order`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_analysis_report::e1_/) | test(/^guard_analysis_report::u1_/)'` (expected: FAIL)
**Commit**: `test(analysis): guard the instrument-error count and the per-run units`

## Step 2: Count instrument errors and keep the units
**Goal**: The rows pass; existing outputs are unchanged.
**Implementation Logic**:
1. `ArmReport.instrument_error: InstrumentError { runs: u64, outcomes: u64 }`, filled where the other invalid causes are counted.
2. `ExpectationTally` keeps, beside its sums, `units: Vec<(RunKey, f64, f64)>` sorted by run key, exposed as `units()`; the sums stay the source of today's numbers.
3. `arm_json` adds the key `instrument_error`; no other key changes; `summary_csv` is not touched.
**Deliverables**: `crates/candler-core/src/analysis/report.rs`: `ArmReport.instrument_error`, `InstrumentError`; `crates/candler-core/src/analysis/tally.rs`: `ExpectationTally::units`; `crates/candler/src/commands/analyze.rs`: `arm_json`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats::t3_/) | test(/^guard_analysis_report::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_analyze::a1_/)'` (expected: PASS)
**Commit**: `feat(analysis): report instrument errors per arm and keep per-run units`

## Step 3: Record the bites
**Goal**: The rows are shown to bite.
**Implementation Logic**:
1. Bites: count instrument errors into another cause (red on `e1_`); order units by insertion (red when the store is read in another order); drop invalid runs' units incorrectly or keep them (red on the sums).
2. Record each in the report.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/arm_counts_v0.md`: section "Bites"
**Consistency Checks**: `grep -q '^## Bites' __reports__/m6a-paired-stats/leaves/arm_counts_v0.md` (expected: PASS)
**Commit**: `docs(analysis): record the bites of the arm counts`
