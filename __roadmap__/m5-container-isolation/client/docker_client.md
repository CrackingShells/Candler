# Docker client

**Goal**: Every docker process the core starts is built by `isolation/docker.rs`. Every control container comes from one `ControlSpec` on one `hardening(limits)`. The trait gains `inspect_many` and `create` for A1 and A8. The agent's argv stays byte-identical (audit A5).
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] OWNED: `crates/candler-core/src/isolation/{docker,driver,testing,gateway,image}.rs`, `isolation/hardening.rs` and `isolation/control.rs` (new), `isolation/container/{run,exec}.rs` (the client and control parts only), `runner/evidence/catalog.rs`, `jobs/orchestrate/controls.rs`, the test support under `crates/candler-core/tests/it/support/` and `crates/candler/tests/it/support/`, new e2e files
- [ ] [nl] It keeps the public signatures of `DockerImageService::{ensure, base}`, which `plugin_environment` builds on
**Success Gates**:
- ✅ [cmd] One client: `test -z "$(git grep -l 'SpawnRequest::builder' -- crates/candler-core/src/isolation crates/candler-core/src/runner/evidence | grep -v isolation/docker.rs)"` and `test -z "$(git grep -l '"docker"' -- crates/candler-core/src | grep -v isolation/docker.rs)"`
- ✅ [cmd] The agent's argv is unchanged: `git diff --quiet 18142ec -- crates/candler-core/tests/fixtures/harness_baseline` and `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_harness_facts::/)'`
- ✅ [cmd] Every control kind carries the hardening and its own limits, and every docker spawn carries only the allowlisted client names: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_container_run::control_kind_/) | test(/^guard_container_run::client_env_/)'`
- ✅ [cmd] The real-image guard passes with Docker up: `cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*control_each_kind_runs_on_the_real_base/)'`
- ✅ [cmd] The existing e2e still pass on images built through the core: E2E
- ✅ [cmd] SPAWN, STD (`-p candler-core -p candler`), LINUX, DIGEST and SUBJ pass
- ✅ [nl] The leaf report records why the gateway's client passed `PATH`, and whether it was kept (with its reason for R01) or removed; the bite of each guard; and the docker spawn sites found against the plan's six
- ✅ [nl] The review of the final diff closed with no open finding. It attacked argv byte-equality against `argv.json`, the control-kind table (a flag dropped, limits unified upward), and the `PATH` reason
**References**: [R04 A5](../../../__reports__/interlude-audit/00-findings_audit_v0.md) — the copies; [R06 D6 and its file layout](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md); [R01 §Process supervision](../../../__design__/architecture.md) — control containers, the client allowlist

## Step 1: Add multi-id inspect and create to the client
**Goal**: `DockerDriver` has `inspect_many` (one spawn for many ids) and `create` (returns the id), with recording fakes, so A1 and A8 can build on them in parallel.
**Implementation Logic**:
1. `inspect_many(ids) -> Vec<Result<Inspected, DockerError>>`: one `docker container inspect <ids…>` spawn. One gone id maps to `NotFound` for that id, the rest keep their facts. Measure the exit code and output when one id is missing, on this engine, and record it in the report.
2. `create(args) -> ContainerId`: `docker create`, with the id read from stdout.
3. Both go in `isolation/driver.rs`, `DockerCli` in `isolation/docker.rs`, and the fakes in `isolation/testing.rs`.
4. A guard for the mapping, written first: one spawn for three ids, and one missing id mapped alone. Its oracle is the measured output, recorded as a fixture.
**Deliverables**: `crates/candler-core/src/isolation/driver.rs`: `DockerDriver::inspect_many`, `DockerDriver::create`; `crates/candler-core/src/isolation/docker.rs`: their `DockerCli` impls; `crates/candler-core/src/isolation/testing.rs`: the fakes; `crates/candler-core/tests/it/guard_container_guard.rs`: `inspect_many_maps_each_id`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/inspect_many_maps_each_id/)'` (expected: PASS)
**Commit**: `feat(isolation): inspect many containers in one call and create by id`

## Step 2: Build every docker process in the one client
**Goal**: No module outside `isolation/docker.rs` names the docker binary, builds its environment or builds a docker `SpawnRequest`. One `ControlSpec` and one `hardening(limits)` serve every container.
**Implementation Logic**:
1. Verify each site against the code before changing it:
   - `isolation/gateway.rs` (its own `DockerCli`);
   - `container/run.rs` (`docker_binary`, `client_env`, `CLIENT_ENV`, `control_argv`, the agent's `SpawnRequest`);
   - `isolation/image.rs` (`compile_argv`, `control_argv_with`, `control_output`, `compile`);
   - `runner/evidence/catalog.rs`;
   - `container/exec.rs` (`collect_args`);
   - `jobs/orchestrate/controls.rs` (`fake_args`).
2. Find out why the gateway's client passes `PATH`, perhaps for credential helpers. Keep it in the one allowlist only with a reason, recorded for R01; otherwise drop it.
3. `isolation/hardening.rs`:
   - `hardening(limits)`: the read-only root, `/tmp` tmpfs, cap-drop ALL, no-new-privileges, and the limits given by the caller;
   - `Role {Agent, Gateway, Egress, Control}` with its one label value.

   No limit changes for any container. Each keeps today's values, passed in.
4. `isolation/control.rs`: `ControlSpec` covers `--rm`, `candler.role=control`, network `none` or `bridge` (the lock compile only), mounts, env, an optional detach, and one mapping from exit to `DockerError`. The probe, packages, compile, catalog, collect and fake-model controls become `ControlSpec`s.
5. The agent's supervised request moves into `docker.rs`. Its argv stays byte-identical; A8 changes it later.
6. Guards first, seen red:
   - `control_kind_*`: a table over the six kinds, asserting each kind's flags and limits. Oracle: R01's hardening list and today's limits, typed by hand from 18142ec. Bite: a flag dropped from the builder, or limits unified upward.
   - `client_env_*`: whatever the caller's environment, every docker spawn's env is exactly the allowlist.
**Deliverables**: `crates/candler-core/src/isolation/hardening.rs`: `hardening`, `Role`; `crates/candler-core/src/isolation/control.rs`: `ControlSpec`; `crates/candler-core/src/isolation/docker.rs`: the agent request builder and the one `ClientEnv`; the callers in `gateway.rs`, `image.rs`, `catalog.rs`, `exec.rs`, `run.rs`, `controls.rs` on them; `crates/candler-core/tests/it/guard_container_run.rs`: `control_kind_*`, `client_env_*`
**Consistency Checks**: `test -z "$(git grep -l 'SpawnRequest::builder' -- crates/candler-core/src/isolation crates/candler-core/src/runner/evidence | grep -v isolation/docker.rs)" && git diff --quiet 18142ec -- crates/candler-core/tests/fixtures/harness_baseline && cargo nextest run -p candler-core --no-tests=fail -E 'test(/control_kind_|client_env_/) | test(/^guard_harness_facts::/)'` (expected: PASS)
**Commit**: `refactor(isolation): build every docker process in one client`

## Step 3: Build test images through the core and run every control on the real base
**Goal**: Every e2e finds or builds its images through the core and removes what it built by id. Every control kind is shown to run on the real base image.
**Implementation Logic**:
1. One shared test-support function, `test_image(context) -> (ImageId, Built)`. It uses `DockerDriver::build` with a label keyed by the context's digest. It finds an image with that label or builds it, and a guard removes the image by id only if it built it. It serves testagent, the fake model and the gateway sidecar in every e2e that built them by raw `docker build` or `require_agent_image`.
2. Tell the coordinator which CI step builds testagent by hand. The coordinator removes it from `.github/workflows/container-e2e.yml` at the merge.
3. `control_each_kind_runs_on_the_real_base` (`guard_container_e2e`): the version probe, the package probe, the catalog capture and the collect run on the real base built through `DockerImageService::base()`, each through its `ControlSpec`. Bite: a builder that sets no `HOME`, which made OpenCode's probe fail in M4.
4. Record `docker images -q` before and after a full E2E run in the report.
**Deliverables**: the shared support function `test_image` in `crates/candler-core/tests/it/support/`, re-used by `crates/candler/tests/it/support/` where the cli e2e need it; `crates/candler-core/tests/it/guard_container_e2e/control_e2e.rs`: `control_each_kind_runs_on_the_real_base`
**Consistency Checks**: `cargo nextest run -p candler-core -p candler --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::/) | test(/^guard_cli_e2e::/)'` (expected: PASS)
**Commit**: `test(isolation): build the e2e images through the core`
