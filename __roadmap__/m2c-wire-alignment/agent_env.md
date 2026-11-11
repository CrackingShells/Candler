# Agent env

**Goal**: The experiment declares the agent environment's packages, the files pruned from it and the import packages each run records. The core and `assets/` name none of them, and the planner axis, its probe, `planner_package`, the binary's `gtpyhop` constant and `provenance.skill_version` are gone.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone: `git merge-base --is-ancestor feat/m2c-wire-alignment HEAD`
- [ ] [cmd] Docker answers: `docker info >/dev/null`
- [ ] [nl] OWNED:
  - `crates/candler-core/src/isolation/{image,layout}.rs` and `isolation/container/run.rs` (its module doc);
  - `crates/candler-core/src/resolve/steps/image.rs`, the lock and the env image key only (not the `inputs` deriver);
  - `crates/candler-core/src/jobs/runtime/{mod,session}.rs` and `jobs/orchestrate/{mod,session}.rs`, the planner lines only;
  - `crates/candler-core/src/runner/mod.rs`, `runner/exec/cell.rs` (the planner field), `runner/exec/mod.rs` (the provenance call only) and `runner/evidence/provenance.rs`;
  - `crates/candler-core/src/experiment/paths.rs` and `resolve/layers.rs`, the two new settings only;
  - `crates/candler/src/commands/supervise.rs`, everything except `eval_infra()`;
  - `assets/docker/env/{Dockerfile,stable.in}`;
  - the tests `crates/candler-core/tests/{guard_image,guard_run_evidence,guard_job_runtime,guard_resolve_surface}.rs`, `crates/candler-core/tests/support/{job_runtime,job_orchestrate}.rs`, `crates/candler/tests/guard_cli_kill_live.rs`, `crates/candler-core/tests/oracle/vectors/field_paths.toml` (rows added only), and the `ALLOWANCE` hunk of `crates/candler-core/tests/guard_layering_i1.rs`.

  `manifest/compare.rs` belongs to `verdict_gating.md`.
**Success Gates**:
- ⬜ [cmd] The planner is gone outside `compare.rs`: `! git grep -n -i -E 'gtpyhop|planner' -- ':(glob)crates/*/src/**' assets ':(exclude)crates/candler-core/src/manifest/compare.rs'`, and `! git grep -n -E 'planner_package|PlannerAxis|PlannerProbe|with_planner|PLANNER_PACKAGE' -- ':(glob)crates/**/*.rs'`
- ⬜ [cmd] `assets/` holds no domain package and no domain prune: `! git grep -n -E 'gtpyhop|libcst|python-arango|python-dotenv|jsonschema|domain\.py|problems\.py' -- assets` and `test ! -e assets/docker/env/stable.in`
- ⬜ [cmd] `provenance.skill_version` is gone: `! git grep -n skill_version -- ':(glob)crates/*/src/**'`
- ⬜ [cmd] A1 prune key: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_image) & test(/prune/)'`. One lock with two different `prune` lists gives two env image tags, and one lock with one list gives one. Oracle: tags derived by hand from the key's definition
- ⬜ [cmd] A2 recorded packages: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_run_evidence) & test(/record_packages/)'`. With `isolation.record_packages = ["x"]` and a fake container read, the manifest's `measured[]` holds `provenance.packages.x.distribution` and `.version`. `comparable` holds neither. Without the setting there is no `provenance.packages.*` entry. Oracle: hand-written manifest rows
- ⬜ [cmd] The resolve surface knows the two new settings: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_resolve_surface)'`, with `field_paths.toml` gaining exactly their two rows and the count tuple updated
- ⬜ [cmd] The guards that remain pass: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_layering_i1) | binary(guard_job_runtime) | binary(guard_job_orchestrate) | binary(guard_run_exec)'` and `cargo nextest run -p candler --no-tests=fail -E 'binary(guard_cli_kill_live)'`
- ⬜ [cmd] G5c passes (cell digests of the golden and parity fixtures are untouched), and STD (`-p candler-core -p candler`), LINUX, SPAWN and SUBJ pass
- ⬜ [nl] The leaf report records P3's answer and the bites:
  - A1: the key computed from the lock alone;
  - A2: the setting ignored, and the values written into `comparable`.
- ⬜ [nl] The Grit reviewer reports no open finding on the leaf's `.rs` changes
**References**: [R01 §Agent environment and secrets (the configuration surface), §Run manifest](../../__design__/architecture.md); [R10 D6, D8, guards A1–A2, probe P3](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md); [R04 A2 evidence table](../../__reports__/m2c-wire-alignment/gap_analysis_planner_v0.md); [R05 D0, D1, D1b, D3](../../__reports__/candler-v1-review/00-findings_v0.md); [R06 KT, intended deviations](../../__reports__/m2b-execution-parity/04-knowledge_transfer_v0.md)

## Step 1: Probe P3, the HTN lock without `stable.in`
**Goal**: Find out whether the six packages, compiled from an experiment's requirements file instead of from `stable.in`, give the same lock bytes, and so whether live HTN cell digests move.
**Implementation Logic**:
1. With the b017d31 code, compile a lock for a requirements set that has only `stable.in`.
2. Compile a lock for a requirements file holding the same six lines under another name, using the same `uv pip compile` invocation as `COMPILE_SCRIPT` in a throwaway script outside the repo.
3. Compare the bytes. A difference would come from `# via` annotations, which name the source file.
4. Record in the leaf report whether they differ and why. That decides what the smoke report must list. Nothing is committed.
**Deliverables**: `__reports__/m2c-wire-alignment/leaves/agent_env_v0.md`: its `## P3` section
**Consistency Checks**: `grep -q '^## P3' __reports__/m2c-wire-alignment/leaves/agent_env_v0.md` (expected: PASS)
**Commit**: `reports(isolation): record whether the htn lock moves without stable.in`

## Step 2: The environment comes from the experiment
**Goal**: The core builds the env from the experiment's requirements alone, prunes the files the experiment names, and keys the env image on both.
**Implementation Logic**:
1. Write A1 first and see it red.
2. Delete `assets/docker/env/stable.in`. In `isolation/image.rs`:
   - remove `STABLE_SOURCE` and its staging;
   - remove the `source_name_ok` exception for it;
   - with no requirements, give the empty lock without compiling.
3. Add the setting `isolation.build.prune`: a list of file names, each `^[A-Za-z0-9._-]+$`, else refused at resolve. Add it to `experiment/paths.rs` and to `resolve/layers.rs::is_setting`, which already covers `isolation.build.*`, so check before adding a line.
   - The Dockerfile takes the names as a build argument and deletes them under `/opt/env`. With none, it deletes nothing.
   - The env image's tag derives from the lock digest and the sorted prune list, so equal locks with different lists never share an image.
   - Keep `LockMismatch` and the `candler.lock` label.
4. Reword `image.rs`'s `LockSource` doc, so that it names no skill's file layout.
The commit body names the migration: an experiment declares its packages in `isolation.build.requirements`, and its prunes in `isolation.build.prune`.
**Deliverables**: `assets/docker/env/Dockerfile`: the prune build argument; `assets/docker/env/stable.in` deleted; `crates/candler-core/src/isolation/image.rs`: `compile_lock` without a floor, and the env tag over the lock and the prune; `crates/candler-core/src/resolve/steps/image.rs`: the prune setting read into the build; `crates/candler-core/src/experiment/paths.rs`: `ISOLATION_BUILD_PRUNE`; `crates/candler-core/tests/guard_image.rs`: the A1 guard; `field_paths.toml`: the row
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_image)'` (expected: PASS)
**Commit**: `feat(isolation)!: take the agent environment from the experiment`

## Step 3: A generic read of named packages replaces the planner axis
**Goal**: Each run records the distribution and version of the import packages the experiment names. Nothing in the core or the binary names a package or a planner.
**Implementation Logic**:
1. Write A2 first and see it red.
2. Add the setting `isolation.record_packages`: a list of import names matching `^[a-z_][a-z0-9_]*$`, else refused, because a name becomes a field-path segment. Add it to `paths.rs` and `layers.rs` and a row in `field_paths.toml`.
3. Generalise the probe:
   - `PlannerProbe`/`PlannerAxis`/`planner_script`/`parse_planner` become a read of N named packages from one control container, returning `distribution` and `version` per name. Keep `PlannerAxis`'s rule: a string for one distribution, an array otherwise.
   - `JobHost::planner` becomes `JobHost::packages(rt, image, names)`. The per-image map and `CellRun::with_planner` follow.
   - The names come from the resolved setting, not from `JobServices`.
   - Delete `planner_package` from `JobServices`, `Runtime` and the test constructors, and delete `PLANNER_PACKAGE` from `supervise.rs`.
4. `runner/evidence/provenance.rs`:
   - write `provenance.packages.<name>.{distribution,version}`;
   - delete `skill_version()`, its `provenance.skill_version` entry and the test `b38_skill_version_from_a_skill_tag_only` in `guard_run_evidence.rs`;
   - keep `eval_infra.*` and `sandbox_contract` as they are; their gating is `verdict_gating.md`'s.
   - The module doc names no planner and no skill version.
5. In `guard_layering_i1.rs`, delete `ALLOWANCE`, `hits_beyond_allowance` and `the_allowance_excuses_its_one_line_and_nothing_else`, and scan the binary's `src` like the other folders. Touch nothing else in that file.
6. Reword `isolation/container/run.rs`'s module doc ("the planner's entries").
The commit body names the migration: `provenance.planner.sandbox.*` becomes `provenance.packages.<import>.*` once the experiment names the package in `isolation.record_packages`, and `provenance.skill_version` is no longer written.
**Deliverables**: `crates/candler-core/src/isolation/image.rs`: the generic package read (`PackageProbe`, `PackageReading` or the names the implementer reports); `crates/candler-core/src/jobs/orchestrate/{mod,session}.rs`: `JobHost::packages`; `crates/candler-core/src/runner/exec/cell.rs`: `CellRun::with_packages`; `crates/candler-core/src/runner/evidence/provenance.rs`: `provenance_values` with packages and without the skill version; `crates/candler/src/commands/supervise.rs` without `PLANNER_PACKAGE`; `crates/candler-core/tests/guard_run_evidence.rs`: the A2 guard; `crates/candler-core/tests/guard_layering_i1.rs` without the allowance; the test constructors without `planner_package`
**Consistency Checks**: `! git grep -n -i -E 'gtpyhop|planner' -- ':(glob)crates/*/src/**' assets ':(exclude)crates/candler-core/src/manifest/compare.rs' && cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_run_evidence) | binary(guard_layering_i1) | binary(guard_job_runtime)'` (expected: PASS)
**Commit**: `feat(isolation)!: record the packages an experiment names, not a planner`
