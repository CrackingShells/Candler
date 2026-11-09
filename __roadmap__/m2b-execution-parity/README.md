# M2b Execution: a container-native isolation runner for Claude Code

## Context
The campaign of milestone M2b (`../candler-v1/contracts/parity/m2b_execution_parity.md`). On M1's contracts it builds the job supervisor, experiment resolution with its preflight steps, run identity and resume, the comparability diff and stratum verdict, the Claude Code driver, and `candler run`, `ps`, `kill` and `watch --json`, with every run inside a Docker container driven through the `docker` CLI.
Eliott ruled (2026-11-06) that M2b keeps every feature of today's host sandbox (htn-creator-eval 0.4.2, contract v5), carried by container mechanisms, and that its essence is the configuration surface (model and endpoint, tools, prompt, skills, MCP sources and configuration, permission mode, budget). The host backend is deferred; the coordinator approved moving Docker from M5 into M2b.
Depth is ordering: L0 (this level) the isolation, store, resolution, driver and evidence blocks; L1 `runs/` planning, execution and the read-side CLI; L2 `runs/jobs/` the detached job runtime, orchestration and `candler run`; L3 `runs/jobs/cli/closeout/` the supervision gates and the equivalence smoke.
Before L0 the coordinator lands Phase 0 on `feat/m2b-execution-parity`: the spikes, the `feature_map/` oracle, the reports, and the scaffold commits S0–S3 (shared traits, types, schemas, dependencies, fixtures and the CI job).

## Reference Documents
- [R01 Architecture](../../__design__/architecture.md) — §Operation API, §Experiment file, §Jobs, runs and supervision, §Process supervision, §Agent environment and secrets, §Harness drivers, §Budget, §CLI surface; invariants I7, I11, I14, I15, I20, I22
- [R02 Milestone leaf](../candler-v1/contracts/parity/m2b_execution_parity.md) — scope and success gates
- [R03 Architecture report](../../__reports__/m2b-execution-parity/00-architecture_v0.md) — normative: decisions, public signatures per leaf, layouts, the field-path table, the ends table, the risk register
- [R04 Test definition](../../__reports__/m2b-execution-parity/01-test_definition_v0.md) — admission rule, the B01–B36 matrix (one owner per property), fixtures, cuts, brief rules, safety rules for tests
- [R05 Spikes](../../__reports__/m2b-execution-parity/02-gateway_spike_v0.md) — gateway design A or B; `03-image_probe_v0.md` beside it fixes the base image and the pinned `FROM`
- [R06 Feature map](feature_map/README.md) — the independent oracle from contract v5: `feature_map.json`, `checks/*.sh`, `reference_flags.txt`, `provenance_fields.txt`; read, never edited by implementers
- [R07 HTN reference](/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion) — tag `htn-creator-eval@0.4.2`, read with `git -C <repo> show htn-creator-eval@0.4.2:<path>`; never written
- [R08 M1 campaign](../m1-contracts/README.md) — the seams M2b builds on and the conventions it inherits; its KT is `__reports__/m1-contracts/01-knowledge_transfer_v0.md`
- [R09 Grit](https://github.com/PCfVW/Amphigraphic-Strict/tree/master/Grit) — the `rust-conventions` skill and its reviewer agent
- [R10 CONTRIBUTING](../../CONTRIBUTING.md) — commit types and scopes, integration through pull requests
- [R11 youzuoqi](/Users/hacker/Documents/explore/youzuoqi) — the earlier docker CLI runner (build/use lock, pinned Claude Code binary, logging proxy); read-only

## Goal
`candler run` executes an experiment from any directory, one Docker container per run, with the token outside the agent container, every container and process destroyed only through I22, resume that replays stored values, and a stratum verdict, all guarded by cornerstone tests with independent oracles.

## Pre-conditions
- [x] [nl] Eliott approved the plan, and the coordinator approved the scope move with conditions (2026-11-06)
- [ ] [cmd] `git merge-base --is-ancestor origin/main feat/m2b-execution-parity`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the checkout's path
- [ ] [cmd] `rustup target list --installed | grep -qx x86_64-unknown-linux-gnu && docker version --format '{{.Server.Version}}'`
- [ ] [cmd] Phase 0 is landed: `for s in '^docs(design): ' '^build(repo): ' '^ci(repo): ' '^feat(schemas)!*: ' '^feat(orchestrator): scaffold'; do git log --format=%s origin/main..feat/m2b-execution-parity | grep -q "$s" || exit 1; done && test -s __roadmap__/m2b-execution-parity/feature_map/feature_map.json`
- [ ] [cmd] The feature map covers contract v5: `cd __roadmap__/m2b-execution-parity/feature_map && test -z "$(comm -23 <(sh ids_from_contract.sh) <(jq -r '.[].id' feature_map.json | sort))"`
- [ ] [nl] The api_review verifier reports no open finding on the scaffold against M2a, M3, M4 and M5
- [x] [nl] Eliott chose the smoke model on RiVault: `RiVault/Reasoning-Small` (Qwen3.8-27B) with `--effort medium`, and approved pushing the branch for a draft PR after the scaffold (2026-11-06)

## Success Gates
- ⬜ [cmd] STD, DRIFT and LINUX pass on `feat/m2b-execution-parity`
- ⬜ [cmd] SUBJ passes over `origin/main..feat/m2b-execution-parity`
- ⬜ [cmd] `cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_container_e2e)'` passes on macOS against Docker Desktop
- ⬜ [cmd] No e2e placeholder is left, and the binary runs more than zero tests: `! git grep -n 'fn placeholder_' -- crates/candler-core/tests` and the summary line of the previous command reports at least one test run
- ⬜ [cmd] The Linux half: `gh run list --workflow container-e2e --commit "$(git rev-parse HEAD)" --json conclusion -q '.[0].conclusion'` prints `success`, and its log names every e2e test R04 marks Linux; until then this gate is recorded as not yet run, never as passed
- ⬜ [cmd] `diff <(cd crates/candler-schemas/schema && printf '%s\n' *.schema.json | sort) __roadmap__/m1-contracts/interfaces/services/import/closeout/expected_roots.txt` prints nothing (M1's list, which ab06c45 extended with `container-record`; A57)
- ⬜ [cmd] At most 40 new guards (the I22 rows in `guard_signal.rs` belong to M1's guard, A49 and A57) and no scaffolding left: `test "$(git diff origin/main...feat/m2b-execution-parity -U0 -- crates ':!crates/candler-core/tests/guard_signal.rs' | grep -c '^+ *///* Guards:')" -le 40 && ! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates`
- ⬜ [cmd] The parent tree is amended and valid: `! git grep -n -e 'Docker is not needed' -e 'not needed before this milestone' -e 'Docker enters the project here' -- __roadmap__/candler-v1 __design__ && git grep -q 'under container isolation' -- __roadmap__/candler-v1/contracts/parity/m2b_execution_parity.md && bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/`
- ⬜ [nl] `runs/jobs/cli/closeout/` is `done`: B36 decides the kill gate (no container labelled for the job and no marked host process after `ps`/`run`, macOS and Linux), and the equivalence smoke decides R02's other gates
- ⬜ [nl] Carved out of R02's first gate (sandbox composition match): the feature_map rows S2, X19 and X32 and the `layout` check are owned by M2c (`parity/alignment/m2c_wire_alignment.md`, amendment A1 on `parity/README.md`; ruling A58). The smoke reports them as failing with that owner, and the KT records them under M2c, never as passed
- ⬜ [nl] Every leaf is `done`, and every required verification and Grit review reports no open finding
- ⬜ [nl] The KT records the intended deviations from `run-config` (each provenance field whose meaning changes, such as the workspace path and env), the accepted macOS gap and what bounds it, and a "For M4" section

## Gotchas
- **One CLI error type and one exit mapping (D27).** Every verb is `async fn run(Args) -> Result<Exit, CliError>`; `main.rs` maps `Ok(Exit::Success)` to 0, `Ok(Exit::Changed { .. })` to 1 and any `CliError` to 2, so an M2b verb that fails exits 2. A non-error status is a new `Exit` variant, never a `CliError` variant or a raw `ExitCode`. After the rebase onto M2a, the store-root lookup, the resolver composition and the `EventSink` come from M2a's `commands/common.rs`, never a copy; `register_m2b_steps` is composed one call per line there, as M2a's steps are.
- **Condition tags.** Every Pre-condition and Success Gate line starts with `[cmd]` (a command whose exit status decides) or `[nl]` (a judgement, naming a proxy check where one exists).
- **Standard gates**, named in every leaf. They are M1's with the base moved:
  - **STD**: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`;
  - **LINUX**: `cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings`;
  - **DRIFT**: `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas -E 'binary(schema_drift)' && test -z "$(git status --porcelain crates/candler-schemas/schema)"`;
  - **SUBJ**: `cog check <base>..HEAD && git log --format=%s <base>..HEAD | awk 'length>72{b=1}END{exit b}'`, with `<base>` the leaf's branch point on `feat/m2b-execution-parity`;
  - **DEP(x)**: `git log --first-parent --format=%s feat/m2b-execution-parity | grep -qx 'merge(<scope>): land x'`;
  - **E2E(f)**: `cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_container_e2e) & test(/f/)'`, which needs Docker. A missing Docker makes the test panic; it never skips.
- **Safety, binding.** In M1 a mutation test sent `kill(-1)` and killed 394 of Eliott's processes, the Claude app among them. The same rules now cover the Docker daemon, which runs Eliott's own containers.
  - **Signals.** Every signal goes through the I22 guard, which signals only process groups this supervisor spawned and re-verified (leader pid and start time). It always refuses ≤1, its own pgid and its own pid.
  - **Destruction.** Every container stop, container `rm` and network `rm` goes through `ContainerGuard`. It destroys only ids recorded in `children/container-<cid12>.json`, or labelled for the current job or a lost one, re-verified with `docker inspect` right before acting. Never by name, prefix or pattern, never "all".
  - **Bite tests.** Mutation, bite and adversarial tests of signalling or destruction use only the recording `Signaller` and the recording `DestroyDriver`, from the `test-support` feature, and run only in `guard_signal` or `guard_container_guard`.
  - **Real-daemon tests.** A test against a real daemon creates its own containers under a per-test `candler.job=<id>` label. It wraps its driver in a test-side allowlist of the ids it created, asserts only on its own label, and destroys only through the guard. No test helper or script runs `docker rm` by name or prefix.
  - **Before every run that spawns processes or containers:** `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)'`.
  - **Restoring a bitten file:** use `git checkout -- <file>`, never `mv`.
  - **Never run:**
    - `kill`, `pkill` or `killall`;
    - `docker rm -f $(…)`;
    - `docker kill` or `docker stop` of an id you did not create;
    - any `docker … prune`.
  - **Tests run as non-root.**
  - **`test-support`** is never enabled on a production dependency edge.
- **Owned files.** Implementers do not edit:
  - `__design__/`, `__roadmap__/`, `__reports__/` (except their own leaf report), or any README;
  - any `mod.rs`, `lib.rs` or `main.rs` module declaration, or any `Cargo.toml`;
  - `crates/candler-schemas/`;
  - `tests/support/mod.rs` or `tests/public_surface.rs`;
  - `.github/` or `.config/nextest.toml`;
  - `assets/docker/testagent/`, `__roadmap__/m2b-execution-parity/feature_map/`, or another leaf's modules or `assets/docker/<dir>/`.
  The scaffold declares every module and stub. A new dependency, module, schema field or design ruling is a request to the coordinator, who lands it in its own `build(repo)`, `feat(schemas)` or `docs(design)` commit.
- **Tests (R04).**
  - **What a leaf commits.** Only the B tests R04 assigns to it, in `tests/guard_<topic>.rs` files with `#![cfg(test)]`.
  - **Doc lines.** Each B test carries three: `/// Guards: <I#|B#> — <property>.`, `/// Oracle: <source>.` and `/// Bite: <one-line mutation>.`
  - **Caps.** At most 4 B tests per leaf, 7 for `container_stop`, and 2 per L2 or L3 leaf. A new B test needs the coordinator's OK and a mutation no other guard catches.
  - **Class A.** These tests live only in `#[cfg(test)] mod scaffold` or `tests/scaffold_<leaf>.rs`, and the leaf's last commit deletes them.
  - **Banned shapes:**
    - argv or lists copied from a builder;
    - re-checks of serde, schemars or a derive;
    - self-blessed goldens;
    - "the fake received a call" when only the fake runs;
    - tests of stubs or defaults;
    - a constant checked against itself;
    - importing `isolation::layout` or `experiment::paths` into a test.
  - **Bite record.** The leaf report gives a tried mutation per B test.
- **Oracle rules** (M1's, extended):
  - Expectations are literals with a provenance comment, from R01, R04, R06 or the OS, or from a real docker or kernel fact reported by the testagent. Never from the code under test.
  - Fakes are environments, not oracles, and every refusal test has its positive control.
  - The `feature_map/` oracle was written by an agent who never saw Candler's code. The B03 and B29 rows were drafted by the coordinator in R04: add rows, never edit them.
- **Docker practice.**
  - Base images are pinned by digest, and every run records its local image ID as `isolation.image`, never a tag.
  - Containers run with:
    - `--init`, `--read-only` and a tmpfs `/tmp`;
    - `--cap-drop ALL` and `--security-opt no-new-privileges`;
    - pids, memory and cpu limits;
    - the labels `candler.job`, `candler.run` and `candler.experiment`;
    - a `--cidfile`;
    - no `--rm`;
    - on a Linux engine, `--user <host uid>:<gid>`.
  - The docker client runs with its own env allowlist (`DOCKER_HOST`, `DOCKER_CONTEXT`, `HOME/.docker`), separate from the agent's, which is built from nothing.
  - Docker Desktop is arm64, and CI's `ubuntu-latest` is amd64. Images are built locally per architecture.
- **Shell safety:**
  - absolute paths only;
  - commands chained with `&&` or under `set -e`;
  - never run git in another checkout;
  - commit messages go in unique `$TMPDIR/m2b-<leaf>-*` files, committed with `git commit -F` after `cog verify --file`.
- **Environment.**
  - In a worktree, first check that `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path, and that `cargo nextest list -p candler-core --message-format json` lists tests from that path.
  - Each implementer uses its own `CARGO_TARGET_DIR`.
  - Local tools: cargo 1.99 with `cargo nextest`, `uv run` (no mamba env), Node, the `x86_64-unknown-linux-gnu` target, and Docker Desktop.
  - Linux-only gates are decided by the `container-e2e` CI job.
- **Grit lessons up front:**
  - one-paragraph schema docs;
  - `BORROW` and `INDEX` annotations in tests;
  - `#![cfg(test)]` in integration tests;
  - explicit lifetimes;
  - every enum variant listed before `_`;
  - source says what is, with no history or meta-discourse.
- **Wire rules** are M1's:
  - `null` is never written and is refused on input;
  - patterns end with `(?![\s\S])`;
  - MCP keys stay camelCase, and every other key is snake_case.
- **Spec defects** found while implementing go to the coordinator as findings. Never implement them faithfully.
- **Verification policy.**
  - **Adversarial verifiers:** `comparability`, `container_stop`, `runs/run_plan`, `runs/jobs/job_runtime`.
  - **Plain verifiers:**
    - `gateway`, which becomes adversarial under design B;
    - `runs/run_exec`, limited to the ends table;
    - `closeout/supervision_gates`, narrowed to proving the e2e gates bite.
  - **Also:** one api_review of the scaffold before L0, and one Grit reviewer per level, which also checks R04's test-shape list.
  - **Every verifier** states each test's oracle, names tautologies, and re-runs three bites from the leaf report plus one of its own. At most two of those four are e2e, and all follow the safety rules.
  - **Findings.** Fixes go back to the implementer, and confirmation goes to the verifier that raised the finding.
- **Critical conditions,** ranked:
  1. A token reaches the agent container, the store, a log or `docker inspect`.
  2. A container or network is destroyed outside I22.
  3. Partial runs are deleted, or a stale resume is accepted.
  4. A wrong "comparable" verdict.
  5. A container outlives its supervisor.
  6. The ends table misclassifies a run.
  7. The bind-mount uid is wrong on Linux.
  8. A configuration path lands under a reserved root, or a typed root does not enter the cell.
- **Nothing is pushed** until Eliott says so, and nothing goes to crates.io. Using RiVault, which spends tokens, needs Eliott's go and his choice of model.
- **M2a shared surfaces** (coordinator, 2026-11-06): M2a lands the clap skeleton in `crates/candler/src/main.rs` (a `Command` enum and one `commands/<verb>.rs` per verb); M2b appends `Run`, `Ps`, `Kill` and `Watch` one line each, adds no CLI dependency of its own, and the second to merge rebases. M2a adds `evaluation::rpc`, `RequestId::new_v7` and uuid `v7` to `candler-schemas`; whoever lands second rebases and re-blesses. `candler-core` has one JSON Schema validator, M2a's pinned `jsonschema`; reuse it. In `__design__/architecture.md` M2b edits only §Process supervision, §Agent environment, §Ends and the container-backend line of "Deliberately open". M2a's answer (2026-11-06): it adds no `EventPayload` or `InvalidReason` variant (one new root, `evaluator-message`); `Supervisor::new` and `SpawnRequestBuilder` stay source-compatible, the stop hook opt-in per spawn only; steps register through `Resolver::register` from `crates/candler`; `isolation.env_lock` is M2b's, and `comparable.environments[]` stays reserved for M5. The run-manifest fields are frozen.

## Status
```mermaid
graph TD
    job_store[Job store: dir, lock, events, slots]:::planned
    comparability[Comparability diff and stratum verdict]:::planned
    image[Image: pinned base and env layers]:::planned
    container_run[Container run: argv, mounts, expiry]:::planned
    container_stop[Container stop: guard, hook, sweeper]:::planned
    gateway[Gateway: token outside the agent]:::planned
    experiment_resolution[Experiment resolution and preflight]:::planned
    claude_code_driver[Claude Code driver]:::planned
    run_evidence[Run evidence: catalog, census, leaks]:::planned
    runs[Runs]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `job_store.md` | 📄 Leaf Task | ⬜ Planned |
| `comparability.md` | 📄 Leaf Task | ⬜ Planned |
| `image.md` | 📄 Leaf Task | ⬜ Planned |
| `container_run.md` | 📄 Leaf Task | ⬜ Planned |
| `container_stop.md` | 📄 Leaf Task | ⬜ Planned |
| `gateway.md` | 📄 Leaf Task | ⬜ Planned |
| `experiment_resolution.md` | 📄 Leaf Task | ⬜ Planned |
| `claude_code_driver.md` | 📄 Leaf Task | ⬜ Planned |
| `run_evidence.md` | 📄 Leaf Task | ⬜ Planned |
| `runs/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
