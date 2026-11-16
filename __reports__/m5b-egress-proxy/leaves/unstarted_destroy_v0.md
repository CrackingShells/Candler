# unstarted_destroy leaf report (v0, PARTIAL)

## Probe P6
Question: how is a record failure injected in `DockerIsolation::run_request` without changing
`src/`? Answer: `DockerIsolation::with_recorder` takes a `ChildRecorder`, which the test builds
over a `JobDir`; the record write goes to `JobDir::children_dir()`. Replacing that directory by a
regular file makes the write fail, and the recording fake needs nothing more. A start-request
failure needs the driver's `attach` to refuse; the fake had no such switch, so
`RecordingDocker::with_attach_refused` was added (`isolation/testing.rs`, owned by the A1 widening).
Decision: the `record` row uses the file-in-the-way fixture, the `start_request` row the new switch.

## Status
PARTIAL. Done: P6; destroy on the `record` and `start_request` rows; item 2 (B6, one stop per run); item 3 (G3, ladder of 8); item 4 (lost sweep allow networks). Item 1 done: the `session_start` row (`unstarted_destroy_session_start`, 4e1e0ff6) guards the destroy on the session-start error path; bite shown (call removed: 1 passed, 1 failed).

## Commits
`git log b6b6c986..HEAD`: P6 docs, destroy in run_request, report, `fix` stop-once/list-once/lost-networks, `test` guards, `test` lost-sweep fake docker.

## Gates
- `guard_unstarted_destroy::` 1 passed; `guard_container_stop::` 2 passed; `guard_container_guard::` 11 passed (incl. `spawn_budget_`).
- fmt PASS; `cargo clippy -p candler-core --all-targets -D warnings` PASS; `cargo doc -p candler-core` PASS; STD `nextest -p candler-core`: 308 passed.
- LINUX (last line):     Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.73s
- SUBJ: PASS
- E2E under the lock:      Summary [ 177.837s] 18 tests run: 18 passed, 453 skipped 
- SPAWN: NOT RUN.

## Guards
- `guard_container_stop::a_cancel_racing_the_deadline_stops_once` (real `ContainerStopHook` over RecordingDocker/RecordingDestroyer). Guards: R03 B6, one stop per run. Oracle: destroyer log is exactly one `stop <id>`; one outcome stored. Bite SEEN: `pre_stop` bypassing the once-cell (`stop_once`): log holds two `stop` lines. Written after the code, not seen red before.
- `guard_container_guard::spawn_budget_paths` ladder row, bound 12 -> 8. Bite SEEN: guard reuse off (`reuse = false` in sweeper): "ladder stop: 9 spawns, at most 8".
- `spawn_budget_container_created_between_hooks`: pre-stop 4 + fresh list 2 + 1 inspect of the new id + 2x(stop, rm) = at most 11; both agents destroyed. Bite (reusing the pre-stop list) NOT executed: such a list cannot name the new container, so the first assertion would fail.
- `spawn_budget_lost_sweep_allow_networks`: one ps, one `network ls`, 3 inspects, 3 rms = 8. Bite SEEN: `networks_of_job` dropped: the allow network is never removed.
- `unstarted_destroy`: rows record, start_request as before; session_start is its own test over the session rig.

## G3 arithmetic
Before 12 = pre-stop (ps 1, inspect_many 1, verify 1, stop 1 = 4) + destroy hook (of_run 2, last inspect 1, sweeper list 2, verify 1, stop 1, rm 1 = 8). Now 8 = 4 + (fresh list 2, stop 1, rm 1): the hook lists once through `ps_inspect_by_label`, takes its last inspect from that answer, hands the list to `LabelSweeper::sweep_listed`, and the guard (`destroy_many_reusing`) reads the facts of ids the pre-stop verified from its memory (dropped on rm), inspecting new ids in one call. Measured on the PATH-fake docker: 8 (bite run: 9).

## Claims checked
- Plan says `destroy_unstarted` is missing: it exists in `runner/exec/mod.rs` and covers the
  `start_attempt` and wait-error paths; only the session-start path lacked it (now added and guarded).
- The destroy verifies by the guard's own inspect (one `inspect`); `collect_and_destroy` body was
  extracted to `destroy_cid` and reused.

## Findings
1. DONE, session_start row: `execute_controlled` takes any `Isolation`, so a wrapper whose plan sets `interactive = false` leaves the child without a stdin pipe and `SessionRun::start` fails with no new seam and no docker; the wrapper logs each destroy by run key (one, the run's).
2. B6 defect (by reading): `wait_or_cancel` calls `RunStopper::request` once per cancel, but the
   supervisor's deadline ladder runs `ContainerStopHook::pre_stop` -> `stop_agent`, a second daemon
   stop when both race. Fix belongs in `ContainerStopHook::stop_agent` (single flight per run), and the
   row must drive the real hook, not `FakeDaemon` (which would count two by construction).
3. G3 arithmetic, from the code: ladder = pre-stop (ps 1 + inspect_many 1 + verify 1 + stop 1 = 4)
   + destroy hook (of_run 2 + last inspect 1 + sweeper list 2 + verify 1 + stop 1 + rm 1 = 8) = 12.
   Target 8 = 4 + (one list 2, stop 1, rm 1): the hook must list once, take the last inspect from that
   list's inspect_many answer, skip re-verifying ids the pre-stop verified, and call the guard with
   those facts. This needs a facts cache shared by the two hooks in `ContainerGuard`, and the
   sweeper's own list removed from the run scope; 8 looks reachable, not built.
4. Lost sweep: `sweeper.rs::collect` destroys only `for_job` and `egress_for_job` networks for
   `SweepScope::Lost`; allow networks need one driver call listing networks by the job label.
   Lost-sweep budget would gain exactly one spawn (network ls). Not built.
5. Out-of-OWNED but within the widened line: `isolation/testing.rs` gained `with_attach_refused`.
6. The destroy's own failure is not reported with the original error (no composite variant exists in
   `IsolationError`); it stays in the guard's events and the job-close sweep.

7. Out of OWNED: `isolation/driver.rs` (two default methods `ps_inspect_by_label`, `networks_of_job`), `isolation/docker.rs` (their DockerCli bodies; `ps_by_label` now delegates), `isolation/model.rs` (`NetworkName::listed`), `tests/it/guard_job_runtime.rs` (its fake docker answered every `network` call with an error, so a lost sweep's new `network ls` turned a Recovered job Incomplete; `network ls` now answers empty). A failed `network ls` makes a lost sweep unknown.
8. Cancel/deadline race: the verdict of the first stop is told to every later asker; a first failure is therefore not retried by the second asker.

## Docker
Before/after (images/containers/networks): same/same/changed. The real-daemon run was the E2E gate under the lock; nothing else created Docker objects.
