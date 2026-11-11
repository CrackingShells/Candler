# Gap Analysis: candler-v1 - No verdict for imported strata

## Problem Statement
M2c's `716f079 feat(manifest)` gates a stratum on three provenance paths:
- `eval_infra.commit` must be recorded and equal in every arm;
- `eval_infra.dirty` must be recorded and false in every arm;
- `sandbox_contract` must be recorded and equal in every arm.

An unrecorded value never counts as equal (R01 §Run manifest; decision D8 of the M2c plan). Imported runs fail all three paths, so no stratum of imported runs gets a verdict. That covers the golden 240 and the mini corpus. Nothing in `src` calls `stratum_verdict` yet. M4 is the first leaf whose gate goes through it (the `init_tools` gate), so this is where the rule for imported runs has to be decided.

## Evidence
The evidence is taken from `feat/m2c-wire-alignment` at 456d8d4, before it lands.

| Fact | Where |
|:-----|:------|
| The verdict reads the gated paths from the arm's `provenance` view, which comes from `measured` | `crates/candler-core/src/manifest/compare.rs`, `provenance_refused` |
| The importer puts `experiment.json` `provenance.*` into `comparable.values`, and into `measured` only the `eval_metadata.json` values whose cell-level value is `null` | R01 §Golden corpus, the import rows for `comparable.values` and `measured` |
| After a fresh import of the mini corpus, `measured` provenance holds only `provenance.planner.sandbox.*` | The M2c session's check |
| The legacy build was dirty: every mini-corpus `experiment.json` records `eval_infra.dirty: true` (`commit: 0e84a75`, `sandbox_contract: v5`) | `crates/candler-core/tests/fixtures/mini-corpus/*/experiment.json` |
| No `src` caller of `stratum_verdict` exists; only `tests/guard_comparability.rs` calls it | `git grep stratum_verdict` |

So imported strata are refused for two independent reasons:
1. **Where the values live.** The commit and the contract are recorded, but in `comparable.values`, where the verdict does not read them.
2. **Their values.** Even if the verdict read them there, the dirty gate refuses a legacy build that recorded `dirty: true`.

## Root Cause
D8 was written for runs Candler executes, where Candler records its own build. The legacy import was designed in M1, before the verdict gated on provenance. Neither M2c's leaf nor M4's gates say what an imported stratum compares on.

## Impact Assessment
- **Scope.** This covers the verdict over imported runs only. Live runs keep D8 as it is.
- **Dependencies.**
  - M4's `init_tools` gate is the first caller.
  - M6's paired statistics need strata with a verdict.
  - Any re-analysis of the golden corpus through Candler needs the same.
- **Risk.** Left open, either every imported stratum stays refused, which makes the golden corpus unusable for comparisons, or the first caller adds an exemption ad hoc and weakens D8 for live runs too.

## Proposed Solution
M4 gains a gate and a Step 1 item:
- Two imported runs of one cell, imported under the same import rules, get a verdict.
- An imported run never shares a stratum verdict with a live run.
- R01 §Run manifest says which recorded values gate an imported stratum, and what a legacy `dirty: true` means there.

## Recommendations
1. Keep D8 for live runs unchanged.
2. Give an imported arm a recorded identity rather than an exemption. For example, gate it on `import.rules` together with the legacy `eval_infra.*` and `sandbox_contract` from `comparable.values`. Decide whether a dirty legacy build is refused, or accepted when every arm records the same dirty commit.
3. Test it with a planted case: an imported stratum and a live run of the same cell are refused together, and two imported runs of one cell get a verdict.
