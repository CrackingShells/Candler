# Level wiring

**Goal**: Each cell of a two-treatment job runs with its own treatment level: `candler run` resolves every level, `JobSpec` carries them in `levels`, and `cells_of` gives each `CellRun` its own level's treatment, sources and harness (T3 wiring, C3). A one-treatment job's spec bytes are unchanged.
**Estimate**: about 70 Sonnet calls, $2.9, plus the planning-chain verifier (about 40 calls, $1.6), which also covers `level_resolve`
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (campaign README, Gotchas); `level_resolve` is merged: `test -f __reports__/m6a-paired-stats/leaves/level_resolve_v0.md`
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/jobs/model\.rs$|^crates/candler-core/src/jobs/orchestrate/(mod|session)\.rs$|^crates/candler/src/commands/(run|supervise)\.rs$|^crates/candler-core/tests/it/guard_job_orchestrate\.rs$|^__reports__/m6a-paired-stats/leaves/level_wiring_v0\.md$`. In `model.rs` only `JobSpec`; in `orchestrate/mod.rs` only `run_job`; in `session.rs` only `cells_of` and `treatment_of`
**Success Gates**:
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_job_orchestrate::/) | test(/^guard_job_spec_file::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_cli_run::/)'`
- ⬜ [cmd] Two fake levels with different sources: each `CellRun` carries its own level's treatment and sources: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_job_orchestrate::t3_/)'`
- ⬜ [cmd] A one-treatment spec serialises to the same bytes: `guard_job_spec_file::p0_` passes unchanged (`git diff --quiet $BASE_LEAF -- crates/candler-core/tests/it/guard_job_spec_file.rs`)
- ⬜ [cmd] DIGEST passes; ownership check prints nothing
- ⬜ [nl] The planning-chain verifier closed with no open finding; the report records each Bite
**References**: [R03 T3 wiring](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md); `jobs/orchestrate/session.rs`: `cells_of`, `treatment_of`; `jobs/model.rs`: `JobSpec`

## Step 1: Write the t3_ job rows, seen red
**Goal**: Rows that pin per-cell treatment at run time.
**Implementation Logic**:
1. In `guard_job_orchestrate.rs`, with the file's existing fakes: `t3_each_cell_runs_its_own_treatment` (two levels whose sources differ: each `CellRun`'s treatment, mounted sources and harness come from its own level); `t3_one_level_job_unchanged` (one level gives the `CellRun`s of today).
2. `Guards:` and `Oracle:` lines.
**Deliverables**: `crates/candler-core/tests/it/guard_job_orchestrate.rs`: the two `t3_` rows
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_job_orchestrate::t3_/)'` (expected: FAIL)
**Commit**: `test(jobs): guard per-cell treatment at run time`

## Step 2: Carry the levels in the job
**Goal**: The rows pass; one-treatment specs keep their bytes.
**Implementation Logic**:
1. `JobSpec` keeps `resolution` (the primary level) and gains `levels: Vec<Resolution>` with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`, empty for one level.
2. `commands/run.rs` resolves each distinct level in declared order (the step order is unchanged within a level) and builds `Levels` for `plan_job`; `commands/supervise.rs` and `run_job` pass the spec through.
3. `cells_of` looks up each cell's level and calls `treatment_of` on that level's resolution; `sources_of`, `record_packages` and `harness_of` follow from it, with no second lookup.
**Deliverables**: `crates/candler-core/src/jobs/model.rs`: `JobSpec.levels`; `crates/candler-core/src/jobs/orchestrate/session.rs`: `cells_of`; `crates/candler-core/src/jobs/orchestrate/mod.rs`: `run_job`; `crates/candler/src/commands/run.rs`; `crates/candler/src/commands/supervise.rs`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_job_orchestrate::/) | test(/^guard_job_spec_file::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_cli_run::/)'` (expected: PASS)
**Commit**: `feat(jobs): run each cell with its own treatment level`

## Step 3: Bites and the planning-chain verifier
**Goal**: The rows bite, and a fresh verifier attacks the whole planning chain.
**Implementation Logic**:
1. Bites: `cells_of` uses the primary level for every cell (red on `t3_each_cell_`); serialise an empty `levels` (red on `p0_`). Record each.
2. Dispatch the verifier (Sonnet, fresh agent) with the brief below verbatim plus the branch name; fix, re-check through SendMessage, report once.
3. **Verifier brief:** "You verify the planning chain of `__roadmap__/m6a-paired-stats/`: the leaves `treatment_setting.md`, `resolve/level_resolve.md` and `resolve/wired/level_wiring.md`, on branch <branch>. Read the leaves, then the code: `runner/plan.rs` (`setting_of_factor`, `fresh_cells`, `Levels`, `plan_runs`, `check_facts`), `commands/preflight.rs`, `commands/run.rs`, `jobs/model.rs`, `jobs/orchestrate/{mod,session}.rs`. Do not read the implementers' reports until you have your own findings. Attack: any path where a cell mounts, records or reports another level's treatment (`cells_of`, `sources_of`, `record_packages`, `harness_of`, `check_factors`); a setting other than `request.treatment` admitted as a factor (I7); an image built twice for equal lock bytes, or shared between different ones; a failing level that does not fail the plan; a one-treatment file whose plan, spec bytes or digests moved (run DIGEST); a test whose expected value came from the code under test. Report each finding with file, line and a failing input. Change nothing."
**Deliverables**: `__reports__/m6a-paired-stats/leaves/level_wiring_v0.md`: sections "Bites", "Verifier"
**Consistency Checks**: `grep -q '^## Bites' __reports__/m6a-paired-stats/leaves/level_wiring_v0.md && grep -q '^## Verifier' __reports__/m6a-paired-stats/leaves/level_wiring_v0.md` (expected: PASS)
**Commit**: `docs(jobs): record the bites and the verifier of the planning chain`
