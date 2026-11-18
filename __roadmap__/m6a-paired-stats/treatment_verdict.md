# Treatment verdict

**Goal**: `stratum_verdict` explains a difference in env lock, image or build inputs between two arms only when a declared `treatment` factor differs between them (T5), mirroring how `tools_unexplained` treats tools; without such a factor the difference is still refused as unexplained.
**Estimate**: about 50 Sonnet calls, $2.1; no verifier
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (README, Gotchas)
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/manifest/compare\.rs$|^crates/candler-core/tests/it/guard_comparability\.rs$|^crates/candler-core/tests/fixtures/comparability/verdicts\.json$|^__reports__/m6a-paired-stats/leaves/treatment_verdict_v0\.md$`; `verdicts.json` is append-only
**Success Gates**:
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_comparability::/) | test(/^guard_analysis_report::a2_/)'`
- ⬜ [cmd] Existing `verdicts.json` rows are byte-identical (additions only): `git diff --numstat $BASE_LEAF -- crates/candler-core/tests/fixtures/comparability/verdicts.json | awk '$2!=0{b=1}END{exit b}'`
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] The report records each Bite
**References**: [R03 T5](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md); `manifest/compare.rs`: `tools_unexplained`, `stratum_verdict`

## Step 1: Append the t5 rows, seen red
**Goal**: New verdict rows pin both sides of T5.
**Implementation Logic**:
1. Append to `verdicts.json`: two arms differing in env lock, image and build inputs with a declared `treatment` factor that differs (comparable, explained); the same with no `treatment` factor (refused, unexplained); a declared `treatment` factor that is equal in both arms while the lock differs (refused).
2. The test reading `verdicts.json` already iterates every row; add a `t5_` test only if a row needs a check the table does not express.
**Deliverables**: `crates/candler-core/tests/fixtures/comparability/verdicts.json`: three appended rows; `crates/candler-core/tests/it/guard_comparability.rs`: `t5_treatment_explains_lock_and_image` if needed
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_comparability::/)'` (expected: FAIL)
**Commit**: `test(analysis): guard treatment as an explanation of lock differences`

## Step 2: Explain lock differences by the treatment
**Goal**: The rows pass, and every existing row keeps its verdict.
**Implementation Logic**:
1. A helper `treatment_explains(one, other, declared) -> bool`, true when a declared factor path is `treatment` (or under it) and the arms' values differ, written beside `tools_unexplained` and in its shape.
2. `stratum_verdict` consults it for env lock, image and build inputs only.
3. Bites, recorded: explain on any declared factor (red on the no-treatment row); explain when the treatment is equal (red).
**Deliverables**: `crates/candler-core/src/manifest/compare.rs`: `treatment_explains`, `stratum_verdict`; `__reports__/m6a-paired-stats/leaves/treatment_verdict_v0.md`: section "Bites"
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_comparability::/) | test(/^guard_analysis_report::a2_/)'` (expected: PASS)
**Commit**: `feat(analysis): let a treatment factor explain lock and image changes`
