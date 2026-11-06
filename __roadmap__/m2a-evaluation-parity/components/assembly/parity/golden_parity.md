# Golden parity: the corpus regrade guard

**Goal**: One ignored guard imports `$GOLDEN` as `htn-creator-eval@0.4.2`, regrades it live twice with the HTN plug-in `htn-creator-eval@0.5.0`, and proves three things. The live rows and rates equal the reference oracle (GP1). The version diff compares 8,880 rows, excuses exactly the 209 rows the equivalence file names, changes none and lists exactly the D-E rows as uncompared (GP2). The second regrade sends new requests that change nothing, under the pinned instrument and with no `deadline` row (GP3).
**Pre-conditions**:
- [ ] [cmd] Every `assembly/` leaf is merged (the parity README's pre-conditions)
- [ ] [cmd] `cd "$RW/$PLUGIN" && uv sync --frozen` succeeds
- [ ] [cmd] `git -C "$R" rev-parse -q --verify 'refs/tags/htn-creator-eval@0.5.0^{commit}'`, cut through that repo's own release flow on Eliott's go; the gate never points at a branch
**Success Gates**:
- ⬜ [cmd] STD, SUBJ and SCAFFOLD pass, and OWNED passes with `^crates/candler/tests/guard_golden_parity\.rs$`
- ⬜ [cmd] SPAWN, then `CANDLER_GOLDEN_CORPUS="$GOLDEN" CANDLER_HTN_PLUGIN="$RW/$PLUGIN" cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'binary(guard_golden_parity)'` passes; the report holds its log and wall-clock
- ⬜ [cmd] The same command fails when either variable is unset
- ⬜ [cmd] From the guard's kept store (`CANDLER_KEEP_STORE=<dir>`): the README's `candler diff … | jq -e …` gate and `diff <(candler show --rates --evaluator htn-creator-eval@0.5.0 --format toml) $ORACLE/rates_by_cell.toml` pass
- ⬜ [nl] The adversarial verifier holds no open BLOCKER or MAJOR: every constant is read from an oracle file, every bite reproduces, the vacuity counts cannot be gamed, and no mismatch can hide behind the equivalence file
**References**: [R03 §Scope move, §Generic diff, §I4, §Truthful instrument](../../../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md); `crates/candler/tests/oracle/{counts.toml,rates_by_cell.toml,na_pairs.txt,equivalence.json}`; `crates/candler-core/src/evaluation/engine/` — `regrade`; [R04 GP1–GP3](../../../../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md)

## Step 1: Write the parity guard
**Goal**: GP1–GP3 run in one ignored binary that reads every expected number from the oracle files.
**Implementation Logic**:
1. `tests/guard_golden_parity.rs` (`#![cfg(test)]`, every test `#[ignore]`). It panics when `CANDLER_GOLDEN_CORPUS` or `CANDLER_HTN_PLUGIN` is unset. One shared fixture imports the corpus into a temp store (or `CANDLER_KEEP_STORE`), asserts 240 runs and 10,320 records first, then regrades twice through the binary (`candler evaluate`, so the CLI path is the one proven).
2. GP1 (R1): the live A–C current records equal `counts.toml`'s `[live_expected]` counts, and row by row on the four fields they equal the imported rows, with the 209 `na_pairs.txt` pairs as `not_applicable {empty_population}`. `candler show --rates --evaluator htn-creator-eval@0.5.0 --format toml` equals `rates_by_cell.toml` byte for byte. Oracle: the oracle files. Bite: give C6's `ratio` params `round = 1` instead of the source's 2 in a scratch copy of the suite (restored with `git checkout --`).
3. GP2 (R2, R1): `candler diff --from htn-creator-eval@0.4.2 --to htn-creator-eval@0.5.0 --equivalence equivalence.json --format json` gives `compared == 8880`, `changed == 0` and `unchanged + excused + changed == compared`. `excused` equals the line count of `na_pairs.txt`, and `equivalence.sha256` equals the sha256 of the committed `equivalence.json` bytes. `compared + Σ uncompared.count == 10320`. The uncompared expectations equal a hand-written literal `D-E1`…`D-E6`, each with count 240. Oracle: the hand literal, `na_pairs.txt` and the file's own bytes. Bites: change one stored value from 3 to 4 in the kept store; change one excused row's live record to `not_measured {reason: instrument_error}` in the kept store, which must show as one `equivalence_mismatch`, never as excused.
4. GP3 (R5, R7, R6): the second regrade's request ids are disjoint from the first's, and it has 8,880 A–C rows. `diff` between the two regrades is empty with no I4 violation. Every live record's instrument digest starts with `git:28be2c15`. Zero records carry `reason: deadline`. Oracle: the pinned sha literal. Bite: reuse the first regrade's request ids.
**Deliverables**: `crates/candler/tests/guard_golden_parity.rs` (GP1, GP2, GP3)
**Consistency Checks**: `CANDLER_GOLDEN_CORPUS="$GOLDEN" CANDLER_HTN_PLUGIN="$RW/$PLUGIN" cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'binary(guard_golden_parity)'` (expected: PASS)
**Commit**: `test(evaluation): prove golden parity against the reference oracle`

## Step 2: Record the run and hand it to the verifier
**Goal**: The leaf report holds the evidence the README gates read, and an adversarial verifier attacks the guard.
**Implementation Logic**:
1. The report under `__reports__/m2a-evaluation-parity/golden_parity/` holds the guard log with wall-clock, the unset-variable failures, the four bites (each tried, each failing), the README's CLI `jq` and rates gates run against the kept store, and the per-cell gate-rate table against the published rates for the KT.
2. Dispatch the adversarial verifier on the guard with the oracle files and R04. One named target: hide a mismatch behind `equivalence.json` (a row naming the wrong run, a nonexistent row, a reversed pair) and show `diff` still fails. The committed file's digest is checked by GP2, and the verifier also attacks the generator: a pair the raw corpus does not show as `pass` 0/0 must not survive into the file. Fixes go back to the implementer through `SendMessage`.
**Deliverables**: `__reports__/m2a-evaluation-parity/golden_parity/00-report_v0.md`
**Consistency Checks**: `test -s __reports__/m2a-evaluation-parity/golden_parity/00-report_v0.md && grep -c 'Guards:' crates/candler/tests/guard_golden_parity.rs | grep -qx 3` (expected: PASS)
**Commit**: `reports(evaluation): record the golden parity run`
