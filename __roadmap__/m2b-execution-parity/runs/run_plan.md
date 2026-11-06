# Run plan: keys, attempts, resume

**Goal**: `candler_core::runner::plan` turns an experiment, its resolution and an optional stored cell into run plans with exact keys and attempt numbers under one experiment lock, and resumes only from a pinned stored cell whose four facts still hold, deleting nothing and sealing every unfinished attempt as `invalid(interrupted)`.
**Pre-conditions**:
- [ ] [cmd] Phase 0 is landed: `for s in '^docs(design): ' '^build(repo): ' '^ci(repo): ' '^feat(schemas): ' '^feat(orchestrator): scaffold'; do git log --format=%s origin/main..feat/m2b-execution-parity | grep -q "$s" || exit 1; done && test -s __roadmap__/m2b-execution-parity/feature_map/feature_map.json`
- [ ] [nl] api_review closed with no open finding on `RunPlan`, `Force`, `PlanCtx` and `PlanOutcome`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
- [ ] [cmd] DEP(job_store): `git log --first-parent --format=%s feat/m2b-execution-parity | grep -qx 'merge(jobs): land job_store'`
- [ ] [cmd] DEP(comparability): `git log --first-parent --format=%s feat/m2b-execution-parity | grep -qx 'merge(manifest): land comparability'`
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_run_plan)'` passes, holding B26, B27 and B28. **B26 no deletion**: a store holding sealed runs and unfinished attempts is killed twice (the second plan runs over what the first left); the test snapshots every existing file's bytes before each plan and asserts them byte-identical afterwards, every unfinished attempt is sealed `invalid(interrupted)`, and the attempts of one cell and replicate number 1, 2, 3 with no gap (R19; the oracle is the file system, read by the test). **B27 pinned cell**: the stored `Comparable` is replayed as given; each of the four changed facts (image ID, Claude Code version, a secret absent, tag moved to another commit) is refused without `--force` with that fact named, plus the control with nothing changed, which plans. **B28 forced deviations**: with `--force`, each of the four changes writes a `deviations[] {path, stored, observed}` entry on every new attempt (two cells, two replicates), and no sealed run's file changes
- ⬜ [cmd] STD, SUBJ and LINUX pass
- ⬜ [nl] The leaf report has a bite record, one mutation per B test (B26: `remove_dir_all` on an attempt without `run.done`; B27: skip the image-ID comparison; B28: write deviations only on the first new attempt), each run only against `guard_run_plan`, each restored with `git checkout --`
- ⬜ [cmd] `! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/runner/plan.rs crates/candler-core/tests/guard_run_plan.rs`
- ⬜ [nl] The required adversarial verifier reports no open finding. It states each test's oracle, names tautologies, and re-runs three bites from the leaf report plus one of its own (at most two e2e). It attacks deletion on resume (any `remove`, `rename-over` or `ReplaceUnfinished` path), attempt numbering after two kills, a concurrent supervisor on the same experiment, missing `--force` deviations on any attempt, and an unpinned stored cell (a cell recomputed from the current files instead of replayed)
**References**: [R01 §Resume, §Run manifest, §Experiment file](../../../__design__/architecture.md) — the four facts, `--force`, the run key; [Architecture §7, §8, §10.1](../../../__reports__/m2b-execution-parity/00-architecture_v0.md) — `RunPlan`, the recovery path, R18 and R19; [Test definition B26 to B28](../../../__reports__/m2b-execution-parity/01-test_definition_v0.md) — oracles and bites; [job_store](../job_store.md) and [comparability](../comparability.md) — the leaves this one consumes

## Step 1: Number runs and take the experiment lock
**Goal**: Every planned run has an exact key `(cell digest, replicate, attempt)`, and one supervisor at a time plans an experiment.
**Implementation Logic**:
1. The S3 shape (D15), taken as is: `plan_runs(ctx: &PlanCtx, file: &ExperimentFile, res: &Resolution, force: Force, mode: PlanMode) -> Result<PlanOutcome, PlanError>` (A8; `PlanMode::{Check, Hold}`), with `PlanCtx { store, experiment_lock }`, `PlanOutcome { plans: Vec<RunPlan>, sealed: Vec<RunKey>, lock: Option<ExperimentLock> }` (neither `Clone` nor `PartialEq`) and `RunPlan { key: RunKey, cell: CellId, stored: Option<Comparable>, attempt: u32, deviations: Vec<Deviation> }`. The stored cell is read from the experiment's stored manifests through `ctx.store`. `PlanCtx` holds no service: the observed facts of Step 3 come from `res` (the fresh resolution), so tests build a `Resolution` per row and no fake service is needed.
2. The experiment lock is `StoreLayout::experiment_lock(&digest)` (`<store>/experiments/<digest>/lock`), taken non-blocking; a second planner on the same experiment gets `PlanError::ExperimentLocked`, never a wait and never a shared plan. With `PlanMode::Check` the lock is taken and released inside the call and `PlanOutcome.lock` is `None` (cli_run's pre-detach check); with `PlanMode::Hold` it is returned in `PlanOutcome.lock` as the opaque `ExperimentLock`, released on drop, which job_orchestrate keeps until `exec_run` finishes the last run (S3's `PlanCtx.experiment_lock`).
3. Cells come from `res`; for each cell and replicate `0..replicates` the attempt number is `Store::next_attempt(cell, replicate)` (M1), so it is derived from what is on disk, never from a counter.
4. `runner/plan` makes no call to a function that removes or renames a run directory: the only store calls are `next_attempt`, `runs()`, `read_manifest` and `seal_interrupted`.
**Deliverables**: `crates/candler-core/src/runner/plan.rs` (`plan_runs`, `PlanMode`, `PlanCtx`, `ExperimentLock`, `PlanError`); `crates/candler-core/tests/scaffold_run_plan.rs` (class-A: three cells give keys with attempt 1; a second planner is refused; the plan is stable when nothing changed)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(scaffold_run_plan)' && cargo clippy -p candler-core --tests -- -D warnings` (expected: PASS)
**Commit**: `feat(runner): number runs and take the experiment lock`

## Step 2: Seal unfinished attempts and never replace them
**Goal**: A killed run's attempt is sealed `invalid(interrupted)` and the new attempt gets the next number, with nothing deleted (B26).
**Implementation Logic**:
1. `plan_runs` makes no recovery call: the caller (`cli_run`, `ps`) runs `jobs::lost::recover` for lost jobs before `plan_runs`, so a lost job's open runs are already sealed. Any `RunEntry {key, done: false}` that remains and belongs to a job whose lock is held fails the plan with `ExperimentLocked`.
2. Then for every `RunEntry {key, done: false}` of the experiment: `Store::seal_interrupted(&key)` (M1, R18; `RunSealed` counts as done). The plan never uses `Existing::ReplaceUnfinished` (R19), so a sealed attempt and its files stay and `next_attempt` returns n+1.
3. A run with `done: true` is never opened for writing.
4. **B26** in `tests/guard_run_plan.rs`: build a store with two sealed runs and two unfinished attempts (one with a partial `events` file); run `plan` once, simulate a kill by leaving the new attempts unfinished, run `plan` again; the test hashes every file under the store before and after each call and asserts equality for the old ones, and reads each manifest back: unfinished attempts are `invalid(interrupted)`, numbered 1, 2, 3.
**Deliverables**: `crates/candler-core/src/runner/plan.rs` (`seal_open`); `crates/candler-core/tests/guard_run_plan.rs` (the B26 test with `/// Guards:`, `/// Oracle:`, `/// Bite:`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_run_plan)'` (expected: PASS)
**Commit**: `feat(runner): seal unfinished attempts and number the next one`

## Step 3: Replay the stored cell and re-check four facts
**Goal**: A resume uses the stored `Comparable` as written, and refuses when one of four facts has changed.
**Implementation Logic**:
1. With `stored: Some(c)`, each `RunPlan.stored` is that `Comparable` verbatim (pinned): `plan` never reads the current `candler.toml`, skill checkout or catalog to rebuild it. A cell in the stored file with no counterpart in the experiment is `PlanError::StoredCellUnknown`.
2. `check_facts(stored: &Comparable, res: &Resolution) -> Vec<Deviation>` is pure and compares the stored cell with the fresh resolution `res`: the image ID (`isolation.image`; a missing image is a deviation with `observed: None`), the Claude Code version (`harness.version`), every secret named in the stored cell is present in `res`'s preflight result, the skill tag-to-commit (`treatment`). Each fact is its own `Deviation` with `path`, `stored` and `observed`; it holds names and digests, never a secret. The gateway probe is not a resume fact; job_orchestrate owns it (D21).
3. Without `force`, a non-empty result is `PlanError::Changed(Vec<Deviation>)`, listing every changed fact (not only the first). The error message names each path.
4. **B27** in `tests/guard_run_plan.rs`: a `Resolution` per row (fresh values), a table of four rows each changing one fact, each expecting `Changed` naming exactly that path, plus the control row (nothing changed, no `force`) that plans; plus a pin row (the experiment file edited after the stored cell, the plan still carries the stored values).
**Deliverables**: `crates/candler-core/src/runner/plan.rs` (`check_facts`, `PlanError::{Changed, StoredCellUnknown}`); `crates/candler-core/tests/guard_run_plan.rs` (the B27 test)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_run_plan)'` (expected: PASS)
**Commit**: `feat(runner): replay the stored cell and refuse changed facts`

## Step 4: Force a resume and record the deviations
**Goal**: A forced resume plans normally, and every new attempt carries each deviation, while sealed runs stay untouched (B28).
**Implementation Logic**:
1. With `Force::Yes`, `check_facts` still runs and its deviations are copied into the `deviations` of every `RunPlan` (the S3 field); `exec_run` writes them into each new manifest's `deviations[]`.
2. The deviations are attached to every new attempt of every cell and replicate, not to the first; a run that is not new (sealed) gets none and is not read for writing.
3. `deviations[]` is outside the manifest hash (nuisance set); it holds `path`, `stored`, `observed` only.
4. **B28** in `tests/guard_run_plan.rs`: two cells by two replicates, each of the four changes in turn, with `force`: every new plan has the deviation for that path with the stored and observed literals the test states; the hashes of every sealed run's files are unchanged (the same file-hash oracle as B26).
**Deliverables**: `crates/candler-core/src/runner/plan.rs` (the forced path inside `plan`); `crates/candler-core/tests/guard_run_plan.rs` (the B28 test)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_run_plan)'` (expected: PASS)
**Commit**: `feat(runner): record forced deviations on every new attempt`

## Step 5: Delete the scaffolding and write the bite record
**Goal**: Only the three guard tests remain, and each has been bitten once.
**Implementation Logic**:
1. Delete `tests/scaffold_run_plan.rs`. Check every B test has its three doc lines and no banned shape (argv copied from a builder, a serde re-check, an expectation computed from the code under test, an import of `isolation::layout` or `experiment::paths` into a test).
2. Write the bite record: one mutation per B test (B26 `remove_dir_all` on an attempt without `run.done`; B27 skip the image-ID comparison; B28 write deviations only on the first new attempt), each run only against `guard_run_plan`, each restored with `git checkout --`. None of them signals or destroys: the recording fakes only.
**Deliverables**: `crates/candler-core/tests/guard_run_plan.rs`; the bite record in the leaf report
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_run_plan)' && ! git grep --untracked -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/runner/plan.rs crates/candler-core/tests` (expected: PASS)
**Commit**: `test(runner): prove resume deletes nothing and records deviations`
