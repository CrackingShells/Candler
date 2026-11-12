# Live confirmation

**Goal**: On a model Eliott chose, session mode is confirmed once live: C1 (the agent waits, a background job's notification wakes it for a second turn, the session closes), C2 (one answer, the session closes `runner.idle_s` later), and one OpenCode run goes through Candler end to end.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [x] [nl] Eliott gave his go and his model choice for at most 5 RiVault runs (2026-11-12: Reasoning-Small), run after a RiVault health check
- [ ] [nl] OWNED: `__reports__/m4-session-runner/live_confirmation_v0.md`, the experiment files and command lines it commits under `__reports__/m4-session-runner/live/`; scratch stores under `/private/tmp/claude-501/m4-live/`
**Success Gates**:
- ⬜ [nl] C1 live: one session run records at least two turns, a notification-started turn, `session.end = closed` and `run.ended_by` `completed`
- ⬜ [nl] C2 live: one session run records one turn and `session.end = closed`, its close about `runner.idle_s` after its turn's end
- ⬜ [nl] OpenCode live: one run through `candler run` with the OpenCode driver ends with a trace read from its export, a valid integrity and a recorded `harness.version`
- ⬜ [nl] The report lists every run (model, cost as the client reports it, `session.*`, `ended_by`), every failure with its class, and nothing left behind (containers, images, networks: the lists before and after are equal)
- ⬜ [cmd] The committed command lines hold no secret value: `! git grep -n -i -E 'sk-|api[_-]?key *=' -- __reports__/m4-session-runner/live`
- ⬜ [cmd] SUBJ passes, and `/private/tmp/claude-501/m4-live` is gone
**References**: [R01 §Session runner, §OpenCode v2](../../../../../../../__design__/architecture.md); [R09 D1–D4, D11](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-approved-plan.md); [M2c's smoke](../../../../../../../__reports__/m2b-execution-parity/equivalence_smoke_report_v2.md) — the shape of a live report

## Step 1: Run the live confirmations
**Goal**: The three live runs are made and recorded, within Eliott's budget.
**Implementation Logic**:
1. RiVault health check; stop and tell the coordinator if it fails.
2. A session experiment with two cells on Eliott's model: C1's prompt (start a 40 s background job that writes outside `/tmp`, report its output when it finishes) and C2's prompt (say hello). Run it with `candler run`, the default `runner.idle_s`.
3. An OpenCode experiment with one cell (a small workspace task with one tool call) on a model OpenCode can reach through the gateway. Run it with `candler run`.
4. At most 5 runs in all, retries included. Commit the experiment files and command lines, never a token.
5. Write the report (`writing-reports`, findings). Remove what the runs created, by id.
**Deliverables**: `__reports__/m4-session-runner/live_confirmation_v0.md`; `__reports__/m4-session-runner/live/{session.toml,opencode.toml,commands.sh}`
**Consistency Checks**: `test -f __reports__/m4-session-runner/live_confirmation_v0.md` (expected: PASS)
**Commit**: `reports(runner): record the live session and opencode runs`
