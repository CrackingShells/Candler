# OpenCode wiring

**Goal**: The run loop executes a driver's collect command inside the agent's container before it is destroyed, reads the output with the offline reader as the authoritative trace, runs an in-container pre-stop with the observed session id, and the binary registers OpenCode.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] The branch is cut from the rebase point the coordination session gave (Step 3 edits the binary's `drivers.rs` and `Cargo.toml`)
- [ ] [nl] OWNED: `crates/candler-core/src/runner/exec/gather.rs`, `crates/candler-core/src/isolation/container/stop.rs`, `crates/candler-core/src/supervise/stop.rs` (the `Action::Exec` dispatch only); `crates/candler/src/drivers.rs` (the OpenCode entry), `crates/candler/Cargo.toml`; new tests. Shared hunks, merged by the coordinator: `runner/exec/mod.rs` (trace assembly), `isolation/container/mod.rs` (collect before destroy), with `session_runner`
**Success Gates**:
- ⬜ [cmd] W1, exited run: a fake OpenCode run (recording docker replaying `opencode_facts` fixtures) exits, `session export` runs inside the container before the destroy, its output is framed `Document`, stored as a raw stream and read by the offline reader into the trace: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^wiring_collect_/)'`
- ⬜ [cmd] W1, stopped run: the pre-stop is a `docker exec` in the agent's container carrying the observed session id; no HTTP call leaves for host loopback; with no id observed yet the pre-stop is skipped and the escalation continues: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^wiring_stop_/)'`
- ⬜ [cmd] The binary selects OpenCode by its name: `cargo nextest run -p candler --no-tests=fail -E 'test(/^registry_/)'`
- ⬜ [cmd] Claude Code runs are unchanged: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_harness_facts)'`
- ⬜ [cmd] SPAWN, STD (`-p candler-core -p candler`), LINUX, DIGEST, GOLDEN and SUBJ pass
- ⬜ [nl] The leaf report records the bites: the export never read (the collect test red), a host-loopback pre-stop (the stop test red)
- ⬜ [nl] The `opencode_wiring` verifier closes with no open finding: it attacks whether the trace is a pure function of the framed raw and the reader (I3) when two readers contribute, whether a container can be destroyed before its export ran, and whether the session id placeholder can be filled from anything but the live reader
**References**: [R01 §Harness drivers (the export goes to `offline_reader`), §Process supervision (stop escalation, containers), I3, I15](../../../../../__design__/architecture.md); [R09 D9, D11](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-approved-plan.md)

## Step 1: Collect an export and read it offline
**Goal**: `CollectSpec::Command` runs, and its output becomes the trace.
**Implementation Logic**:
1. Tests first (`wiring_collect_*`), red.
2. Before the destroy step, run each `CollectSpec::Command` in the agent's container (an exec through the isolation, the command's placeholders filled from the observed ids), frame its stdout with the offline reader's framing, store it as a named raw stream, and feed it to `offline_reader`.
3. When the driver's offline reader yields sessions, they are the trace; the live reader's sessions are used otherwise (Claude Code: no command, unchanged). The trace header names the reader that produced it.
**Deliverables**: `crates/candler-core/src/runner/exec/gather.rs`: `collect_commands`; the trace-assembly hunk of `runner/exec/mod.rs`; the collect hunk of `isolation/container/mod.rs`; `crates/candler-core/tests/guard_wiring.rs`: `wiring_collect_*`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^wiring_collect_/)|binary(guard_harness_facts)'` (expected: PASS)
**Commit**: `feat(runner): collect a harness's export and read it offline`

## Step 2: Stop from inside the container
**Goal**: A driver's `Action::Exec` pre-stop runs in the agent's container with the session id.
**Implementation Logic**:
The container stop hook runs an `Action::Exec` pre-stop as `docker exec <cid> …` after re-verifying the container, filling `{session_id}` from `LiveHandle::observed()` at stop time; with no observed id the step is recorded as skipped. A failed or timed-out pre-stop continues the escalation, as an HTTP one does today. Tests `wiring_stop_*` first.
**Deliverables**: `crates/candler-core/src/isolation/container/stop.rs`: the exec pre-stop; `crates/candler-core/src/supervise/stop.rs`: `Action::Exec` dispatch; `crates/candler-core/tests/guard_wiring.rs`: `wiring_stop_*`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^wiring_stop_/)|binary(guard_signal)'` (expected: PASS)
**Commit**: `feat(isolation): run a driver's pre-stop inside its container`

## Step 3: Register OpenCode
**Goal**: An experiment naming OpenCode gets its driver, skill tree, reader and base context.
**Implementation Logic**:
Add the OpenCode entry to `Registry` (its `HarnessId`, `assets/docker/base-opencode`), the dependency in `crates/candler/Cargo.toml`, and the registry test's OpenCode row.
**Deliverables**: `crates/candler/src/drivers.rs`: the OpenCode entry; `crates/candler/Cargo.toml`; `crates/candler/tests/guard_registry.rs`
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/^registry_/)'` (expected: PASS)
**Commit**: `feat(adapter-opencode): register the driver in the binary`
