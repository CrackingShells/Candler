# Analyze output

**Goal**: `analyze` reports one comparison per (declared factor, pair of its levels), blocked by the other declared factors, each with its test, effect, interval, each block's risk difference, the reading ("effect" or "inconclusive") and Holm per comparison and role (C6, C7); a one-factor store gives today's output except the mid-p p-value.
**Estimate**: about 110 Sonnet calls, $4.5, plus its verifier (about 40 calls, $1.6)
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (campaign README, Gotchas); L0 and `blocking` are merged: `for l in exact_tests cmh resampling arm_counts blocking; do git log --format=%s | grep -q "^merge(.*): land $l" || exit 1; done`
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/analysis/(report|mod|fisher)\.rs$|^crates/candler/src/commands/analyze\.rs$|^crates/candler-core/tests/it/guard_(analysis_report|stats)\.rs$|^crates/candler/tests/it/guard_analyze\.rs$|^crates/candler-core/tests/oracle/vectors/analyze_c6_.*$|^__reports__/m6a-paired-stats/leaves/analyze_output_v0\.md$`. In `report.rs`: `compare_pairs`, `entries_of`, `adjust`, `Pairs`; in `fisher.rs`: nothing but removing the now-unused call path if clippy asks (`fisher_two_sided` and `fisher.toml` stay, read by the `t1_` oracles)
**Success Gates**:
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_analysis_report::/) | test(/^guard_stats::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_analyze/)'`
- ⬜ [cmd] `vectors/` gains only new files: `git diff --numstat $BASE_LEAF -- crates/candler-core/tests/oracle/vectors | awk '$2!=0{b=1}END{exit b}'`
- ⬜ [cmd] A one-factor store equals today's output except the p-value, which is the mid-p: `cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_analyze::a1_/) | test(/^guard_analyze::c6_one_factor_/)'`
- ⬜ [cmd] DIGEST passes; ownership check prints nothing
- ⬜ [nl] Every changed existing guard (the `a2_` 2×2 rows by design, and any other) is listed in the report with its reason and was ruled on by the coordinator before merge
- ⬜ [nl] The verifier closed with no open finding; the report records each Bite
**References**: [R03 §Comparisons, §Readings](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md); `analysis/report.rs`: `compare_pairs`, `entries_of`

## Step 1: Write the c6_ rows, seen red
**Goal**: Rows that pin the comparisons rule end to end.
**Implementation Logic**:
1. In `guard_analysis_report.rs`: `c6_two_by_two_gives_two_blocked_comparisons` (treatment blocked by model, model blocked by treatment); `c6_reading_effect_or_inconclusive` (never "no difference"); `c6_holm_per_comparison_and_role`.
2. In `candler/tests/it/guard_analyze.rs`: `c6_one_factor_store_keeps_todays_output` (today's JSON with only the p-value replaced by the mid-p, the expected mid-p from `midp.toml`); `c6_json_and_csv_name_the_test_and_blocks`.
3. New oracle files go under `crates/candler-core/tests/oracle/vectors/analyze_c6_*`; existing files are not edited.
**Deliverables**: the five `c6_` rows above; `crates/candler-core/tests/oracle/vectors/analyze_c6_*.json`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_analysis_report::c6_/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_analyze::c6_/)'` (expected: FAIL)
**Commit**: `test(analysis): guard blocked comparisons in analyze`

## Step 2: Compare by factor, blocked
**Goal**: `compare_pairs` builds comparisons by the rule and runs each through `blocking`.
**Implementation Logic**:
1. `compare_pairs` enumerates (declared factor, level pair); for each, `entries_of` builds per expectation the blocks (`blocks_of` / `unit_blocks_of`) and calls `test_blocked`; `adjust` calls `holm_families`.
2. The call to `fisher_two_sided` is replaced by the battery; `fisher_two_sided` itself stays.
3. Each `Tested` carries the reading: "effect" when its interval excludes 0, else "inconclusive".
**Deliverables**: `crates/candler-core/src/analysis/report.rs`: `compare_pairs`, `entries_of`, `adjust`, `Pairs`, `Reading`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_analysis_report::c6_/) | test(/^guard_stats::/)'` (expected: PASS)
**Commit**: `feat(analysis): compare declared factors blocked by the others`

## Step 3: Report it in JSON and CSV
**Goal**: `analyze` prints the comparisons; the one-factor output is today's but for the p-value.
**Implementation Logic**:
1. JSON: each comparison gains `factor`, `levels`, `battery`, `fallback`, `blocks` (each block's key and risk difference) and `reading`; existing keys keep their names and meaning.
2. CSV: one row per comparison and expectation as today; new columns are appended after the existing ones, so a reader of today's columns is unaffected.
3. List every existing guard whose expected output changed (the `a2_` 2×2 rows: 4 pairs become 2 comparisons) with its reason; stop and ask the coordinator for a ruling before changing any of them.
**Deliverables**: `crates/candler/src/commands/analyze.rs`: `comparison_json`, `summary_csv`; `crates/candler-core/tests/it/guard_analysis_report.rs`: the ruled `a2_` changes
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_analysis_report::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_analyze/)'` (expected: PASS)
**Commit**: `feat(analysis): report blocked comparisons in analyze`

## Step 4: Bites and verifier
**Goal**: The rows bite, and a fresh verifier finds nothing open.
**Implementation Logic**:
1. Bites: keep one-factor pairs (red on `c6_two_by_two_`); Fisher's p instead of the mid-p (red on `c6_one_factor_`); one Holm family across comparisons (red); "no difference" for a spanning interval (red). Record each.
2. Dispatch the verifier (Sonnet, fresh agent) with the brief below verbatim plus the branch name; fix, re-check through SendMessage, report once.
3. **Verifier brief:** "You verify the leaf `__roadmap__/m6a-paired-stats/resolve/wired/analyze_output.md` on branch <branch>. Read the leaf, the campaign README's statistical design, the diff of `analysis/report.rs`, `commands/analyze.rs` and the test files, and the leaf's base with `git diff $BASE_LEAF`. Do not read the implementer's report until you have your own findings. Attack: an existing oracle or vector edited so a test goes green (every changed existing guard must be in the report with a reason); a comparison set that differs from the rule (one per declared factor and level pair, the others as blocks); per-run units that do not reach the weighted test; the Holm family; a one-factor store whose output changed beyond the p-value; a reading of 'no difference'. Report each finding with file, line and a failing input. Change nothing."
**Deliverables**: `__reports__/m6a-paired-stats/leaves/analyze_output_v0.md`: sections "Changed guards", "Bites", "Verifier"
**Consistency Checks**: `grep -q '^## Changed guards' __reports__/m6a-paired-stats/leaves/analyze_output_v0.md && grep -q '^## Verifier' __reports__/m6a-paired-stats/leaves/analyze_output_v0.md` (expected: PASS)
**Commit**: `docs(analysis): record the bites and the verifier of analyze output`
