# Process supervisor

**Goal**: `candler_core::frames` writes and reads `raw-frames/1` files, and `candler_core::supervise` spawns children in their own group with a fsynced child record, stops them by escalation with events, enforces deadlines, and reaps a group from its record, leaving no survivor in the group; the stray sweep is an injected `StraySweeper` that M2b implements.
**Pre-conditions**:
- [ ] [cmd] DEP(wire_foundations), DEP(wire_trace), DEP(wire_execution) and DEP(harness_traits) hold: `for l in wire_foundations wire_trace wire_execution harness_traits; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`
- [ ] [nl] api_review closed with no open finding on the supervisor's inputs (`StopSpec`, `Action`, `ExitStatus`, `ChildRecord`, `EventPayload`)
- [ ] [cmd] `rustup target list --installed | grep -qx x86_64-unknown-linux-gnu && command -v perl`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_frames)'` passes, holding two B tests: for arbitrary bytes cut into arbitrary chunks, the frames' data concatenate to the input and frame N equals the N-th piece of `split_inclusive(b'\n')` (the standard library is the oracle); a table of nine literal cases (under `Lines`: empty stream, `"\n"`, `"a"`, `"a\n"`, `"a\nb"`, `"\r\n"`, `"\n\n"`, invalid UTF-8; under `Whole`: `"a\nb"` as one frame) with the exact records the test states
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_supervise)'` passes, holding ten B tests: no survivor; sweeper called; record on disk; forged record; handles; events; secret canary; pipe held; pre-stop failure; loopback (each described in the steps below)
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'test(proc_stat)'` passes: the `/proc/<pid>/stat` parser reads field 22 from literal lines whose `comm` holds `) (`, spaces and a final `)`
- ⬜ [cmd] `cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings` passes
- ⬜ [cmd] STD and SUBJ pass
- ⬜ [nl] The Linux run of `guard_supervise` is decided by the PR's CI; the leaf report records the macOS run log
- ⬜ [nl] The required adversarial verifier reports no open finding, having attacked deadline races, fd leaks, a zombie at record time, the `cfg(target_os = "linux")` code line by line, escalation under faults, and URL parsing
- ⬜ [nl] The leaf report has a bite mutation per B test
**References**: [R01 §Process supervision, §Agent environment and secrets, §Raw traces, §Job](../../../../__design__/architecture.md) — every rule; [R07](../../../candler-v1/contracts/parity/README.md) — `m2b_execution_parity.md` builds the job runner on this API

## Step 1: Write and read framed raw streams
**Goal**: A stream's bytes become a `raw-frames/1` file one line per frame, and the file reads back as frames.
**Implementation Logic**:
1. `FrameWriter::create(dir: &Path, stream: StreamName, mode: FrameMode, clock: Clock, source: Option<Digest>) -> io::Result<FrameWriter>` writes the header to `<dir>/<stream>.jsonl.tmp`; `write(&mut self, bytes: &[u8], t_mono_ns: Option<u64>)` under `FrameMode::Lines` emits one frame per completed line (its `\n` kept), buffering a partial line, and flushes after each frame; under `FrameMode::Whole` it buffers, and `finish` emits the stream as one frame (a document, for a `Document` reader); `finish(self) -> io::Result<RawFileInfo {path, digest: Digest, frames: u64}>` emits the trailing partial line as its own frame, gzips with header mtime 0 to `<stream>.jsonl.gz` and renames it into place.
2. `frame_bytes(stream, mode: FrameMode, &[u8], source: Digest) -> Vec<RawRecord>` frames a finished stream after the fact (`clock: unrecorded`, no `t_mono_ns`), for the importer.
3. `FrameReader::open(&Path) -> Result<FrameReader, FramesError>` reads a plain or gzipped file: `header()`, then an iterator of `RawFrame`; it refuses a missing header and a second one.
4. `tests/guard_frames.rs` (`proptest` for the property, a literal table for the cases).
**Deliverables**: `crates/candler-core/src/frames/mod.rs` (`FrameWriter`, `FrameMode`, `FrameReader`, `RawFileInfo`, `frame_bytes`, `FramesError`); `crates/candler-core/tests/guard_frames.rs`
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_frames)'` (expected: PASS)
**Commit**: `feat(runner): write and read framed raw streams`

## Step 2: Spawn children in their own group with a record
**Goal**: Every child starts in a new process group with an exact environment and the marker, and its record is on disk before `spawn` returns.
**Implementation Logic**:
1. `EventSink: Send + Sync { fn emit(&self, run: Option<&RunKey>, payload: EventPayload); }`; the sink owns the envelope's `seq`, `job_id` and `at`. `Supervisor::new(Arc<dyn EventSink>)`.
2. `SpawnRequest::builder(argv: Vec<String>)` with `.env(Vec<(String, String)>)` (the exact environment; nothing is inherited), `.marker(Marker)` (sets `CANDLER_RUN`), `.run(RunKey)`, `.cwd(PathBuf)`, `.stdin(StdinSpec: Null | Bytes(Vec<u8>) | Pipe)`, `.stdout(OutputSpec)` and `.stderr(OutputSpec)` with `OutputSpec: Framed {dir, stream, mode: FrameMode, observer: Option<Box<dyn FrameObserver>>} | Pipe | File(PathBuf) | Null`, where `FrameObserver: Send { fn frame(&mut self, index: u64, frame: &RawFrame); }` sees each frame once it is flushed (how a live reader gets its frames), `.deadline(Duration)`, `.stop(StopSpec)`, `.record_dir(PathBuf)` (required: without records nothing can be reaped), `.build() -> Result<SpawnRequest, SuperviseError>`. `SpawnRequest` is `#[non_exhaustive]`; its `Debug` prints environment keys, never values.
3. `Supervisor::spawn(&self, SpawnRequest) -> Result<Child, SuperviseError>`: `process_group(0)`, `env_clear`; then reads the start time (Linux: `/proc/<pid>/stat` field 22 and `/proc/sys/kernel/random/boot_id`; macOS: `libproc` BSD info `sec`/`usec`; else `unavailable`), writes `<record_dir>/<pid>.json` (`child-record/1`) with fsync of file and directory, emits `process.spawned`, and returns.
4. `parse_proc_stat(&str) -> Result<u64, SuperviseError>` is a pure function (the `comm` field ends at the last `)`), guarded by `proc_stat` in a `#[cfg(test)] mod guards` with literal lines.
5. `liveness(&ChildRecord) -> Liveness (Alive | Gone)`: alive only when the pid exists and its start time equals the record's; `unavailable` is `Gone`.
6. No `unsafe`: no `pre_exec`, so no parent-death signal (R01). The Linux child subreaper and the marker scans belong to the stray sweep, which M2b builds (`m1_contracts.md`).
7. Guards in `tests/guard_supervise.rs`: **record on disk** (a child that sleeps; when `spawn` returns, the record file parses and its pid, pgid and start equal what the OS reports through `ps -o pid=,pgid=,lstart=`); **forged record** (the same record with `start` changed by one tick, and with `unavailable`, reads `Gone` while the child runs, and the true record reads `Alive`).
**Deliverables**: `crates/candler-core/src/supervise/{mod.rs,spawn.rs,record.rs,platform.rs}` (`EventSink`, `Supervisor`, `SpawnRequest`, `SpawnRequestBuilder`, `StdinSpec`, `OutputSpec`, `FrameObserver`, `Child`, `SuperviseError`, `Liveness`, `liveness`, `parse_proc_stat`); `crates/candler-core/tests/guard_supervise.rs`
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_supervise) | test(proc_stat)' && cargo clippy --target x86_64-unknown-linux-gnu -p candler-core --tests -- -D warnings` (expected: PASS)
**Commit**: `feat(runner): spawn children in their own group with a record`

## Step 3: Stop children by escalation and enforce deadlines
**Goal**: A child ends by exit, deadline or stop, through R01's escalation, with an event per step.
**Implementation Logic**:
1. `Child::record(&self) -> &ChildRecord`; `take_stdin`, `take_stdout` and `take_stderr` give the pipe once for a `Pipe` spec; `stop_handle(&self) -> StopHandle` (`Clone + Send`; `request()` starts the escalation once, from any task); `Child::wait(self) -> Result<Exit, SuperviseError>` and `Child::stop(self) -> Result<Exit, SuperviseError>`. `Exit {status: ExitStatus, ended: Ended (Exited | Deadline | Stopped), steps: Vec<StepRecord {step: StopStep, at: Timestamp, outcome: String}>, group_empty: bool, raw: Vec<RawFileInfo>, stderr_tail: Vec<u8>}`: `group_empty` covers the pgid only, `raw` lists each finished `Framed` stream, and `stderr_tail` holds the last 64 KiB of a `Framed` or `File` stderr (for `ProcessExit`). `Supervisor::run_captured(&self, SpawnRequest, limit: usize) -> Result<Captured {exit: Exit, stdout: Vec<u8>, stderr: Vec<u8>}, SuperviseError>` runs a probe with both outputs piped and kept up to `limit` bytes each.
2. Escalation: the pre-stop action (a `Command` run as a supervised child bounded by the grace; an `Http` call made only when `is_loopback(url)`), then the stop signal and grace, then `killpg(TERM)`, then `killpg(KILL)`, then the stray sweep through the injected `StraySweeper: Send + Sync { fn sweep(&self, marker: &Marker) -> Vec<u32>; }` (`Supervisor::with_sweeper`), run on a blocking thread, returning the pids it found and killed; without a sweeper the step is recorded with outcome `no sweeper`. M1 ships no sweeper: the `/proc` and same-user marker scans are M2b's (`m1_contracts.md`). Each step emits `process.stop_step`; the end emits `process.exited` and, for survivors of the sweep, `run.stray`. A failed or timed-out pre-stop continues the escalation.
3. `is_loopback(&str) -> bool`: parses scheme, userinfo, host and port; accepts scheme `http` only (no TLS client in M1), no userinfo, and host `localhost`, an IPv4 in `127.0.0.0/8` or `[::1]`; the call follows no redirect.
4. `wait` returns at process exit plus at most the grace for draining output, even when a grandchild keeps the pipe open.
5. Guards in `tests/guard_supervise.rs`: **handles** (bytes written to a `cat` child through `take_stdin` come back through `take_stdout`; a `StopHandle` request from another task while `wait` is pending ends the child `Stopped`; `run_captured` of `printf` returns the literal bytes the test states); **events** (a stopped child's sink receives the literal ordered list of kinds and steps the test states, and each envelope validates against the published `event-envelope` schema with `jsonschema`); **secret canary** (a canary value in `.env` appears in no file under `record_dir`, no event and no `Debug` output, checked with `tests/support::assert_clean`); **pipe held** (a grandchild holding stdout for 60 s does not hold `wait` past deadline + grace + 2 s); **pre-stop failure** (a two-row table: an HTTP pre-stop to a closed loopback port and a command pre-stop that sleeps past the grace each still reach TERM, then KILL); **loopback** (about ten hand-written URLs: `http://127.0.0.1:8080/x`, `http://127.1.2.3/`, `http://[::1]:9/`, `http://localhost/` accepted; `http://127.0.0.1.evil.com/`, `http://127.0.0.1@evil.com/`, `http://evil.com@127.0.0.1/` (any userinfo is refused), `https://127.0.0.1/`, `http://[::ffff:127.0.0.1]/`, `http://0.0.0.0/` refused).
**Deliverables**: `crates/candler-core/src/supervise/{stop.rs,http.rs}` (`Exit`, `Ended`, `StepRecord`, `StopHandle`, `Captured`, `StraySweeper`, `is_loopback`); `crates/candler-core/tests/guard_supervise.rs`
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_supervise)'` (expected: PASS)
**Commit**: `feat(runner): stop children by escalation and enforce deadlines`

## Step 4: Reap a process group from its child record
**Goal**: A lost job's children are stopped from their records alone, with the same escalation.
**Implementation Logic**:
`Supervisor::reap(&self, run: Option<&RunKey>, record: &ChildRecord, stop: &StopSpec) -> Result<ReapReport, SuperviseError>`: when `liveness` is `Alive`, run the escalation on the record's pgid and sweep by its marker; when `Gone`, sweep only (both through the injected sweeper, if any). `ReapReport {was_alive: bool, steps, strays: Vec<u32>, group_empty: bool}`. It emits the same events as a stop, under `run`.
**Deliverables**: `crates/candler-core/src/supervise/reap.rs` (`Supervisor::reap`, `ReapReport`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(runner): reap a process group from its child record`

## Step 5: Prove a stopped process tree leaves no survivor
**Goal**: The OS, not the supervisor, confirms that a stopped tree and a reaped one leave nothing running.
**Implementation Logic**:
1. `tests/fixtures/supervise/tree.sh`: a POSIX shell that ignores TERM, starts two grandchildren, one detached into a new session with `perl -MPOSIX -e 'setsid; …'`, and writes every pid to a file given as argument.
2. **No survivor** (`guard_supervise`): stop the tree through `Child::stop`, then reap a second tree from its record alone; for every written pid still in the group at spawn, `kill(pid, 0)` fails with `ESRCH` or the OS start time differs from the one read at spawn, and the group is empty. **Sweeper called**: a test-side `StraySweeper` that kills the written pids outside the group (the detached one) is called once per stop, after `kill_group`, with the run's marker; that pid is then gone, and a `run.stray` event names it.
3. Then delete the leaf's scaffolding and write the bite record.
**Deliverables**: `crates/candler-core/tests/fixtures/supervise/tree.sh`; `crates/candler-core/tests/guard_supervise.rs`
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_supervise) | binary(guard_frames)' && ! git grep --untracked -n 'mod scaffold' -- crates/candler-core/src/frames crates/candler-core/src/supervise` (expected: PASS)
**Commit**: `test(runner): prove a stopped process tree leaves no survivor`
