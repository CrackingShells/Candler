# Regrade throughput in the core and helper

**Goal**: The core sends one request per run, evaluator and stage, syncs store files with `fsync(2)` and never syncs scratch copies, and the helper starts and exits cheaply. Every outcome is unchanged.
**Pre-conditions**:
- [ ] [cmd] `git log --first-parent --format=%s feat/m2a-evaluation-parity | grep -qx 'merge(evaluation): land golden_parity'`
- [ ] [nl] The implementer has read the profile `__reports__/m2a-evaluation-parity/throughput/00-regrade_profile_v0.md`. Its scratch instrumentation is `$TMPDIR/m2a-regrade-profile.instrumentation.diff`, used only in a scratch copy and never committed.
**Success Gates**:
- ✅ [cmd] STD, SUBJ, SCAFFOLD and PYSTD pass, and OWNED passes with `^(crates/candler-core/(src/(evaluation/(engine|context|client)/|store/|supervise/record\.rs$|frames/mod\.rs$)|tests/(guard_engine\.rs$|fixtures/engine/))|crates/candler/tests/guard_golden_parity\.rs$|python/candler-evaluator/(src|tests|examples)/|__reports__/m2a-evaluation-parity/throughput/regrade_throughput/)`
- ✅ [cmd] PERF12, release build, `--concurrency 4`: the evaluate wall is 12 s or less (31 s before). Its 444 records equal the pre-leaf records row by row on `(status, value, weight, cause)`. The plug-in is spawned at most 13 times (12 requests plus 1 describe; 421 before).
- ✅ [cmd] PERF12 import in a release build takes 1.5 s or less (5.85 s before).
- ✅ [cmd] `git grep -nE '\.sync_(all|data)\(' -- crates/candler-core/src` prints nothing outside the one sync module
- ✅ [cmd] With `CANDLER_HTN_PLUGIN` pointing at a scratch plug-in copy whose suite is invalid, `binary(guard_golden_parity)` fails all three tests in under 60 s in total
**References**: [R01 §Evaluator protocol](../../../../../../__design__/architecture.md) — "imports the checker libraries once, then forks one child per item"; [R01 §Store layout](../../../../../../__design__/architecture.md) — blobs, outcome files, index; [profile §3](../../../../../../__reports__/m2a-evaluation-parity/throughput/00-regrade_profile_v0.md) — the culprits with file and line; [rulings §Regrade throughput](../../../../../../__reports__/m2a-evaluation-parity/l1/00-rulings_v0.md)

PERF12 is the profile's subset: run 1 of each of the 12 cells, copied under `$TMPDIR` with the corpus root files, with the store, `XDG_STATE_HOME` and `XDG_CONFIG_HOME` under `$TMPDIR` too. Its baseline is 444 items: 384 pass, 24 fail, 28 blocked, 8 `not_applicable`. Record its pre-leaf outcome files once, before Step 1, as the row-equality reference.

## Step 1: Send one request per run, evaluator and stage
**Goal**: A request carries every applicable entry of one evaluator at one stage, so a run costs one interpreter per evaluator, not one per item.
**Implementation Logic**:
1. `engine/plan.rs` groups only entries linked by `depends_on`, so every independent entry gets its own request and its own Python process. On PERF12 that is 420 requests for 444 items, and each pays about 135 ms of boot and imports. R01 meant a request to carry the run's items: the helper imports once and forks one child per item.
2. Group every applicable entry of one evaluator by stage into one batch, ordered by the existing dependency `order`. Stages still separate dependencies across evaluators. A dependent of a failed or blocked entry stays `blocked`, and `prior[]` still carries the records of earlier items of the same request.
3. The request deadline stays the sum of its items' deadlines plus the margin (`engine/run.rs`). GP3's zero-`deadline` assertion must hold in throughput_proof.
4. Add a row to ENG1, with no new guard ID. Use the synthetic plug-in with two independent entries of one evaluator and one entry of a second evaluator. The first two share one request, the third has its own. Read the request ids from the outcome files, or the helper pid from a kind of the synthetic plug-in. Oracle: a hand literal. Bite: group by dependency component again.
**Deliverables**: the grouping in `engine/plan.rs` and its use in `engine/`, the ENG1 row; the commit body carries PERF12 before and after
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_engine)'` (expected: PASS)
**Commit**: `perf(evaluation): send one request per run, evaluator and stage`

## Step 2: Sync with fsync, and never sync scratch copies
**Goal**: The store keeps its write, sync and rename order, at `fsync(2)` cost.
**Implementation Logic**:
1. On macOS, Rust's `File::sync_all` and `sync_data` issue `F_FULLFSYNC`: 4.15 ms per small file, against 0.06 ms for `fsync(2)`. The core calls them on every blob, outcome file, index append, child record and frames file, and on every materialised scratch copy. That is 86% of import wall time and about 36 ms of every request.
2. One `pub(crate)` sync module under `store/` (`sync_file`, `sync_data`, `sync_dir`) calls `rustix::fs::fsync` and `fdatasync`. The workspace already enables rustix `fs`. Every site uses it. Per the rulings, "fsynced" in R01 means `fsync(2)`, which is the guarantee `sync_all` already gives on Linux.
3. Remove the sync from `context/materialise.rs`. The materialised workspace is scratch for one request and is removed after it. I11 is about write-through, not durability, so RC3 still applies.
4. Run SPAWN before the workspace tests. The order write, sync, rename, sync directory is unchanged everywhere else. Add no new guard, because durability is not observable in a test. The evidence is the grep gate and the PERF12 timings.
**Deliverables**: the sync module and its call sites in `store/`, `supervise/record.rs`, `frames/mod.rs` and `evaluation/`; the commit body carries the PERF12 import before and after
**Consistency Checks**: `cargo nextest run --workspace` (expected: PASS)
**Commit**: `perf(run-store): sync with fsync and leave scratch copies unsynced`

## Step 3: Start the helper lazily, and exit after the last frame
**Goal**: A helper process spends its time on items, not on building models or tearing down.
**Implementation Logic**:
1. Importing `candler_evaluator` costs about 80 ms per process, mostly building pydantic models at import in `frames.py` and `loop.py`. Interpreter teardown costs about 20 ms.
2. Build the models on first use, or import the model modules lazily, so `describe` and `evaluate` pay only for what they validate. Once the response frame is flushed and stdin is at EOF, flush stdout and stderr and leave with `os._exit(0)`.
3. HR1, HR2 and PYM1 pass unchanged. If lazy construction would change what the wire accepts, report it and leave that part out.
4. Measure with `python -X importtime -m candler_evaluator <echo plug-in>` and PERF12.
**Deliverables**: the lazy model access in `python/candler-evaluator/src/candler_evaluator/` and the exit path; the commit body carries the import time before and after
**Consistency Checks**: `cd python/candler-evaluator && uv run --frozen pytest -q` (expected: PASS)
**Commit**: `perf(evaluator-helper): start lazily and exit after the last frame`

## Step 4: Fail the corpus guard fast after a failed build
**Goal**: A fixture build that fails costs one build, not one per test process.
**Implementation Logic**:
1. Each nextest process takes the fixture lock. When the build has failed, no `complete.json` exists, so the next process rebuilds the whole corpus. That is how one bite run took 4,234 s.
2. Before panicking, a failed build writes `failed.json` with its marker and the failure message. A process that takes the lock and finds a `failed.json` with the same marker panics with that message. A new build happens only when the marker differs: a new binary, rules, manifest or suite.
3. The guard's regrade concurrency becomes 8.
**Deliverables**: the failure marker in `crates/candler/tests/guard_golden_parity.rs`
**Consistency Checks**: the fail-fast success gate (expected: PASS)
**Commit**: `test(evaluation): fail the corpus guard fast after a failed build`

## Step 5: Record the measurements
**Goal**: The leaf report holds one PERF12 row per step, with the command that produced it.
**Implementation Logic**:
1. `__reports__/m2a-evaluation-parity/throughput/regrade_throughput/00-report_v0.md` holds:
   - the PERF12 table (wall, user, sys, spawns and items/s at concurrency 1, 4 and 8) before Step 1 and after each step;
   - the row-equality check;
   - the bites;
   - the findings.
**Deliverables**: `__reports__/m2a-evaluation-parity/throughput/regrade_throughput/00-report_v0.md`
**Consistency Checks**: `test -s __reports__/m2a-evaluation-parity/throughput/regrade_throughput/00-report_v0.md` (expected: PASS)
**Commit**: `reports(evaluation): record the regrade throughput leaf`
