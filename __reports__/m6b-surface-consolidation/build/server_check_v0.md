# server_check build report

## First reads
- Job start: `run_job` (`jobs/orchestrate/mod.rs`) plans, opens the session, then runs `Scheduler::run`. A refusal there is `refused(what, error)`: one stderr line and a failed outcome with no registry code.
- A run's container was composed inline in `drive` (`runner/exec/mod.rs`). It is now `compose_container` (isolation plan, harness env, input mounts; writes no run record), called by `drive` and by the check (`jobs/orchestrate/servers.rs`, `probe_group`).
- Servers: `candler_harness::mcp::servers(&Cell)` gives the `mcp.servers` table; `url` is remote, `command`/`args`/`env` is stdio.
- Doubles: none started a command. `Isolation::start_probe` (default: Unavailable) and `ProbingIsolation` (`isolation/testing.rs`) are new.
- Design: the check groups the plans by (image, declared servers), probes each group once before `Scheduler::run` on a scratch workspace under the job's scratch directory (removed after), with `interactive = true` so stdin stays open. Docker impl: `docker create -i` (command wrapped in `sh -c 'exec "$@" 2>&1'`), `docker start`, wait `SERVER_START_WAIT`, inspect `State.Running`, `docker logs` (tail of 4096 bytes), destroy through the guard by id on both paths.
- Changed existing guards: none. The refactor left the runner and `guard_job_orchestrate` guards green unchanged.

## Bites
- Check not called before the first run: exits-refuses and stays-up red. Reverted.
- Refusal drops the output (`output: String::new()`): `server_that_exits_refuses_the_job_with_its_stderr` red. Reverted.
- Stdin closed (`interactive = false`): `server_that_stays_up_lets_the_job_start` red (the double's server exits at end of input). Reverted.
- Remote servers started (url used as the command): `remote_server_is_skipped` red. Reverted.
- Real-container bite (skip the destroy) was not made; the Running case shows the held-open stdin against Docker.
- `job_without_stdio_servers_starts_no_check` passes without a check; it bites only against a check that probes with no stdio server.

## D3
1. `server_that_exits_refuses_the_job_with_its_stderr` (guard). Q: does a server that exited stop the job before any run, with its output in the refusal? Oracle: literal `boom: no module named x`; the job runs in a child of the test binary so the stderr line is read. Fails when: the check runs late, or drops the output. Lives while the refusal text is user-facing.
2. `server_that_stays_up_lets_the_job_start` (guard). Q: does a running server leave the job alone, with stdin held open? Oracle: the one probed command and one run request. Fails when: the check refuses a running server or closes stdin. Lives with the check.
3. `remote_server_is_skipped` (guard). Q: is a `url` server never started? Oracle: no probed command. Fails when: remote servers are probed. Lives with the check.
4. `job_without_stdio_servers_starts_no_check` (guard). Q: does a job with no stdio server pay for no probe? Oracle: no probed command, one run. Fails when: the check probes with nothing declared. Lives with the check.
5. `server_that_exits_refuses_the_job_in_docker` (confirm, ignored; also a Running case whose command exits at end of input). Q: does the Docker impl see an exited command and its output in a real container set up as a run's, and remove it by id? Oracle: literal `server-boom`; `docker inspect` of every id that carried the label. Fails when: the wrapper, start, inspect or logs are wrong, or the destroy is skipped. Kept as the only real-engine case.

## Verifier
Verifier: fresh Sonnet agent, brief verbatim. Findings and dispositions:
1. The probe container lacks the harness overlay (`prepared.env_overlay`, the driver's overlay), the prepared files and skill files that `drive` writes before composing: DEFECT, reported to the coordinator, not fixed (needs `driver.plan` and file writes into the scratch, beyond A2's extraction). Network, mounts and plan env are the run's through `compose_container`.
2. The real-container test covered only an exit: FIXED. It now also probes `sleep 30` and requires Running, against a real `create -i` plus detached start. The test calls `start_probe`, not `check_servers`; the job-level path is covered by the double guards.
3. Racy poll and leak: FIXED for the poll (it runs until the probes finish, and requires two containers). A failing run still leaves what `World` does not remove; recorded.
4. Destroy result dropped, cancel not observed, serial probes: follow-up.
5. Needs `sh` in the image, and refuses an image without it ("could not run"): follow-up.
6. Grouping omits inputs and env (follows the leaf's "distinct pair"): follow-up.
7. Refusal is a supervisor stderr line, not a typed record: accepted by the coordinator.
8. Silent skips of non-string `command`, `args` and `env`: follow-up.
9. Guards not seen red first: accepted, recorded; all four bites made after.
10. The double reads `plan.interactive`, which the check sets: the Docker side is covered by the real-container Running case.
11. `compose_container` is at `crates/candler-core/src/runner/exec/mod.rs:146`, called by `drive` and by `probe_group` (`jobs/orchestrate/servers.rs`).
Re-check by the same verifier: findings 3 and 11 closed; 2 closed after the Running case was changed to `read line || exit 3; sleep 30`, which exits if stdin is closed; finding 1 stays open as a defect for the coordinator; no new finding.

## Follow-ups
- No `codes.toml` row: a pre-run refusal uses `refused()` with no registry code, as the tip does; a coded variant would need `jobs/refusal.rs`.
- The `sh -c 'exec "$@" 2>&1'` wrapper assumes `sh` in the image, because the `capture` seam returns stdout only.
- The guards build their experiment TOML by hand (`support/` is not owned) and read the refusal line from a child process.
- The guards were written together with the check, not seen red first; their bites are above.
- The real-container test uses the testagent image through `TestImage`, which removes it on drop; a failed test leaves the image and network (removed by id during this build).
- The probe's destroy result is dropped, a cancel during the probes is not observed, and servers are probed serially (up to `SERVER_START_WAIT` per server and group).
- The grouping key is (image, declared servers): cells with the same pair but other inputs or env are not probed.
- A server whose `command` is not a string, and non-string `args` or `env` values, are skipped silently.
- Open defect: the probe container lacks the harness env overlay and the prepared and skill files a run has (verifier finding 1).
