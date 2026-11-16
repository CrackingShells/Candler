# fixB leaf (v0): findings

Branch `m5/fixB`, base b6d5f339. Nothing pushed or merged.

## Commits
| subject | item |
|---|---|
| `test(runner): pin a stop that races the agent's own exit` | 1, 2 |
| `test(isolation): prove an evaluator-only input stays out of the agent` | 3 |
| `fix(isolation): destroy the agent container when its client cannot start` | 4 |
| `docs(design): record how a stop that races an exit is classified` | 5 |
| `style(isolation): add the missing Grit annotations` | 6 |

`cargo check --all-targets -p candler-core -p candler` passes on each of the five.

## Gates
| gate | result |
|---|---|
| SPAWN | PASS (guard_signal, after `cargo clean -p candler-core`) |
| fmt, clippy `-p candler-core -p candler`, doc | PASS |
| LINUX | PASS |
| STD (nextest, both crates) | PASS |
| DIGEST, GOLDEN | PASS |
| SUBJ | PASS |
| E2E | PASS on re-run: core 16/16, cli 3/3. First run: `controls_e2e_pinned_harness_passes` failed on "c1 closed 2989 ms ... not within 3000 to 4000 ms" (a timing margin on a loaded machine, not touched by this round). |
| OWNED | PASS: only the files the items name plus this report |

SPAWN ran once before the gate batch, not before the first two guard runs (those spawn only the fake client).

## Guards and bites
| guard | oracle | bite and failing message |
|---|---|---|
| `guard_container_record::stop_that_races_the_clients_own_exit_is_still_a_stop` (rows: BudgetStop, Killed) | the stop causes' own ends; a fake client that exits 0 by itself two seconds after the cancel, inside the 5 s relay | `stopped = cause.filter(\|_\| exit.ended == Ended::Stopped)`: `BudgetStop: the reason  left: Some(Infrastructure)  right: Some(BudgetStop)` |
| `guard_container_record::agent_container_is_destroyed_when_its_client_cannot_start` | fake client's `container rm` count: run's destroy plus the job-close sweep (2) against the sweep alone (1) | destroy removed from the spawn-failure path: `the run's own destroy and the job-close sweep's both reached the client  left: 1  right: 2` |
| `plugin_env_e2e::plugin_environment_serves_live_mcp_tools` (extended) | evaluator input declared with `InputAudience::Evaluators`; client looks for the canary by name and bytes | audience set to Agent: red at `the evaluator-only input is declared and resolved  left: 0  right: 1` |

Bite caveat for item 3: the Agent-audience bite is caught by the new resolution assertion, before the in-container check runs. The in-container canary assertion under that bite was not observed (one more e2e run would show it).

## Claims checked
- Item 1: no existing guard pinned `drive`'s line (`runner/exec/mod.rs:745` before my edit). `guard_run_exec` tables `classify_end` only, with the stop as an input.
- Item 1, deadline row: not built. A deadline with a cancel in the rig collides with the job's own wall cap, which is itself a BudgetStop. The deadline half stays pinned in `classify_end` rows only; the `!= Ended::Deadline` part of the filter has no integration guard.
- Item 2: there is no local/container `runner.mode`. `runner.mode` is `headless` or `session` (`resolve/steps/session.rs`); every run goes through `drive` (`exec_run`, only caller `jobs/orchestrate/schedule.rs:215` and `controls.rs:797`) and `require_isolation` refuses a run with no isolation (`runner/exec/spawn.rs`, "never falls back to the host"). The line and `classify_end` are therefore shared; no mode rows exist or are possible. The guard runs the headless path.
- Item 3: `Fragment::load` forces `InputAudience::Agent` (`resolve/steps/plugin_environment.rs:273`), so the fixture cannot declare an evaluator input. The test declares it the way an experiment does (an `ExperimentInput` of audience Evaluators passed as a second group to `resolve_input_groups`, against the plug-in directory). No src change and no fixture change were needed; the fixture already held `evaluator/canary.txt`.
- Item 4: the fix is a few lines: `destroy_unstarted` calls `collect_and_destroy` (which ignores its `Exit`) on the `start_attempt` error and on the `waited` error. The session-start error path (`child.stop()` then return, ~line 551) has the same gap and is not changed or guarded.
- Item 6: `.ok().flatten()` in `image.rs::fragment_of` and `run.rs::selected_fragment` is deliberate (the resolver's plug-in environment step refuses with its own code), so both got EXPLICIT, not propagation. `spawn.rs` detached `tokio::spawn` now `drop(...)` with EXPLICIT. No further violations found in the touched files.

## Findings
- Behaviour change: none to a released shape; item 4 destroys earlier than before on an error path.
- Request: decide whether the session-start error path should also destroy (same helper).
- Request: decide whether a deadline-with-cancel integration row is wanted (needs a job wall cap that does not itself stop the job).

## Docker
Images, containers and networks before and after differ (one image id, one container `83150c9db419`, one network id gone). My tests remove by exact id and assert it; I did not remove those ids, so other sessions' activity is the likely cause, but I cannot attribute it. Pulls: none. E2E tests run: all of `guard_container_e2e::` (16) and `guard_cli_e2e::` (3), including `guard_container_e2e::plugin_env_e2e::plugin_environment_serves_live_mcp_tools`; no new e2e test names.
