# Preflight checks: build report

Built: N2 (plus validation of `--set replicates`), G5, N4 and C1 with its cap bounds. N6 is held by the coordinator.

## First reads
- N2: `plan_runs` read `file.replicates()` for `run_order` (`crates/candler-core/src/runner/plan.rs`). `replicates` is a setting (`resolve/layers.rs`), so `--set replicates=1` passed the known-path check and the plan ignored it. Confirmed.
- `replicates` enters no digest: `guard_resolve::job_caps_and_replicates_are_settings_and_per_run_caps_are_values` (`crates/candler-core/tests/it/guard_resolve.rs:586`) asserts it resolves into `settings`, which never reach a cell digest; the plan uses it only for `run_order`. `t3_one_level_plans_the_old_bytes` (oracle digests) passes unchanged.
- G5: `GroundTruthStep` reaches `treatment_exposures` (`resolve/steps/ground_truth.rs:299-360`) only when the experiment has ground truth, so an experiment without evaluator-only inputs had no skill-tree check. The new check is in `TreatmentIdentityStep` (`resolve/steps/identity.rs`, `skill_tree_refusal`), which calls `SkillTree::archive_command` and `SkillTree::files` and, for the path, the new `SkillTree::subtree` that `archive_command` now also uses (one owner of `<source_root>/<name>`). It records a failing check `skill_tree@1` on the step's report, which names the pinned commit, the source and the subtree path; no `codes.toml` row. `GroundTruthStep` is unchanged and now overlaps it: it could rely on the new check (follow-up only).
- N4: the rule is the required `treatment` of `RawExperimentFile` (`crates/candler-schemas/src/execution.rs`, field `treatment`, deserialised as `TreatmentSpec`), so a file with a `request.treatment` factor and no `[treatment]` failed with "missing field `treatment`". The field is now optional; `ExperimentFile::build` refuses a file with neither a table nor a factor of `request.treatment` (`MissingField`), and a level or base that names no source is still refused by `TreatmentIdentityStep`. `ExperimentFile::treatment()` returns an `Option`; `request_settings` inserts the base only when there is one. A file with `[treatment]` serialises as before (`skip_serializing_if`).
- N6 (held): no run-time refusal of a prompt starting with `/` exists in `candler-core`, `candler-harness`, `candler-harness-claude-code` or `candler` (`Cell::prompt` and `render_prompt`, `crates/candler-harness/src/spec.rs:1389-1467`, have none). There is no rule to move; building one needs a new refusal and a `codes.toml` row, which the leaf forbids. Waits for Eliott.

## Guards changed
- `guard_treatment_source.rs`: `resolve` passes the claude-code layout `SkillTree::new(".claude/skills", ".claude/skills")` to the step (its constructor takes it now). The `two_checkouts` fixture holds its skill at `.claude/skills/demo/SKILL.md` (was `SKILL.md` at the repo root, which the new check would refuse) and gains a later commit tagged `bare` without the subtree. The three existing tests assert the same things on it.
- The guard G5 sits in this file, not in `guard_claude_code_driver.rs`, by the coordinator's ruling.

## Bites
- N4: demanding the table again (`treatment.is_none()` refuses): red on `treatment_factor_needs_no_default`.
- G5: `skill_tree_refusal` that accepts any archive result: red on `skill_tree_missing_at_the_commit_is_refused`.
- N2: `plan_runs` reading `file.replicates()` again: red on `set_replicates_is_honoured` (left 10, right 2).
- N4: the guard was red on the tip (the refusal above); no fix, so no bite.
- C1: no guard (a test of the counts against `PlanOutcome` would be tautological).

## D3
- `skill_tree_missing_at_the_commit_is_refused`: guard (I7, G5). Question: does resolve refuse a pinned commit without the subtree, naming it, and accept one with it? Oracle: the subtree `.claude/skills/demo`, written by hand, present at `v1`, removed at `bare`. Lives as long as the step. Fails if the step does not check the commit.
- `set_replicates_is_honoured`: guard (I12). Question: does an explicit `replicates` decide the run count, with its source recorded? Oracle: two cells times one replicate is two runs; the layered resolution names the explicit argument. Fails if the plan reads the file's count.


## Verifier
First pass, findings and dispositions:
1. `--set replicates` unvalidated: fixed. `check_replicates` (schemas) is the one validator, used by `ExperimentFile::build` and by `request_settings`; 0 is refused as `Zero`. A non-integer or out-of-range value is refused as `NotPositive`, whose text ("not a finite number above 0") fits it only roughly: no code says "not a count". Needs a ruling. Validator guard `replicates_are_checked_once` (schemas) was written after the validator, so it was not seen red.
2. Refusal dropped the cause: fixed.
3. G5 fixture cannot tell a tag from a commit: not fixable at this step. The step refuses a tag and a commit that differ before it checks the tree, so the tag's commit and the pinned commit are one commit whenever the check runs; no fixture separates them. The guard separates the pinned commit from the working tree (HEAD is `bare`).
4. Cap bounds: fixed. `PlanOutcome::summary(&Resolution)` returns cells, new runs, reused runs, runs times the per-run `wall_s` and the job caps (`budget.*` settings).
5. `G5` id in a doc comment: fixed.
6, 7: accepted.
Re-check: see the final message.

## Follow-ups
- `PlanOutcome::summary` takes the resolution for the caps; `output_contract` prints it.
- `docs/contract/experiment-file.md` was regenerated (`CANDLER_BLESS_CONTRACT=1`) and `tests/vectors/field_classes.toml` of candler-schemas changed one row (`treatment` is optional); both are outside the leaf's OWNED regex, and the second trips the DIGEST numstat check (one deleted line, a hand table of field classes, not a digest).
- `plan_runs` falls back to the file's count when the resolution holds no `replicates` setting, because `request_settings` does not put the file's count into the resolution; adding it prints a new `replicates` row in `resolve`.
- `compose_resolver` lost its `skills` parameter: the services carry it (`M2bServices::skills`), one source for both steps.
- `GroundTruthStep` could drop its own skill-tree read once it relies on `skill_tree@1`.
