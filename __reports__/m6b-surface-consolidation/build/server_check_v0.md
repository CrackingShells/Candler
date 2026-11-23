# server_check build report (partial)

## First reads
- Job start: `run_job` in `jobs/orchestrate/mod.rs` plans, opens the session, then runs `Scheduler::run`. A refusal there is `refused(what, error)`: one stderr line and a failed outcome. It carries no registry code, so `codes.toml` gained no row (a new `OrchestrateError` variant would need `jobs/refusal.rs`, which is not owned).
- A run's container was composed inline in `drive` (`runner/exec/mod.rs`). It is now `compose_container` (isolation plan, harness env, input mounts), called by `drive` and by the check.
- Servers: `candler_harness::mcp::servers(&Cell)` gives the `mcp.servers` table; a server with `url` is remote, one with `command`/`args`/`env` is stdio. The check reads it from each cell's comparable values.
- Double: none existed that starts a command. `Isolation::start_probe` (default: Unavailable) and `ProbingIsolation` in `isolation/testing.rs` are new.
- Design: the check groups the plans by (image, declared servers) and probes each group once, before `Scheduler::run`, on a scratch workspace under the job's scratch dir that it removes. Docker impl: create (with `-i`, command wrapped in `sh -c 'exec "$@" 2>&1'`), `docker start`, wait `SERVER_START_WAIT`, inspect `State.Running`, `docker logs`, then destroy through the guard by id.

## Bites
- Check not called before the first run: `server_that_exits_refuses_the_job_with_its_stderr` and `server_that_stays_up_lets_the_job_start` red (a run starts, nothing probed). Reverted.
- Not made: stdin closed, remote started, stderr dropped. `remote_server_is_skipped` and `job_without_stdio_servers_starts_no_check` pass without a check, so they only bite against a check that starts those servers.

## D3
Not written.

## Verifier
Not dispatched (call cap).

## Follow-ups
- Left: the real-container test `server_that_exits_refuses_the_job_in_docker`, the remaining bites, the D3 answers, the verifier.
- The guards assert the probed command and run count; the refusal text (key and output) is not asserted yet because `refused` prints to stderr only.
- Guards were written with the check, not seen red first.
- `guard_server_check.rs` composes the experiment TOML by hand because `support/` is not owned.
