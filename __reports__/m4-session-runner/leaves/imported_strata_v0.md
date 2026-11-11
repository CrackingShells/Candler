# Leaf report: imported_strata (findings, v0)

Branch `m4/imported_strata`, cut from `feat/m4-session-runner` at 914f192. Decision D7 is the oracle of every verdict row; live strata keep D8.

## 1. Commits
| sha | subject |
|:----|:--------|
| 6859461 | test(manifest): state what an imported stratum compares on |
| 4e70771 | feat(manifest): give imported strata a verdict on their recorded build |
| 28ae1a0 | test(manifest): compare a real imported stratum |

(The report commit follows these; the round-1 fix is folded into the first two by fixup.) A Grit review found three wording defects and one style point (an `unreachable!` instead of a silent drop); they were folded in by fixup and autosquash.

## 2. Gates
| gate | result | last relevant line |
|:-----|:-------|:-------------------|
| V1 `binary(guard_comparability)` | PASS | 4 tests run: 4 passed |
| Live rows unedited vs a26fe53 | PASS | no `-` line in the verdicts diff (25 rows added before C64, none changed) |
| V2 `test(/^imported_stratum_/)` | PASS | 2 tests run: 2 passed |
| SPAWN (`guard_signal`) | PASS | 9 tests run: 9 passed |
| STD, `-p candler-core` | PASS | fmt ok; clippy clean; nextest 185 passed, 14 skipped; doc clean |
| LINUX, `-p candler-core --tests` | PASS | Finished, no warning |
| DIGEST | PASS | numstat 0, `field_paths.toml` no removed line, `guard_cell_digest` 9/9, core digest binaries 39/39 |
| SUBJ over 914f192..HEAD | PASS | No errored commits; every subject at most 72 characters |
| OWNED | PASS | only `compare.rs`, `verdicts.json`, `guard_comparability.rs` |
| DRIFT, GOLDEN | not run | no schemas file and no import, resolve or run path touched |

## 3. Guards
All three are in `guard_comparability.rs`; the table is `verdicts.json`.

**`stratum_verdict_table`, rows `imported_*` (25).**
- Guards: I7 and R01 §Run manifest (D7): an imported stratum is gated on its import and recorded legacy build, a mixed stratum is refused, live rows follow D8.
- Oracle: D7, written as literal expectations before the code. One row per class: same build pools a dirty flag; a declared factor; measured provenance not read; other rules (two and three arms); rules absent in every or one arm; commit, flag or contract unrecorded; empty commit; commit, flag or contract different; mixed with live (either order); no legacy build values; and one live row with a dirty measured build that stays refused.
- Seen red first: 17 of the first 18 refused or misnamed on the old verdict, the live row green. The C01 to C64 rows stayed green.
- Bites, each with the failing assertion seen:
  - The imported rule applied to live arms too: C01 to C63 and C64 turned red ("expected comparable, got Refused([import.rules, ...])"), as did the live row, and the two mixed rows.
  - Dirty pooled across commits (the commit residual dropped for imported strata): `imported_commit_differs_refused`, `imported_commit_unrecorded_refused`, `imported_commit_empty_refused`, `imported_no_legacy_build_refused` turned red.

**`imported_stratum_of_the_mini_corpus_is_comparable` (V2).**
- Guards: I7 and D7: runs 6, 17 and 12 of the mini corpus, imported by the real importer under rules that carry a file digest, form a comparable stratum once the skill version is declared, though the legacy build is dirty. Asserts every run sealed first.
- Oracle: the two `experiment.json` files diffed by hand with `jq -S` (they differ in `plan.out_dir`, which goes to `import`, and in the skill version at three paths); both record commit `0e84a75`, `dirty: true`, contract `v5`.
- Bite: send imported strata through the measured provenance gate: left `Refused([commit, dirty, sandbox_contract])`, right `Comparable`.

**`imported_stratum_with_other_rules_is_refused`.**
- Guards: D7: an arm imported under other rules refuses the stratum at `import.rules`.
- Oracle: the two digests the test chooses, and D7's naming of `import.rules`.
- Bite: drop the `import.rules` check: left `Comparable`, right `Refused([import.rules])`.

## 4. Probe answers
None.

## 5. Findings
1. **Behaviour change: none.** D7 only accepts strata refused today. No stored digest moved (DIGEST), and no input that was accepted became refused: the live path is untouched, and an all-live stratum takes the same branch and function as before.
2. **Names and shapes: none differ.** `ArmView` gains `is_imported()` and `import_rules()`. Two constants, `import` and `import.rules`, name the refusal paths.
3. **Conservative reading, flag for a ruling.** D7 says the legacy commit is "recorded". The imported gate also treats an empty or non-string commit as unrecorded, as D8 does for live arms (row `imported_commit_empty_refused`). It only refuses more among strata refused today. Remove the row and the clause if D7 means presence only.
4. **A real import is refused unless its rules carry a digest.** The test support (`legacy_rules::rules()`) builds rules without `source_digest`, so `ImportRecord.rules` is `None`. Under D7 the stratum of such an import is refused at `import.rules`; the V2 guard builds its own rules with `source_digest(...)`. Any caller that imports from programmatic rules and later wants a verdict hits this. It follows from D7 and needs no change here.
5. **Raw-trace row.** The row for "no `base_commit`, no legacy build values" cannot omit `base_commit` while `ImportRecord.base_commit` is required (schemas 0.6.0): the reader refuses the manifest. The row carries an import record whose `comparable.values` hold no legacy build values, which is what the verdict reads; `base_commit` gates nothing. Revisit after M3's `schemas@0.7.0`.
6. **Dirty pooling and declared factors: defect found by the verifier, fixed in round 1.**
   - The first version left the equality of the legacy commit, dirty flag and contract to the comparable diff, which a declared factor (`provenance`, `provenance.eval_infra`, or the path itself) drops: arms of two legacy builds pooled.
   - Fix (the coordinator's ruling): `imported_refused` compares the three values itself, with the `leaf_key` comparison of the live gate, whatever is declared, and names the differing path. It only refuses strata that 914f192 already refused.
   - Rows added first and seen red (7 of 7): `imported_{commit,dirty,contract}_differs_under_declared_parent_refused` (`declared: ["provenance"]`), `imported_{commit,dirty,contract}_differs_under_declared_own_path_refused` (the path itself declared) and `imported_commit_differs_under_declared_middle_refused` (`provenance.eval_infra`); each expects `refused` naming that path. The table now holds 25 `imported_` rows.
   - Bite: drop the equality comparison from `imported_refused`: all 7 rows turn red (`rows that disagree: ... expected {"refused":[...]}, got Comparable`).
7. **`_doc` line of `verdicts.json` does not mention the `import` key.** Editing line 1 would show as a removed line against a26fe53 and fail the live-rows gate, so the guard's doc describes it instead.
8. **Not done here:** the `imported_strata` verifier (a coordinator step), R01's rule (owned by `design_record`).
9. **Footprint:** the target dir `/private/tmp/claude-501/m4-target-imported_strata` and scratch `/private/tmp/claude-501/m4-imported-scratch` are removed. No Docker, RiVault or download was used.
