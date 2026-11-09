# Supervision gates

**Goal**: With a real Docker daemon, a SIGKILLed supervisor leaves, after `ps` or `run` recovers the job, no container or network labelled for the job, no marked host process and no canary anywhere in the store, and the eight e2e guards of M2b are shown to bite when the sweeper and the label query are mutated.
**Pre-conditions**:
- [ ] [cmd] The Phase 0 scaffold is landed: `for s in '^docs(design): ' '^build(repo): ' '^ci(repo): ' '^feat(schemas)!*: ' '^feat(orchestrator): scaffold'; do git log --format=%s origin/main..feat/m2b-execution-parity | grep -q "$s" || exit 1; done && test -s __roadmap__/m2b-execution-parity/feature_map/feature_map.json`
- [ ] [nl] api_review closed with no open finding on the e2e layout (`tests/guard_container_e2e.rs` with one `#[path]` module per leaf, the `AllowlistDriver` helper)
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
- [ ] [cmd] DEP(job_runtime), DEP(job_orchestrate), DEP(cli_run), DEP(cli_ps_kill_watch), DEP(run_exec), DEP(container_run), DEP(container_stop) and DEP(gateway) hold: `for l in job_runtime job_orchestrate cli_run cli_ps_kill_watch run_exec container_run container_stop gateway; do git log --first-parent --format=%s feat/m2b-execution-parity | grep -qx "merge(.*): land $l" || exit 1; done`
- [ ] [cmd] `docker version --format '{{.Server.Version}}'` prints a version (a missing Docker makes the e2e tests panic, never skip)
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_container_e2e) & test(/b36/)'` passes on macOS against Docker Desktop. B36 SIGKILLs a real supervisor the test spawned and holds a `Child` for, then recovers the lost job through `jobs::lost::classify` and `recover`, and asserts: `docker ps -a --filter label=candler.job=<id> -q` is empty, `docker network ls --filter label=candler.job=<id> -q` is empty, no host process carries this job's `CANDLER_RUN`, every run is sealed `invalid(interrupted)`, and the canary is absent from the store, job dir, events, `supervisor.log` and logs (`assert_clean`, with its non-empty-scan assertion). Oracle: the daemon's own label filters and the OS
- ⬜ [cmd] The macOS e2e suite passes whole: `cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_container_e2e)'` runs seven tests under design B (`b06_kernel_facts`, `b08_self_expiry`, `b12_sweep_scope`, `b13_client_killed_container_gone`, `b17_canary_absent`, `b18_internal_network_no_route`, `b36_supervisor_sigkill_nothing_left`), none skipped
- ⬜ [cmd] The Linux half: `gh run list --workflow container-e2e --commit "$(git rev-parse HEAD)" --json conclusion -q '.[0].conclusion'` prints `success`, and `gh run view <id> --log` names all eight tests including `b09_host_uid_sealable`, none skipped. Until the CI run exists and passes, this gate is recorded as **not yet run**, never as passed
- ⬜ [cmd] STD passes
- ⬜ [cmd] LINUX passes (`cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings`)
- ⬜ [cmd] SUBJ passes, with `<base>` the leaf's branch point on `feat/m2b-execution-parity`
- ⬜ [cmd] No schema is touched, so DRIFT has nothing to bless: `git diff --quiet <base>..HEAD -- crates/candler-schemas`
- ⬜ [cmd] No scaffolding: `! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/tests`
- ⬜ [cmd] Its e2e module is filled: `! git grep -n 'fn placeholder_supervision_gates' -- crates/candler-core/tests`. S3 declares the module with one ignored `placeholder_supervision_gates` test that panics under `--run-ignored only`; this leaf replaces it with its own guards, so the container gate never counts an empty module as passed
- ⬜ [nl] The leaf report records the B36 bite (the lost-job path skips the container sweep) and the two gate-bite proofs of step 3, each restored with `git checkout --`
- ⬜ [nl] The narrowed verifier reports no open finding: it proves the e2e gates bite by mutating the sweeper (a no-op `LabelSweeper::sweep`) and the label query (the `candler.job=<id>` filter dropped from `ps_by_label`), re-running B36, B12 and B13 under rule 7, with at most two of them e2e, and it states each test's oracle
- ⬜ [nl] The accepted macOS gap is recorded: setsid'd host descendants have no subreaper on macOS, so B36 asserts on the marked processes it can see there and Linux CI decides the rest
**References**: [R01 §Process supervision](../../../../../../__design__/architecture.md) — lost-job reaping; [R03 §6, §8 L3, §10.1, §15 risk 5](../../../../../../__reports__/m2b-execution-parity/00-architecture_v0.md) — the lost-job path and the guard's scope; [R04 B36, safety rules for tests, expected e2e names](../../../../../../__reports__/m2b-execution-parity/01-test_definition_v0.md) — oracle, allowlist wrapper, F3

## Step 1: Write B36 against a real daemon
**Goal**: A supervisor killed mid-run leaves nothing labelled for its job once recovery has run.
**Implementation Logic**:
1. The module for this leaf in `tests/guard_container_e2e.rs` (declared by S3 through `#[path]`) holds `b36_supervisor_sigkill_nothing_left`. The job id is `t-b36-<random>`. Every docker call in the test and in the supervisor it spawns goes through `AllowlistDriver<RealDriver>`, which panics on `stop`, `rm`, `network_rm` or a signal for any id the test did not create. The supervisor child records the ids it creates in a side file the test seeds its own allowlist from. The test asserts only on `candler.job=<job id>`.
2. The supervisor is the test binary re-executed with `--exact supervisor_child` (as in B32), running `supervise_entry(spec, orchestrate)` (spec on stdin, output in `supervisor.log`, D8) over the testagent image in its `ignore-term` mode, the real gateway sidecar, and a local fake upstream that echoes `Authorization`. The token is a fresh `openssl`-style random canary returned by a test `SecretSource`.
3. When the agent container is running (polled by the job's label), the test SIGKILLs the pid of the `Child` it holds (never another pid, F3). It then checks the lock is free and no `job.ended` exists, so `classify` returns `Lost`, calls `recover` with the real `LabelSweeper` and `ContainerGuard` (the path `ps` and `run` take), and asserts as in the gate. The positive control is the same job read before the kill: one agent container and one gateway are present, so the empty list afterwards is a change, not an absence.
4. Doc lines: `Guards: I15, I19, I20, I22 — a killed supervisor leaves nothing labelled for its job, no marked process, no network, no canary.` / `Oracle: docker ps -a and network ls filtered by the test's own label; assert_clean.` / `Bite: the lost-job path skips the container sweep.`
**Deliverables**: the S3-declared e2e module for this leaf under `crates/candler-core/tests/` (`b36_supervisor_sigkill_nothing_left`, the `supervisor_child` re-exec helper)
**Consistency Checks**: `cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_container_e2e) & test(/b36/)'` (expected: PASS)
**Commit**: `test(orchestrator): prove a killed supervisor leaves no container`

## Step 2: Run the whole e2e suite and wire the Linux half
**Goal**: The macOS suite runs all seven tests green, and the CI job that decides the Linux half names all eight.
**Implementation Logic**:
1. Run the pre-spawn check, then the whole `guard_container_e2e` binary, and record the test names that ran and the run time (under 90 s per leaf, R04 rule 9).
2. Read `.github/workflows` for the `container-e2e` job (this leaf does not edit CI) and confirm it runs the same filter on `ubuntu-latest` and asserts the eight names; a missing assertion is a spec defect for the coordinator.
3. After the branch is pushed (Eliott's call, not this leaf's), read the run with `gh run list --workflow container-e2e --commit "$(git rev-parse HEAD)"`. Without a run, the Linux gate is recorded as not yet run.
**Deliverables**: the leaf report `__reports__/m2b-execution-parity/supervision_gates_report_v0.md` (the names that ran, the timings, the Linux status)
**Consistency Checks**: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)' && cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_container_e2e)'` (expected: PASS)
**Commit**: `test(orchestrator): run the e2e gates on a real daemon`

## Step 3: Prove the e2e gates bite, then close the leaf
**Goal**: Each of the sweeper and the label query, mutated, makes the e2e guards that depend on it fail, and the repository is restored.
**Implementation Logic**:
1. Before each mutation: `git diff --quiet HEAD -- crates/candler-core/src`, `cargo clean -p candler-core`, the `guard_signal` run. The mutations are temporary edits of another leaf's file, never committed.
2. **Sweeper**: make `LabelSweeper::sweep` return an empty `ContainerSweep`. Expect B36 and B13 to fail (containers labelled for the job remain). Clean up only through the allowlist wrapper's `Drop` guard and the test's own recorded ids.
3. **Label query**: drop the `candler.job=<id>` filter from `ps_by_label`. Run only B12, because only inside `AllowlistDriver`, which panics on any id the test did not create, is this mutation safe; confirm the panic names a foreign id, and that no container of Eliott's is touched. Never run this mutation outside the wrapper, and never against B36 or B13.
4. Restore each file with `git checkout -- <file>`, rebuild after `cargo clean -p candler-core`, and confirm `git status --porcelain crates` is empty and `docker ps -a --filter label=candler.job=<id> -q` prints nothing for each test's job id.
5. Write the bite record: the B36 mutation, the two proofs, each with the failing assertion quoted, and the restore command run.
**Deliverables**: the leaf report `__reports__/m2b-execution-parity/supervision_gates_report_v0.md` (the bite record)
**Consistency Checks**: `git diff --quiet HEAD -- crates && cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_container_e2e)' && ! git grep --untracked -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/tests` (expected: PASS)
**Commit**: `reports(orchestrator): record the e2e gate bites`
