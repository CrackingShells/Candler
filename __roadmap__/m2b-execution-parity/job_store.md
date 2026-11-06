# Job store: dir, lock, events, slots

**Goal**: `candler_core::jobs` keeps a job's files (`job.json`, `events.jsonl`, `lock`, `children/`, `control/`), the machine-wide run slots and the lost-job rule, implements `JobView` (R8) and fills `jobs::lost::recover` (R17), so that a job's state is always read from the OS lock and the event log, never from a field.
**Pre-conditions**:
- [ ] [cmd] Phase 0 is landed: `for s in '^docs(design): ' '^build(repo): ' '^ci(repo): ' '^feat(schemas): ' '^feat(orchestrator): scaffold'; do git log --format=%s origin/main..feat/m2b-execution-parity | grep -q "$s" || exit 1; done && test -s __roadmap__/m2b-execution-parity/feature_map/feature_map.json`
- [ ] [nl] api_review closed with no open finding on the S3 stubs this leaf fills or consumes (`JobView`, `ChildEntry`, `ContainerSweeper`, `jobs::lost::recover`, `EventSink`, `Runtime.supervisor`)
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_job_store)'` passes, holding B01 and B02. **B01 slot peak**: two real supervisor processes (the test re-executes its own binary as workers) share one slots directory with `slots.count = 2` and run many holds of at least 100 ms; each worker appends `A <id>` after acquiring and `R <id>` before releasing to one `O_APPEND` log; the test computes the peak overlap from that log and asserts it equals 2 (liveness control) and never exceeds 2 (the OS log is the oracle; the code's own counters are not read). **B02 lost table**: a real second descriptor holds or releases the flock and the test writes or omits `job.ended`; the four rows held+no ended = running, free+no ended = lost, free+ended = ended, free+torn last line and no ended = lost match the R01 sentence "lock free and no `job.ended` event"
- ⬜ [cmd] STD, SUBJ and LINUX pass
- ⬜ [nl] The leaf report has a bite record, one mutation per B test (B01: the acquire loop `0..n` becomes `0..n+1`; B02: `EWOULDBLOCK` is treated as free), each run only against `guard_job_store`, each restored with `git checkout --`
- ⬜ [cmd] `! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/jobs crates/candler-core/tests/guard_job_store.rs`
- ⬜ [nl] No verifier required; the gates are `[cmd]`
**References**: [R01 §Job, §Budget, §Process supervision](../../__design__/architecture.md) — the lost rule, slots and the I22 guard; [Architecture §7, §10.1, §10.3](../../__reports__/m2b-execution-parity/00-architecture_v0.md) — the stubs this leaf fills, the job directory and slot layouts; [Test definition B01, B02](../../__reports__/m2b-execution-parity/01-test_definition_v0.md) — oracles and bites

## Step 1: Write the job directory and read its children
**Goal**: A job directory is created and read atomically, and its `children/` reader reports every entry it cannot read.
**Implementation Logic**:
1. `JobDir::create(store: &StoreLayout, id: JobId, rec: &JobRecord) -> Result<JobDir, JobError>` makes `<store>/jobs/<id>/` with `children/` and `control/`, writing `job.json` by temp file, fsync of file and directory, rename. `JobDir::open(store, &JobId) -> Result<JobDir, JobError>`; `record(&self) -> Result<JobRecord, JobError>`; `set_state(&self, JobState) -> Result<(), JobError>` (same atomic write); `children_dir()` and `control_dir()` return the paths for the supervisor's `record_dir`.
2. `write_child(&self, &ContainerRecord) -> Result<(), JobError>` writes `children/container-<cid12>.json` atomically, as soon as the cid is known (R10: the record is what makes a container destroyable for the current job).
3. `impl JobView for JobDir`: `job()`; `children() -> Result<Vec<ChildEntry>, JobViewError>` lists `children/` only (never `control/`) and returns `Process(ChildRecord)` for a `child-record/1`, `Container(ContainerRecord)` for a `container-record/1`, and `Unreadable { path, error }` for torn JSON, an unknown schema, an io error (mode 000) or a leftover `.tmp`; it never skips an entry, so the guard sees what it cannot read. Entries are sorted by file name. `is_lost(&self, &JobId)` calls `lost::classify` (Step 3).
4. A job's `state` field in `job.json` is a hint for `ps` display only; no function in this leaf reads it to decide anything.
**Deliverables**: `crates/candler-core/src/jobs/dir.rs` (`JobDir`, `JobError`, the `JobView` impl, `read_children`); `crates/candler-core/tests/scaffold_job_store.rs` (class-A: round trip of each entry kind, one torn file, one mode-000 file, one leftover `.tmp`, each yielding `Unreadable`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(scaffold_job_store)' && cargo clippy -p candler-core --tests -- -D warnings` (expected: PASS)
**Commit**: `feat(jobs): write the job directory and read its children`

## Step 2: Append and tail the job's event log
**Goal**: `events.jsonl` has one sequential writer at a time across processes, and a reader never returns a partial line.
**Implementation Logic**:
1. `JobEvents::open(dir: &JobDir) -> Result<JobEvents, JobError>`; `append(&self, run: Option<&RunKey>, payload: EventPayload) -> Result<Envelope, JobError>` takes an exclusive `flock` on `events.jsonl` (the supervisor, `candler kill` and `recover` all append, so the supervisor does not own `seq` alone), truncates a torn trailing line, sets `seq` to the last line's `seq + 1` (1 for an empty file), stamps `at` and `job_id`, writes one line and fsyncs before releasing.
2. `impl EventSink for JobEvents`: `emit` calls `append`; because `emit` returns `()`, a failure is held in `health(&self) -> Result<(), JobError>`, which the orchestrator polls, and is never swallowed.
3. `tail(&self, from: TailPos) -> Result<Tail { events: Vec<Envelope>, next: TailPos }, JobError>` with `TailPos(u64)` a byte offset; it returns complete lines only and leaves a torn last line for the next call. An unknown event kind is kept as read (I18); a line that is not JSON ends the read with an error naming its offset.
**Deliverables**: `crates/candler-core/src/jobs/events.rs` (`JobEvents`, `TailPos`, `Tail`, the `EventSink` impl, `health`); `crates/candler-core/tests/scaffold_job_store.rs` (class-A: two processes appending 200 events each give seq 1..400 strictly increasing; a torn trailing line is truncated; `tail` from a saved offset returns only the new lines)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(scaffold_job_store)'` (expected: PASS)
**Commit**: `feat(jobs): append and tail the job event log`

## Step 3: Hold the job lock and classify a job
**Goal**: A job is `Running`, `Ended` or `Lost` from the OS lock and the event log alone (B02).
**Implementation Logic**:
1. `JobLock::acquire(dir: &JobDir) -> Result<JobLock, JobError>` takes a non-blocking exclusive `flock` on `lock` through `rustix` (already used by `store/index.rs`), retrying for about 500 ms so that a concurrent `probe` is never read as a second supervisor; a lock still held after that is `JobError::Held`. The lock lives as long as the value (the supervisor's life).
2. `JobLock::probe(dir: &JobDir) -> Result<LockState, JobError>` with `LockState {Held, Free}`: `EWOULDBLOCK` is `Held`; any other error is `Err`, never `Free`. A successful probe lock is dropped at once.
3. `jobs::lost::classify(dir: &JobDir) -> Result<JobLiveness, JobError>` with `JobLiveness {Running, Ended(schema::JobEnd), Lost}`: it reads the last complete `job.ended` event through `JobEvents::tail`, tolerating a torn last line, and the probe. Held and no `job.ended` is `Running`; free and no `job.ended` is `Lost`; any `job.ended` is `Ended` (held+ended is a supervisor still exiting, and `Ended` is the safe reading: nothing is recovered). S3's orchestration result is `JobOutcome { end: schema::JobEnd, runs }` (D13); `Ended` carries the schema `JobEnd`.
4. **B02** in `tests/guard_job_store.rs`: the four-row table of the Gate, driven by a second descriptor on the same `lock` file (held by the test, released by the test) and by writing or omitting `job.ended` and a torn last line; each row asserts `classify`.
**Deliverables**: `crates/candler-core/src/jobs/lock.rs` (`JobLock`, `LockState`, `JobError::Held`); `crates/candler-core/src/jobs/lost.rs` (`classify`, `JobLiveness`); `crates/candler-core/tests/guard_job_store.rs` (the B02 test with its `/// Guards:`, `/// Oracle:` and `/// Bite:` lines)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_job_store) | binary(scaffold_job_store)'` (expected: PASS)
**Commit**: `feat(jobs): classify a job as running, ended or lost`

## Step 4: Share run slots across jobs
**Goal**: No more than the effective parallelism of runs hold a slot at once on the machine (B01).
**Implementation Logic**:
1. `Slots::open(dir: &Path, count: usize) -> Result<Slots, JobError>` creates `<n>.lock` for `n in 0..count`; the effective count is the smaller of `slots.count` and `Budget.max_parallel`, and the caller passes that.
2. `Slots::try_acquire(&self, holder: Holder) -> Result<Option<Slot>, JobError>` takes a non-blocking exclusive `flock` on the first free `<n>.lock`, then writes `<n>.json` `{job, run, pid, since}` (a private struct, not a schema type). `Slots::acquire<'a>(&'a self, holder: Holder) -> BoxFuture<'a, Result<Slot, JobError>>` polls `try_acquire` every 50 ms.
3. `Slot` is released on drop: it removes `<n>.json` and unlocks. `Slots::held(&self) -> Vec<SlotInfo { n, holder: Option<Holder>, stale: bool }>`: a free `flock` with a leftover `.json` is stale (a crashed holder), reported and never treated as held.
4. **B01** in `tests/guard_job_store.rs`: an `#[ignore]`d worker test that the parent test runs through `std::env::current_exe()` as two processes with `slots.count = 2` and a shared state directory; each worker loops acquire, `A <id>`, hold at least 100 ms, `R <id>`, release, into one `O_APPEND` log. The test reads the log and computes the peak overlap itself; it asserts peak equals 2 (a liveness control, so a lock that never grants proves nothing) and never above. Every B-test doc has its three lines.
**Deliverables**: `crates/candler-core/src/jobs/slots.rs` (`Slots`, `Slot`, `Holder`, `SlotInfo`); `crates/candler-core/tests/guard_job_store.rs` (the B01 test and its worker)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_job_store)'` (expected: PASS)
**Commit**: `feat(jobs): share run slots across jobs by flock`

## Step 5: Recover a lost job, then delete the scaffolding
**Goal**: `jobs::lost::recover` leaves a lost job with no host child, no labelled container and every open run sealed, and the leaf's class-A scaffolding is gone.
**Implementation Logic**:
1. `jobs::lost::recover<'a>(sup: &'a Supervisor, sweep: &'a dyn ContainerSweeper, dir: &'a JobDir) -> BoxFuture<'a, Result<RecoverReport, RecoverError>>`. It acquires the job lock (`Held` gives `RecoverReport::NotLost`), re-classifies under the lock (`Ended` gives `already_ended`), then does the three things of R17.
2. Reap: every `ChildEntry::Process` and every `control/*.json` goes to `sup.reap(run, &record, &StopSpec)` with a generous grace; the run comes from the record's marker when it parses. The supervisor adopts a record that verifies against the OS (M1), so the successor's empty table is not a refusal. The process-marker sweep is the `StraySweeper` already installed in the `Supervisor` passed in (`ProcMarkerSweeper`, owned by job_runtime, D11); `recover` has no sweeper and no guard of its own.
3. Sweep: `sweep.sweep(&SweepScope)` for the lost job with `SweepScope::Lost(job)` (D14: every labelled container is in scope). When `ContainerSweep.unknown`, append `container.sweep_incomplete {run?, scope, reason}` (R25); for destroyed containers with no record, append `run_stray_with_containers` (R20).
4. Seal: every run whose manifest `job` equals this job and has no `run.done` goes through `Store::seal_interrupted(&RunKey)` (M1, `store/run.rs`; a `RunSealed` result counts as done), and `run.ended(interrupted)` is appended for it. A sealed run with no `run.ended` gets one whose `ended_by` comes from `measured[] run.ended_by`, else `unknown`.
5. Last, append `job.ended(failed)` (a lost job recovered gets `failed`, D13) and `set_state(Ended)`. `recover` is idempotent and deletes nothing: a second call returns `already_ended` with zero events appended.
6. Class-A tests use `Supervisor::with_signaller` with the recording fake (only under the `test-support` feature, never a real signal) and a fake `ContainerSweeper`: one lost job with two records, one open run and one sealed run; then the same job again (idempotent); then a held lock (`NotLost`).
7. Then delete `tests/scaffold_job_store.rs` after moving its still-wanted cases into `guard_job_store`'s `#[cfg(test)]` helpers or dropping them, and write the bite record: the B01 and B02 mutations of the Gate, one at a time, run only against `guard_job_store`, each restored with `git checkout --`. No mutation touches signalling; a run that spawns runs the pre-spawn check first (`git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)'`).
**Deliverables**: `crates/candler-core/src/jobs/lost.rs` (`recover`, `RecoverReport`, `RecoverError`); `crates/candler-core/tests/guard_job_store.rs`; the bite record in the leaf report
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_job_store)' && ! git grep --untracked -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/jobs crates/candler-core/tests` (expected: PASS)
**Commit**: `test(jobs): prove the slot peak and the lost table`
