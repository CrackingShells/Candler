# Agent container record

**Goal**: A run's agent container gets its `children/container-<cid12>.json` (`container-record/1`, `role: agent`) as soon as its cidfile names it, so every destroy path, the job-close `CurrentJob` sweep included, finds it recorded.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone: `git merge-base --is-ancestor fix/v1-review-code-gaps HEAD`
- [ ] [nl] OWNED: `crates/candler-core/src/isolation/(mod|container/mod)\.rs`, `crates/candler-core/src/runner/exec/mod\.rs`, `crates/candler-core/src/jobs/orchestrate/session\.rs`, `crates/candler-core/tests/support/job_orchestrate\.rs`, `crates/candler-core/tests/guard_container_record\.rs` (new), and `crates/candler-core/tests/guard_job_orchestrate\.rs` only where a literal event order must change
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_container_record)'` passes and runs at least three tests
- ⬜ [cmd] Record: on a fake docker whose inspect answers a labelled agent, `DockerIsolation::record_container` writes `children/container-<cid12>.json`. Its record has `role: agent`, the inspect's `created_at` and `image_id`, and the container's own `candler.job`, `candler.run` and `candler.experiment` labels. A second call leaves the file byte-identical, and an absent cidfile writes nothing and returns `Ok(None)`. Oracle: the fake's scripted inspect, and the `container-record/1` schema file
- ⬜ [cmd] Job close: a run whose agent's run-scoped `rm` fails leaves the container to the job-close `SweepScope::CurrentJob` sweep, which destroys it, with `ContainerFound::Recorded` in its `container.destroyed` event. On `main` the same scenario is refused `GuardRefusal::Unrecorded`. Oracle: the recording `DestroyDriver`'s verb list and the event
- ⬜ [cmd] Ladder: on `Ended::Stopped`, the record exists in `children/` before the stop hook's destroy step runs. Oracle: the file's presence, checked from inside a recording sweeper's `sweep` call
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_job_orchestrate) | binary(guard_run_exec) | binary(guard_container_guard)'` passes, and `container.created` is still emitted for the agent of every run that has a cidfile
- ⬜ [cmd] STD, LINUX and SUBJ pass
- ⬜ [nl] The leaf report records one bite per new guard: the role set to `Gateway` (record guard); the poll in `drive` removed, keeping the post-wait try (ladder guard); and both the poll and the post-wait try removed (job-close guard). Each bite is seen to fail
- ⬜ [nl] The Grit reviewer reports no open finding on the leaf's `.rs` changes
**References**: [R01 §Process supervision](../../__design__/architecture.md) — container records, the destroy rule; [R04 §10.1](../../__reports__/m2b-execution-parity/00-architecture_v0.md) — "written as soon as the cid is known", R10; [R02 table C context](../../__reports__/candler-v1-review/00-findings_v0.md); [R03](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/conformance-docs-handover.md) — gap 1

## Step 1: The isolation writes an agent's record from its cidfile
**Goal**: The isolation, which knows the cidfile and the docker reader, can turn a run's cidfile into a `container-record/1` written through the job's recorder.
**Implementation Logic**:
1. `isolation/mod.rs`: add to the `Isolation` trait `fn record_container<'a>(&'a self, plan: &'a ContainerPlan, key: &'a RunKey) -> BoxFuture<'a, Result<Option<ContainerId>, IsolationError>>`. It has a default body that returns `Ok(None)`, so isolations without containers, and the test fakes, need no code. Its doc says that it records the run's container once its id is known, that it may be called many times, and that it returns the recorded id.
2. `isolation/container/mod.rs`:
   - `DockerIsolation` gains a `recorder: Option<Arc<dyn ChildRecorder>>` field and `#[must_use] pub fn with_recorder(self, recorder: Arc<dyn ChildRecorder>) -> Self` (`ChildRecorder` is `isolation/gateway.rs:152`; `JobDir` implements it). Without a recorder, `record_container` returns `IsolationError::Unavailable { detail: "DockerIsolation::with_recorder" }`, as `run_request` does for its sweeper.
   - The impl:
     1. Read `plan.cidfile` and parse a `ContainerId`. An absent or unparsable file means `Ok(None)`: the client has not written it yet.
     2. Inspect the id through `self.guard.reader()`. A failed inspect means `Ok(None)` too: the container is not yet, or no longer, inspectable, and the caller tries again.
     3. Build the `ContainerRecord` from the inspect: `cid`, `created_at`, `image_id`, and `ContainerLabels` from the container's own `candler.job`, `candler.run` and `candler.experiment` labels. The record must agree with what `ContainerGuard::check_record` compares, and the labels are what the guard re-verifies. Refuse with `IsolationError::Plan` when the job label differs from `plan.labels`' job, or the run label differs from `Marker::for_run(key)`.
     4. Write it with `recorder.record_child`, then return `Ok(Some(cid))`.
   - Reuse `DockerGateway::record_of`'s builder sequence (`ContainerRecord::builder()`, `crate::supervise::now()` for `spawned_at`). Do not move or change that function.
3. Guards in the new `tests/guard_container_record.rs`, which starts with `#![cfg(test)]`, against the recording fakes of `test-support` and a temporary `JobDir`:
   - the record's fields, as in the Record gate;
   - idempotence: two calls give one byte-identical file;
   - an absent cidfile gives `Ok(None)` and no file.
**Deliverables**: `crates/candler-core/src/isolation/mod.rs`: `Isolation::record_container`; `crates/candler-core/src/isolation/container/mod.rs`: `DockerIsolation::with_recorder`, `impl Isolation for DockerIsolation { fn record_container }`; `crates/candler-core/tests/guard_container_record.rs`: the record and idempotence guards
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_container_record)'` (expected: PASS)
**Commit**: `feat(isolation): write an agent container's record from its cidfile`

## Step 2: The run records its agent while it is live
**Goal**: Every run records its agent container while the docker client runs, before any destroy path can reach the container, and the job session hands the isolation its recorder.
**Implementation Logic**:
1. `runner/exec/mod.rs::drive`:
   - While `wait_or_cancel` runs, poll `isolation.record_container(&container, &plan.key)` every 100 ms (`tokio::time::interval`, `MissedTickBehavior::Delay`) until it returns `Some`. Use a `tokio::select!` beside the wait, so the wait is never delayed by the poll.
   - After the wait returns, and before any `?` on its result, call it once more if nothing was recorded yet.
   - Emit `EventPayload::container_created(cid, ContainerRole::Agent)` right after the record is written, as `JobSession::open` does for the gateway. Remove the late cidfile read and its event (today at l.397, "The agent's container record is its cidfile").
   - A `record_container` error is logged through the sink as today's errors are, and never fails the run: the label sweep still bounds the container.
2. `jobs/orchestrate/session.rs::JobSession::open`: pass `Arc::new(job_dir.clone())` to the `DockerIsolation` through `.with_recorder(…)`, next to `.with_sweeper(…)`. `// BORROW:` and `// TRAIT_OBJECT:` annotations as in the lines around.
3. `tests/support/job_orchestrate.rs`: `Placed` delegates `record_container` to its inner `DockerIsolation`. If the fake docker client writes no cidfile, nothing changes. If a literal event order in `guard_job_orchestrate.rs` moves because `container.created` now comes earlier, update that order and report it as a deviation.
4. Guards in `tests/guard_container_record.rs`:
   - Job close: run a cell through a `DockerIsolation` over recording fakes whose agent `rm` fails once, then run the session's close sweep with `SweepScope::CurrentJob` over the same `JobDir` view. Assert the agent destroyed and `ContainerFound::Recorded`.
   - Ladder: a run ended by its stop handle (`Ended::Stopped`), with a recording `ContainerSweeper` that asserts the record file exists when `sweep` is called.

   Both use only the recording `Signaller` and `DestroyDriver` and spawn only first-party commands. Apply SPAWN before running them.
**Deliverables**: `crates/candler-core/src/runner/exec/mod.rs`: the poll and the post-wait try in `drive`, and the agent's `container_created` emitted after the record; `crates/candler-core/src/jobs/orchestrate/session.rs`: `.with_recorder(` in `JobSession::open`; `crates/candler-core/tests/support/job_orchestrate.rs`: `fn record_container` in `impl Isolation for Placed`; `crates/candler-core/tests/guard_container_record.rs`: the job-close and ladder guards
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_container_record) | binary(guard_job_orchestrate) | binary(guard_run_exec)'` (expected: PASS)
**Commit**: `fix(runner): record the agent container while its run is live`
