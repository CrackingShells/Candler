# Imported strata

**Goal**: A stratum of imported runs gets a verdict from what the import recorded (amendment A4, decision D7): two imported runs of one cell under the same import rules and the same legacy build compare, an imported run never shares a stratum with a live run, and live strata keep D8 unchanged.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] The implementer knows that the M2c handover's "imported manifests record neither" predates A2: the legacy `provenance.eval_infra.*` and `provenance.sandbox_contract` are in `comparable.values` (R04)
- [ ] [nl] OWNED: `crates/candler-core/src/manifest/compare.rs`, `crates/candler-core/tests/guard_comparability.rs`, `crates/candler-core/tests/fixtures/comparability/verdicts.json`, `crates/candler-core/tests/support/comparability.rs`
**Success Gates**:
- ⬜ [cmd] V1: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_comparability)'`, with one row per D7 class (prefix `imported_`):
  - two imported arms, same `import.rules`, legacy commit, dirty flag (`true`) and contract: comparable;
  - different `import.rules`: refused, naming `import.rules`;
  - `import.rules` absent in every arm: refused, naming `import.rules`;
  - a legacy commit or contract unrecorded in `comparable.values` in every arm: refused, naming the path;
  - different legacy commits: refused, naming `provenance.eval_infra.commit`;
  - one imported arm and one live arm: refused, naming `import`;
  - an imported arm whose import record has no `base_commit` and no legacy build values, as a raw-trace import has (M3 makes `ImportRecord.base_commit` and `legacy_integrity` optional in `schemas@0.7.0`): refused, naming the unrecorded legacy paths. `base_commit` itself gates nothing.
  Oracle: rows written from D7, not from the code
- ⬜ [cmd] Live strata are unchanged: no existing row is edited or removed: `! git diff a26fe53 -- crates/candler-core/tests/fixtures/comparability/verdicts.json | grep -q '^-[^-]'`
- ⬜ [cmd] V2: a real mini-corpus import gives `comparable` for a stratum of its arms that differ only in declared factors: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^imported_stratum_/)'`
- ⬜ [cmd] STD (`-p candler-core`), LINUX, DIGEST and SUBJ pass
- ⬜ [nl] The leaf report records V1's bites: the imported rule applied to live arms too (a live row turns red), dirty pooled across commits (the different-commit row turns red)
- ⬜ [nl] The `imported_strata` verifier closes with no open finding: it attacks whether live D8 is weakened in any path, whether a mixed stratum can pass, and whether pooling admits anything beyond an equal commit, flag, contract and rules
**References**: [R04 Gap analysis A4](../../__reports__/m2c-wire-alignment/gap_analysis_imported_strata_v0.md); [R09 D7](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-approved-plan.md); [R01 §Run manifest, §Golden corpus](../../__design__/architecture.md) — provenance gating, the import record

## Step 1: Write the imported rows
**Goal**: The table states D7 before the code does, and the new rows are red.
**Implementation Logic**:
Give the row shape an optional `import` (absent for a live arm; present with an optional `rules` digest for an imported one), so no existing row changes. Write the D7 rows of V1, legacy values set in `comparable.values` as the importer writes them. Run the guard: the imported rows fail, every C01–C64 row passes.
**Deliverables**: `crates/candler-core/tests/fixtures/comparability/verdicts.json`: the `imported_` rows; `crates/candler-core/tests/guard_comparability.rs`: the `import` column
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_comparability)'` (expected: FAIL)
**Commit**: `test(manifest): state what an imported stratum compares on`

## Step 2: Gate an imported stratum on its import
**Goal**: The verdict reads an imported arm's identity from its import record and its recorded legacy build.
**Implementation Logic**:
1. `ArmView` gains the import record's presence and its `rules` digest.
2. In `stratum_verdict`: a stratum mixing imported and live arms is refused, naming `import`. A stratum of imported arms skips the `measured[]` provenance gate and instead requires `import.rules` recorded and equal, and `provenance.eval_infra.commit`, `provenance.eval_infra.dirty` and `provenance.sandbox_contract` recorded in `comparable.values` (their equality stays the comparable diff's job). A live stratum goes through D8 exactly as before.
3. The doc of `stratum_verdict` states the imported rule and the assumption behind dirty pooling: one legacy launch is one checkout.
**Deliverables**: `crates/candler-core/src/manifest/compare.rs`: `ArmView` (import), `stratum_verdict`, `imported_refused`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_comparability)'` (expected: PASS)
**Commit**: `feat(manifest): give imported strata a verdict on their recorded build`

## Step 3: Prove it on a real import
**Goal**: The mini corpus, imported by the real importer, yields a comparable stratum.
**Implementation Logic**:
Import the mini corpus through the test support (`support/legacy_import.rs`), build the arms of one stratum (its runs differing only in a factor the test declares), and assert `Comparable`; then flip one arm's import rules and assert the refusal names `import.rules`. If the corpus holds no such pair, say so in the report and build the second arm by importing the same cell twice under the same rules.
**Deliverables**: `crates/candler-core/tests/guard_comparability.rs`: `imported_stratum_of_the_mini_corpus_is_comparable`, `imported_stratum_with_other_rules_is_refused`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^imported_stratum_/)'` (expected: PASS)
**Commit**: `test(manifest): compare a real imported stratum`
