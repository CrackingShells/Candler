# CLI: ps, kill, watch

**Goal**: `candler ps`, `candler kill <id> | --all [-y]` and `candler watch <id> --json` list, stop and follow jobs from their job directories, recovering a lost job through `jobs::lost::recover`, and `kill` without `-y` refuses before any docker call.
**Pre-conditions**:
- [ ] [cmd] Phase 0 is landed: `for s in '^docs(design): ' '^build(repo): ' '^ci(repo): ' '^feat(schemas): ' '^feat(orchestrator): scaffold'; do git log --format=%s origin/main..feat/m2b-execution-parity | grep -q "$s" || exit 1; done && test -s __roadmap__/m2b-execution-parity/feature_map/feature_map.json`
- [ ] [nl] api_review closed with no open finding on the `Ps`, `Kill` and `Watch` variants and stub files S3 appended to the `Command` enum, and on `jobs::lost::recover`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
- [ ] [cmd] DEP(job_store): `git log --first-parent --format=%s feat/m2b-execution-parity | grep -qx 'merge(jobs): land job_store'`
- [ ] [cmd] DEP(container_stop): `git log --first-parent --format=%s feat/m2b-execution-parity | grep -qx 'merge(isolation): land container_stop'`
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler -E 'binary(guard_cli_kill)'` passes, holding B31: with a PATH-fake `docker` that appends its argv to a log file, `candler kill --all` without `-y` and with stdin closed exits non-zero with a refusal message and the log is empty (zero docker calls); with `-y` (the control) the log holds the labelled `ps` call and exit status is zero. The oracle is the fake's log file and the exit status, both read by the test; no argv is taken from a builder
- ⬜ [cmd] STD, SUBJ and LINUX pass
- ⬜ [nl] The leaf report has a bite record for B31: the `yes` default becomes true, run only against `guard_cli_kill`, restored with `git checkout --`
- ⬜ [cmd] `! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler/src/commands/ps.rs crates/candler/src/commands/kill.rs crates/candler/src/commands/watch.rs crates/candler/tests`
- ⬜ [nl] No verifier required; the gates are `[cmd]`
**References**: [R01 §CLI, §Job, §Process supervision](../../../__design__/architecture.md) — the verbs, `-y`, the lost rule; [Architecture §6, §7, §8, §10.1](../../../__reports__/m2b-execution-parity/00-architecture_v0.md) — stop, kill and lost sequences, the stub files; [Test definition B31](../../../__reports__/m2b-execution-parity/01-test_definition_v0.md) — oracle and bite; [job_store](../job_store.md) — `JobDir`, `JobEvents`, `classify`, `recover`

## Step 1: List jobs with `candler ps`
**Goal**: `candler ps` shows each job's id, experiment, state and progress, and recovers lost ones.
**Implementation Logic**:
1. In `crates/candler/src/commands/ps.rs`, `pub fn run(args: PsArgs, out: &mut dyn Write) -> Result<ExitCode, CliError>`; S3 already placed the `Ps` variant and its dispatch arm, so this leaf edits the stub file only (not `main.rs`, `lib.rs`, `mod.rs` or any `Cargo.toml`; a missing dependency is reported as a spec defect).
2. For every directory under `<store>/jobs/`, `JobDir::open`, then `jobs::lost::classify`: `Running`, `Ended(end)` or `Lost`; the `state` field of `job.json` is never used. Progress is `runs done / runs planned` counted from `JobEvents::tail` (`run.started` and `run.ended` or `run.failed` events). Experiment comes from `JobRecord`. Output is one row per job: id, experiment, state, progress.
3. A `Lost` job is recovered once through `jobs::lost::recover(&sup, &sweep, &dir)` with the real `LabelSweeper` and a `Supervisor`, then shown as `ended`; a `recover` error is shown on the row (`lost: recover failed: <reason>`), never hidden, and the command's exit status is non-zero.
4. An unreadable job directory is a row marked `unreadable` with its path, not a skipped line.
**Deliverables**: `crates/candler/src/commands/ps.rs` (`run`, `PsArgs`, `render_rows`); `crates/candler/tests/scaffold_cli.rs` (class-A: a store with one running (held lock), one ended and one lost job renders three rows with the right states)
**Consistency Checks**: `cargo nextest run -p candler -E 'binary(scaffold_cli)' && cargo clippy -p candler --tests -- -D warnings` (expected: PASS)
**Commit**: `feat(cli): list jobs with their state and progress`

## Step 2: Stop jobs with `candler kill`
**Goal**: `kill` stops a named job or all jobs only after confirmation, through the guard-verified paths, and recovers what a dead supervisor left.
**Implementation Logic**:
1. In `commands/kill.rs`, `pub fn run(args: KillArgs, input: &mut dyn BufRead, out: &mut dyn Write) -> Result<ExitCode, CliError>` with `KillArgs {id: Option<JobId>, all: bool, yes: bool}`; exactly one of `id` and `all`. `-y` and `--yes` are the same flag and default to false.
2. Confirmation first: unless `yes`, print the job ids that would be stopped and read one line; anything but `y` or `yes`, or a closed stdin, is a refusal (non-zero exit, message `refused: pass -y to confirm`). A refusal happens before the job directories are classified for stopping, before the `Supervisor` is built and before any docker call.
3. A `Running` job: call M1's `Supervisor::reap` on a `ChildRecord` built from `job.json`'s supervisor identity (pid, pgid = pid after setsid, start time), with a `StopSpec` grace as long as job_runtime's teardown bound (D16); it verifies the record against the OS and runs through the I22 guard; no function in this leaf calls a signal API. The supervisor's SIGTERM handler (`job_runtime`) ends the job `killed`. If `job.ended` is still absent after the grace, call `jobs::lost::recover`. An `Ended` job is reported and left alone; a `Lost` job goes to `recover`.
4. With `--all`, every `Running` or `Lost` job is handled in listing order; one failure is reported and does not stop the others, and the exit status is non-zero if any failed.
5. The docker side is only what `recover` and the supervisor do, through `LabelSweeper` and the real `DockerDriver`; `kill` never runs `docker` itself.
**Deliverables**: `crates/candler/src/commands/kill.rs` (`run`, `KillArgs`, `confirm`, `stop_job`); `crates/candler/tests/scaffold_cli.rs` (class-A: confirm accepts `y` and `yes`, refuses empty input, an unknown word and a closed stdin; a lost job reaches `recover` with a fake sweeper)
**Consistency Checks**: `cargo nextest run -p candler -E 'binary(scaffold_cli)'` (expected: PASS)
**Commit**: `feat(cli): stop jobs with a confirmation and a guarded reap`

## Step 3: Follow a job with `candler watch`
**Goal**: `watch <id> --json` prints a job's events as they are appended, one JSON document per line.
**Implementation Logic**:
1. In `commands/watch.rs`, `pub fn run(args: WatchArgs, out: &mut dyn Write, stop: &AtomicBool) -> Result<ExitCode, CliError>` with `WatchArgs {id: JobId, json: bool}`; `--json` is required in M2b (a human form is out of scope and without it the command refuses).
2. It prints every event already in the log, then polls `JobEvents::tail(TailPos)` every 200 ms from the saved byte offset, writing each `Envelope` as the line it was read (events are not re-serialised), and exits zero when it reads `job.ended`, or when `classify` reports `Lost` (after one `recover`, so that the closing event is there). A closed stdout (broken pipe) ends it without an error.
3. An unknown event kind is printed as read (I18); a malformed line ends the command with the offset in the message.
**Deliverables**: `crates/candler/src/commands/watch.rs` (`run`, `WatchArgs`)
**Consistency Checks**: `cargo nextest run -p candler -E 'binary(scaffold_cli)'` (expected: PASS, with a class-A case: a writer thread appends five events and `job.ended`, and `watch` prints six lines in seq order and exits zero)
**Commit**: `feat(cli): follow a job's events with watch`

## Step 4: Prove kill refuses without confirmation, then delete the scaffolding
**Goal**: `kill --all` without `-y` makes no docker call, and the scaffolding is gone (B31).
**Implementation Logic**:
1. `crates/candler/tests/fixtures/cli/docker` is a POSIX shell script that appends its `$@` to the file named by `FAKE_DOCKER_LOG` and prints nothing, exit 0. The test puts a directory holding it first on `PATH`, creates a store with one lost job, runs the real `candler` binary (`assert_cmd`-free: `std::process::Command` on `env!("CARGO_BIN_EXE_candler")`) with stdin closed.
2. **B31** in `tests/guard_cli_kill.rs`: (a) `kill --all` without `-y`: exit status non-zero, stderr holds the refusal, the log file is empty; (b) the control, `kill --all -y`: exit status zero and the log holds the labelled `docker ps` call of the sweep. The doc has `/// Guards:`, `/// Oracle:` (the log and the exit status) and `/// Bite:` (the `yes` default becomes true).
3. Delete `tests/scaffold_cli.rs`, moving nothing that duplicates B31. Write the bite record. The bitten file is `kill.rs`, restored with `git checkout --`. Because a bite of the confirmation can run a live path, run the pre-spawn check first (`git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)'`), and run the bitten binary against the PATH-fake only (a lost job with no live record), so it can send no signal and destroy no container.
**Deliverables**: `crates/candler/tests/guard_cli_kill.rs` (the B31 test); `crates/candler/tests/fixtures/cli/docker`; the bite record in the leaf report
**Consistency Checks**: `cargo nextest run -p candler -E 'binary(guard_cli_kill)' && ! git grep --untracked -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler/src/commands crates/candler/tests` (expected: PASS)
**Commit**: `test(cli): prove kill refuses without confirmation`
