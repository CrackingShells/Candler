# Leaf report: m2a regrade_throughput

Branch `m2a/regrade_throughput`, cut from `feat/m2a-evaluation-parity` (d2b2acd). Nothing pushed.

## Commits
- 83b4221 perf(evaluation): send one request per run, evaluator and stage
- 1ad8bcb perf(run-store): sync with fsync and leave scratch copies unsynced
- 4def8c1 perf(evaluator-helper): start lazily and exit after the last frame
- 9cd1dbf fix(evaluator-helper): read the item pipe to eof after the child exits
- cef1299 test(evaluation): fail the corpus guard fast after a failed build
- the report commit `reports(evaluation): record the regrade throughput leaf`

## Gates
- STD: PASS. fmt clean; clippy `--workspace --all-targets -D warnings` clean; `cargo nextest run --workspace`: 299 run, 299 passed, 7 skipped; `cargo doc` with `-D warnings` clean. SPAWN ran first and passed (4/4 `guard_signal`).
- Linux clippy (`--target x86_64-unknown-linux-gnu --workspace --tests`): PASS.
- SUBJ: PASS (`cog check` and the 72-column subject check over `feat/m2a-evaluation-parity..HEAD`). Every commit passes `cargo check --all-targets`.
- OWNED: PASS, no changed path outside the regex. SCAFFOLD: PASS (guard_engine 4 tests, 4 `Guards:`; guard_golden_parity 3 and 3; no `mod scaffold`).
- PYSTD: PASS (7 passed, ruff check and format clean). HR1, HR2 and PYM1 are among the 7.
- Sync grep: `git grep -nE '\.sync_(all|data)\(' -- crates/candler-core/src` prints nothing.
- Fail-fast: with `CANDLER_HTN_PLUGIN` on a scratch plug-in whose suite is `{"schema":"suite/1","expectations":"not a list"}`, `binary(guard_golden_parity)` fails 3 of 3 in 45.7 s.
- PERF12: final c=4 wall 8.60 s, 444 records equal to the pre-leaf run, 13 spawns, import 1.05 s. See the caveat on c=8 below.

## PERF12
Corpus: the 12-run subset (444 records). Release binaries, `/usr/bin/time -l`, one run per cell, on a shared 12-core laptop. Items/s is 444 over wall. Spawns count `process.spawned` events. Commands per row: `perf12.sh <binary> <label> 1 4 8`, which is `candler --store S import corpus12 --rules R` and `candler --store S evaluate --plugin M --suite U --concurrency C`. Rows come from `rows.py`, keyed by (run, entry) with (status, value, weight, cause).

| state | c | wall s | user s | sys s | spawns | items/s | rows equal |
|---|---|---|---|---|---|---|---|
| before Step 1 | 1 | 111.98 | 77.07 | 16.73 | 421 | 4.0 | reference |
| before Step 1 | 4 | 33.58 | 90.88 | 19.62 | 421 | 13.2 | reference |
| before Step 1 | 8 | 23.53 | 112.92 | 24.97 | 421 | 18.9 | reference |
| after Step 1 | 1 | 32.50 | 24.42 | 5.13 | 13 | 13.7 | yes |
| after Step 1 | 4 | 11.72 | 30.49 | 6.75 | 13 | 37.9 | yes |
| after Step 1 | 8 | 9.73 | 35.93 | 8.02 | 13 | 45.6 | yes |
| after Step 2 | 1 | 27.28 | 22.42 | 4.41 | 13 | 16.3 | yes |
| after Step 2 | 4 | 8.38 | 24.30 | 5.16 | 13 | 53.0 | yes |
| after Step 2 | 8 | 6.35 | 28.69 | 6.19 | 13 | 69.9 | yes |
| Step 3 helper before (path install) | 1 | 29.19 | 22.88 | 4.64 | 13 | 15.2 | yes |
| Step 3 helper before (path install) | 4 | 10.84 | 29.79 | 6.52 | 13 | 41.0 | yes |
| Step 3 helper before (path install) | 8 | 8.15 | 35.55 | 8.08 | 13 | 54.5 | yes |
| after Step 3 | 1 | 28.27 | 22.95 | 4.83 | 13 | 15.7 | yes |
| after Step 3 | 4 | 8.63 | 24.78 | 5.52 | 13 | 51.5 | yes |
| after Step 3 | 8 | 6.48 | 29.34 | 6.54 | 13 | 68.5 | no, 1 row (before the fix) |
| after Step 4 (final HEAD) | 1 | 27.70 | 22.63 | 4.61 | 13 | 16.0 | yes |
| after Step 4 (final HEAD) | 4 | 8.60 | 24.81 | 5.47 | 13 | 51.6 | yes |
| after Step 4 (final HEAD) | 8 | 6.55 | 29.64 | 6.73 | 13 | 67.8 | no, 1 row (before the fix) |

Import of the 12 runs: 5.66 s before, 7.66 s after Step 1 (a slow cold run, unchanged code), 1.20 s after Step 2, 1.05 s final. Step 4 changes only a test file. With the real plug-in 0.5.0 (its helper is pinned from git, so Step 3 is not in it) the final binary gives c=4 9.58 s, 13 spawns, rows equal.

Step 3 rows use a scratch copy of the plug-in whose helper is a path install of this worktree. Its "before" row ran on a busier machine than the others. The direct measurement is `python -X importtime`-style wall of `import candler_evaluator.__main__` in the plug-in environment: 0.23 to 0.24 s before, 0.20 to 0.21 s after.

## Bites
- Step 1: `guard_engine` `independent_entries_of_one_evaluator_share_one_request`. Grouping by the head of the dependency component instead of `(stage, evaluator)` makes it fail (3 request ids against 2). Tried; file restored.
- Step 4: with the marker comparison made always false, the three tests fail in 133.8 s instead of 45.7 s. Tried; file restored.
- Step 2: no guard exists. The evidence is the empty sync grep and the c=4 wall 11.72 s to 8.38 s. Step 3: no bite; HR1, HR2 and PYM1 unchanged.

## Findings
1. c=8 row-equality flake, fixed in 9cd1dbf. Before the fix, 5 of 25 c=8 regrades with the Step 3 helper had one row turn not_measured / instrument_error ("item process exited without an outcome"), always a different row; 0 of 14 before Step 3. Cause: in `isolate.py` `_await`, when `select` timed out and `waitpid(WNOHANG)` then found the child done, the loop broke without reading the pipe the child had just filled. The loop now drains what the pipe holds before it decides. The cancel path returns `Abandoned` and never decides an outcome, so it has no such read; the child has no parent-side timeout (it arms its own alarm). Guard: `test_an_outcome_written_before_the_reap_is_not_lost` forces one empty `select` after the child has written and left (monkeypatch of `select.select` in the test, no hook in `src`); it fails with the early break restored and passes with the fix.
   Evidence after the fix: 25 regrades at c=8 on the path-install helper, selector `5023b88*` (740 records, 21 spawns), 25 of 25 equal to the pre-leaf reference (binary at d2b2acd, real plug-in, c=4), 0 rows differing. c=8 wall 9.76 to 11.50 s, median 10.2 s (about 72 items/s); c=4, three runs, 13.76 to 14.95 s, all equal.
2. Step 3 buys little: about 30 ms of import per helper process and about 20 ms of teardown, 13 processes per regrade. It does not reach the real plug-in until the helper pin moves (helper_release).
3. 100 items/s on the 240-run corpus: the 12-run subset reaches 68 to 70 items/s at c=8, with about 82 ms of CPU per item and 1 s of fixed start-up. Scaling from c=1 to c=8 is 4.2 times. 100 items/s needs about 10 cores free of other work at the current per-item cost, so it is unreachable on a shared laptop at c=8. The item fork and the kind's own Python work are what remains.
4. The scratch plug-in needed the helper's editable path to be relative through a symlink; an absolute path in `uv.lock` made `uv sync --frozen` fail from `/var` paths. This is a scratch-only matter.
