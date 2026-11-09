# Leaf report: m2a golden_parity (L3)

Branch `m2a/golden_parity` in `/Users/hacker/Documents/tmp/claude-worktrees/Candler/m2a-golden_parity`, cut from `feat/m2a-evaluation-parity` (69997c7). Nothing pushed.

## Commits
- 9d691ce fix(evaluation): read a suite file written as json
- 3147c13 fix(evaluation): run a uv plug-in inside its environment
- ca030eb test(evaluation): prove golden parity against the reference oracle
- the report commit `reports(evaluation): record the golden parity run`

## Gates
- STD: PASS. fmt clean; clippy `--workspace --all-targets -D warnings` clean; `cargo nextest run --workspace`: "298 tests run: 298 passed, 7 skipped" (the 7 are the three ignored guards and four other ignored tests); `cargo doc` with `-D warnings` generated.
- SPAWN (before every spawning run, core src committed and clean): `cargo clean -p candler-core` then "4 tests run: 4 passed" for `guard_signal`.
- DRIFT: PASS (`schema_drift` 2 passed, schema tree clean, no diff against origin/main for the two named schemas).
- Linux clippy (`--target x86_64-unknown-linux-gnu --workspace --tests -D warnings`): PASS.
- SUBJ: PASS ("No errored commits", subjects at most 72 characters).
- OWNED with the widened regex (`^crates/(candler/tests/guard_golden_parity\.rs$|candler-core/(src/evaluation/(engine|registry)/|tests/(guard_engine\.rs$|fixtures/engine/))|candler-schemas/src/evaluation\.rs$)`): PASS for the code commits; the only other file is this report.
- SCAFFOLD: PASS (no `mod scaffold`; 3 `#[test]` = 3 `Guards:` lines).
- Leaf gates: `uv sync --frozen` in `$RW/$PLUGIN` succeeds ("Audited 53 packages"); tag `htn-creator-eval@0.5.0^{commit}` resolves to e96257d92d1d39bcbd0a4cf6b65ea0f0091e49c3; the plug-in tree in `$RW` equals the tagged tree (`tree-equal`, checked before the run); `$RW` has no change.
- Guard run, first full pass: PASS. Wall-clock 1471 s (nextest "3 tests run: 3 passed (3 slow)", 1463.455 s). Log `$TMPDIR/m2a-gp/run1.log`; last lines:
  `PASS [1461.814s] guard_golden_parity the_version_diff_compares_everything_it_can_and_excuses_only_the_named_rows`, `PASS [1462.864s] ... the_second_regrade_sends_new_requests_and_changes_nothing`, `PASS [1463.455s] ... the_live_ac_rows_and_rates_equal_the_reference`, `Summary [1463.455s] 3 tests run: 3 passed (3 slow), 0 skipped`. Fixture timings: import 135.7 s, regrade 1 642.5 s, regrade 2 680.8 s (concurrency 4). Disk free 186 GB before, 183 GB after.
- Unset variables: each fails all three tests in 0.03 s. `CANDLER_GOLDEN_CORPUS is not set: the golden parity guard needs it`; `CANDLER_HTN_PLUGIN is not set: the golden parity guard needs it`.
- README gates against the kept store: the `candler diff ... | jq -e` gate exits 0 (`true`; compared 8880, changed 0, excused 209, equivalence sha256 equal to `shasum -a 256` of `equivalence.json`, compared + uncompared = 10320). `diff <(candler show --rates --evaluator htn-creator-eval@0.5.0 --format toml) $ORACLE/rates_by_cell.toml` is empty.

## Guards
- GP1 `the_live_ac_rows_and_rates_equal_the_reference` (R1). Oracle: `counts.toml` `[live_expected]` (7790 pass, 209 not_applicable, 415 fail, 466 blocked), `na_pairs.txt`, `rates_by_cell.toml`, all from the raw corpus by `gen_oracle.py`; the imported side is the store's own 0.4.2 rows. Checks the 8880 live rows and `show` counts, every row on status, value, weight and cause (209 pairs: imported zero-weight pass with legacy cause, live `not_applicable` with `empty_population`), and the rates bytes. Bite: in a scratch copy of the manifest and `candler-plugin/` tree (no `.venv`) under `$TMPDIR/m2a-gp/plugin-bite/`, C6 `params.round` 2 changed to 1 (the only semantic change; the file was rewritten with `json.dump(indent=1)`). Full run, 1738 s: GP1 FAILED at `("5023b88c...", 1, 1, "C6") differs from the import`, left (imported 0.4.2) value 0.08, right (live) 0.1, which is 0.08 rounded to one place; GP2 FAILED (diff exit 1); GP3 passed, as both regrades were bitten alike. Scratch copy and store deleted; `$RW` never edited.
- GP2 `the_version_diff_compares_everything_it_can_and_excuses_only_the_named_rows` (R2, R1). Oracle: hand literal D-E1..D-E6 x 240, `na_pairs.txt` line count, `shasum` of `equivalence.json`, `counts.toml` total. Bites on the kept store, each restored from a backup: (a) one regrade-2 record, C3 of run 5023b88c-r1-a1, count 3 changed to 4: GP2 FAILED and GP3 FAILED ("changed between regrades"); GP1 passed because it reads regrade 1's files; (b) the live record of the first excused pair (C-X4, 5023b88c run 12) set to `not_measured {reason: instrument_error}`: `diff` prints `compared 8880, changed 1, excused 208`, one change of class `equivalence_mismatch` on C-X4; all three guards FAILED. After restore, all three pass again.
- GP3 `the_second_regrade_sends_new_requests_and_changes_nothing` (R5, R7, R6). Oracle: the pinned sha literal `git:28be2c15` and regrade 1's raw rows. Checks disjoint request ids, 8880 rows, per-key equality of the four fields, no I4 violation with each of 8880 live I4 keys held twice, every live record names an instrument with a pinned digest, no `deadline` cause over 17,760 records. Bite (throwaway commit on scratch branch `scratch/gp3-bite`, SPAWN, then deleted with `git branch -D`; core src on `m2a/golden_parity` is clean): in `crates/candler-core/src/evaluation/client/mod.rs`, the evaluate request id became a per-process counter (`01a1be8e-6658-7130-b3eb-<counter>`), so regrade 2 reuses regrade 1's ids. All three guards FAILED at the `evaluate` step ("8398 request(s) failed", exit 2), after 4234 s in all (each process rebuilds the fixture in turn). Regrade 2 never completed, so the disjointness assertion itself was not reached: the engine refuses reused ids first. Two earlier GP3 bites were invalid and are discarded: a hash of run, evaluator and stage in `run.rs` collided inside one regrade (418 failures in regrade 1); a hash of run and batch in `run.rs` changed only the work-directory id, and the records take their id from the client, so the guard passed (1478 s) and bit nothing.

## One-off records
- Stores: kept store `$TMPDIR/m2a-gp/kept` 356 MB (`store/` 276 MB). Each bite store (bite1 GP1, bite3 GP3 x3) was about 356 MB and is deleted.
- Plug-in `htn-creator-eval@0.5.0`, rules sha256 `5d25aa60db68a8340bad2856a8c9fb1e1f99d13dce161d6f08539afb22e56952`, asserted on every run manifest by the guard.
- Plug-in tree in `$RW` equals the tagged tree (commit e96257d92d1d39bcbd0a4cf6b65ea0f0091e49c3).

## Per-cell rates (live equals published, byte for byte)
| cell | model | skill | gate | gate % | quality | quality % |
|---|---|---|---|---|---|---|
| `bb140e82` | claude-haiku-4-5 | 0.2.3 | 611/696 | 87.8 | 442/460 | 96.1 |
| `b6220c4b` | claude-haiku-4-5 | 0.2.4 | 688/771 | 89.2 | 423/437 | 96.8 |
| `574932f0` | claude-haiku-4-5 | 0.2.5 | 680/764 | 89.0 | 435/456 | 95.4 |
| `9b080341` | claude-haiku-4-5 | 0.2.6 | 766/849 | 90.2 | 427/448 | 95.3 |
| `b2cc61e1` | claude-sonnet-4-6 | 0.2.3 | 907/1003 | 90.4 | 380/383 | 99.2 |
| `e3081cce` | claude-sonnet-4-6 | 0.2.4 | 867/931 | 93.1 | 400/401 | 99.8 |
| `d1899b5b` | claude-sonnet-4-6 | 0.2.5 | 905/991 | 91.3 | 411/414 | 99.3 |
| `aaa18a95` | claude-sonnet-4-6 | 0.2.6 | 904/928 | 97.4 | 372/373 | 99.7 |
| `e7b37dd6` | claude-sonnet-5 | 0.2.3 | 928/1022 | 90.8 | 448/455 | 98.5 |
| `8bafa1e2` | claude-sonnet-5 | 0.2.4 | 971/1070 | 90.7 | 432/440 | 98.2 |
| `5023b88c` | claude-sonnet-5 | 0.2.5 | 939/1023 | 91.8 | 482/487 | 99.0 |
| `6600d541` | claude-sonnet-5 | 0.2.6 | 962/1011 | 95.2 | 428/435 | 98.4 |

## Findings and deviations
1. SPEC DEFECT, fixed in Candler (9d691ce): `candler evaluate` read every suite file as TOML, but the shipped plug-in suite is `suite/suite.json`. Added `SuiteFile::from_json_str` and a reader choice by extension; ENG1 row with a JSON fixture.
2. SPEC DEFECT, fixed in Candler (3147c13): the plug-in's `command` is a bare `python -m htn_creator_candler`, which resolved to a different interpreter than the synced environment. Spawns now run with `VIRTUAL_ENV`, `<env>/.venv/bin` first on `PATH` and `UV_PROJECT_ENVIRONMENT`; ENG1 row with a bare-python fixture. The HTN repo and tag were not touched.
3. Nextest runs each test in its own process, so the three guards share one fixture through a file lock and a completion marker (binary, rules, manifest, suite and corpus hashes). A failed build leaves no marker, so each of the three processes rebuilds in turn; a bite that breaks the build costs three builds. Editing the kept store and rerunning is the supported way to bite GP2.
4. The GP3 disjointness assertion has not been seen failing by itself (finding above): reused ids are caught earlier by the engine.
5. GP1 reads regrade 1's files and `show`, so a change only in regrade 2's values (bite GP2a) is caught by GP2 and GP3, not GP1.
6. For the KT: the 12 cells gate between 87.8 % (haiku-4-5 0.2.3) and 97.4 % (sonnet-4-6 0.2.6).
