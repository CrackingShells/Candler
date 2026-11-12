# Session runner

**Goal**: A cell with `runner.mode = "session"` runs its harness with stdin open, closes the input only once the agent has yielded with nothing pending and `runner.idle_s` has passed (D1–D3), records how the agent stopped, and leaves every headless cell, command line and digest as it was.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] D2's branch is settled (`session_facts_v0.md`, or Eliott's ruling): where the pending list comes from
- [ ] [nl] OWNED: `crates/candler-core/src/experiment/paths.rs`, `crates/candler-core/src/resolve/steps/session.rs` (new) and its registration in `resolve/steps/mod.rs`, `crates/candler-core/src/isolation/{mod,model}.rs` (`ContainerPlan.interactive`), `crates/candler-core/src/isolation/container/run.rs` (the `-i` hunk), `crates/candler-core/src/runner/exec/{session.rs (new),ends.rs}`, `crates/candler-core/src/runner/evidence/**` (`session.*`); `crates/candler-core/tests/oracle/vectors/field_paths.toml` (rows added only), `crates/candler-core/tests/guard_resolve_surface.rs` (the count); new tests. Shared hunks, merged by the coordinator: `runner/exec/mod.rs`, `isolation/container/mod.rs` (with `opencode_wiring`)
**Success Gates**:
- ⬜ [cmd] S1 (stub replay of the `session_facts` fixtures, fake docker, injected clock): a notification starts turn 2; the input closes `idle_s` after the last turn's end, never mid-turn and never while anything is pending; `idle_s = none` runs to the cap: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^session_close_/)'`
- ⬜ [cmd] S2: a closed session records `session.turns`, `session.yielded_at_ms`, `session.pending_at_end`, `session.end = closed`, `run.ended_by` `completed` and `census.init_tools`; a looping one records `session.end = cap`; a job kill or budget stop mid-session is the experimenter's end (I20): `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^session_measures_/)'`
- ⬜ [cmd] S3 (I9): the same fake-model run with and without the observation hooks gives the same tool calls and turns, and a hook whose log write fails still exits 0 with no output: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^session_hooks_/)'`
- ⬜ [cmd] S4 (container e2e: real Claude Code, the gateway sidecar, the fake model): `c1` gives two turns and closes; `c2` gives one turn and closes `idle_s` later: `cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'test(/^session_e2e_/)'` with Docker up
- ⬜ [cmd] S5: an experiment file with `runner.mode = "session"` in `[fixed]`, and one with it as a factor level, resolve on a session-capable driver with `runner.idle_s` derived as 60 (this keeps passing if path classes start refusing unknown dotted paths, since `experiment/paths.rs` classifies both); a session cell on a driver without the session capability is refused; `runner.idle_s` without session mode is refused; H4 still passes, so headless cells resolve as on cf3de30: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^session_resolve_/)|test(/^harness_resolution_/)'`
- ⬜ [cmd] P1 (D6): a model factor whose levels are tables under `model.*` gives each cell its own flags in the driver's argv: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^session_profiles_/)'`
- ⬜ [cmd] The headless argv is unchanged: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^harness_env_/)'`; the path-class oracle gains rows only: `! git diff cf3de30 -- crates/candler-core/tests/oracle/vectors/field_paths.toml | grep -q '^-[^-]'`
- ⬜ [cmd] SPAWN, STD (`-p candler-core`), LINUX, DIGEST, GOLDEN and SUBJ pass
- ⬜ [nl] The leaf report records the bites: idle counted from the last event instead of the turn's end (S1 red), a closed session recorded as a cap (S2 red), a hook that exits 2 (S3 red), `-i` missing (S4 red), `runner.mode` defaulted into every cell (S5 and H4 red)
- ⬜ [nl] The `session_runner` verifier closes with no open finding: it attacks whether the close can ever fire mid-turn or with pending work (clock skew, an event between the turn end and the timer, a notification racing the close), whether hooks can alter behaviour, and whether an experimenter's stop is ever charged to the arm
**References**: [R01 §Session runner, §Process supervision, §Ends](../../../../../__design__/architecture.md); [R09 D1–D6](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-approved-plan.md); `__reports__/m4-session-runner/leaves/session_facts_v0.md` — the protocol facts

## Step 1: Opt a cell into session mode
**Goal**: `runner.mode` and `runner.idle_s` resolve as cell values without touching any existing cell.
**Implementation Logic**:
1. `experiment/paths.rs` names both as cell values; `field_paths.toml` gains their rows and `guard_resolve_surface.rs` its new count. No core default: an absent `runner.mode` is headless, and an explicit `headless` is a cell of its own.
2. A preflight step (`resolve/steps/session.rs`) validates `runner.mode` (`headless` or `session`), refuses `runner.idle_s` unless the mode is `session`, derives `runner.idle_s = 60` in session mode when absent (`ValueSource::Derived`, I12), accepts `none`, and refuses a session cell whose driver lacks `Capabilities.session`.
3. S5 and P1 first, seen red.
**Deliverables**: `crates/candler-core/src/experiment/paths.rs`: `RUNNER_MODE`, `RUNNER_IDLE_S`; `crates/candler-core/src/resolve/steps/session.rs`: `SessionModeStep`; `crates/candler-core/tests/guard_session.rs`: `session_resolve_*`, `session_profiles_*`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^session_resolve_/)|test(/^session_profiles_/)|test(/^harness_resolution_/)|binary(guard_resolve_surface)'` (expected: PASS)
**Commit**: `feat(experiment): opt a cell into session mode`

## Step 2: Keep a session's stdin open
**Goal**: In session mode, and only then, the harness's stdin is a pipe the runner writes the opening input to.
**Implementation Logic**:
`ContainerPlan` gains `interactive`, set from `runner.mode`; `run_argv` adds `-i` only when it is set, so the headless argv is unchanged (H2). The run request uses `StdinSpec::Pipe`; the runner writes the driver's `SessionSpec` opening bytes and keeps the handle. The live observer gains a channel to the exec loop carrying the driver's `PendingState` (turn ended, pending items) as events arrive.
**Deliverables**: `crates/candler-core/src/isolation/{mod,model}.rs`: `ContainerPlan::interactive`; `crates/candler-core/src/isolation/container/run.rs`: `run_argv`; `crates/candler-core/src/runner/exec/spawn.rs`: the pending channel
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^harness_env_/)|binary(guard_container_run)'` (expected: PASS)
**Commit**: `feat(isolation): keep a session's stdin open`

## Step 3: Close a session once the agent has yielded
**Goal**: The close rule is a pure state machine over turn ends, pending items and an injected clock, and the run records how the agent stopped.
**Implementation Logic**:
1. `runner/exec/session.rs`: on a turn end with nothing pending, start the idle timer from that turn end; any later event resets it; when it reaches `idle_s`, close the input (the driver's close action); `none` never closes. After closing, wait for the harness to exit; past the driver's grace, the normal stop escalation runs and `session.close_forced` is recorded.
2. Record `session.*` (shared names) in `measured[]`; a closed session is `completed`, caps and budget as today (`ends.rs`). Claude Code's `--max-turns` bounds one turn, not the session (`session_facts` F1), so in session mode the cell's turn limit counts turns across the session: Candler sums each `result`'s `num_turns`, and on reaching the limit it ends the run as a cap (`session.end = cap`). The dollar budget is already cumulative in the harness. If D5 uses a hook log, collect it as a raw stream so the measures can be re-derived.
3. S1, S2 and S3 against a stub harness replaying the `session_facts` fixtures through the fake docker, written first and seen red.
**Deliverables**: `crates/candler-core/src/runner/exec/session.rs`: `CloseRule`, `SessionMeasures`; `crates/candler-core/src/runner/exec/ends.rs`; `crates/candler-core/tests/guard_session.rs`: `session_close_*`, `session_measures_*`, `session_hooks_*`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^session_close_/)|test(/^session_measures_/)|test(/^session_hooks_/)'` (expected: PASS)
**Commit**: `feat(runner): close a session only once the agent has yielded`

## Step 4: Drive real sessions against the fake model
**Goal**: The real harness, the real gateway and the fake model produce the C1 and C2 shapes end to end.
**Implementation Logic**:
S4 in the container-e2e group (ignored by default, run with Docker): a session run of `c1` and of `c2` through `candler run`, the fake model behind the gateway sidecar, a short `idle_s`. Name the tests `session_e2e_*` so the coordinator can add them to CI's expected-guard list. Remove only the containers and images the tests created.
**Deliverables**: `crates/candler-core/tests/e2e/session_e2e.rs`: `session_e2e_c1_waits_then_closes`, `session_e2e_c2_closes_at_idle`
**Consistency Checks**: `cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'test(/^session_e2e_/)'` (expected: PASS)
**Commit**: `test(runner): drive real sessions against the fake model`
