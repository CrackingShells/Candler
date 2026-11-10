# Settings out of cell

**Goal**: A setting written in the experiment file's `[fixed]` or in a factor level reaches the resolver as a setting and never the cell. `comparable.values` and the cell digest hold no path that `is_setting` classes as a setting, whatever route set it, so the file route M2c makes the only one for `isolation.build.*`, `isolation.record_packages`, `request.*` and `model.token_secret` keeps R01's rule that a setting never reaches a cell digest.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone after L2: `git merge-base --is-ancestor feat/m2c-wire-alignment HEAD && git log --first-parent --format=%s feat/m2c-wire-alignment | grep -qE '^merge\(.*\): land digest_invariance$'`
- [ ] [nl] OWNED: `crates/candler-core/src/runner/plan.rs` (`fresh_cells`), and a new guard binary `crates/candler-core/tests/guard_settings_out_of_cell.rs` or rows in an existing plan guard. No schemas change: the classification is the core's (`resolve/layers.rs::is_setting`), which this leaf reads and does not edit
**Success Gates**:
- ⬜ [cmd] C1: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/settings_out_of_cell/)'`. A parameterized table of experiment files, each planned through the route `candler run` uses. One row per setting class written in `[fixed]`: `isolation.build.requirements`, `isolation.build.prune`, `isolation.record_packages`, `request.inputs`, `model.token_secret`. One row for a setting given as a factor level. One control row with no setting. In every row, `comparable.values` holds no setting path, and the cell digest equals the control's. The settings still reach the resolution as settings (the image step sees the prune list). Oracle: the hand-written table and the control's digest
- ⬜ [cmd] The digest guards hold: `guard_cell_digest`, `guard_experiment_file` (W2), `guard_run_plan`, `guard_job_orchestrate`, `guard_import_mini`, and GOLDEN
- ⬜ [cmd] STD (`-p candler-core -p candler`), LINUX, SPAWN and SUBJ pass
- ⬜ [nl] The leaf report records C1's bite (the filter dropped, which turns every setting row red), and lists every existing test whose expected digest moved, with the setting that had leaked into it. An expected digest edited "until green" without that cause is a finding
**References**: the smoke's P4 finding P4-F1 in `__reports__/m2b-execution-parity/equivalence_smoke_report_v2.md`; R01 §Agent environment and secrets (the configuration surface); M2b's F7; `resolve/layers.rs::is_setting`

## Step 1: Filter settings out of every file cell
**Goal**: The cell build drops setting paths from the file's cell values, as it already drops paths given by `--set`.
**Implementation Logic**:
1. Write C1 first and see its setting rows red on the base.
2. In `fresh_cells`, filter the file's cell values (`effective`) by `is_setting` as well as by the explicit-argument overlap, so a setting from the file stays in the resolution's settings and leaves `comparable.values` and the declared paths.
3. Run the digest guards; a moved expected digest is reported with its cause, never edited blind.
**Deliverables**: `crates/candler-core/src/runner/plan.rs`: `fresh_cells` filters settings; `crates/candler-core/tests/guard_settings_out_of_cell.rs`: C1
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/settings_out_of_cell/)'` (expected: PASS)
**Commit**: `fix(experiment): keep settings written in the file out of the cell`
