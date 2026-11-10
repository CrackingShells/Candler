# Declared inputs

**Goal**: The experiment's `[[inputs]]` are hashed and stored as blobs before the job. Agent inputs reach `comparable.inputs` through the single `inputs` deriver and are mounted read-only, file by file. Evaluator-only inputs reach the manifest's `evaluation_inputs` and never the isolation (I8).
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone after L0: `git merge-base --is-ancestor feat/m2c-wire-alignment HEAD && git log --first-parent --format=%s feat/m2c-wire-alignment | grep -qx 'merge(schemas): land wire'`
- [ ] [cmd] Docker answers, for P2: `docker info >/dev/null`
- [ ] [nl] OWNED:
  - `crates/candler-core/src/experiment/inputs.rs` (new);
  - `crates/candler-core/src/resolve/steps/image.rs`, the `inputs` deriver only;
  - `resolve/layers.rs` and `experiment/paths.rs`, the `request.inputs` setting lines only;
  - `crates/candler-core/src/jobs/model.rs` (`JobSpec::evaluation_inputs`);
  - `runner/exec/cell.rs`, the evaluation-inputs field, and `runner/exec/mod.rs`: `mount_inputs`, the manifest's `evaluation_inputs`;
  - `jobs/orchestrate/session.rs`, the inputs line of `cells_of`;
  - `crates/candler/src/commands/run.rs::plan_job`;
  - `crates/candler-core/tests/guard_declared_inputs.rs` (new), and a new support module if needed.

  The `field_paths.toml` row for `request.inputs`, if `guard_resolve_surface` asks for one, is reported, not added: `treatment_source.md` owns that file at L1.
**Success Gates**:
- ⬜ [cmd] I1: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^declared_input_/)'`.
  - A file input and a directory input of audience `agent` give one `{kind, digest}` each in `comparable.inputs`, with the digest W3's list digest.
  - Every file is mounted with `readonly` at `at/<path>` in the recorded docker argv.
  - A run whose output copies an input file is sealed `invalid(input_leak)`.
  Oracle: W3's vectors and hand-written argv expectations
- ⬜ [cmd] I2: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^evaluator_only_input_/)'`.
  - An evaluator-only input, whose file holds a canary string, is absent from `JobSpec.inputs`, from every docker argv, from the workspace, from `tree.json` and from `comparable`.
  - It is present in the manifest's `evaluation_inputs` with its list blob and file blobs in the store.
  - The canary string occurs in no file of the job dir or the run dir.
  Oracle: the canary
- ⬜ [cmd] I3: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^declared_input_refused_/)'`. These are refused, each naming the input: a duplicate kind, kind `build_inputs`, `at` absolute or holding `..`, a missing path, a symlink in a directory input, an empty directory. Oracle: hand-written
- ⬜ [cmd] I4: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^declared_input_with_build_inputs/)'`. An experiment with `isolation.build.{dockerfile,context}` and declared agent inputs resolves to one sorted `inputs` root holding both, with no `DerivedTwice`. Oracle: hand-written
- ⬜ [cmd] Resume compares inputs: a changed byte in a declared input is refused on resume, naming `inputs.<kind>` (`guard_run_plan` or the new binary)
- ⬜ [cmd] G5c, `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_run_exec) | binary(guard_job_orchestrate) | binary(guard_run_plan) | binary(guard_image)'`, STD (`-p candler-core -p candler`), LINUX, SPAWN and SUBJ pass
- ⬜ [nl] The leaf report records P2's answer and the bites:
  - I1: a mount without `readonly`; one file of a directory dropped;
  - I2: every input routed into `JobSpec.inputs`;
  - I3: `..` accepted;
  - I4: a second deriver of `inputs`.
- ⬜ [nl] The `declared_inputs` verifier closes with no open finding. It attacks I8 through the argv, the workspace, `tree.json`, the job dir and `job.json`; symlinks, `..`, and one file under two kinds; and the list digest against W3
**References**: [R01 §Agent environment and secrets (inputs mounted read-only), §Run manifest, §Invariants I8, I14, I17](../../../__design__/architecture.md); [R10 D2, D3, guards I1–I4, probe P2](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md); [R03 F5](../../../__reports__/m2c-wire-alignment/gap_analysis_v0.md); campaign README "Shared constraints" (input blobs, the single `inputs` deriver)

## Step 1: Probe P2, many read-only file mounts
**Goal**: Know whether one read-only bind mount per file, for a tree the size of the HTN servers directory, works under the container's flags with the parent directories still writable.
**Implementation Logic**:
1. Build a throwaway tree outside the repo with about 40 files in nested directories.
2. With `candler-testagent`, or any local image the e2e uses, run one container that has the same read-only-root and tmpfs flags as `isolation/container/run.rs`, one `--mount type=bind,readonly` per file under `/work`. Check that:
   - each file reads;
   - writing a mounted file fails;
   - creating a new file beside a mounted one succeeds.
3. Remove the container by its exact id, and the tree.
4. Record the answer, and the decision: file mounts, or one read-only directory mount from a materialised tree.
**Deliverables**: `__reports__/m2c-wire-alignment/leaves/declared_inputs_v0.md`: its `## P2` section
**Consistency Checks**: `grep -q '^## P2' __reports__/m2c-wire-alignment/leaves/declared_inputs_v0.md` (expected: PASS)
**Commit**: `reports(isolation): record how many read-only input mounts docker takes`

## Step 2: Resolve and store the inputs
**Goal**: `run.rs` hashes and stores every declared input, and the image step derives the merged `inputs` root.
**Implementation Logic**:
1. Write I3 and I4 first and see them red.
2. `experiment/inputs.rs` takes the experiment's `inputs()` and the file's directory.
   - It walks each `path`, refusing the I3 classes, stores every file with `Store::put_blob` and stores the list's RFC 8785 bytes.
   - It returns three things: the agent `{kind, digest}` list; the per-file mounts (`InputDigest { path: at/<path>, digest: file sha256 }`) for agent inputs only; and the evaluator `{kind, digest}` list.
   - Use `candler_schemas::execution::input_list_digest`.
3. `plan_job`:
   - puts the agent list in the setting `request.inputs`;
   - fills `JobSpec.inputs` with the mounts;
   - fills the new `JobSpec.evaluation_inputs` with the evaluator list.
4. The image step's deriver of `inputs` (`resolve/steps/image.rs`, about l.262) merges `build_inputs` with `request.inputs`, sorted. No other step derives `inputs`.
**Deliverables**: `crates/candler-core/src/experiment/inputs.rs`: the input resolver (its name in the leaf report); `crates/candler/src/commands/run.rs::plan_job`; `crates/candler-core/src/jobs/model.rs`: `JobSpec::evaluation_inputs`; `crates/candler-core/src/resolve/steps/image.rs`: the merged deriver; `crates/candler-core/tests/guard_declared_inputs.rs`: I3, I4
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^declared_input_refused_/) | test(/^declared_input_with_build_inputs/)'` (expected: PASS)
**Commit**: `feat(experiment): hash and store the inputs an experiment declares`

## Step 3: Mount agent inputs, record evaluator inputs
**Goal**: Agent inputs are mounted read-only at their workspace paths. Evaluator-only inputs are recorded in the manifest and never leave the store.
**Implementation Logic**:
1. Write I1 and I2 first and see them red.
2. `mount_inputs` handles the per-file mounts of a directory input, creating the parent directories in the workspace, or uses P2's other decision.
3. B24 and the `tree.json` hiding already work from `JobSpec.inputs`; check them.
4. The runner writes `JobSpec.evaluation_inputs` into the manifest's `evaluation_inputs`.
5. Nothing else reads the evaluator list.
**Deliverables**: `crates/candler-core/src/runner/exec/mod.rs`: `mount_inputs`, the manifest's `evaluation_inputs`; `crates/candler-core/src/runner/exec/cell.rs`; `crates/candler-core/src/jobs/orchestrate/session.rs`: `cells_of`; `crates/candler-core/tests/guard_declared_inputs.rs`: I1, I2, the resume refusal
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^declared_input_/) | test(/^evaluator_only_input_/)'` (expected: PASS)
**Commit**: `feat(runner): mount agent inputs and record evaluator-only inputs`
