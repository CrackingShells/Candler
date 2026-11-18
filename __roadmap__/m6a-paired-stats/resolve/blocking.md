# Blocking

**Goal**: For a comparison of two levels of one declared factor, Candler builds the blocks from the level combinations of the other declared factors (C5), chooses the test the battery fixes for the data (C1, before any outcome is seen), runs it, and groups Holm families per comparison and role (C7).
**Estimate**: about 80 Sonnet calls, $3.3, plus a short verifier (about 30 calls, $1.2)
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (campaign README, Gotchas); L0 is merged (`resolve/README.md` pre-conditions)
- [ ] [nl] The golden probe's answer is read from the campaign Progress table: if eval tasks are cells, the task is a declared factor of the golden store and a block like any other
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/analysis/blocking\.rs$|^crates/candler-core/src/analysis/report\.rs$|^crates/candler-core/tests/it/guard_stats_blocking\.rs$|^__reports__/m6a-paired-stats/leaves/blocking_v0\.md$`. In `report.rs`, only additive fields on `Tested` and `Comparison` (no change to `compare_pairs`, `entries_of` or `adjust`, which are `analyze_output`'s)
**Success Gates**:
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_blocking::/)'`
- ⬜ [cmd] Rows show: blocks exclude the compared factor; 1 block gives the mid-p; ≥2 blocks give CMH; CMH falls back to the exact conditional test when Mantel–Fleiss fails; a weighted expectation gives the permutation test and BCa; Holm runs per comparison and per role
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] The verifier closed with no open finding; the report records each Bite
**References**: [R03 §Comparisons, §The battery, §Multiplicity](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md)

## Step 1: Write the rows, seen red
**Goal**: Rows that pin block selection, test choice and the Holm family.
**Implementation Logic**:
1. `blocks_exclude_the_compared_factor` (a 2×2 store: comparing treatment gives model blocks, comparing model gives treatment blocks); `one_block_uses_midp`; `many_blocks_use_cmh`; `cmh_falls_back_to_exact_when_mantel_fleiss_fails`; `weighted_uses_permutation_and_bca`; `holm_family_is_per_comparison_and_role` (gate and quality expectations of one comparison are adjusted apart, and two comparisons never share a family).
2. Expected values come from the L0 engines' own vectors or from a hand-built table, never from `blocking.rs`.
**Deliverables**: `crates/candler-core/tests/it/guard_stats_blocking.rs`: the six rows
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_blocking::/)'` (expected: FAIL)
**Commit**: `test(analysis): guard blocks and test choice`

## Step 2: Build blocks and choose the test
**Goal**: The rows pass.
**Implementation Logic**:
1. `pub fn blocks_of(runs: &[RunView], compared: &FieldPath, a: &Value, b: &Value, declared: &[FieldPath], expectation: &ExpectationId) -> Vec<Stratum>` (binary) and `unit_blocks_of(..) -> Vec<UnitBlock>` (weighted), keyed by the other declared factors' values, in a deterministic order.
2. `pub enum Battery { MidP, Cmh, ExactStrata, Permutation }` and `pub fn choose(kind: OutcomeKind, blocks: usize) -> Battery`, a function of the design only; `test_blocked` runs it, and when CMH's `mantel_fleiss_holds` is false it reports the exact conditional mid-p and records that the fallback ran.
3. `pub fn holm_families(tested: &mut [Tested])` groups by (comparison, role) and applies the existing `holm` adjustment.
4. Additive fields on `Tested` (`battery`, `blocks`, `fallback`, `per_block: Vec<BlockEffect>`) and `Comparison` (`factor`, `levels`), defaulted so today's code still builds.
**Deliverables**: `crates/candler-core/src/analysis/blocking.rs`: `blocks_of`, `unit_blocks_of`, `Battery`, `choose`, `test_blocked`, `holm_families`, `BlockEffect`; `crates/candler-core/src/analysis/report.rs`: the additive fields of `Tested` and `Comparison`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_blocking::/) | test(/^guard_stats::/)'` (expected: PASS)
**Commit**: `feat(analysis): build blocks and choose each comparison's test`

## Step 3: Bites and verifier
**Goal**: The rows bite, and a fresh verifier finds nothing open.
**Implementation Logic**:
1. Bites: include the compared factor in the block key (red); choose the test from the p-values (a forking path: red on a row where it changes the choice); one Holm family for gate and quality (red). Record each.
2. Dispatch the verifier (Sonnet, fresh agent, about 30 calls) with the brief below verbatim plus the branch name; fix, re-check through SendMessage, report once.
3. **Verifier brief:** "You verify the leaf `__roadmap__/m6a-paired-stats/resolve/blocking.md` on branch <branch>. Read the leaf, the campaign README's statistical design, `analysis/blocking.rs`, the `report.rs` diff and `guard_stats_blocking.rs`. Do not read the implementer's report until you have your own findings. Attack: block keys that include the compared factor or miss a declared one; a test choice that depends on the outcomes; the Mantel–Fleiss fallback path; the Holm family grouping; a nondeterministic block order; a row whose expected value came from the code under test. Report each finding with file, line and a failing input. Change nothing."
**Deliverables**: `__reports__/m6a-paired-stats/leaves/blocking_v0.md`: sections "Probe answer", "Bites", "Verifier"
**Consistency Checks**: `grep -q '^## Bites' __reports__/m6a-paired-stats/leaves/blocking_v0.md && grep -q '^## Verifier' __reports__/m6a-paired-stats/leaves/blocking_v0.md` (expected: PASS)
**Commit**: `docs(analysis): record the bites and the verifier of blocking`
