# Session facts

**Goal**: What Claude Code does in a stream-json session with stdin left open is recorded, and a scripted fake model drives the real binary through the production gateway sidecar without spending a token.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] Docker is up: `docker info >/dev/null`
- [ ] [nl] OWNED: `assets/docker/fakemodel/**`, `crates/candler-harness-claude-code/tests/fixtures/sessions/**`, `__reports__/m4-session-runner/leaves/session_facts_v0.md`
**Success Gates**:
- ⬜ [cmd] The fake model's image builds: `docker build -q assets/docker/fakemodel` (the image removed by its id afterwards)
- ⬜ [cmd] Each session fixture holds its stream, its hook log, its timings and its provenance: `for c in c1 c2; do d=crates/candler-harness-claude-code/tests/fixtures/sessions/$c; test -f $d/stdout.jsonl && test -f $d/hooks.jsonl && test -f $d/expected.toml || echo $c; done; test -f crates/candler-harness-claude-code/tests/fixtures/sessions/PROVENANCE.toml` prints nothing and exits 0
- ⬜ [nl] The report answers every question of Step 2 with its evidence, and states D2's branch: the pending list comes from the Stop payload, or from stream events, or from neither (then the coordinator asks Eliott before L1)
- ⬜ [nl] The report states D5's branch: hooks installed by `--settings` load under `--setting-sources project,local`, or they do not and the alternative used; and whether stream-json carries hook events
- ⬜ [cmd] Docker holds no image or container the probe made: the lists after equal those recorded before
- ⬜ [cmd] SUBJ passes
**References**: [R01 §Session runner, §Agent environment and secrets](../../__design__/architecture.md) — the end rule and the container contract; [R07](/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion/.claude/skills/htn-creator-workspace/infrastructure/__reports__/waiting_on_processes_eval/01-architecture_eval_sandbox_v0.md) — the end rule, the Stop payload, controls; [TLS fake precedent](../../crates/candler-core/tests/fixtures/gateway/) — `tls_upstream.py`, the CA path `/run/gw/ca.pem`

## Step 1: Script a fake model
**Goal**: A container serves the Anthropic Messages API over TLS from a script, so the real harness runs with no tokens.
**Implementation Logic**:
1. `assets/docker/fakemodel/`: a Dockerfile and a server using only the Python standard library, TLS from a CA generated at start (as `tls_upstream.py` does), the CA handed to the gateway sidecar at `/run/gw/ca.pem`.
2. It answers `POST /v1/messages` with streamed (SSE) and non-streamed responses, `count_tokens`, and every other path Claude Code calls (quota probe, title generation). It logs an unknown path and answers it harmlessly.
3. The script is chosen by the request's model id:
   - `c1` (must wait): turn 1 calls the Bash tool with `run_in_background` on a 20 s job that writes outside `/tmp` (which is `noexec`), then answers; when a user message carries the job's completion, it answers once more;
   - `c2` (must stop): turn 1 answers "hello".
**Deliverables**: `assets/docker/fakemodel/Dockerfile`; `assets/docker/fakemodel/fakemodel.py` (scripts `c1`, `c2`)
**Consistency Checks**: `docker build -q assets/docker/fakemodel` (expected: PASS)
**Commit**: `build(runner): add a scripted fake model image`

## Step 2: Probe a session through the gateway
**Goal**: The session protocol's facts are known for the pinned Claude Code.
**Implementation Logic**:
Run the base image's Claude Code (the version `assets/docker/base` pins) under Candler's hardening, through the production gateway sidecar, against the fake model: `--input-format stream-json --output-format stream-json --verbose`, the prompt written to stdin, stdin left open. Install observation hooks (Stop, SessionEnd, Notification) through a `--settings` file outside the workspace, each appending its payload to a log in the per-run HOME and exiting 0 with no output. Answer, with evidence:
1. the event order of a turn, and which stdout event marks a turn's end;
2. the Stop and SessionEnd payloads: do they list background tasks, monitors, crons or pending wake-ups, and under which keys;
3. whether `--include-hook-events` (or another flag) puts hook events on stdout;
4. whether `--settings` hooks load under `--setting-sources project,local`, while user settings in the per-run HOME do not;
5. the latency from the job's end to the notification's user message, and whether init is emitted again on later turns;
6. every API call the harness makes besides the turns;
7. whether `--max-turns` and `--max-budget-usd` count across notification-started turns;
8. what closing stdin does: through `docker run -i`, `--init` and `timeout`, does EOF reach `claude`, does SessionEnd fire, what is the exit status;
9. the latency of a hook-log append seen from the host bind mount (Docker Desktop here; Linux in CI);
10. whether ScheduleWakeup or cron tools exist in this mode.
Store the c1 and c2 sessions as fixtures (`stdout.jsonl`, `hooks.jsonl`, `timings.toml`, `expected.toml` by hand), with `PROVENANCE.toml` naming the harness version and the fake model's digest.
**Deliverables**: `crates/candler-harness-claude-code/tests/fixtures/sessions/{PROVENANCE.toml,c1/*,c2/*}`
**Consistency Checks**: `test -f crates/candler-harness-claude-code/tests/fixtures/sessions/c1/stdout.jsonl && test -f crates/candler-harness-claude-code/tests/fixtures/sessions/c2/stdout.jsonl` (expected: PASS)
**Commit**: `test(adapter-claude-code): record stream-json sessions against the fake model`

## Step 3: Report, and compare one real model if Eliott says go
**Goal**: The report settles D2's and D5's branches, and says whether a real model's stream differs in shape from the fake's.
**Implementation Logic**:
1. Optional, only after Eliott's go and model and a RiVault health check: one c1 run on a real model; record only the differences in shape (event kinds, hook keys), not a fixture.
2. Write the leaf report (`writing-reports`, findings): the answers, D2's branch, D5's branch, and anything the fake model could not imitate. Remove every container and image the probe made, by id.
**Deliverables**: `__reports__/m4-session-runner/leaves/session_facts_v0.md`
**Consistency Checks**: `grep -qi 'D2' __reports__/m4-session-runner/leaves/session_facts_v0.md && grep -qi 'D5' __reports__/m4-session-runner/leaves/session_facts_v0.md` (expected: PASS)
**Commit**: `reports(runner): record the claude code session facts`
