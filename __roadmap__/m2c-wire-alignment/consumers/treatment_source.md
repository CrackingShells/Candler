# Treatment source

**Goal**: The experiment's `[treatment] source` is the one route to the treatment's checkout. It resolves against the experiment file's directory, its commit is resolved before the digest, and it is recorded in the treatment's `sources[]` entry as written. It never enters `comparable`, and `request.treatment_source` is gone.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone after L0: `git merge-base --is-ancestor feat/m2c-wire-alignment HEAD && git log --first-parent --format=%s feat/m2c-wire-alignment | grep -qx 'merge(schemas): land wire'`
- [ ] [nl] OWNED:
  - `crates/candler-core/src/experiment/paths.rs` and `resolve/layers.rs`, the `request.treatment_source` lines and the `source` resolution;
  - `resolve/preflight.rs` if needed, and `resolve/steps/identity.rs`;
  - `runner/exec/cell.rs`, its source field, and `runner/exec/mod.rs`, its `skill_files` source;
  - `jobs/orchestrate/session.rs`: `TREATMENT_SOURCE`, the source line of `cells_of`, and `sources_of`;
  - `crates/candler/src/commands/run.rs`: `request_settings` and `require_source`;
  - `crates/candler/src/commands/supervise.rs`, the skill archive's source only;
  - `crates/candler-core/tests/oracle/vectors/field_paths.toml` and the count tuple of `guard_resolve_surface.rs` (this leaf alone edits them at L1);
  - `crates/candler-core/tests/support/job_orchestrate.rs` and `crates/candler/tests/support/cli_run.rs`, their treatment-source lines;
  - `crates/candler-core/tests/guard_treatment_source.rs` (new).
**Success Gates**:
- ⬜ [cmd] T1: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^source_only_/)'`. Two experiment files that differ only in `[treatment] source`, pointing at two git checkouts of one commit, resolve to equal cell digests, and their treatment `sources[]` entries differ only in `from`. Oracle: `git rev-parse` in the temp checkouts
- ⬜ [cmd] T2: `cargo nextest run -p candler --no-tests=fail -E 'binary(guard_cli_run) & test(/treatment_source/)'`.
  - A relative `source`, run from another directory, resolves against the experiment file.
  - A tag with no source is refused, naming `treatment.source`.
  - A tag moved in the checkout after a run is refused on resume, naming `treatment.commit`, and `--force` records the deviation.
  Oracle: hand-written expectations
- ⬜ [cmd] The setting is gone: `! git grep -nF 'request.treatment_source' -- ':(glob)crates/*/src/**'`, and G5b holds (`field_paths.toml` loses that row)
- ⬜ [cmd] `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_resolve_surface) | binary(guard_run_plan) | binary(guard_job_orchestrate)'` and `cargo nextest run -p candler --no-tests=fail -E 'binary(guard_cli_run)'` pass
- ⬜ [cmd] G5c, STD (`-p candler-core -p candler`), LINUX, SPAWN and SUBJ pass
- ⬜ [nl] The leaf report records the bites:
  - T1: the source copied into `comparable.treatment`;
  - T2: the source resolved against the cwd; the refusal of a tag without a source dropped.
- ⬜ [nl] The `treatment_source` verifier closes with no open finding. It attacks a source-only change, resolution against the file and not the cwd, an absent `from` when there is no source (existing `sources[]` bytes unchanged), and the resume refusal
**References**: [R01 §Run manifest (`sources[]`, I12), §Agent environment and secrets (the configuration surface), §Invariants I7, I14, I17](../../../__design__/architecture.md); [R10 D4, guards T1–T2](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md); [R03 "Treatment source"](../../../__reports__/m2c-wire-alignment/gap_analysis_v0.md); campaign README "Shared constraints", `request.treatment`

## Step 1: The identity step reads the source from the treatment
**Goal**: `request.treatment` carries a `TreatmentSpec`. The identity step resolves `source` against the experiment file, resolves the tag to a commit there, derives `TreatmentRef` without the source, and records `from`.
**Implementation Logic**:
1. Write T1 first and see it red. Today a `source` key is refused by `TreatmentRef`'s `deny_unknown_fields` at `identity.rs:165`.
2. `identity.rs` reads `TreatmentSpec` (`candler_schemas::execution`).
   - The checkout is `source` resolved against the experiment file's directory. `FileValues::relative_to` (`resolve/layers.rs`) learns to resolve the `source` key inside the `request.treatment` object, or the step receives the file's directory; report which.
   - A tag or commit with no source is refused, naming `treatment.source`.
   - The derived `treatment` root is `spec.reference()`.
   - Its `sources[]` entry is `ValueSource::derived_from(step, source as written)`.
3. `sources_of` (`orchestrate/session.rs`) keeps that entry.
4. `skill_files` (`runner/exec/mod.rs`) and the skill archive in `supervise.rs` take the checkout from the resolved source, in place of the setting.
**Deliverables**: `crates/candler-core/src/resolve/steps/identity.rs`: `TreatmentIdentityStep` over `TreatmentSpec`, `derived_from`; `crates/candler-core/src/resolve/layers.rs`: the source's resolution; `crates/candler-core/src/jobs/orchestrate/session.rs`: `sources_of`; `crates/candler-core/tests/guard_treatment_source.rs`: T1
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^source_only_/)'` (expected: PASS)
**Commit**: `feat(experiment): read the treatment's source from the experiment file`

## Step 2: One route only
**Goal**: `request.treatment_source` and its `--set` route are gone, and the binary refuses a treatment it cannot resolve.
**Implementation Logic**:
1. Write T2 first and see it red.
2. Delete `REQUEST_TREATMENT_SOURCE` and `TREATMENT_SOURCE` and their uses, from `paths.rs`, `layers.rs` and `session.rs`.
3. `run.rs::request_settings` passes the file's `TreatmentSpec` whole, and `require_source` becomes the refusal named in Step 1.
4. In `field_paths.toml`, remove the `request.treatment_source` row and update the count tuple in `guard_resolve_surface.rs`.
5. The test supports set `[treatment] source` instead of the setting.
6. The commit body names the migration: write `source = "<checkout>"` under `[treatment]` instead of `--set request.treatment_source=…`.
**Deliverables**: `crates/candler/src/commands/run.rs`: `request_settings`, the refusal; `crates/candler-core/src/experiment/paths.rs` without `REQUEST_TREATMENT_SOURCE`; `field_paths.toml`, `guard_resolve_surface.rs`; `crates/candler/tests/support/cli_run.rs`, `crates/candler-core/tests/support/job_orchestrate.rs`; the T2 guard in `crates/candler/tests/guard_cli_run.rs`
**Consistency Checks**: `! git grep -nF 'request.treatment_source' -- ':(glob)crates/*/src/**' && cargo nextest run -p candler --no-tests=fail -E 'binary(guard_cli_run)'` (expected: PASS)
**Commit**: `feat(experiment)!: replace the treatment source setting with the file's`
