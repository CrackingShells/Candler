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
PARTIAL (turn cap). Done: step 1 (P6), step 2 for the `record` and `start_request` rows. Not done:
the `session_start` row, step 3 (B6 row), G3 and the lost-sweep allow networks.

## Commits
See `git log b6b6c986..HEAD`: docs P6, fix destroy, this report.

## Gates
- `guard_unstarted_destroy::` PASS (1 test). Suite filter guard_container_record, guard_container_stop,
  guard_container_guard, run_exec with it: 20 passed.
- `cargo clippy -p candler-core --all-targets -- -D warnings`: PASS. SPAWN, LINUX, E2E, SUBJ, DIGEST: not run.
- G3 (`guard_container_guard::spawn_budget_`) and the lost-sweep gate: NOT DONE.

## Guards
`guard_unstarted_destroy::unstarted_destroy` (rows record, start_request). Guards: R03 B2 a created
container whose record or start request fails is destroyed by its id through the guard. Oracle: hand
derived call list (one create, stop and rm of the returned id). Bite: the destroy dropped from the
error arm of `run_request`; failing message: "record: the created container is destroyed by its id:
... left: [] right: [stop <id>, rm <id>]". Written green with the code, not seen red first.

## Claims checked
- Plan says `destroy_unstarted` is missing: it exists in `runner/exec/mod.rs` and covers the
  `start_attempt` and wait-error paths; only the session-start path lacked it (now added, unguarded).
- The destroy verifies by the guard's own inspect (one `inspect`); `collect_and_destroy` body was
  extracted to `destroy_cid` and reused.

## Findings
1. Spec defect, session_start row: needs a full `drive` runtime (session_rig) to make `SessionRun::start`
   fail; the one-line fix in `runner/exec/mod.rs` has no guard yet. Needs a ruling or a larger budget.
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

## Docker
No real-daemon run; nothing created. Images, containers, networks unchanged. No e2e added.
