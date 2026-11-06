# CLI: run

**Goal**: `candler run <experiment>` from any directory composes M2b's and M2a's preflight steps through `Resolver::register`, resolves and plans, writes `job.json`, starts a detached supervisor with the serialised spec on its stdin, prints the job id and returns at once; the hidden supervise verb enters `jobs::supervise_entry(spec, orchestrate)`.
**Pre-conditions**:
- [ ] [cmd] The Phase 0 scaffold is landed: `for s in '^docs(design): ' '^build(repo): ' '^ci(repo): ' '^feat(schemas): ' '^feat(orchestrator): scaffold'; do git log --format=%s origin/main..feat/m2b-execution-parity | grep -q "$s" || exit 1; done && test -s __roadmap__/m2b-execution-parity/feature_map/feature_map.json`
- [ ] [nl] api_review closed with no open finding on `register_m2b_steps`, the `Command` enum's `Run` variant and the hidden supervise variant, `request.harness`, `request.treatment` and `request.treatment_source`, `M2bServices`, and the stdin spec handoff (D8)
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
- [ ] [cmd] DEP(job_runtime), DEP(job_orchestrate), DEP(job_store), DEP(run_plan), DEP(experiment_resolution), DEP(run_evidence) and DEP(claude_code_driver) hold: `for l in job_runtime job_orchestrate job_store run_plan experiment_resolution run_evidence claude_code_driver; do git log --first-parent --format=%s feat/m2b-execution-parity | grep -qx "merge(.*): land $l" || exit 1; done`
- [ ] [cmd] The CLI skeleton exists, from M2a or from S3 (R24): `git grep -q 'enum Command' -- crates/candler/src/main.rs && test -f crates/candler/src/commands/run.rs`
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler -E 'binary(guard_cli_run)'` passes. B35 runs the built `candler run` from an unrelated temp dir over a PATH-fake `docker` whose `run` sleeps a few seconds, and asserts: the launcher exits while the supervisor named in `job.json` is alive (`kill(pid, 0)` and start time) and the job lock is held (a test-side non-blocking `flock` fails with `EWOULDBLOCK`); after `job.ended`, no process whose environment carries a `CANDLER_RUN` of this job remains (`ps eww` text), and every container id the fake handed out has a matching `rm` line in its log. Oracle: the OS and the PATH-fake `docker` log
- ⬜ [cmd] STD passes
- ⬜ [cmd] LINUX passes (`cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings`)
- ⬜ [cmd] SUBJ passes, with `<base>` the leaf's branch point on `feat/m2b-execution-parity`
- ⬜ [cmd] No schema is touched, so DRIFT has nothing to bless: `git diff --quiet <base>..HEAD -- crates/candler-schemas`
- ⬜ [cmd] No scaffolding: `! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler/src/commands/run.rs crates/candler/tests/guard_cli_run.rs`
- ⬜ [cmd] This leaf adds no CLI dependency and edits no module declaration: `git diff --quiet <base>..HEAD -- crates/candler/Cargo.toml crates/candler/src/main.rs`
- ⬜ [nl] The leaf report records the B35 bite: wait on the supervisor in the launcher (no detach); the launcher then exits only after `job.ended` and B35 fails; restored with `git checkout --`
- ⬜ [nl] Reading `commands/run.rs` confirms that the resolver composition registers M2a's `EnvironmentStep` and M2b's steps only through `Resolver::register` (no second resolver), that `isolation.env_lock` is M2b's and `environments[]` is never written, and that no path is read relative to the process cwd except one the user typed on the command line (I14)
**References**: [R01 §CLI surface over the jobs, §Run identity and resume](../../../../../__design__/architecture.md) — what `run` does and prints; [R03 §3 decision 15, §8 L2, §11, §16 F2 F3](../../../../../__reports__/m2b-execution-parity/00-architecture_v0.md) — composition, the request settings, the field paths; [R04 B35](../../../../../__reports__/m2b-execution-parity/01-test_definition_v0.md) — oracle and bite; [README, M2a shared surfaces](../../../README.md) — the clap skeleton and the step registration M2a answered

## Step 1: Compose the resolver
**Goal**: One `Resolver` carries M2b's four steps and M2a's `EnvironmentStep`, each registered through `Resolver::register`.
**Implementation Logic**:
1. `compose_resolver(config: &Config) -> Result<Resolver, ResolveError>` starts from M1's resolver, calls `register_m2b_steps(r, services)` (the experiment_resolution leaf: steps `image_id`, `catalog`, `harness_identity`, `treatment_identity`; D3, D6), where `services` is `M2bServices { supervisor, images, catalog, harness_supported }` (declared by S3; this leaf only builds it): `supervisor` is the runtime's `Arc<Supervisor>`, `images` and `catalog` are the real implementations over it, and `harness_supported` is taken from the claude_code driver's `supported()`. Then registers M2a's `EnvironmentStep` with `Resolver::register(Box::new(..))`. Each call takes `self` and returns `Result<Self, ResolveError>`, so a duplicate step name is an error, not a silent second step. The composition lives in `crates/candler`, as M2a answered (2026-11-06).
2. `request_settings(file: &ExperimentFile) -> Vec<(FieldPath, JsonValue)>` carries the typed `harness` and `treatment` tables as the settings `request.harness` and `request.treatment` (F2 (a), R21), and fills `request.treatment_source` (D2) beside them: a path resolved against the experiment file's directory, or against the caller's cwd when it came from the CLI. A step reads all three through `ResolveContext::get`. They are settings: they never reach `Comparable`.
3. Class A tests (`mod scaffold`): both step sets are present and in a legal order; a second `register_m2b_steps` fails; `request_settings` round-trips a harness request; `request.treatment_source` resolves against the file's directory from a file and against the cwd from the CLI; `harness_supported` equals the driver's `supported()`.
**Deliverables**: `crates/candler/src/commands/run.rs` (`compose_resolver`, `request_settings`)
**Consistency Checks**: `cargo nextest run -p candler -E 'test(/run::scaffold/)'` (expected: PASS)
**Commit**: `feat(cli): compose the M2b and M2a preflight steps`

## Step 2: Resolve, plan and prepare the job's spec
**Goal**: From any directory, one experiment file becomes a resolution, a plan and a spec the supervisor can load.
**Implementation Logic**:
1. `run_command(args: RunArgs) -> Result<ExitCode, CliError>` (`RunArgs {experiment: PathBuf, fields: Vec<(FieldPath, String)>, force: bool}`, filled by M2a's clap skeleton): the experiment path resolves against the caller's cwd; paths inside the file resolve against the file's directory (I14, decision 11). The store root comes from user `Config`, never from the cwd. `--<field> <value>` overrides the file (file first, CLI over it); `--effort <v>` is the cell value `model.effort` (D17), and `model.token_secret` (D9) is read from the file like any cell value.
2. `Resolver::resolve(ResolveInput)` runs preflight in the foreground, so a missing image, a missing secret or a failed catalog capture fails here, loudly, before a job dir exists. `run_plan::plan(&ExperimentFile, &Resolution, stored)` gives the `Vec<RunPlan>`; resume and `--force` are that leaf's. A refusal prints the `Deviation` list and exits non-zero.
3. The spec (D8): `JobDir::create`, then `job.json` with `spec_digest` set to the SHA-256 of the serialised `JobSpec` bytes (no secret value, no absolute path outside the store). No spec file is written; the bytes travel on the supervisor's stdin (step 3). `JobSpec` is serde-internal, never stored.
4. Class A tests: an experiment resolved from two cwds gives one `spec_digest`; a CLI override changes the cell digest only when the path is a cell value.
**Deliverables**: `crates/candler/src/commands/run.rs` (`RunArgs`, `run_command`, `spec_bytes`, `CliError`)
**Consistency Checks**: `cargo nextest run -p candler -E 'test(/run::scaffold/)'` (expected: PASS)
**Commit**: `feat(cli): resolve and plan a run from any directory`

## Step 3: Start the detached supervisor and return
**Goal**: `run` prints the job id and returns while the supervisor holds the lock.
**Implementation Logic**:
1. After step 2, call `jobs::runtime::detach(DetachArgs {exe: current_exe(), job_dir, spec: spec_bytes, env: allowlist})` (the job_runtime leaf): it re-execs `candler supervise --job <id>` with the bytes on stdin and the child's stdout and stderr in `<job-dir>/supervisor.log`. The allowlist is `PATH`, `HOME`, `XDG_STATE_HOME`, `DOCKER_HOST`, `DOCKER_CONTEXT` and the names in `secrets[]` (names resolve in the supervisor; the values are never read here). `detach` returns once the lock is held.
2. Print the job id on stdout, nothing else, and exit 0. A `RuntimeError::DidNotStart` exits non-zero naming the job dir and removes nothing.
3. The hidden verb enters the supervisor: a clap variant declared by S3 (not shown in `--help`) reads the spec from stdin (digest checked against `job.json` before the lock) and calls `jobs::supervise_entry(spec, orchestrate)`, which is where this leaf composes job_runtime with job_orchestrate (D12). If S3 declared no such variant, report a spec defect; this leaf edits no declaration.
4. B35 in `crates/candler/tests/guard_cli_run.rs` named `b35_run_returns_supervisor_outlives`, using `env!("CARGO_BIN_EXE_candler")`, a temp store, a temp cwd, and a fixture experiment with one cell. Doc lines: `Guards: I15, I16 — run returns at once, the supervisor outlives it holding the job lock, and after job.ended nothing marked or unmatched remains.` / `Oracle: the OS; the PATH-fake docker log.` / `Bite: wait on the supervisor in the launcher (no detach).`
**Deliverables**: `crates/candler/src/commands/run.rs` (`start_job`); `crates/candler/tests/guard_cli_run.rs` (`b35_run_returns_supervisor_outlives`)
**Consistency Checks**: `cargo nextest run -p candler -E 'binary(guard_cli_run)'` (expected: PASS)
**Commit**: `feat(cli): start a job detached and print its id`

## Step 4: Close the leaf
**Goal**: The scaffolding is gone and the bite is on record.
**Implementation Logic**:
1. Delete every `mod scaffold` in `run.rs`, confirm `guard_cli_run` holds only B35, and write the bite record in the leaf report: the B35 mutation tried, restored with `git checkout --`, the failing assertion quoted.
2. The real-model run from an unrelated directory belongs to `equivalence_smoke`, not here.
**Deliverables**: the leaf report `__reports__/m2b-execution-parity/cli_run_report_v0.md`
**Consistency Checks**: `cargo nextest run -p candler -E 'binary(guard_cli_run)' && ! git grep --untracked -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler/src crates/candler/tests` (expected: PASS)
**Commit**: `test(cli): prove run detaches and leaves nothing behind`
