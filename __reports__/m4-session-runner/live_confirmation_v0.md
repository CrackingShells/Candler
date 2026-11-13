# M4 live confirmation: findings (v0)

Date: 2026-11-13

---
type: findings
topic: m4-session-runner
date: 2026-11-13
version: v0
prior-version: none
key-metric: live confirmations passed: 4 of 4 (prior: N/A, delta: N/A)
decision-required: confirm
---

## Headline Result

metric: live confirmations passed (session C1, C2, C3 and one OpenCode run through `candler run`)
value: 4 of 4 (session C1, C2, C3 and one OpenCode run through `candler run`, all valid)
unit: confirmations
prior: N/A (first run; 3 of 4 before F1 and F2 were fixed)
direction: new

Session mode holds on a real model: C1, C2 and C3 are valid, `ended_by = completed`, `session.end = closed`, and each closed 60.15 to 60.16 s after its last turn ended (`runner.idle_s` 60). The OpenCode run, made after F1 and F2 were fixed and merged (73f2913, 5982a0c; tip 39ef391), is valid, `ended_by = completed`, its trace read from the export, `harness.version` 2.2.7. Model `RiVault/Reasoning-Small`. 4 of 5 runs spent; run 5 untouched. An earlier OpenCode attempt died on an environment fault before the model (F4) and is not counted.

## Results Tables

### Runs (one job, `01a1d08a-a12a-71b5-9f48-0f8a7ce79b46`, Claude Code 2.1.311, `runner.mode = session`, default idle)

| Cell | Integrity | `run.ended_by` | `session.turns` | `run.num_turns` | `session.end` | `close_forced` | `pending_at_end` | Cost (client-side `run.cost_usd`) |
|:--|:--|:--|:--:|:--:|:--|:--|:--|--:|
| C1: 40 s job, report its file | valid | completed, exit 0 | 2 | 4 | closed | false | [] | 0.0666 USD |
| C2: say hello | valid | completed, exit 0 | 1 | 1 | closed | false | [] | 0.0450 USD |
| C3: two 20 s jobs, long answer | valid | completed, exit 0 | 3 | 7 | closed | false | [] | 0.0740 USD |

### OpenCode run (job `01a1d0c3-fa72-7e80-a23b-d9c191262e8c`, OpenCode 2.2.7 through the gateway, scratch root under `$HOME`)

| Cell | Integrity | `run.ended_by` | Turns | `harness.version` | Trace source | Cost (client-side) | Tool calls |
|:--|:--|:--|:--:|:--|:--|--:|:--|
| OC: one shell call writing `oc_out.txt`, reply `done` | valid | completed, exit 0, 7.5 s | 2 assistant steps (`tool-calls`, then `stop`) | 2.2.7 (`comparable.harness`, session `harness_version`) | export: reader `opencode-export@0.1.0`, raw streams `stdout stderr export gateway_census resolved` | 0 USD (`info.cost` 0; tokens in 6,984, out 449, cache read 4,704; no price from the endpoint) | `shell` 1; the file holds `oc-live`; `census.source` gateway |

The stream held two `retry` events: `provider.rate-limit`, HTTP 429 from the endpoint (attempts 2 and 3, about 1.6 s and 3.6 s apart), which OpenCode retried itself before the call succeeded. The reader and the run were unaffected.

Total client-side cost of the session cells 0.1856 USD; with OpenCode (0 reported) the four runs total 0.1856 USD. The endpoint reports no price; the figure is Claude Code's own `total_cost_usd`, cumulative over the session.

### Close timing

| Cell | Last turn ended (`yielded_at_ms`) | Run duration | Close after last turn | Required band (`idle_s` to `idle_s` + 1 s) |
|:--|--:|--:|--:|:--|
| C1 | 44,319 ms | 104,478 ms | 60,159 ms | 60,000 to 61,000 ms: inside |
| C2 | 1,439 ms | 61,585 ms | 60,146 ms | inside |
| C3 | 32,089 ms | 92,247 ms | 60,158 ms | inside |

### Turns, from the raw stream (`result` events and their `origin`)

| Cell | Turn 1 | Turn 2 | Turn 3 |
|:--|:--|:--|:--|
| C1 | prompt: starts the job, `task_started`, result 2 API turns | `origin = task-notification`: reads `c1_out.txt` (content `c1-finished`), result 2 | none |
| C2 | prompt: "Hello!", result 1 | none | none |
| C3 | prompt: two `task_started`, result 3, 263 characters | `task-notification`: "still waiting on the second", result 1, 105 characters | `task-notification`: long final answer, 5,158 characters, result 3 |

### Spend, refusals and what they cost

| Event | Model calls | Runs counted |
|:--|:--:|:--:|
| Job 1: three cells, each `run.failed isolation_failed` at 0.1 s (F3) | 0 | 0 |
| Jobs 2 to 8: refused at job start, `jobs.session_controls_failed` (F4) | 0 | 0 |
| Job 9: the three cells above | yes | 3 |
| OpenCode job `01a1d0c3-...`: one cell | yes | 1 |
| `candler run opencode.toml` on the pre-fix tip: refused at preflight (F1) | 0 | 0 |
| `candler run opencode.toml` on 39ef391: preflight passes (`harness.version` 2.2.7), the agent container never starts: `docker run` exit 125 (F4 again) | 0 | 0 |

### Docker

| List | Before | After |
|:--|:--|:--|
| `docker ps -aq` | 3 ids | equal |
| `docker volume ls -q` | 33 ids | equal |
| `docker network ls -q` | 5 ids | equal except the built-in `bridge`, `cce2c9dbb49b` now `3bab0778bc55` (Docker's own, as `opencode_facts` F9 found) |
| `docker images -q` | 12 ids | equal except `08dec1d8ad53` (the `candler-env:e3b0c442...` image), now `c7550f010b90` under the same tag (F2) |

Created by the runs and removed by exact id: `candler-fakemodel:src-ba47615a90dd` (4191538ec4e1), `candler-gateway:latest` (bccc534a311a), `candler-base:src-7d118552da5c` (763c9a2edc15, the OpenCode base built by the preflight; its layers came from the BuildKit cache, and no download was observed or asked for). Containers and networks of the job were destroyed by the job itself; the lists show none left. No prune, kill or stop was used.

### OpenCode attempts on the fixed tip (39ef391)

| Attempt | Scratch root | Result |
|:--|:--|:--|
| 1: job `01a1d0b3-6ef7-720b-a423-e091fa7c6905` | `/private/tmp/claude-501/m4-live`, freshly created | `run.ended` 60 ms after start, exit 125, `ended_by` unknown, `invalid(infrastructure)`; stderr stream: `invalid mount config for type "bind": bind source path does not exist: /host_mnt/private/tmp/.../workspace`. No model call, not counted. |
| 2: job `01a1d0c3-fa72-7e80-a23b-d9c191262e8c` | `$HOME/Documents/tmp/m4-live` | valid run, table above. Counted as run 4. |

Mount-check log (a new nested directory made each time like a run workspace, then `docker run --rm --entrypoint true --mount type=bind,source=<dir>,target=/x candler-base:src-319fda768a95`; local times):

| Root | Attempts | Outcome |
|:--|:--|:--|
| `/private/tmp/claude-501/m4-live` | 45, every 20 s, 02:02:08 to 02:16:52 | all 45 failed; gave up at 02:17:12 (15 minutes) |
| `$HOME/Documents/tmp/m4-live` | 1, 02:17:29 | passed; the run followed at once |

Docker before and after: images, containers, volumes and networks equal after removing, by exact id, the three images the run built: `candler-base:src-7d118552da5c` (7b1ee6e60337), `candler-env:e3b0c442...-src-7d118552da5c` (cd6a41c38458), `candler-gateway:latest` (e8e1cbd93d08). The earlier attempt's three images (c2bb5468b58d, 796d620b4b15, 97ce0ad79a09) were removed too. The fakemodel image is not used by a headless job.

## Observations

| Signal | Baseline / Expected | Observed [source] | Interpretation |
|:--|:--|:--|:--|
| Notification turn (C1) | at least 2 turns, one started by a notification | 2 turns; turn 2 has `origin.kind = task-notification` [source: raw `stdout`, manifest] | the recorded fixtures hold on a real model |
| Close after last turn (C1 to C3) | `idle_s` to `idle_s` + 1 s (control verdict band) | 60.146 to 60.159 s [source: manifest `run.duration_ms`, `session.yielded_at_ms`] | no early or late close; the close rule held on a model that thinks and writes long text |
| C3 coalescing | two jobs finishing together, notifications possibly coalesced | not coalesced: two separate notification turns, 3 turns in all [source: raw `stdout`, `hooks.jsonl`] | the case the fake fixtures lacked is a multi-turn case; the tracker covered each turn with its Stop line (`pending_at_end` empty) |
| Close between C3's turns 2 and 3 | no close while a notification is pending or racing | none; turn 3 started after turn 2 ended and the session closed only after turn 3 [source: Stop lines at 22.8 s and 32.1 s] | correct |
| Long final message (C3) | closes only after the 5,158-character turn | closed 60.158 s after it | correct |
| `session.stop_text_mismatches` | the brief names it as a diagnostic | not recorded: `session_runner` fix round 3 removed it, and no such path is in the manifest [source: manifest measured paths, session_runner report] | nothing to report; the rule has no text gate |
| `session.control` | present on a session run | present, `controls/319fda768a95-f004aca897dca152.json` [source: manifest] | the control gate ran before the real cells and passed |
| OpenCode preflight | version probe reads `opencode v2.2.7` | exit 1 before the fix; `harness.version` 2.2.7 on 39ef391 [source: preflight output, manifest] | F1, fixed |
| OpenCode trace | read from the export, valid integrity | reader `opencode-export@0.1.0`, `integrity` valid, session `ended_by` completed, one `shell` call [source: manifest, `trace.jsonl`] | the driver works end to end on a real model through the gateway |
| Provider 429s | none expected | two `provider.rate-limit` retry events, then success [source: raw `stdout`] | the endpoint throttled for about 5 s; OpenCode's own retry absorbed it |

## Charts & Visualizations

Close after each session's last turn, against the required band (text chart, ms after the turn's end):

```
         60,000        60,500        61,000   (band: idle_s .. idle_s + 1 s)
C1 close      |   *          |             |      60,159
C2 close      |  *           |             |      60,146
C3 close      |   *          |             |      60,158
```

Caption: every close is within 0.16 s of `runner.idle_s`, far inside the 1 s upper bound.

## Contradictions & Surprises

- F1 (fixed, 73f2913): the OpenCode driver, recorded green against the fake model, could not start a live `candler run`; its probe needs a writable home and the control container is read-only.
- F2 (fixed, 5982a0c): the env image tag `candler-env:<lock>` was shared by both harnesses; resolving the OpenCode experiment retagged the Claude Code env image on the OpenCode base.
- F4: seven session jobs in a row were refused by the control gate for a Docker mount fault, and the refusal text does not say so; the same fault then killed the OpenCode run at `docker run`. It appeared on a root freshly created under `/private/tmp` that did not clear in 15 minutes; a root under `$HOME` worked at once.

## Steering Questions

- [now] Confirm closing the leaf: session mode and the OpenCode driver are both confirmed live on Reasoning-Small, 4 of 5 runs spent; run 5 untouched.
- [done] F1 and F2 are fixed and merged (73f2913, 5982a0c).
- [next run] F4: should `jobs.session_controls_failed` name the control run's invalid reason and its stderr tail? The cause here took a snapshot of the control's scratch store to find.
- [next run] F3: should a treatment whose repository holds no `.claude/skills/<name>` be refused at resolve instead of failing every run with `isolation_failed`?
- [later] C3 did not coalesce its two notifications on this model. A case where the agent itself reads both at once (one turn, `num_turns` 0 shape) is still unrecorded live.

## Findings detail

| # | Finding | Class | Evidence | Flag |
|:--|:--|:--|:--|:--|
| F1 | **Fixed** (73f2913, tip 39ef391). `candler run opencode.toml` was refused: `image_id@1 ... the harness version cannot be read: opencode (probe opencode --version): the probe exited with status 1`, plus two follow-on step errors. The probe runs under `timeout 10` in a `--read-only`, `--network none`, `--cap-drop ALL` container with no writable home. OpenCode creates `~/.local/share/opencode/log` at start and dies: `PlatformError: Unknown: FileSystem.makeDirectory (/home/agent/.local/share/opencode/log)`. The same image prints `opencode v2.2.7` when the root is writable. | Candler defect (probe container versus the driver's needs; not model, not RiVault) | the control argv of `control_argv_with` run by hand on the OpenCode base: the `PlatformError` above; the preflight line, twice; `docker run --entrypoint opencode <image> --version` (writable): `opencode v2.2.7` | **Finding**; run not made, no retry |
| F2 | **Fixed** (5982a0c). `candler-env:e3b0c442...` was keyed by the lock only. The OpenCode preflight built the env on `candler-base:src-7d118552da5c` and moved the tag; its label `candler.cc_version` read 2.2.7 and its layers held `/usr/local/bin/opencode`. A later Claude Code resolve rebuilt it. Every resolve built a new image id (08dec1d8ad53, b64b355fb919, c7550f010b90 for the same inputs). | Candler defect or design gap | `docker history`, `docker image inspect` labels, `isolation.image` printed by `candler resolve` | **Finding** |
| F3 | A treatment with no `.claude/skills/<name>` in its repository: three `run.failed isolation_failed` events within 0.1 s, the `skill-archive` process exiting 128; no message names the missing path. My first treatment repository was the cause. | My setup, plus an opaque failure | events 311 to 322 of job `01a1d084-...` | Finding (diagnosability) |
| F4 | `jobs.session_controls_failed`: `the run of c1 was not valid and completed (valid: false, completed: false)`. The control run exited 125 after 46 ms: `docker: Error response from daemon: invalid mount config for type "bind": bind source path does not exist: /host_mnt/private/tmp/.../workspace`. Directories created under the scratch root were invisible to Docker Desktop's file share (`docker run --mount` on a fresh directory failed 6 times over 30 s, while `-v`, which creates a missing source, hid it). It cleared by itself after about nine minutes. Job 1 had passed the same controls before I removed and recreated the scratch store. It recurred on a freshly created scratch root for the OpenCode attempt: 45 mount checks over 15 minutes all failed, and one check under `$HOME` passed. The run's 60 ms `invalid(infrastructure)` was classified correctly, but neither the job output nor the refusal named the cause; only the run's `stderr` raw stream did. | Environment (Docker Desktop file sharing), with Candler's message hiding it | snapshot of the control's scratch run: `manifest.json` (`run.exit` 125) and its `stderr` raw stream | Finding (diagnosability); 7 refusals, 0 model calls |

## Pointers

- Experiment files and command lines: `__reports__/m4-session-runner/live/session.toml`, `opencode.toml`, `commands.sh`
- Leaf: `__roadmap__/m4-session-runner/traits/core/runs/closeout/revalidation/live_confirmation.md`
- Facts: `__reports__/m4-session-runner/leaves/session_facts_v0.md`, `session_runner_v0.md`, `session_controls_v0.md`, `opencode_facts_v0.md`, `opencode_driver_v0.md`
- Shape of a live report: `__reports__/m2b-execution-parity/equivalence_smoke_report_v2.md`
