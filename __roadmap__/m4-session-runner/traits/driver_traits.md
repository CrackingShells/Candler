# Driver traits

**Goal**: The driver traits express what both harnesses need (decision D9), every new item has a default so existing implementers compile, the Claude Code driver implements them, and one shared fixture suite runs against every driver.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] Read first: `__reports__/m4-session-runner/leaves/opencode_facts_v0.md` and `session_facts_v0.md`; D2's and D5's branches as they settle them
- [ ] [nl] OWNED: `crates/candler-harness/**`, `crates/candler-harness-claude-code/**`
**Success Gates**:
- ⬜ [cmd] K1: the shared suite passes for Claude Code and both fake shapes: `cargo nextest run -p candler-harness --features conformance --no-tests=fail -E 'test(/^conformance_/)' && cargo nextest run -p candler-harness-claude-code --no-tests=fail -E 'test(/^conformance_/)'`
- ⬜ [cmd] The headless Claude Code command line is unchanged: `git diff --quiet a26fe53 -- crates/candler-harness-claude-code/tests/guard_claude_code_driver.rs && cargo nextest run -p candler-harness-claude-code --no-tests=fail -E 'binary(guard_claude_code_driver)'`
- ⬜ [cmd] The reader's id is unchanged (I3 ties traces to it): `! git diff a26fe53 -- crates/candler-harness-claude-code/src/lib.rs | grep -E '^[-+].*READER'`
- ⬜ [cmd] The workspace builds and passes with the core's test fakes untouched (new items have defaults): `git diff --quiet <base> -- crates/candler-core crates/candler && cargo nextest run --workspace`, `<base>` being the leaf's branch point
- ⬜ [cmd] STD (`-p candler-harness -p candler-harness-claude-code`), LINUX, DIGEST and SUBJ pass
- ⬜ [nl] The leaf report records K1's bites: a suite check that only a Lines reader with an init event can pass (the Document fake turns red), and a session capability declared without a session spec (refused)
- ⬜ [nl] The `driver_traits` verifier closes with no open finding: it takes every fact of `opencode_facts_v0.md` and attacks whether the traits can express it without a Claude-shaped default (the provider configuration under a run-time base URL, state files, a census without an init event, a skill layout, an in-container interrupt that needs the session id, an authoritative export)
**References**: [R01 §Harness drivers, §OpenCode v2, §Session runner](../../../__design__/architecture.md); [R09 D2, D5, D9](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-approved-plan.md); [the two fake shapes](../../../crates/candler-harness/tests/guard_two_shapes.rs) — the seed of the shared suite

## Step 1: Extend the traits
**Goal**: Every fact D9 lists has a trait item, each with a default that keeps today's behaviour.
**Implementation Logic**:
In `candler-harness` (names from the campaign's shared names):
1. `Capabilities.session` (default false).
2. `HarnessDriver::gateway() -> GatewayEnv {base_url, auth_token, reserved}`, or the provider-configuration form `opencode_facts` shows OpenCode needs: `prepare` receives the run-time base URL and the token's variable name and writes the configuration. Pick one form both drivers use; report the choice. `opencode_facts` found that OpenCode's `opencode.json` takes `{env:VAR}` for `baseURL` and `apiKey`, so a `GatewayEnv` of variable names can serve both: the core sets the variables, and OpenCode's `prepare` writes a configuration that names them.
3. `HarnessDriver::state_paths() -> Vec<StatePath {env, kind: Dir | File, subpath}>` (default empty).
4. `Observed.init: Option<InitReport {tools, mcp_servers}>`, the raw JSON values the census reads today, and a census source that fits both shapes: the harness's own init event (Claude Code), or a collected raw stream that a reader turns into an `InitReport` (OpenCode has no init event and no tool-listing command; the coordination session backs a gateway record of each model request's tool names, pending Eliott's approval of the `opencode_wiring` amendment). The source is labelled on the report (observed by the harness, or by the gateway), so a census from one source never counts as equal to another's or to an unrecorded one. A source that yields nothing leaves the census unrecorded, never an empty set. `Capabilities.census = false` remains for a harness with neither.
5. The session surface: `HarnessDriver::session(cell) -> Option<SessionSpec>` (the input bytes that open the session; how to close it), `HookSpec` (the observation hooks and where their payloads go, if D5 needs a log), and a parse of stream events or hook records into `PendingState` (turn ended; pending tasks, monitors, crons, wake-ups).
6. `Action::Exec(CommandSpec)`, a pre-stop the core runs inside the agent's isolation, and the placeholder `{session_id}` the core fills from the live reader's observed ids. OpenCode's interrupt needs the server child's random port and password, which only a command inside the container can read (`/proc`), so the command, not the core, discovers them.
7. `classify_exit` can classify from more than the exit status: OpenCode exits 1 for a failure, an API interrupt and an abort alike, and only the last `error` event or the export's `outcome` tells them apart. If today's signature takes only the status, extend it with the reader's observed end, with a default that keeps Claude Code's classification.
8. The trait `candler_harness::skills::SkillTree`, with the methods of the core's `runner::exec::cell::SkillTree`; the core's trait stays until L2 removes it.
**Deliverables**: `crates/candler-harness/src/{driver.rs,spec.rs,reader.rs}`: `GatewayEnv`, `StatePath`, `InitReport`, `SessionSpec`, `HookSpec`, `PendingState`, `Action::Exec`; `crates/candler-harness/src/skills.rs`: `SkillTree`
**Consistency Checks**: `cargo nextest run --workspace` (expected: PASS)
**Commit**: `feat(runner): let a driver supply gateway, state, census and session`

## Step 2: Share one fixture suite
**Goal**: Each driver crate runs the same checks on its own recorded fixtures.
**Implementation Logic**:
Add `candler_harness::conformance` behind the feature `conformance`: given a driver and its fixture cases (recorded stream, optional export, expected sessions, observed ids, init report, end class), it checks the reader (detect, feed, finish, observed), `classify_exit`, `collect`, the command's invariants (no secret in argv, a cwd), the stop spec, and the capability rules (session implies a session spec; census implies an init report or a census source; `execution_outside_child` implies a pre-stop). Move the two fake shapes of `guard_two_shapes.rs` onto it as its first cases. Tests are named `conformance_*`. Write it before Step 3, and see the Claude Code cases fail.
**Deliverables**: `crates/candler-harness/src/conformance.rs`: `run`, `Case`; `crates/candler-harness/Cargo.toml` (feature `conformance`); `crates/candler-harness/tests/conformance_shapes.rs`
**Consistency Checks**: `cargo nextest run -p candler-harness --features conformance --no-tests=fail -E 'test(/^conformance_/)'` (expected: PASS)
**Commit**: `test(runner): share one fixture suite across drivers`

## Step 3: Implement the traits for Claude Code
**Goal**: The Claude Code driver supplies every new fact and passes the suite on its headless and session fixtures.
**Implementation Logic**:
1. `gateway`: today's `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_API_KEY`; `state_paths`: empty, so its container argv stays as it is; `InitReport` from its init event.
2. Session: `--input-format stream-json`, the prompt as the first stdin message, the hooks of D5 (a `--settings` file outside the workspace, or the stream's hook events, per `session_facts`), each hook command `… || true; exit 0`; the pending parse from what `session_facts` found: a turn ends at `result`; the pending list is the Stop payload's `background_tasks` and `session_crons`, and on a turn with no Stop line (an `error_max_turns` or `error_max_budget_usd` result) it is the latest `system/background_tasks_changed`. The headless command line is unchanged.
3. `ClaudeSkills` (the `.claude/skills/<name>` layout) implemented here against the new trait.
4. The crate's conformance test runs the suite on the existing stream fixtures and the session fixtures of `session_facts`.
**Deliverables**: `crates/candler-harness-claude-code/src/{driver.rs,prepare.rs}`; `crates/candler-harness-claude-code/src/session.rs`: the session spec, hook spec and pending parse; `crates/candler-harness-claude-code/src/skills.rs`: `ClaudeSkills`; `crates/candler-harness-claude-code/tests/conformance.rs`
**Consistency Checks**: `cargo nextest run -p candler-harness-claude-code --no-tests=fail -E 'test(/^conformance_/)|binary(guard_claude_code_driver)'` (expected: PASS)
**Commit**: `feat(adapter-claude-code): add session mode and the new driver facts`
