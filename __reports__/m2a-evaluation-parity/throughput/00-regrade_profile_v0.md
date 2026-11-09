# Regrade profile v0: where the time goes in `candler evaluate`

Candler at `5f6e615` (branch `m2a/golden_parity`), plug-in `htn-creator-eval@0.5.0`. Observation only. Nothing was committed or edited in `$W`, `$RW` or the main checkouts. Instrumentation lived in a detached worktree, now removed. Its diff is `$TMPDIR/m2a-regrade-profile.instrumentation.diff` and the Python trace wrapper is `$TMPDIR/m2a-regrade-profile.trace-wrapper.py`. The scratch stores, the corpus copy and the plug-in environment were deleted. `df` before and after: 185 GiB free.

## Verdict

The Rust core is not slow. Every request is its own Python process, and a request carries about one item.

- `plan.rs` splits the suite by dependency chain. Of 37 items per run, 36 become separate requests (35 of them one item each, plus one chain of 3).
- Each request pays about 135 ms of interpreter and import time (21 ms boot, 80 ms `candler_evaluator`, 34 ms the plug-in) to run 1 item.
- Inside each fork, the checker re-imports `libcst` (116 ms) or `arango` + `jsonschema` (220 ms). That is another ~40 ms per item on average.
- On top of that, about 36 ms per request goes to macOS `F_FULLFSYNC`.
- The release binary is no faster than the debug one. `candler` itself uses under 0.2 s of CPU per 420 requests.
- Import's 136 s is almost entirely fsync wait. The same import takes 5.85 s with fsync, 0.82 s without.

## 1. Measurements

Fixture: 12 of the 240 runs, one run-1 per cell across the 12 cells (31 MB copy of the corpus, 516 records). Machine: 12 cores, shared, other work running. Store root, `XDG_STATE_HOME` and `XDG_CONFIG_HOME` were all under `$TMPDIR/m2a-profile/`. The subset `S` is 12 runs, 444 items (384 pass, 24 fail, 28 blocked, 8 n/a).

`candler` below is `$W/target/debug/candler` (debug) or the release build of the same commit. `suite` is the plug-in's `suite/suite.json`. `<store>` is a fresh `cp -R` of an imported store.

Base command, wrapped in `/usr/bin/time -l` for every row:

```
candler --store <store> evaluate --plugin <manifest> --suite <suite> --concurrency N
```

| Metric | Value | Command |
|---|---|---|
| Baseline wall, release, c=1 | 103.55 s (74.1 user + 15.5 sys = 89.6 CPU-s, 0.87 cores) | base command, N=1 |
| Baseline wall, release, c=4 | 31.19 s (85.6 + 17.9 = 103.4 CPU-s, 3.3 cores) | N=4 |
| Baseline wall, release, c=8 | 22.45 s (107.8 + 23.9 = 131.7 CPU-s, 5.9 cores) | N=8 |
| Baseline wall, debug, c=1 / 4 / 8 | 103.73 / 31.34 / 24.32 s | same, debug binary |
| Debug vs release | no difference at c=1 (CPU 89.69 vs 89.62 s), so `candler` is under 0.2 s CPU of the total | |
| Throughput at the c=4 baseline | 444 items / 31.2 s = 14 items/s. Extrapolated to 240 runs: 620 s, against the observed ~650 s. | |
| Scaling | speedup 3.3x at c=4, 4.6x at c=8. CPU-seconds grow with c (90, 103, 132). | |
| Items / requests / plug-in processes | 444 items; 420 evaluate requests plus 1 describe = 421 spawns; 408 requests have 1 item, 12 have 3, 12 hold the engine's n/a rows | `grep -c process.spawned` on stderr; outcome file record counts |
| Python interpreters started | 421 (parent) plus 444 forked item children, about 865 | `fork_call` marks in the trace wrapper |
| `uv` invocations | 1 (`uv sync --frozen`, 1.0 s, first call only). 0 on every later evaluate; the `ready` marker short-circuits it. | event `process.spawned` with marker `environment-…` |
| Describe-only fixed start | 0.17 s (`--runs 'zzz*'`) | |
| Files and bytes per request | 3 artifacts, 44,294 B, copied, fsynced, chmodded and re-hashed. 420 requests: 1,260 files, 18.6 MB. | `PHASE` eprintln in `send` |
| Outcome files, index appends | 432 files (36 per run). One fsynced index line each, under `flock`. | |
| Peak RSS | 62 MB (largest process) | `time -l` |
| Import, debug, 12 runs | 7.21 s real, 2.49 user + 0.29 sys | `candler --store D import corpus12 --rules htn-results-rules.toml` |
| Import, release, 12 runs | 5.58 to 5.86 s real, 0.28 to 0.40 user + 0.31 to 0.64 sys | same, release |
| Import, release, no fsync | 0.82 s real (0.25 user, 0.20 sys) | same, scratch build with `PROF_NOSYNC=1` |
| `F_FULLFSYNC` unit cost | 4.15 ms per small file (`fsync(2)` and no sync: 0.06 ms) | python `fcntl.F_FULLFSYNC` loop, 100 files |
| Python boot | bare `python -c pass`: 10 to 22 ms. `import htn_creator_candler.__main__`: 120 to 157 ms. `import pydantic` alone: 52 ms. | `python -X importtime`, `time` |

Experiments, each in a scratch build (see section 3 for what each one changes). Counts were identical in all: 28 blocked, 24 fail, 8 n/a, 384 pass.

| Variant (S, release) | c=1 | c=4 | c=8 |
|---|---|---|---|
| Baseline | 103.6 s | 30.9 s | 20.5 s |
| No fsync | | 27.0 s | 17.0 s |
| Batch per (evaluator, stage) | 18.6 s | 8.7 s | |
| Batch, no fsync | | 8.5 s | |
| Batch, preload checkers in the parent | | 4.3 s | |
| Batch, no fsync, preload | 12.8 s | 4.3 s | 3.3 s |

The c=8 baseline here is 20.5 s against 22.45 s above; the run-to-run spread on this shared machine is about 10%.

## 2. Where the time goes, one regrade

Per request at c=1 (mean 243.6 ms, 420 requests, 102.5 s wall). Timestamps come from a Python wrapper (`wrap.py`) as the plug-in command, plus the `process.spawned` and `stop_step` events. The Rust side comes from `PHASE` eprintln timings in a scratch build.

| Phase | ms | % of wall | Where |
|---|---|---|---|
| Rust: `assemble` (tree read, glob) | 0.3 | 0.1 | `context/assemble.rs` |
| Rust: `materialise` (3 files copy + fsync + chmod + rehash) | 13.4 | 5.5 | `context/materialise.rs:134` |
| Rust: spawn, `write_record` (2 fsyncs; overlaps boot) | 8.7 | (hidden) | `supervise/record.rs:29,32` |
| Rust: outcome `commit` (file fsync, dir fsync, index `sync_data` under `flock`) | 13.6 | 5.6 | `store/outcome.rs:133`, `index.rs:167` |
| Rust: remove workspace | 0.6 | 0.2 | |
| `exec` + interpreter boot to first Python line | 21.0 | 8.6 | |
| `import candler_evaluator` (pydantic models) | 79.6 | 32.7 | `frames.py:16`, `_models/*` |
| `import` plug-in + `htn_creator_eval` | 34.5 | 14.2 | `kinds/_common.py:13` |
| Read request, parse, identities | 0.7 | 0.3 | |
| `fork` to child start | 1.8 | 0.7 | `isolate.py` |
| Kind body in the child | 57.3 | 23.5 | `_runner.py:236` |
| Reap child, build record (pydantic), write frames | 1.8 | 0.7 | |
| Interpreter teardown after `serve` returns, until Rust sees the exit | 20.3 | 8.3 | measured 18 ms of it as normal exit vs `os._exit` |
| Sum | 243.6 | 100 | |

Summing the rows gives 243.6 ms. Two checks:

- Gap from one process's exit to the next spawn: 26.6 ms mean, 11.2 s total. This is the Rust pre- and post-work, 27.3 ms by `PHASE`.
- Spawn to exit: 217 ms mean (median 180 ms, max 487 ms).

Grouped:

| Group | % |
|---|---|
| Python process start + imports (boot, the two imports, teardown) | 63.8 |
| Kind bodies | 23.5 |
| Rust per-request work, almost all fsync | 11.3 |
| Fork, reap, record | 1.4 |

Within the 57.3 ms kind body:

- About 38 ms is the checker module's own heavy imports, re-done in every fork. Basis: with the checkers preloaded in the parent, the same items average 19.3 ms each (37 items, 715 ms).
- The remaining ~19 ms is the real work: `runpy` re-executing the checker, staging a temp directory, `dup2` capture.

Per-kind means in the child, from the trace:

| Kind | n | mean ms |
|---|---|---|
| `run_script` | 6 | 146 |
| `metric_extract` | 5 | 72 |
| `run_script_module` | 16 | 46 |
| `ratio` | 1 | 179 |
| `grep_*` | 5 | 0.9 |

The 8 grep items are the only cheap ones.

Fixed cost of the call: 0.17 s of describe, once. A first call also builds the environment (1.0 s, `uv`, cached wheels).

Python import costs measured in isolation:

| Import | ms |
|---|---|
| `validate_domain` | 118 (libcst 116) |
| `validate_mapping_config` | 222 (`arango.client` 161, `jsonschema` 51) |
| `check_solvability` | 10 |

## 3. Culprits, ranked by cost

### 1. One request, so one Python process, per independent item

Cost: about 62% of wall by itself (boot, imports, teardown are 64% of the per-request time; the experiment shows the full effect).

- **Where:** `crates/candler-core/src/evaluation/engine/plan.rs:151-205`. A batch is a connected component of `depends_on` among entries of the same evaluator. `engine/run.rs` (`run_batch`/`attempt`/`send`) then sends one request per batch. `client/session.rs:300` spawns one process per request. Of 37 suite entries, only one dependency edge exists (a chain of 3). So 36 batches per run.
- **Why wasteful:** the process pays about 135 ms of boot and imports to run 1 item. The engine also redoes assemble, materialise, 3 file fsyncs, a child-record write (2 fsyncs) and an outcome commit (3 fsyncs) per item. Batches of different components in the same stage and evaluator have no ordering constraint.
- **Measured saving:** merging all batches of the same (stage, evaluator) into one request (scratch change to `plan()`): 103.6 to 18.6 s at c=1, 30.9 to 8.7 s at c=4. That is 5.6x and 3.6x, with identical verdict counts. Method: `PROF_MERGE=1` in a scratch build, same 12-run store, same command.
- **Fix shape:** have the planner emit one batch per (run, evaluator, stage). Keep component order inside it (dependencies first), as `run.rs` already supports, since `attempt` handles blocked entries. Only a cross-evaluator dependency forces a later stage. Keep one outcome file per request, as the format wants. This is a planning-only change; the engine already runs a batch's items in order and applies dependency blocking inside it.

### 2. Heavy checker imports repeated in every item fork

Cost: about 38 ms of the 57 ms kind mean, 16% of wall at c=1 (about 40 ms per item average, up to 220 ms for the mapping checker).

- **Where:** `kinds/_runner.py:236` (`runpy.run_module` in the forked child). `candler_evaluator/isolate.py` forks per item after the parent has imported only `htn_creator_eval`. `validate_domain` pulls `libcst` (116 ms), `validate_mapping_config` pulls `arango` and `jsonschema` (220 ms).
- **Why wasteful:** with one item per request there is nothing to amortise. After fix 1 the cost remains per fork, because fork happens before the checker imports. Preloading in the parent pays it once per request.
- **Measured saving:** after batching, preloading 8 checker modules in the parent cuts 8.7 s to 4.3 s at c=4 and 18.6 s to 12.8 s at c=1 (this includes no-fsync). Method: `wrap2.py` as the plug-in command, a scratch wrapper (not the real plug-in). `_runner.py` already drops only the modules listed in `_FRESH_PREFIXES` and those the run imported itself, so preloaded dependencies survive.
- **Fix shape:** let a plug-in declare modules to warm before the first fork (or have the plug-in `__main__` import its checkers, keeping the fresh-module rules in `_runner.py`). Worth a parity check, since `gtpyhop` state is deliberately dropped between runs.

### 3. `F_FULLFSYNC` on every small file, and 8 fsyncs per request

Cost: 36 ms per request (about 15% at c=1), and 100% of import's wall time.

- **Where, evaluate path:**
  - `context/materialise.rs:134` (per copied artifact, `copy.sync_all()` on a file that is deleted a moment later).
  - `supervise/record.rs:29,32` (file plus directory, called synchronously in async code from `supervise/spawn.rs:673`).
  - `store/outcome.rs:133`, `store/mod.rs:219` (`sync_dir`), `store/index.rs:167` (`sync_data` under the exclusive `flock`, so appends serialise).
- **Where, import:** `store/mod.rs:219,231` (`write_new_synced`, `sync_dir`) on every blob; `store/blob.rs:146`; `store/run.rs:196` and the many `sync_dir` calls in `run.rs`.
- **Why wasteful:** Rust's `sync_all`/`sync_data` on macOS is `F_FULLFSYNC` (4.15 ms per file measured, against 0.06 ms for none). The workspace copies are scratch files that `remove_dir_all` deletes after the request; they need no durability. The child record and the per-blob sync serialise on the disk.
- **Measured saving:**
  - Evaluate, baseline: 30.9 to 27.0 s at c=4, 20.5 to 17.0 s at c=8, 13% to 17% of wall.
  - After batching the saving is tiny (8.7 to 8.5 s), because the fsync count drops from about 3,400 to about 100.
  - Import, 12 runs: 5.85 s to 0.82 s (7.1x). That is 5 s of idle wall, 86% of import. Extrapolated 240 runs: ~117 s to ~16 s. CPU is only 0.4 to 0.6 s, so the 136 s debug number is fsync wait.
  - Method: `PROF_NOSYNC=1`, with the sync calls replaced by a no-op in a scratch build.
- **Fix shape:**
  - Drop the sync on the materialised workspace copies and on child records.
  - For import and outcome files, fsync once per run or per batch (write all, then sync once) instead of per blob. Sync directories once, not per file.
  - Move the index `sync_data` outside the flock or batch it.
  - Keep durability where the format promises it (sealed run, published outcome file).

### 4. Per-request file copying and rehashing

Cost: 13.4 ms per request, 5.5% (about 1,260 file copies, hashes and chmods), mostly the fsyncs of culprit 3 plus copy and hash of 44 KB.

- **Where:** `context/materialise.rs` (`copy_artifact`: copy, `sync_all`, `chmod`, `digest_of_file`), called from `engine/run.rs:551`. The same 3 files are rebuilt for each of a run's 36 requests.
- **Why wasteful:** materialised once per run would be enough.
- **Saving:** after fix 1, 36 requests per run become 1, removing ~0.48 s of CPU and I/O per run (measured: 13.4 ms times 36). Without fix 1, a per-run workspace shared by the run's batches would remove the same.
- **Fix shape:** with one request per (run, evaluator, stage) this is solved. Hash the stored blob digest and skip the rehash if the copy is read-only and sealed.

### 5. Python import weight of `candler_evaluator` itself

Cost: 79.6 ms per request, 32.7% at c=1 before batching, about 6% of an optimal batched run.

- **Where:** `candler_evaluator/frames.py:16-18` (pydantic `RootModel`, `_models.evaluator_message` 13.8 ms self), `loop.py:13-14` (`DescribeResult`, `OutcomeRecord`), `candler_evaluator/__init__.py`. Cold `import pydantic` is 52 ms; model class creation is the rest.
- **Why wasteful:** the protocol models are built at import to validate a handful of frames per request. After batching it is paid once per run.
- **Saving:** after fix 1 it falls from 33% to about 0.08 s of 1 s per run. To go further, lazy-import the models or validate with a thin non-pydantic path. Estimated, not measured separately.
- **Fix shape:** lazy construction of rarely used models (`DescribeResult` is needed only for describe). A warm worker pool removes it entirely.

### 6. Interpreter teardown, 20 ms per request

- **Where:** after `serve()` returns, `candler_evaluator/__main__`, `loop.py` run. Measured 18 ms of normal exit against `os._exit` with the same imports loaded.
- **Saving:** about 8% of per-request time before batching, 0.3% after. Not worth fixing on its own. A `flush` then `os._exit(0)` after the final frame would remove it.

### Classic culprits checklist

| Culprit | Found? | Evidence |
|---|---|---|
| One process or one `uv` invocation per item or per small request | **Yes, per request, not `uv`.** 421 plug-in processes for 444 items. `uv` runs once, on the first call only. | spawn events, `uv` marker count 0 after the first call; plan.rs |
| Re-importing heavy modules in every fork or request | **Yes.** libcst, arango, jsonschema per fork; pydantic and models per request. | `-X importtime`; wrap trace |
| Sleep, poll or timer loops in client, supervisor or helper | **No.** `POLL = 10 ms` in `supervise/stop.rs` runs only when a group still has members; the normal path exits at once. Python `_POLL_S = 0.2` is a `select` timeout that wakes on pipe readiness. The 2 s grace was never reached. | read of `stop.rs:600,616`, `isolate.py:_await`; per-item wait is the plug-in's own exit (20 ms of Python teardown) |
| fsync or rewrite of a whole file per record | **Fsync per file, yes** (about 8 per request, 2 per blob). No whole-file rewrite: the index append reads one byte. | `index.rs:127-167`; `F_FULLFSYNC` measured |
| Re-hashing or re-copying a run's artifacts per item | **Yes.** 3 files copied, hashed and fsynced for each of 36 requests per run. | `PHASE files=3` |
| Re-reading or re-parsing the manifest, suite or index per item | **Mostly no.** The suite is parsed once; the registry is loaded twice per call (CLI and engine, trivial). `assemble` re-reads a run's tree and manifest per request: 0.3 ms. | `PHASE assemble_us` |
| O(n^2) scans over the store or outcome files | **No** on this subset (the index append is O(1)). Not tested at 240 runs. | |
| Lock contention that serialises concurrent requests | **Partly.** The index `flock` is held across `sync_data`, so outcome commits serialise (about 4 to 5 ms each). The child-record fsync blocks a runtime thread. Not the dominant limit: no-fsync at c=8 recovers 17%. | |
| stderr or stdout pipe handling that stalls | **No.** The client discards frames to EOF, joins stderr within 1 s; never hit. | |
| Concurrency that does not scale | **Sub-linear but CPU-bound.** c=4: 3.3x, c=8: 4.6x. Utilisation at c=8: 5.9 of 8 cores. | table 1 |

## 4. Projected throughput

Measured on the 12-run subset (444 items):

| Config | Wall at c=4 | Items/s |
|---|---|---|
| Baseline | 30.9 s | 14 |
| Batch per (evaluator, stage) | 8.7 s | 51 |
| Plus preload | 4.3 s | 103 |
| Plus preload, c=8 (12 runs on 8 permits, two waves) | 3.3 s | 136 |
| Batched + preload + no fsync, c=1 | 12.8 s | 35 (so 1.06 s per run) |

Extrapolation to 240 runs (estimate): at c=1, one batched, preloaded run costs 1.06 s wall and 1.03 s CPU. If c=8 reaches the measured 66% efficiency (5.3 effective cores), 240 runs take about 240 x 1.06 / 5.3 = 48 s, or about 185 items/s. With c=12 and a well-filled queue, perhaps 250 items/s. That is 8,880 items in 35 to 50 s, against 650 s now (13 to 18x). The fix for import (no per-blob fsync) takes 117 s to about 16 s.

What bounds it then, from the trace of one batched, preloaded run (959 ms in the process):

| Part | ms per run |
|---|---|
| Process start: boot 21, `candler_evaluator` 76, plug-in 33 | ~130 |
| Preload (libcst, arango, jsonschema, `htn_creator_scripts`) | ~122 |
| 37 forked items, 19 ms mean each (`runpy` re-exec, temp-dir staging, libcst parse) | ~715 |

The per-item fork and checker run is 75% of what is left. At 15 to 30 ms CPU per item on 12 cores, the ceiling for the current checker approach is about 400 to 600 items/s. 1,000 items/s is not reachable by fixing the engine alone. It needs one of:

- a warm worker pool (removes the ~250 ms fixed cost per run),
- running checkers in-process without fork where they are known pure,
- caching a run's parsed `domain.py` across the several checkers that read it,
- grading only items whose inputs changed.

These are guesses at saving, not measured.

## 5. What I could not measure

- **A full 240-run evaluate.** I did not spend the 10 minutes. The 650 s figure is the user's; my 12-run numbers extrapolate to 620 s at c=4, which agrees. The projected 240-run throughput after fixes (section 4) is extrapolated from 12 runs, not measured. Queue-filling, disk and thermal behaviour at 240 runs may differ.
- **O(n^2) in the store at 240 runs.** The index append is O(1) and `read_tree` is per-run, so none is expected, but I did not run enough outcome files to see it.
- **A sample of the Rust binary.** I did not use `sample <pid>`. Release against debug showed identical wall and CPU, which already puts `candler`'s own CPU under 0.2 s. The Rust timings come from `eprintln` in a scratch build, so they are exact but include the `eprintln` itself (microseconds).
- **Per-fork cost attribution below 19 ms.** I split the 57 ms kind body into about 38 ms of imports and about 19 ms of real work by comparing against the preload run. I did not `cProfile` each checker.
- **Equality of the batched verdicts beyond counts.** The batched and preloaded runs gave identical status counts (28/24/8/384). I did not diff the 444 records, and the preload variant changes module caching, so a parity check is needed before adopting fix 2.
- **Whether `F_FULLFSYNC` costs the same on the user's disk under load.** The 4.15 ms was measured on this machine's SSD while it was shared.
- **Debug-build import.** One run is 7.21 s (the first, concurrent with a cargo build, was 12.6 s); I did not repeat it under quiet conditions beyond that.
- **`sample`/`ps` CPU-idle analysis beyond CPU-seconds against wall.** CPU-busy figures come from `/usr/bin/time -l`, which counts all child processes.
