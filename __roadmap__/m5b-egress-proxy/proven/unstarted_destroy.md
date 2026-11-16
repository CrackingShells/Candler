# Unstarted destroy

**Goal**: A container that was created but never ran is destroyed by its creator, by the id `docker create` returned, through the guard in `SweepScope::Run`, on all three paths that leak it today (B2), and a deadline that races a cancel stores one outcome and issues one stop (B6).
**Estimate**: $3.0 (Sonnet, one sitting)
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] L0 is merged: `git log --format=%s | grep -q '^merge(isolation): land guard_reuse'`
- [ ] [nl] OWNED: `crates/candler-core/src/isolation/container/mod.rs` (`run_request`), `crates/candler-core/src/runner/exec/mod.rs` (the session-start error path), a new `crates/candler-core/tests/it/guard_unstarted_destroy.rs`, and the stop-race table's file `crates/candler-core/tests/it/guard_container_stop.rs` for the B6 row. The plan did not list the last two; `guard_container_stop.rs` is the file whose `stop_run(answer, deadline)` table holds the stop rows. No rule of `guard.rs` widens: it already admits destroying an unrecorded container in `SweepScope::Run`. A1 adds `crates/candler-core/src/isolation/container/{guard,stop,sweeper}.rs`, `crates/candler-core/src/isolation/{driver,docker,testing}.rs` and the fake drivers in `crates/candler-core/tests/it/support/` for the moved G3 and lost-sweep gates
**Success Gates**:
- ✅ [cmd] A run that fails before its container runs leaves no container, on each path (record failure, start request failure, session start failure): `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_unstarted_destroy::/)'`
- ✅ [cmd] A deadline and a cancel that race store one outcome and issue one stop: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_container_stop::/)'`
- ✅ [cmd] The destroy uses A1's single verification and no path widens: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_container_guard::/)'`
- ✅ [cmd] G3 from `guard_reuse` (A1 below): the destroy hook takes a fresh list and reuses only the pre-stop's verification facts for ids it already verified, verifying new ids, so the ladder stop of a job of 9 costs at most 8 (pre-stop 4 + list 2 + stop 1 + rm 1); a container created between the hooks is verified and destroyed, and reusing the pre-stop's list turns that row red: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_container_guard::spawn_budget_/)'`
- ✅ [cmd] A lost job's allow networks are destroyed by the lost sweep (A1 below): `sweeper.rs` names only the internal and egress networks, and a lost job has no records, so they are found by one new driver call (`docker network ls` by the job label) and the lost-sweep spawn budget is re-derived: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_container_guard::/)'`
- ✅ [cmd] SPAWN, STD (`-p candler-core`), LINUX, E2E and SUBJ pass (README, standard gates)
- ✅ [nl] P6 is answered in the leaf report. The report records each Bite and any defect found in the stop race
**References**: [R03 B2, B6, Test design](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5b-approved-plan.md) — decisions and guards; [R01 §Process supervision](../../../__design__/architecture.md) — destroying containers; the code: `run_request` (`container/mod.rs`, the `?` after `record_agent` leaks a created container), `runner/exec/mod.rs` (the `child.stop()` and return on a session-start error), `wait_or_cancel`

## Step 1: Probe P6, how a record failure is injected
**Goal**: Find the cheapest way to make `record_agent` fail, so the guard's fixture is real.
**Implementation Logic**:
1. Question: how is a record failure injected in `DockerIsolation::run_request` with the recording fake (`isolation/testing.rs`: `with_created`, `calls`)? Candidates: a job directory made read-only, a record store fake, a failing `record_agent`.
2. Decision it feeds: the fixture of step 2's `record` row. If no injection exists without changing `src/`, the leaf reports it as a finding before building the row.
3. The probe script is not kept.
**Deliverables**: `__reports__/m5b-egress-proxy/leaves/unstarted_destroy_v0.md`: section "Probe P6" (the way found, the decision)
**Consistency Checks**: `grep -q 'Probe P6' __reports__/m5b-egress-proxy/leaves/unstarted_destroy_v0.md` (expected: PASS)
**Commit**: `docs(isolation): record the answer of probe P6`

## Step 2: Destroy a container that never ran
**Goal**: Each path that created a container and then failed before it ran destroys it, by id, through the guard.
**Implementation Logic**:
1. Guard first, seen red: `guard_unstarted_destroy::unstarted_destroy`, one parameterised guard with rows `record` (the record fails), `start_request` (the start request fails after the record), `session_start` (the session cannot start, so `runner/exec/mod.rs` returns before the wait). Integration with the recording fake (the consumer is the shared daemon). Oracle: the fake's call log has one `create` and, for the id it returned, one verification then one `rm`, and the record, if made, is unchanged. Bite: one path's destroy dropped.
2. `run_request`: on an error after `create` returned an id, destroy that id by the guard in `SweepScope::Run` (one verification), then return the original error. The destroy's own failure is reported with the original, never replaces it.
3. `runner/exec/mod.rs`: the session-start error path stops the child and destroys the container the same way before it returns.
**Deliverables**: `crates/candler-core/src/isolation/container/mod.rs`: the destroy in `run_request`; `crates/candler-core/src/runner/exec/mod.rs`: the destroy on the session-start error path; `crates/candler-core/tests/it/guard_unstarted_destroy.rs`: `unstarted_destroy`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_unstarted_destroy::/)'` (expected: PASS)
**Commit**: `fix(isolation): destroy a container that was created but never ran`

## Step 3: One row for the deadline that races a cancel
**Goal**: The existing stop-race table holds the case where a deadline and a cancel arrive together (B6).
**Implementation Logic**:
1. Find the table first: the `stop_run(answer, deadline)` rows in `guard_container_stop.rs`, which `wait_or_cancel` (`runner/exec/mod.rs`) serves. If no table carries a cancel row, report it as a finding and add the row where the stop rows are.
2. The row: a deadline and a cancel race; the run stores one outcome (the first to arrive) and issues one stop. Oracle: hand-written, one outcome and one `docker stop` in the recording fake. Bite: the deadline overwrites the cancel's outcome.
3. A defect the row finds is fixed in `runner/exec/mod.rs` here.
**Deliverables**: `crates/candler-core/tests/it/guard_container_stop.rs`: the cancel-versus-deadline row of the stop table
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_container_stop::/)'` (expected: PASS)
**Commit**: `test(isolation): pin the deadline racing a cancel`
