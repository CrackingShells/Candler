# Container create

**Goal**: The agent's container is created with `docker create`, its record is written, and then `docker start -a [-i] <id>` is spawned as the supervised child. No cidfile is written or polled. The stream, the exit facts and the stop path stay as they are (A8).
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] OWNED: `crates/candler-core/src/isolation/container/{run,mod,exec}.rs`, `crates/candler-core/src/runner/exec/mod.rs`, the cidfile code in `isolation/gateway.rs`, `crates/candler-core/tests/fixtures/harness_baseline/argv.json`, `tests/it/{guard_harness_facts,guard_session,guard_container_record}.rs`, the existing files and Dockerfile of `assets/docker/testagent/`, new e2e files; it never edits `container/{guard,sweeper,stop}.rs` (`guard_reuse` owns them)
**Success Gates**:
- ⬜ [cmd] The record exists before the start spawn; a failed create spawns nothing and leaves no record; a crash between create and start leaves a recorded container that the lost-job sweep reaps: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_container_record::create_/)'`
- ⬜ [cmd] No cidfile is left: `! git grep -n -i 'cidfile' -- crates/candler-core/src` and `! git grep -n 'RECORD_POLL' -- crates/candler-core/src`
- ⬜ [cmd] On the real testagent, built through the core: exit code 3, a signal exit, a session stdin echo, a stop mid-run through the stop hook, and a byte-exact stream equal to the `docker run` frames of the same script: `cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*agent_create_then_attach/)'`
- ⬜ [cmd] SPAWN, STD (`-p candler-core -p candler`), LINUX, DIGEST, E2E and SUBJ pass
- ⬜ [nl] The change to `harness_baseline/argv.json` is reported to the coordinator before the leaf merges, and the coordination session's ruling is recorded
- ⬜ [nl] The leaf report records what `start -a` relays (stdout, stderr, the exit code, a signal, stdin) as measured on this engine, and each bite
- ⬜ [nl] The one L2 verifier (shared with `guard_reuse`, on the merged diff) closed with no open finding. It attacks stdin, exit facts, the stop hook and the lost-supervisor path across create and attach
**References**: [R04 A8](../../../../__reports__/interlude-audit/00-findings_audit_v0.md); [R01 §Process supervision, Containers](../../../../__design__/architecture.md); [R06 D8](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md)

## Step 1: Measure what start -a relays
**Goal**: Know, on this engine, whether `docker start -a [-i]` relays the stream bytes, the exit code, a signal exit and stdin as `docker run` does. The design of step 2 rests on the answer.
**Implementation Logic**:
1. On the testagent image (found or built through `test_image`), run one script twice: through `docker run`, and through `docker create` then `docker start -a -i`. The script emits a known byte sequence with binary bytes and no trailing newline, echoes stdin, and exits 3.
2. Compare the bytes, the exit codes, and a SIGTERM to the client.
3. Check that `docker create` takes two `--network` flags on Engine 25+, as `run` does (`run.rs:185`).
4. Record the answers in the leaf report. Nothing is committed but the report. If `start -a` differs in a way the run depends on, stop and report.
**Deliverables**: the leaf report's section "start -a"
**Consistency Checks**: `test -f __reports__/m5-container-isolation/leaves/container_create_v0.md` (expected: PASS)
**Commit**: `reports(isolation): record what docker start relays`

## Step 2: Create, record, then attach
**Goal**: The record is written synchronously from the id `create` returns, before the supervised `start -a` is spawned. Nothing polls.
**Implementation Logic**:
1. Guards first, seen red: `create_records_before_start`, `create_failure_spawns_nothing`, `create_then_crash_is_reaped`. Oracle: the order of the recording fakes' calls. Bite: the record written after the start spawn.
2. `DockerIsolation::run_request` calls `DockerDriver::create`, writes the container record (unchanged `container-record/1` shape), then builds the supervised `start -a` request through the one client. `-i` is added when the plan is interactive.
3. Remove `record_when_known`, `RECORD_POLL`, the cidfile path in the plan, and the gateway's unused `--cidfile`.
4. Collect reads the recorded id, never a cidfile.
5. `argv.json` changes. Stop and report the diff to the coordinator before going on.
**Deliverables**: `crates/candler-core/src/isolation/container/{mod,run}.rs`: create, record, attach; `crates/candler-core/src/runner/exec/mod.rs`: the polling removed; `crates/candler-core/src/isolation/gateway.rs`: `--cidfile` removed; `crates/candler-core/tests/fixtures/harness_baseline/argv.json` (after the ruling); `crates/candler-core/tests/it/guard_container_record.rs`: `create_*`
**Consistency Checks**: `! git grep -n -i 'cidfile' -- crates/candler-core/src && cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_container_record::/) | test(/^guard_harness_facts::/) | test(/^guard_session::/)'` (expected: PASS)
**Commit**: `refactor(isolation): create the agent container before attaching to it`

## Step 3: Run the real testagent through create and attach
**Goal**: The real image runs through the new path with the same observable behaviour.
**Implementation Logic**:
`agent_create_then_attach` builds a table over five cases:
- exit code 3;
- a signal exit;
- a session stdin echo;
- a stop mid-run through the stop hook, with the container destroyed by id;
- a byte-exact stream.

The stream oracle is step 1's `docker run` frames of the same script. It uses `test_image` and removes what it built.
**Deliverables**: `crates/candler-core/tests/it/guard_container_e2e/create_e2e.rs`: `agent_create_then_attach`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*agent_create_then_attach/)'` (expected: PASS)
**Commit**: `test(isolation): run the agent through create and attach`
