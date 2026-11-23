# Preflight checks: build report

Built: N2, G5, C1's method. N4 stopped (its rule is in a file this leaf does not own); N6 is held by the coordinator.

## First reads
- N2: `plan_runs` read `file.replicates()` for `run_order` (`crates/candler-core/src/runner/plan.rs`). `replicates` is a setting (`resolve/layers.rs`), so `--set replicates=1` passed the known-path check and the plan ignored it. Confirmed.
- `replicates` enters no digest: `guard_resolve::job_caps_and_replicates_are_settings_and_per_run_caps_are_values` (`crates/candler-core/tests/it/guard_resolve.rs:586`) asserts it resolves into `settings`, which never reach a cell digest; the plan uses it only for `run_order`. `t3_one_level_plans_the_old_bytes` (oracle digests) passes unchanged.
- G5: `GroundTruthStep` reaches `treatment_exposures` (`resolve/steps/ground_truth.rs:299-360`) only when the experiment has ground truth, so an experiment without evaluator-only inputs had no skill-tree check. The new check is in `TreatmentIdentityStep` (`resolve/steps/identity.rs`, `skill_tree_refusal`), which calls `SkillTree::archive_command` and `SkillTree::files` and, for the path, the new `SkillTree::subtree` that `archive_command` now also uses (one owner of `<source_root>/<name>`). It records a failing check `skill_tree@1` on the step's report, which names the pinned commit, the source and the subtree path; no `codes.toml` row. `GroundTruthStep` is unchanged and now overlaps it: it could rely on the new check (follow-up only).
- N4 (stopped): the rule that demands a `[treatment]` default is in `candler-schemas`, not an owned file. `ExperimentFile::from_toml_str` of a file with a `request.treatment` factor of table levels (name, tag, source each) and no `[treatment]` table fails with `document is not an experiment file (missing field `treatment`)`. R18 cites `RawExperimentFile` at `execution.rs:1099-1133`. The guard `treatment_factor_needs_no_default` was written, seen red with that message, and removed (not committed). A fix means a `schemas` commit (R19: touches no other crate).
- N6 (held): no run-time refusal of a prompt starting with `/` exists in `candler-core`, `candler-harness`, `candler-harness-claude-code` or `candler` (`Cell::prompt` and `render_prompt`, `crates/candler-harness/src/spec.rs:1389-1467`, have none). There is no rule to move; building one needs a new refusal and a `codes.toml` row, which the leaf forbids. Waits for Eliott.

## Guards changed
- `guard_treatment_source.rs`: `resolve` passes the claude-code layout `SkillTree::new(".claude/skills", ".claude/skills")` to the step (its constructor takes it now). The `two_checkouts` fixture holds its skill at `.claude/skills/demo/SKILL.md` (was `SKILL.md` at the repo root, which the new check would refuse) and gains a later commit tagged `bare` without the subtree. The three existing tests assert the same things on it.
- The guard G5 sits in this file, not in `guard_claude_code_driver.rs`, by the coordinator's ruling.

## Bites
- G5: `skill_tree_refusal` that accepts any archive result: red on `skill_tree_missing_at_the_commit_is_refused`.
- N2: `plan_runs` reading `file.replicates()` again: red on `set_replicates_is_honoured` (left 10, right 2).
- N4: the guard was red on the tip (the refusal above); no fix, so no bite.
- C1: no guard (a test of the counts against `PlanOutcome` would be tautological).

## D3
- `skill_tree_missing_at_the_commit_is_refused`: guard (I7, G5). Question: does resolve refuse a pinned commit without the subtree, naming it, and accept one with it? Oracle: the subtree `.claude/skills/demo`, written by hand, present at `v1`, removed at `bare`. Lives as long as the step. Fails if the step does not check the commit.
- `set_replicates_is_honoured`: guard (I12). Question: does an explicit `replicates` decide the run count, with its source recorded? Oracle: two cells times one replicate is two runs; the layered resolution names the explicit argument. Fails if the plan reads the file's count.


## Verifier
Findings of the first pass (the verifier was not re-run: the call cap was reached):
1. N2: `--set replicates` is not validated (0 plans no run; a non-integer or out of range value falls back to the file's count). Not fixed: refusing it is a new refusal beyond the leaf's rows. Open, for the coordinator.
2. G5: the refusal dropped the `files()` error text. Fixed: the explanation now ends with the driver's message.
3. G5 bite: the fixture cannot tell the tag from the commit (each tag sits on the commit it pins); a moved-tag case is not added. Open.
4. C1: the summary omits the cap bounds. Recorded as a follow-up; needs a ruling. Open.
5. The `G5` id in a test doc comment. Fixed (removed).
6. The source assertion is a loose substring check and the guard does not go through `--set` parsing. Accepted (nit).
7. A failing `skill_tree@1` drops the step's derived `treatment`, as the existing step errors do. Accepted (not new).

## Follow-ups
- `PlanSummary` holds cells, new runs and reused runs. The cap bounds (runs times the per-run `wall_s`, and the job caps) are not in `PlanOutcome`: they come from the resolution, which `resolve` has. `output_contract` computes them where it prints, or the outcome gains them then.
- `plan_runs` falls back to the file's count when the resolution holds no `replicates` setting, because `request_settings` does not put the file's count into the resolution; adding it prints a new `replicates` row in `resolve`.
- `compose_resolver` lost its `skills` parameter: the services carry it (`M2bServices::skills`), one source for both steps.
- `GroundTruthStep` could drop its own skill-tree read once it relies on `skill_tree@1`.
