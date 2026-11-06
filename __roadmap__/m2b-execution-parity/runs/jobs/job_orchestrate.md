# Job orchestrate: gateway, caps

**Goal**: `candler_core::jobs::orchestrate` runs one job's plans under `max_parallel` and the machine slots, owns the per-job network and gateway from creation to guarded teardown, builds `DockerIsolation` once the gateway answers, and maps a crossed job cap to `invalid(budget_stop)` on live runs only.
**Pre-conditions**:
- [ ] [cmd] The Phase 0 scaffold is landed: `for s in '^docs(design): ' '^build(repo): ' '^ci(repo): ' '^feat(schemas): ' '^feat(orchestrator): scaffold'; do git log --format=%s origin/main..feat/m2b-execution-parity | grep -q "$s" || exit 1; done && test -s __roadmap__/m2b-execution-parity/feature_map/feature_map.json`
- [ ] [nl] api_review closed with no open finding on `Runtime` (`supervisor: Arc<Supervisor>`), `JobSpec`, `JobOutcome`, `JobBody`, `JobControl`, `JobCause`, `RunControl`, `JobMeter`, `RunOutcome` (with `usage`), the `model.token_secret` setting, `Gateway::start`, `SweepScope::CurrentJob` and `ContainerGuard::{destroy, destroy_network}`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
- [ ] [cmd] DEP(job_store), DEP(run_plan), DEP(run_exec), DEP(container_run), DEP(container_stop) and DEP(gateway) hold: `for l in job_store run_plan run_exec container_run container_stop gateway; do git log --first-parent --format=%s feat/m2b-execution-parity | grep -qx "merge(.*): land $l" || exit 1; done`
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_job_orchestrate)'` passes. B34 drives a literal run table (two sealed valid runs, two live) over a PATH-fake `docker` whose `run` sleeps, crosses `wall_s`, and asserts: each live run is sealed `invalid(budget_stop)` with `ended_by = budget_stop`, and `JobOutcome.end` is `budget_stop`; each sealed run keeps its status and its files' `sha256`; no new run starts after the crossing (the fake's `run` count equals the table's); a job under the cap, with the same table, changes nothing (control). Oracle: R01 §Budget and the literal table
- ⬜ [cmd] STD passes
- ⬜ [cmd] LINUX passes (`cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings`)
- ⬜ [cmd] SUBJ passes, with `<base>` the leaf's branch point on `feat/m2b-execution-parity`
- ⬜ [cmd] No schema is touched, so DRIFT has nothing to bless: `git diff --quiet <base>..HEAD -- crates/candler-schemas`
- ⬜ [cmd] No scaffolding: `! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/jobs/orchestrate crates/candler-core/tests/guard_job_orchestrate.rs`
- ⬜ [cmd] No destroy verb outside the guard: `cargo nextest run -p candler-core -E 'binary(guard_layering_i22)'`
- ⬜ [nl] The leaf report records the B34 bite: apply the cap to `all_runs` instead of `live_runs`; the sealed runs' status or digests change and B34 fails; restored with `git checkout --`
- ⬜ [nl] Reading `orchestrate` confirms that the gateway and the network are destroyed only through `ContainerGuard::destroy` and `destroy_network`, that the token is resolved only inside `Gateway::start`, and that every error path after the network exists reaches teardown
**References**: [R01 §Budget, §Jobs, runs and supervision](../../../../__design__/architecture.md) — caps, `max_parallel`, slots; [R03 §5, §7, §8 L2, §9, §10.1](../../../../__reports__/m2b-execution-parity/00-architecture_v0.md) — the job sequence, `Gateway::start` (R16), `destroy_network` (R9), the destroy rule (R10); [R04 B34](../../../../__reports__/m2b-execution-parity/01-test_definition_v0.md) — oracle and bite

## Step 1: Create the job's network and start its gateway
**Goal**: A job owns one labelled internal network and one answering gateway before any run starts, and a failed start leaves neither behind.
**Implementation Logic**:
1. `JobSession::open<'a>(rt: &'a Runtime, spec: &'a JobSpec) -> BoxFuture<'a, Result<JobSession, OrchestrateError>>`:
   1. take the network name from `NetworkName::for_job` in `isolation/model.rs` (A6); `DockerDriver::network_create(&name, internal: true, &Labels)` with the labels `candler.job=<job id>` and `candler.role` absent (a network carries only the job label, R9);
   2. take the token holder from the setting `model.token_secret` (D9), which names one `secrets[]` entry; resolution has already refused an absent entry, and an unreadable one is `OrchestrateError::TokenSource` before any container exists;
   3. `Gateway::start(&*rt.supervisor, &*rt.docker, &job, &net, &secrets, name)`, with `name` the value of `model.token_secret`,, where the `SecretSource` resolves the secret by name (`resolve(name)`) at call time and nothing here stores or logs the value; then `Gateway::probe(&handle)` exactly once (D21: no retry loop here); a `GatewayStatus` other than healthy (`Unavailable { reason }` included) tears down and returns `OrchestrateError::Gateway { reason }`;
   4. build `DockerIsolation` once with `DockerIsolation::new(handle.clone(), docker, guard)` (R14, A2), then set `rt.isolation = Some(Arc::new(..))` and `rt.gateway = Some(JobGateway { service, handle })` on the `Runtime`, which `JobBody` received by value (A4); job_runtime builds it with both `None`.
2. A failure at any point after 1.1 destroys what exists, in reverse order, through the guard, and returns the error; `job.started` was already emitted right after the lock (D10), so the body returns `JobOutcome { end: JobEnd::Failed, runs: [] }` and job_runtime emits `job.ended(failed)` before any run starts (D21): the stream is well formed.
3. Class A tests (`#[cfg(test)] mod scaffold`) over the recording `DockerDriver` and `DestroyDriver` fakes: the call order network, gateway, probe; a failed probe leaves no network.
**Deliverables**: `crates/candler-core/src/jobs/orchestrate/{mod.rs,session.rs}` (`orchestrate`, `JobSession`, `OrchestrateError`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'test(/orchestrate::scaffold/)'` (expected: PASS)
**Commit**: `feat(jobs): start the job gateway on its own network`

## Step 2: Run the plans under max_parallel and the slots
**Goal**: Plans run concurrently up to the smaller of `Budget.max_parallel` and the free machine slots, each in its own slot.
**Implementation Logic**:
1. `orchestrate(rt: Runtime, spec: JobSpec, ctl: JobControl) -> BoxFuture<'static, JobOutcome>` (the `JobBody` shape, D12; job_runtime injects it, cli_run composes `supervise_entry(spec, orchestrate)`): open the session, then drive `spec.plans` in order through a `JoinSet`, at most `max_parallel` in flight. Before each spawn: `Slots::acquire(job, run)` (the slot is held until `exec_run` returns), and a check of the job's cancel receiver (`JobCause::Killed` seen) and the caps (step 3); a cancelled or capped job starts nothing. Planning is `plan_runs(.., PlanMode::Hold)`; the returned `ExperimentLock` is held until the last run ends.
2. Each plan subscribes one `RunControl { cancel, meter }` from `ctl` (D7; the meter is shared by all runs) and calls `exec_run(&rt, &plan, &slot, &run_ctl)`, recording the `RunOutcome` in `live_runs` while in flight and in `sealed_runs` after. Retries of a gateway end are `run_exec`'s; orchestrate never creates an attempt.
3. A run that panics or errors in `exec_run` is not retried: its key goes to `live_runs` until `run_plan`'s `seal_interrupted` can seal it, and `JobOutcome.end` is `failed`.
4. Class A tests: concurrency never exceeds `min(max_parallel, slots)`; plans start in order.
**Deliverables**: `crates/candler-core/src/jobs/orchestrate/schedule.rs` (`Scheduler`, `live_runs`, `sealed_runs`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'test(/orchestrate::scaffold/)'` (expected: PASS)
**Commit**: `feat(jobs): run a job's plans under the parallel cap`

## Step 3: Map a crossed job cap to budget_stop on live runs
**Goal**: The first job cap that crosses stops runs in flight with the cause `budget_stop`, starts nothing more, and leaves sealed runs alone.
**Implementation Logic**:
1. `caps_crossed(budget: &Budget, totals: &Totals) -> Option<CapKind>` is pure: `Totals {runs_started, tokens, cost_usd, elapsed}` (tokens and cost from `JobMeter`'s atomic counters plus sealed `RunOutcome.usage`) against `max_runs`, `max_tokens`, `max_cost_usd`, `wall_s`. A job cap never reads `budget.per_run.*`, which are cell values and end one run `cap` (valid).
2. On a crossing, `stop_live(&live_runs, &tx)` sends `Some(JobCause::BudgetStop)` on the job's `watch::Sender`, which `run_exec` selects on in exactly the runs in `live_runs` (a run not yet started never subscribes; a sealed one is not iterated). Each run stops through its own ladder, so `classify_end` gives `invalid(budget_stop)` by the first rule of R2; `sealed_runs` is never iterated. The scheduler then starts no run and `JobOutcome.end` is `JobEnd::BudgetStop`.
3. A job watcher reacts to `JobMeter` changes and the wall clock and calls `caps_crossed` on each (D7).
4. B34 in `tests/guard_job_orchestrate.rs`, named `b34_cap_stops_live_runs_only`. Doc lines: `Guards: I20 — crossing a job cap gives invalid(budget_stop) on live runs only; sealed runs keep their status; no new run starts.` / `Oracle: R01 §Budget; a literal run table.` / `Bite: apply the cap to all_runs instead of live_runs.`
**Deliverables**: `crates/candler-core/src/jobs/orchestrate/caps.rs` (`caps_crossed`, `Totals`, `CapKind`, `stop_live`); `crates/candler-core/tests/guard_job_orchestrate.rs` (`b34_cap_stops_live_runs_only`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_job_orchestrate)'` (expected: PASS)
**Commit**: `feat(jobs): stop live runs when a job cap crosses`

## Step 4: Tear down through the guard and close the leaf
**Goal**: After the last run is sealed, the gateway and the network are destroyed only through the guard, a failed teardown is evented, and the scaffolding is gone.
**Implementation Logic**:
1. `JobSession::close<'a>(self, scope: &'a SweepScope) -> BoxFuture<'a, TeardownReport>` (called with `SweepScope::CurrentJob`): `ContainerGuard::destroy(&gateway_ref, &SweepScope::CurrentJob)` (D14, the job teardown scope; the sidecar is recorded, so the current-job rule passes), then `destroy_network(&net)`. A refusal or error is not fatal: it emits `container.sweep_incomplete {scope, reason}` and the report names it; the job's `JobOutcome.end` stays what the runs said. It runs on every exit path, including a cancelled job and a failed start.
2. Delete the class-A scaffolding, then write the bite record in the leaf report: the B34 mutation tried and restored with `git checkout --`, the failing assertion quoted.
**Deliverables**: `crates/candler-core/src/jobs/orchestrate/teardown.rs` (`JobSession::close`, `TeardownReport`); the leaf report `__reports__/m2b-execution-parity/job_orchestrate_report_v0.md`
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_job_orchestrate) | binary(guard_layering_i22)' && ! git grep --untracked -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/jobs/orchestrate` (expected: PASS)
**Commit**: `test(jobs): prove a job cap spares sealed runs`
