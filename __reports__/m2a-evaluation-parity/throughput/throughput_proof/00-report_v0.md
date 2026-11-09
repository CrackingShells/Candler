# Throughput proof: golden parity against `htn-creator-eval@0.5.1` (v0)

Branch `m2a/throughput_proof`, cut from `feat/m2a-evaluation-parity` (2768ba3). Nothing pushed.

## Verdict

| gate | result |
|---|---|
| GP1, GP2, GP3 against 0.5.1 | PASS, 3 of 3 |
| README `candler diff ... \| jq -e` gate, `--to htn-creator-eval@0.5.1` | PASS (jq exit 0, prints `true`) |
| rates diff against `rates_by_cell.toml` | PASS (empty, exit 0) |
| Whole guard run at most 5 min | PASS, 166.3 s (175 s with the build) |
| Each regrade at most 89 s | PASS, 63.7 s and 55.0 s |
| Import at most 30 s | FAIL, 45.6 s |

Parity holds on all 8,880 compared rows. Only the import gate fails; it is a CPU bound of the debug binary that the guard runs (see "What bounds it").

## Timings

The guard's `timings.txt` (concurrency 8, `CONCURRENCY = "8"` in the guard), against the 0.5.0 run of the golden_parity report.

| step | before (0.5.0) | after (0.5.1) | speed-up | gate | |
|---|---|---|---|---|---|
| import, 240 runs, 10,320 records | 135.7 s | 45.6 s | 3.0x | at most 30 s | FAIL |
| regrade 1, 8,880 items | 642.5 s | 63.7 s | 10.1x | at most 89 s | PASS |
| regrade 2, 8,880 items | 680.8 s | 55.0 s | 12.4x | at most 89 s | PASS |
| guard, nextest summary | 1463.5 s | 166.3 s | 8.8x | at most 300 s | PASS |

Items per second: regrade 1 is 8,880 / 63.67 = 139.5; regrade 2 is 8,880 / 55.04 = 161.3. The gate is 100 items/s (89 s for 8,880 items). Before: 13.8 and 13.0 items/s.

Disk free on `/System/Volumes/Data`: 187 GB before the run, 185 GB with the kept store (302 MB) and a probe store (295 MB), 186 GB after both were deleted.

## Guard log

```
SLOW [>120.000s] ... (three tests)
PASS [ 165.895s] (1/3) candler::guard_golden_parity the_second_regrade_sends_new_requests_and_changes_nothing
PASS [ 165.918s] (2/3) candler::guard_golden_parity the_version_diff_compares_everything_it_can_and_excuses_only_the_named_rows
PASS [ 166.307s] (3/3) candler::guard_golden_parity the_live_ac_rows_and_rates_equal_the_reference
Summary [ 166.308s] 3 tests run: 3 passed (3 slow), 0 skipped
```

Command: SPAWN (`guard_signal`, "4 tests run: 4 passed"), then `CANDLER_GOLDEN_CORPUS="$GOLDEN" CANDLER_HTN_PLUGIN="$RW/$PLUGIN" CANDLER_KEEP_STORE=<dir> cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'binary(guard_golden_parity)'`. `$RW` was at 36188dea, its plug-in tree equal to the tag `htn-creator-eval@0.5.1`; `complete.json` names the rules sha256 `5d25aa60...6952`. The guard binary is the debug `candler` (`CARGO_BIN_EXE_candler`).

## CLI gates (kept store, debug `candler` of this worktree)

- `candler diff --from htn-creator-eval@0.4.2 --to htn-creator-eval@0.5.1 --equivalence crates/candler/tests/oracle/equivalence.json --format json | jq -e ...`: exit 0, `true`. The fields: compared 8880, changed 0, excused 209, unchanged 8671, uncompared total 1440 (compared + uncompared = 10320), `equivalence.sha256` 293b1cf0...a5ca equal to `shasum -a 256` of the file.
- `diff <(candler show --rates --evaluator htn-creator-eval@0.5.1 --format toml) crates/candler/tests/oracle/rates_by_cell.toml`: no output, exit 0.

## What bounds it now

The profile (§4) projected 185 to 250 items/s. The two regrades reach 139 and 161 items/s, so the projection was not met, but the 100 items/s gate is, by 1.4 to 1.6 times.

Probe, one more regrade on a fresh import with `/usr/bin/time -l`, concurrency 8, debug binary: 58.7 s wall, 290.1 s user plus 114.3 s system, which is 404 s of CPU for 8,880 items, or 45 ms of CPU per item (PERF12 on the 12-run subset gave about 82 ms per item with 1 s of fixed start-up). That is 6.9 cores busy on average. The laptop has 12 cores and ran at load average 7 to 12 from other work during the guard, so the regrade is bound by free cores, not by I/O or by the sync path: at 45 ms per item, 12 free cores would give about 265 items/s, and the observed rate is the share of cores the shared machine left. The system share (28 percent of CPU) is the item fork and the pipes.

Import probe, one process, same binary: 45.5 s wall, 40.6 s user, 3.5 s system, 384 MB resident. It is single-threaded and CPU-bound (4.4 ms per record) and runs in the unoptimised debug build that nextest builds. No leaf of the throughput campaign changed the import path, and 135.7 s to 45.6 s is the effect of the machine and of the batching of writes in the store sync change, not of a targeted import fix. A release build was not measured (a Candler release build costs several GB of disk); it is the first thing to try if the 30 s import gate matters, together with parallelising the per-run parse.

## Findings

1. FAIL, import gate: 45.6 s against 30 s. Bound: single-threaded CPU in a debug binary. Not weakened. Options for the coordinator: measure the release binary; add a throughput leaf for the import (parse the runs in parallel); or move the gate to the release binary.
2. The 0.5.1 guard text carries one stale phrase the leaf did not change (the leaf allows only the identity): the module doc still says the guards need "about half an hour". It now takes under 3 minutes.
3. Guard concurrency is 8 (the 0.5.0 report said 4). The `timings.txt` figures above are at 8.
4. The deferred adversarial verifier has not run; this report does not repeat golden_parity's bites.
