# Preflight checks: build report (partial)

Built: N2 only. G5, N6 and N4 stopped at the first read (spec defects below); C1's method not built. The leaf stopped at its call cap of 47 with the verifier not yet dispatched.

## First reads
- N2: `plan_runs` read `file.replicates()` at `crates/candler-core/src/runner/plan.rs` (the `run_order` call). `replicates` is a setting (`resolve/layers.rs`), so `--set replicates=1` passed the known-path check and was ignored by the plan. Confirmed.
- `replicates` enters no digest: `guard_resolve::job_caps_and_replicates_are_settings_and_per_run_caps_are_values` (`crates/candler-core/tests/it/guard_resolve.rs:586`) asserts it resolves into `settings`, which never reach a cell digest. `plan_runs` uses it only for `run_order`. The experiment digest comes from the cell digests, so it does not move either (`t3_one_level_plans_the_old_bytes` passes unchanged).
- G5 (defect): the claim that nothing checks the skill subtree at the pinned commit is false on the tip. `GroundTruthStep` (`crates/candler-core/src/resolve/steps/ground_truth.rs:299-360`, `treatment_exposures`) archives `SkillTree::archive_command` at the pinned commit and calls `SkillTree::files`, which refuses a missing `SKILL.md` (`crates/candler-harness/src/skills.rs`, `files`). On a git failure it says only "the skill tree could not be archived" and names no path. So the real gap is the message, and the check lives in a step that the guard of the leaf (a driver test) cannot reach. The leaf needs a ruling: reword that refusal to name the path, or move the check to `TreatmentIdentityStep`.
- N6 (defect): no run-time refusal of a prompt starting with `/` exists in `candler-core`, `candler-harness`, `candler-harness-claude-code` or `candler` (colgrep searches for `starts_with('/')`, `slash`, `prompt` there found only path checks; `Cell::prompt` and `render_prompt` in `crates/candler-harness/src/spec.rs:1389-1467` have none). The rule has no owner to call, so "the code it gets at run time today" does not exist. R18's own note says the check was never found. Needs a ruling: a new rule with a new refusal (and a `codes.toml` row, which the leaf forbids).
- N4: the refusal `TreatmentIdentityStep` raises for a treatment with no source (`identity.rs:133-139` doc, the `ok_or_else` in `run`) is on the resolved `request.treatment` value, and the base `[treatment]` is inserted by `request_settings` (`crates/candler/src/commands/preflight.rs:268-315`). No rule demanding a `[treatment]` default was located; the first read was not finished.

## Guards changed
None. Added: `set_replicates_is_honoured` (`guard_run_plan.rs`).

## Bites
- N2: `plan.rs` reading `file.replicates()` again: red on `set_replicates_is_honoured` (left 10, right 2), seen before the fix.

## D3
- `set_replicates_is_honoured`: guard (I12). Question: does an explicit `replicates` decide the run count, with its source recorded? Oracle: two cells times one replicate is two runs; the layered resolution names the explicit argument. Lives as long as the plan reads settings. Fails if the plan reads the file's count.

## Verifier
Not dispatched (cap reached).

## Follow-ups
- `plan_runs` falls back to the file's count when the resolution holds no `replicates` setting, since `request_settings` does not put the file's count into the resolution; adding it there would print a new `replicates` row in `resolve` (a CLI output change for `output_contract`).
- Left: G5, N4, N6 (rulings above), C1 `PlanOutcome::summary`, bites for them, verifier.
