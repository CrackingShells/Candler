# Guard reuse

**Moved to M5b** by amendment A8 (`__reports__/candler-v1-review/04-findings_amendment_a8_v0.md`): not part of M5a; kept as the source of M5b's plan.

**Goal**: The destroy guard verifies the containers of a destroy decision with one multi-id inspect, re-uses the facts for that decision, and lists a run's containers by the run label (A1). A stopped run in a job of 9 containers costs at most 8 docker calls, down from 37, and no refusal rule widens.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] OWNED: `crates/candler-core/src/isolation/container/{guard,sweeper,stop}.rs`, `crates/candler-core/tests/it/guard_container_guard.rs`, new e2e files; it keeps the `pub(crate)` signatures of `ContainerGuard` that `container/mod.rs` calls, and it does not edit `container/{run,mod,exec}.rs` (`container_create` owns them)
**Success Gates**:
- ⬜ [cmd] The spawn budget holds: ladder stop ≤ 8, normal exit and exec pre-stop at their hand-derived bounds, counted from a PATH-fake docker log filtered to `ls`, `inspect`, `stop` and `rm`: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_container_guard::spawn_budget_/)'`
- ⬜ [cmd] Every existing refusal still refuses, a fact verified for one container is never applied to another, and a container gone between `ls` and `inspect` is skipped, never destroyed: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_container_guard::/)'`
- ⬜ [cmd] On a real daemon, `inspect_many` with one removed id returns the rest and `NotFound` for it, and the sweep destroys exactly the run's own containers, by id: `cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*destroy_verifies_by_one_inspect/)'`
- ⬜ [cmd] SPAWN, STD (`-p candler-core`), LINUX, DIGEST, E2E and SUBJ pass
- ⬜ [nl] The leaf report states each path's spawn count before and after, each bite, and the old call-order pins that changed and why. A pin's change is deliberate: it guarded the old sequence
- ⬜ [nl] The one L2 verifier (shared with `container_create`, on the merged diff) closed with no open finding
**References**: [R04 A1](../../../../__reports__/interlude-audit/00-findings_audit_v0.md); [R01 §Process supervision, Destroying containers](../../../../__design__/architecture.md); [R06 D7](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md)

## Step 1: Verify once per destroy decision
**Goal**: The guard's verification is one `inspect_many` per decision, and its facts serve that decision's stop and rm.
**Implementation Logic**:
1. Verify the audit's count against the code first, and record it: the 37 spawns for a stopped run in a 9-container job, from `ps_by_label`, `verify` and `destroy_with`.
2. Guards first, seen red:
   - `spawn_budget_*`: a table over the ladder stop, the normal exit and the exec pre-stop. Oracle: a count derived by hand from the protocol (list once, inspect once, then the stop and rm of each target), checked against a PATH-fake docker's log, so a loop hidden inside `inspect_many` is caught. Bite: `inspect_many` spawning once per id.
   - `reuse_never_crosses_containers`: facts verified for container A are never used to destroy B. Bite: reuse keyed by a short-id prefix.
   - `vanished_between_list_and_inspect`: a container gone between `ls` and `inspect` is skipped and reported, never destroyed.
3. The rule (D7): a container's id, labels and creation time are immutable, so one verification holds until its rm. Refusals (another job, a lost job, a label or creation mismatch) are unchanged.
4. A run-scoped listing filters by the run label in `docker container ls --filter label=candler.run=<key>`, not by the job label and then in memory.
5. Update the existing call-order pins (two inspects, then inspect, stop, inspect, rm) to the new sequence, and list each one in the report.
**Deliverables**: `crates/candler-core/src/isolation/container/guard.rs`: `ContainerGuard::verify_many`, the reuse in `destroy_with`; `sweeper.rs` and `stop.rs`: run-label listing and one verification per decision; `crates/candler-core/tests/it/guard_container_guard.rs`: `spawn_budget_*`, `reuse_never_crosses_containers`, `vanished_between_list_and_inspect`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_container_guard::/)'` (expected: PASS)
**Commit**: `perf(isolation): verify a destroy with one inspect of every container`

## Step 2: Show it on a real daemon
**Goal**: The multi-id inspect and the run-scoped sweep behave on a real daemon as the fakes say.
**Implementation Logic**:
`destroy_verifies_by_one_inspect` creates three labelled containers from an image found or built through `test_image`. It removes one by id, runs `inspect_many` over all three, then sweeps the run. It asserts the `NotFound` mapping, that only the run's containers are gone, and that the other job's container is untouched. It removes what it created by id at the end, even on failure.
**Deliverables**: `crates/candler-core/tests/it/guard_container_e2e/destroy_e2e.rs`: `destroy_verifies_by_one_inspect`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*destroy_verifies_by_one_inspect/)'` (expected: PASS)
**Commit**: `test(isolation): verify a destroy on a real daemon`
