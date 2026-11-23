# Analysis core: order, summary, spend

**Goal**: A comparison's A and B follow a natural order of its levels, never the cell digests (F2); the core derives a decision summary from `Report`, one decision per tested entry (C6); the core sums the per-run costs it already records and counts the runs it covered and the runs without a total (C8); `analyze` and `inspect <job>` print both as added keys.
**Estimate**: about 30 Sonnet calls ($1.2), cap 45; at 45 calls, stop, commit what is green, list what is left `[V]`
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (build README, Gotchas)
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/analysis/(mod|report|summary|spend)\.rs$|^crates/candler/src/commands/(analyze|inspect)\.rs$|^crates/candler-core/tests/it/(main|guard_analysis_report|guard_known_truth|guard_analysis_summary)\.rs$|^crates/candler/tests/it/(guard_golden_design|guard_analyze|guard_inspect)\.rs$|^__reports__/m6b-surface-consolidation/build/analysis_core_v0\.md$`. In `analyze.rs` and `inspect.rs`, keys are only added: no existing key is renamed, removed or reshaped (`output_contract` and `analysis_record` change these files later, in sequence)
**Success Gates**:
- ⬜ [cmd] Targeted tests: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/orientation_ignores_cell_digests|summary_agrees_with_entries|spend_sums_recorded_costs_and_counts_the_rest/) | test(/^guard_analysis_report::/) | test(/^guard_known_truth::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_golden_design::/) | test(/^guard_analyze::/) | test(/^guard_inspect::/)'`
- ⬜ [cmd] Ownership check prints nothing; `cargo clippy -p candler-core -p candler --all-targets -- -D warnings`
- ⬜ [cmd] DIGEST passes (build README, Standard gates): the new order moves no digest
- ⬜ [nl] Only the four sign guards of R16 §Guards that change were re-pinned (`guard_golden_design.rs:402-433`, `guard_analysis_report.rs:1428-1514`, `guard_known_truth.rs:581-724`, `guard_analyze.rs:1252-1312`), each once, each with its reason in the report; every oracle of the three new guards is counted by hand in the test; the verifier closed with no open finding
**References**: [R16 §Verdicts F2, C6, C8, §D3, §Guards that change](../../../../__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md); R18 `leaves/analysis_outputs_v0.md` §F2, §C6, §C8 (the design suggestions: the natural order, the decision fields, the spend fields); [R01 §Analysis statistics](../../../../__design__/architecture.md)

## Step 1: Read the comparison and write the three guards, seen red
**Goal**: Each new guard answers its D3 question and fails on the tip.
**Implementation Logic**:
1. Through colgrep (index `crates/candler-core` and `crates/candler` of this worktree), read `compare_pairs` and `Comparison` in `analysis/report.rs`, `Tested` and the omitted-entry reasons, and where `run.cost_usd` is read for one run (`run_values`). Verify R16's claim that A and B follow cell digest order today; if it is false on the tip, stop and report to the coordinator.
2. `orientation_ignores_cell_digests`: two stores with the same levels whose cell digests sort in opposite order (one unrelated setting differs) give the same `Comparison.levels` and the same sign of B minus A.
3. `summary_agrees_with_entries`: on the golden-shape fixture used by `guard_golden_design.rs:402-433`, every tested entry has exactly one decision, whose `reject` equals the entry's, whose labels equal `Comparison.levels`; every omitted entry appears with its reason. It asserts relations, not numbers.
4. `spend_sums_recorded_costs_and_counts_the_rest`: three sealed runs, two with `run.cost_usd` and one without, plus one unsealed run: the sum is the two costs, written in the test; `runs_counted` is 2; `runs_without_total` is 1.
5. Each guard's doc comment carries a `Guards:` line. Put them in `guard_analysis_report.rs` (F2) and a new `guard_analysis_summary.rs` (C6, C8) registered in `main.rs`.
**Deliverables**: `crates/candler-core/tests/it/guard_analysis_report.rs`: `orientation_ignores_cell_digests`; `crates/candler-core/tests/it/guard_analysis_summary.rs`: `summary_agrees_with_entries`, `spend_sums_recorded_costs_and_counts_the_rest`; `crates/candler-core/tests/it/main.rs`: the module line
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/orientation_ignores_cell_digests|summary_agrees_with_entries|spend_sums_recorded_costs_and_counts_the_rest/)'` (expected: FAIL)
**Commit**: `test(analysis): guard the level order, the summary and the spend`

## Step 2: Order compared levels by their values
**Goal**: `orientation_ignores_cell_digests` passes; the four sign guards are re-pinned once.
**Implementation Logic**:
1. In `compare_pairs`, sort the levels before pairs are formed by a natural order of the values: scalars by value, a treatment by `(name, tag)` text then commit, any other value by its canonical JSON text. No option; the declared order stays out because `analyze` can run without the experiment file.
2. Run the four guards of R16 §Guards that change; where one flips because the other level is now A, re-pin it to the new order and record in the report which level became A and why. Any other test that changes stops the implementer (build README, Behaviour changes).
**Deliverables**: `crates/candler-core/src/analysis/report.rs`: `compare_pairs` and the order function it calls; the four re-pinned guards
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/orientation_ignores_cell_digests/) | test(/^guard_analysis_report::/) | test(/^guard_known_truth::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_golden_design::/) | test(/^guard_analyze::/)'` (expected: PASS)
**Commit**: `fix(analysis): order compared levels by their values, not cell digests`

## Step 3: Derive the decision summary and the spend in core
**Goal**: The two remaining guards pass; both are core operations over data the core already holds.
**Implementation Logic**:
1. `analysis/summary.rs`: `Summary { decisions, omitted }` built from `Report`: one `Decision` per tested entry (labels A and B, B minus A, the interval, `p_holm`, `reject`, the family size) and one omitted entry per untested one with its reason. `Report` holds it. No job state, no new statistic.
2. `analysis/spend.rs`: `Spend { cost_usd, runs_counted, runs_without_total }` summed over the sealed runs' recorded `run.cost_usd`; an unsealed run is not counted. Tokens are not summed (a Future row).
3. Neither struct is `Serialize` here: the wire record is `analysis_record`'s (D1).
**Deliverables**: `crates/candler-core/src/analysis/summary.rs`: `Summary`, `Decision`, `Omitted`; `crates/candler-core/src/analysis/spend.rs`: `Spend`, `spend_of`; `crates/candler-core/src/analysis/report.rs`: `Report.summary`, arm spend; `crates/candler-core/src/analysis/mod.rs`: the modules
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/summary_agrees_with_entries|spend_sums_recorded_costs_and_counts_the_rest/) | test(/^guard_analysis_report::/)'` (expected: PASS)
**Commit**: `feat(analysis): derive a decision summary and a spend from the report`

## Step 4: Print the summary and the spend
**Goal**: `analyze` prints `summary` and each arm's `spent`; `inspect <job>` prints the job's `spent`; no existing key changes.
**Implementation Logic**:
1. `analyze.rs`: add the key `summary` beside today's keys, and `spent` inside `arm_json`; the CSV output is not touched.
2. `inspect.rs`: `render_job` adds `spent` for the job.
3. If an existing `guard_analyze` or `guard_inspect` oracle compares whole JSON objects and the new key breaks it, stop and ask the coordinator before changing it.
**Deliverables**: `crates/candler/src/commands/analyze.rs`: `summary` and `arm_json`'s `spent`; `crates/candler/src/commands/inspect.rs`: `render_job`'s `spent`
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_analyze::/) | test(/^guard_inspect::/) | test(/^guard_golden_design::/)'` (expected: PASS)
**Commit**: `feat(cli): print the summary and the spend in analyze and inspect`

## Step 5: Bites, the report and the verifier
**Goal**: The guards are shown to bite, and the leaf is verified.
**Implementation Logic**:
1. Bites, each made, seen red and reverted: sort the levels by cell digest again (red on `orientation_ignores_cell_digests`); drop one omitted entry or flip one decision's `reject` (red on `summary_agrees_with_entries`); count an unsealed run or treat a missing cost as 0 and count it (red on `spend_sums_recorded_costs_and_counts_the_rest`).
2. Write the report (build README, Every leaf): the re-pinned guards with their reasons, the Bites, D3's four answers for each new guard, follow-ups.
3. Dispatch a fresh Sonnet verifier (model passed explicitly) with this brief verbatim, adding only the branch name. **Verifier brief:** "You verify the leaf `__roadmap__/m6b-surface-consolidation/consolidated/build/analysis_core.md` on branch <branch>. Read the build README (Gotchas), R16 (`__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md`) rows F2, C6, C8, §D3 and §Guards that change, then the diff from the leaf's branch point and the code through colgrep. Assume something is wrong. Attack: A and B still depending on anything but the levels (a hash, an insertion order, a map iteration); a re-pinned guard beyond the four R16 names, or a re-pin that hides a changed sign of the effect rather than a renamed level; a decision list that can disagree with the entries, or that computes a statistic the entries do not hold; a spend that hides a missing cost, counts an unsealed run or reads anything but recorded values; an existing `analyze` or `inspect` key renamed, removed or reshaped; an oracle read from the code under test; a guard without a `Guards:` line or a bite; a comment that carries history or ids; a moved digest. Report each finding with the step, `path:line` and the evidence. Change nothing."
4. Fix the findings; have the same verifier re-check through SendMessage; record each finding and its disposition in the report.
**Deliverables**: `__reports__/m6b-surface-consolidation/build/analysis_core_v0.md`: sections `## Guards changed`, `## Bites`, `## D3`, `## Verifier`, `## Follow-ups`
**Consistency Checks**: `for s in 'Guards changed' Bites D3 Verifier Follow-ups; do grep -q "^## $s" __reports__/m6b-surface-consolidation/build/analysis_core_v0.md || exit 1; done` (expected: PASS)
**Commit**: `reports(api): record the analysis core build`
