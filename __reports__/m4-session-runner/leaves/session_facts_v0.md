# session_facts: what Claude Code does in a stream-json session

Probe leaf of `m4-session-runner`. Claude Code 2.1.311 (the `assets/docker/base` pin) ran under Candler's container hardening, through the production gateway sidecar, against the scripted fake model, with stdin left open. **No token was spent and no real model was called.** Step 3's optional real-model comparison was not run: it has no go from Eliott and still waits for it. Every answer below comes from the fake model alone.

## Verdict

| Question | Answer |
|:--|:--|
| **D2 branch** | The pending list comes from the **Stop hook payload**: `background_tasks` and `session_crons` are always present. Stream events cover background tasks and monitors only (`background_tasks_changed`); wake-ups and crons appear in the payload alone. Branch "neither" is not taken: no ruling from Eliott is needed before L1. |
| **D5 branch** | Hooks installed by `--settings` **load** under `--setting-sources project,local` while user settings in the per-run HOME do not. Stream-json carries hook events only with `--include-hook-events`, and without the hook's input payload. The log stays needed (see Q3). |
| Real-model comparison | Not run. Waits for Eliott's go, a model and a RiVault health check. |

## Done

| Item | Evidence |
|:--|:--|
| Fake model image, scripts `c1` and `c2` | `assets/docker/fakemodel/`; `docker build -q` passes; its image removed by id |
| Sessions c1 and c2 as fixtures | `crates/candler-harness-claude-code/tests/fixtures/sessions/{PROVENANCE.toml,c1,c2}`; each holds `stdout.jsonl`, `hooks.jsonl`, `timings.toml`, hand-written `expected.toml` |
| Gateway path | Every call went agent, sidecar (dummy token swapped for the fake's credential, checked by the fake), fake over TLS with the CA at `/run/gw/ca.pem` |

## Probe answers

Clock: container minus host = -0.83 ms, round trip 0.14 ms (40 ping-pongs over the bind mount). Times are host-side.

| Q | Question | Evidence | Decision |
|:--|:--|:--|:--|
| 1 | Event order of a turn; turn-end marker | Prompt turn: `system/init`, `assistant` (text), `assistant` (tool_use), `system/background_tasks_changed`, `system/task_started`, `user` (tool_result), `assistant`, `result`. Notification turn: `system/task_updated`, `system/task_notification`, `system/background_tasks_changed`, `system/init`, `assistant`, `result`. | The `result` event ends a turn. A turn with an error also ends with a `result` (Q7). |
| 2 | Stop and SessionEnd payloads | Stop carries `background_tasks[{id,type:"shell",status,description,command}]` and `session_crons[{id,schedule,recurring,prompt}]`. A Bash `run_in_background` job and a Monitor both appear in `background_tasks` (both `type:"shell"`, so not told apart there). ScheduleWakeup appears in `session_crons` as `recurring:false` with the wake prompt; CronCreate as `recurring:true`. SessionEnd carries `reason:"other"` and no lists. A monitor stays listed while its events start turns. | D2's fallback is not needed. A recurring cron never leaves the list, so only a cap ends such a session. |
| 3 | Hook events on stdout | `--include-hook-events` adds `system/hook_started` and `system/hook_response` for each Stop, before `result`. `hook_response` has `output`, `stdout`, `stderr`, `exit_code`, `outcome`, but not the hook's input: it is empty for a silent hook. A hook that echoes its stdin puts the payload in `output`, with no change to turns, calls or kinds. No event appeared for SessionEnd. | D5: stdout can replace the log for Stop only if the hook echoes; SessionEnd is never on stdout. Keep the log. |
| 4 | `--settings` hooks under `project,local` | Control run: the `--settings` hook fired, the project `/work/.claude/settings.json` hook fired, the `~/.claude/settings.json` (user) hook did not. | Yes, they load. |
| 5 | Notification latency; init per turn | Job end to `task_notification` on stdout: median 7.5 ms (6.1 to 9.6, n=10). Notification to the next `init`: median 88 ms; to the first `assistant`: 139 ms. `init` is emitted again at the start of every later turn (prompt-, notification- and wake-up-started alike). | Candler cannot take `init` as once per session. |
| 6 | API calls besides turns | At the fake, only `POST /v1/messages?beta=true` (streaming), one per model round trip. No `count_tokens`, quota, title or `/v1/models` call in `--print` mode, also under `acceptEdits` with `--allowedTools Bash`. At the sidecar: one `HEAD /api/hello` per process start with no credential, answered 401 by the sidecar itself (no `up_status`, so `signals_of` ignores it; M2b recorded it). Traffic to other hosts has no route on the internal network and is not observable. | Nothing else to imitate. |
| 7 | `--max-turns` and `--max-budget-usd` across notification turns | `num_turns` is per turn (2, then 1). `--max-turns 2`: the notification turn ran. `--max-turns 1`: turn 1 ended in `result/error_max_turns` with **no Stop hook**, yet the job still woke the agent and the notification turn succeeded. The budget is cumulative: `--max-budget-usd 0.002` passed turn 1 (0.0016) and stopped the notification turn (0.0024) with `error_max_budget_usd`, `is_error`, and exit 1 after EOF. | A turn cap is not a session cap: after `error_max_turns` the session goes on. Candler must count turns itself for `session.end = cap`. The budget flag counts across turns. |
| 8 | What closing stdin does | Through `docker run -i`, `--init` and `timeout`, EOF reaches `claude`: it exits 0, SessionEnd fires (`other`), median 154 ms to exit. Pending **background tasks and monitors survive EOF**: claude waited for the job (and monitor events), ran the notification turns, then exited (c1: ~20 s, monitor: ~50 s). A pending **wake-up or cron is dropped**: claude exits at once and the wake-up never fires. EOF mid-turn: the turn completes, then exit 0. | D2's rule is right in kind: closing with a wake-up or cron pending loses it. Closing with only background tasks pending is safe, since claude waits. |
| 9 | Hook-log append latency | Written to seen on the host bind mount: median 2.3 ms, max 11 ms (n=51, 1 ms polling; bare file ping-pong 0.07 ms each way). The Stop line was written before its `result` appeared on stdout in 32 of 32 turns (0.5 to 55 ms earlier). | Reading the log when `result` arrives is safe on Docker Desktop. Linux in CI not measured. |
| 10 | ScheduleWakeup, cron tools | `init.tools` lists `ScheduleWakeup`, `CronCreate`, `CronDelete`, `CronList`, `Monitor`, `Task`, `Workflow` and more. Monitor and Cron* are deferred behind `ToolSearch`. A wake-up fired at the next minute boundary after 60 s (76 s here) as a `command_lifecycle` started/completed pair (`wakeup_source:"loop_wakeup"`) around a normal turn with no `origin`. A cron fired the same way. | They exist and they work in this mode. |

I9 check: the same c1 run with and without hooks gave identical event kinds, tool calls and API calls.

## Findings

| # | Finding | Flag |
|:--|:--|:--|
| F1 | `error_max_turns` and `error_max_budget_usd` are per-turn `result` events, not session ends; no Stop hook fires on the error turn. The pending list for that moment comes only from `background_tasks_changed`. | Affects `session_runner`: count caps in Candler; read pending from stream events when no Stop line exists. |
| F2 | `origin` on `result`: absent for prompt, wake-up and cron turns; `{kind:"task-notification",producer:"session-task",runId?}` for background-task and monitor turns. | Useful to attribute turns. |
| F3 | Requests carry a `role:"system"` message after the user message; the fake skips system-role messages when it finds the last user turn. | Fake detail. |
| F4 | The fake model id is unknown to Claude Code (`unrecognized_model` on stderr); cost is priced at a default, 0.0008 per 100-in/20-out turn, `costBasis:"unknown"`. | Budget results here do not say real prices. |
| F5 | The fake image id is not reproducible across builds (the source sha256 is). `PROVENANCE.toml` records both. | Deviation noted. |
| F6 | Not imitated or not tried: thinking blocks, prompt caching, permission prompts, parallel tool calls, sub-agents, the real model's use of a notification, non-gateway telemetry. Permission mode was `bypassPermissions` (and one `acceptEdits` run). | What the real-model run would check. |
| F7 | Not reproduced in the rig: the `/opt/env` volume and the cell's settings and MCP files. The rig is scratch and not committed (the brief). | Deviation. |
| F8 | Three extra fake scripts (wake-up, monitor, cron) ran from a scratch copy to answer Q2, Q8 and Q10. They are not in the committed fake, which keeps `c1` and `c2` as the shared names. | Deviation; no shared name changed. |

No spec defect, no behaviour change, no digest moved. Nothing in Rust was touched, so STD, LINUX, DRIFT, GOLDEN and SPAWN do not apply.

## Gates

| Gate | Result |
|:--|:--|
| Fake model builds, image removed by id | PASS (`docker build -q` printed an id; removed) |
| Fixture files present | PASS (prints nothing) |
| D2 and D5 stated | PASS (this report) |
| Docker lists equal before and after | PASS (`docker images -q` 12 equal; `docker ps -aq` 3 equal; no `sf-` networks; no new volumes) |
| SUBJ (`cog check`, subjects at most 72 characters), OWNED | PASS |

## Commits

3d3e5e0 `build(runner): add a scripted fake model image`; 5db6af1 `test(adapter-claude-code): record stream-json sessions against the fake model`; the report commit `reports(runner): record the claude code session facts` follows.

Guards: none; this leaf has no guard ids.
