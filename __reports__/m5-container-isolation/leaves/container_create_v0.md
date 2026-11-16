# container_create: leaf report (v0)

Type: findings. Branch `m5/container_create`, cut from `feat/m5-container-isolation` at db16953.

## 1. Commits

| sha | subject |
|:--|:--|
| 72ba6ba | refactor(isolation): create the agent container before attaching to it |
| fe0c340 | test(isolation): run the agent through create and attach |

(The report commit follows these two. The `argv.json` hunk is in the first commit, ruled approved.)

## 2. start -a: what it relays (Engine 29.9.2, measured on busybox testagent)

| case | result |
|:--|:--|
| stdout and stderr bytes (binary `\000\377\001ab`, no trailing newline, `err`) | identical to `docker run -i`, byte for byte |
| exit code 3 | the client exits 3, as `run` does |
| stdin | relayed only when the container was created with `-i` AND started with `-i`; created without `-i` the echo is empty; created with `-i` but started without it the client hangs (`cat` never sees EOF) |
| SIGTERM to the client | `start -a` leaves at once (exit 0 under the supervisor, 13 ms) after proxying the signal: the agent's TERM trap runs and the container exits 0 (container log holds `got-term`), but the client does not wait for the container, so the agent's last words after the signal are NOT in the client's stdout. `docker run` waits and prints them |
| two `--network` flags on `docker create` | accepted on 29.9.2 (the create argv is the run argv with the verb changed) |

The signal exit fact is the client's: `Ended::Stopped`, `Code 0`, not `Signal`. The container's own state is the proof the signal reached the agent.

## 3. Gates

| gate | result | last line |
|:--|:--|:--|
| worktree is its own checkout | PASS | `cargo metadata` prints the worktree path |
| no cidfile, no RECORD_POLL in `crates/candler-core/src` | PASS | both `git grep` negations hold |
| `guard_container_record::create_` | PASS | 4 passed |
| SPAWN | PASS | 9 passed |
| STD (`-p candler-core -p candler`: fmt, clippy `-D warnings`, nextest, doc) | PASS | nextest 412 passed, 25 skipped (final tree) |
| LINUX | PASS | clippy `--target x86_64-unknown-linux-gnu --workspace --tests` clean |
| DIGEST | PASS | vectors numstat 0 changes; `guard_cell_digest` 9 passed; the six oracle/import guards 45 passed |
| E2E (full) | PASS | 15 tests run: 15 passed (14 existing plus the new one) |
| real image `agent_create_then_attach` | PASS | 1 passed (6.6 s) |
| SUBJ | PASS | `cog check db16953..HEAD`; subjects at most 72 characters |
| GOLDEN, DRIFT | not run | no schemas change, no import/resolve/plan change |

## 4. Guards, oracles and bites

| guard | `Guards:` line | oracle | bite and failing message |
|:--|:--|:--|:--|
| `guard_container_record::create_records_before_start` | create, then the record from the container's own inspect, then the request that attaches; create is the only call before the inspect | order of the recording fake's calls, scripted inspect as literals, the `container-record/1` schema file | record call removed from `run_request`: `left: 1, right: 2` on the call count |
| `create_failure_spawns_nothing` | a refused create leaves no record and no inspect; a container of another job or run is refused before anything is recorded | fake call list, children listing, error variants | job check removed from `agent_record`: `another job: Ok(())` |
| `create_then_crash_is_reaped` | a crash between create and start leaves a recorded container the job's sweep finds by its record alone and destroys | destroy driver calls (`stop`, `rm`), no label listing in the fake | record write removed: `left: []`, right the `stop`/`rm` pair |
| `create_collect_destroys_the_runs_own_container` | collect destroys the container the run created, scoped by the run's key; a run that created nothing destroys nothing | destroy driver calls | id lookup keyed wrong: `left: []`, right the `stop`/`rm` pair |
| `guard_harness_facts::harness_env_start_argv_equals_baseline` | start argv `docker start -a <id>` headless, `-a -i <id>` interactive | `argv.json` `headless_start` / `interactive_start` | `-i` dropped: `interactive_start left: [docker,start,-a,<id>]`; `-i` added to headless: `headless_start left: [..,-a,-i,<id>]` |
| `guard_container_e2e::create_e2e::agent_create_then_attach` | real testagent: exit 3, frames equal `docker run`'s of the same script, stdin echo, stop through the hook destroys by id, a signal reaches the agent | `docker run --rm` frames of the same script (data without times), `docker logs`/`inspect` of the id | `-i` dropped from the start: `stdin: Code { code: 143 }` |

`create_then_crash_is_reaped` is not the lost-job sweep (`sweep_lost`): that one discovers containers by label only and never reads records, so it reaps a created-but-unstarted container with or without a record. The record decides discovery for `CurrentJob` and `Run` scopes and the guard's rights; the test uses `CurrentJob`.

## 5. Claims checked

| claim | what the code showed |
|:--|:--|
| `DockerDriver::create` returns the id, `attached_request` exists | true; `create` prepends the verb, so `run_request` passes the argv without `docker create` |
| gateway `--cidfile` unused, removed | already removed by `docker_client`; no change |
| `-i` "added when the plan is interactive" | measured: needed on BOTH create and start (section 2) |
| `run.rs:185` two `--network` flags | still there, now on `docker create`; accepted by the engine |
| record reads cidfile through `record_container` | replaced: the id comes from create, held in a per-run map in `DockerIsolation` |
| collect reads the cidfile | now reads that map; the job-label mismatch refusal in `collect_and_destroy` became unreachable (a mismatching container is refused at record time) and was removed |

## 6. Findings

1. **Rulings applied.** `argv.json`: `headless_run` `run` becomes `create`, the `--cidfile` pair goes, plus the new `headless_start` and `interactive_start` entries. OWNED widened by the coordinator: `isolation/{mod,model,refusal}.rs`, test support and guards in both crates, renames `create_argv` and `build_start_request`.
2. **Further edits outside the approved list (veto-able):**
   - `isolation/docker.rs` and `isolation/driver.rs`: `DockerDriver::attach` (default: the PATH lookup of `attached_request`; `DockerCli` runs its own binary). Without it the start client always ran the PATH `docker`, so a fake-binary job host (`FakeHost`) would have run the real docker on a fake id.
   - `tests/fixtures/docker/fake-docker`: new `<verb>.early` canned output, printed before the delay.
   - `tests/it/guard_container_guard.rs`: `b10`'s collect section moved to `guard_container_record::create_collect_destroys_the_runs_own_container`.
   - `tests/it/support/{job_orchestrate,session_rig}.rs`, `guard_declared_inputs.rs`, `guard_container_run.rs`, `guard_container_e2e/{container_run_e2e,control_e2e}.rs`, `crates/candler/tests/it/{guard_cli_observe.rs,support/cli_run.rs}`: adapted to the async `run_request`, the plan's `record_dir` and the create/start verbs.
   - `assets/docker/testagent/testagent.sh`: modes `exit3`, `stream-bytes`, `echo-stdin`, `term-echo` and `timeout` (the expiry prefix reaches a mode).
3. **Recorded residual (accepted):** if create succeeds but the following inspect or record write fails, `run_request` returns an error, spawns nothing, and the container is left unrecorded; the label sweep refuses an unrecorded container, so it leaks. Same gap as before; `guard.rs` and `stop.rs` are not extended here.
4. **Behaviour change to the stop path:** the supervised `start -a` client exits on SIGTERM without waiting for the container, so output the agent prints after the signal is lost from the client's stdout (section 2). The stop hook's own `docker stop` still ends the container. For the coordination session if a harness depends on a final frame after the stop signal.
5. **Behaviour changes, none recorded or digested:** the agent's argv verb is `create`; a failed `create` is now `IsolationError::Docker` before a run starts, where a failed `run` was a child exit.
6. **Review (Sonnet, Grit):** five minor points: three fixed (doc reflow, two comment placements), one pre-existing line left, one false alarm.
7. `plugin_environment` calls `DockerIsolation::plan`/`run_request` in its e2e: `run_request` is now `async` (awaited), the coordinator adapts the call.
8. CI: add `guard_container_e2e::create_e2e::agent_create_then_attach` to the expected list.

## 7. Docker

- Before: 16 images, 3 containers, 5 networks. After: the same images and containers; one network id differs and is not mine (every network I made carried `candler.test=create_attach` and was removed by name; a leftover test container and image from two failed runs were removed by id).
- Throwaway containers of the measurement (four, then one more): removed by id.
- Pulled: none.
- e2e test, full name: `guard_container_e2e::create_e2e::agent_create_then_attach`.
