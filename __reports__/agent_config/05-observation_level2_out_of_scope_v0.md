# Level 2 of M6.5: what was found outside its scope (v0)

Date: 2026-11-25

Ten findings from M6.5's level 2 leaves (`install_claude_code`, `install_opencode`, `preflight_check`), from their reviews and verifiers, and from the seam review of the merged level. Each one lies outside the leaf that found it, or was judged not worth the leaf's fix round, so no leaf fixed it. Each section below is one observation, written so that a fresh agent can pick it up cold.

---
type: observation
topic: agent_config
spotted-during: implementing, reviewing and merging the M6.5 level 2 leaves, and the level 2 seam review
date: 2026-11-25
domain: code
confidence: confirmed (O1–O5, O7–O10); plausible (O6)
urgency: per item, below
deferred-because: each lies outside the topic of the leaf that met it, or costs more than the defect it removes at this stage
---

## Summary

| # | Observation | Urgency | Where |
|---|---|---|---|
| O1 | An ignored OpenCode guard has been broken on main since 8d88cb50 | medium | `guard_opencode_driver::opencode_real_binary_is_interrupted_by_the_stop_script` |
| O2 | The host's read of the hook log refuses a link only at the last path component | low | `read_home_log` |
| O3 | A symlinked `.claude` in a task checkout redirects the hook-file writes | low | Claude Code `prepare` |
| O4 | A harness-check refusal points at the setting that is present, not at its cause | low | `setting_pointer` |
| O5 | Output that is not a check report is an experiment refusal, not a check that could not run | low | both `read_config_check` |
| O6 | The check grouping key splits more than the verdict needs | low (cost) | `group_key`, `files_digest` |
| O7 | The collision targets are derived twice | low (debt) | CLI `declared.targets`, supervisor `spec.inputs` |
| O8 | The job-start check takes no cancel signal, and paths are compared without Unicode normalisation | low | `check_config`, `refuse_instruction_collision` |
| O9 | OpenCode's census reads `Instructions from:` lines that OpenCode does not escape | low | OpenCode `loaded` |
| O10 | A second timing guard fails under load | low | `guard_container_record::stopped_run_has_its_agent_recorded_before_the_hook_sweeps` |

Two lessons from the level are process, not code, and are in the [last section](#process-lessons).

## O1. An ignored OpenCode guard has been broken on main since 8d88cb50

**What was noticed.** `opencode_real_binary_is_interrupted_by_the_stop_script` (`#[ignore]`, needs `CANDLER_OPENCODE_IMAGE`) takes `driver.collect(..).first()` as the export `Exec` spec. Since 8d88cb50 (`fix(adapter-opencode): collect the files an OpenCode run wrote`, on `origin/main`) the first spec is the `**/*` file spec, so the guard fails with `no collect` before Docker starts. Without the variable it fails with `NotPresent`. CI skips it because it is ignored, so nobody sees it fail.

**Location map.** `crates/candler-harness-opencode/tests/it/guard_opencode_driver.rs` (the guard); `crates/candler-harness-opencode/src/driver.rs` (`collect`).

**Hand-off questions.** Is the guard run anywhere, by hand or by a release checklist? Working theory: find the `Exec` spec among the specs instead of taking the first.

**Scope boundary.** Do not change what `collect` returns.

## O2. The host's read of the hook log refuses a link only at the last component

**What was noticed.** `read_home_log` opens `hooks.jsonl` with `O_NOFOLLOW`, regular files only, capped at 4 MiB. A link that the agent makes at `~/.candler` itself still resolves on the host, so a host file named `hooks.jsonl` under the link's target could be archived. This is host-side safety. It is not the agent editing its own home, which is allowed by design.

**Location map.** `crates/candler-core/src/runner/exec/` (`read_home_log`, `collect_home`).

**Hand-off questions.** Working theory: open each component with `openat` + `O_NOFOLLOW` from the home's descriptor (rustix), or check that the parent is a real directory owned by the run.

**Scope boundary.** Do not change where the hook log is written.

## O3. A symlinked `.claude` in a task checkout redirects the hook-file writes

**What was noticed.** `create_dir_all` and `create_new` follow a symlinked parent that already exists in the workspace. A task repository that ships `.claude` as a link sends the consumer's hook files through it. No payload can reach this; only a task checkout can.

**Location map.** `crates/candler-harness-claude-code/src/prepare.rs`; the core's staging of prepared files.

**Hand-off questions.** Should staging refuse a prepared path whose parent is a link, as `read_home_log` does for the log?

## O4. A harness-check refusal points at the setting that is present, not at its cause

**What was noticed.** `setting_pointer` chooses `/fixed/hooks`, `/fixed/instructions.workspace`, `/fixed/instructions.global` or `/fixed` by which values the cell sets. It does not look at what the harness rejected. Two consequences:
- A cell that also sets hooks, and whose `opencode.json` is rejected because of `mcp` or `tools.*`, is pointed at `/fixed/hooks`.
- A value set by a factor or an arm is pointed at `[fixed]`, which does not hold it. The pages in `docs/contract/driver-traits.md` promise "or the factor that sets it".

The level 2 seam fix unified the spelling, so the pointer is now always a key of the file. It did not change how the pointer is chosen.

**Location map.** `crates/candler-core/src/jobs/orchestrate/config.rs` (`setting_pointer`); `crates/candler-core/src/runner/plan.rs` (`refuse_instruction_collision`); `docs/contract/driver-traits.md` (Notes).

**Hand-off questions.** Can the cell's resolution say which level set a value? Or should the contract promise only the cell?

## O5. Output that is not a check report is refused as an experiment error

**What was noticed.** Some check output is non-empty but is not a report: a crash trace, a start that failed before the model, or a `claude doctor` whose first line changed in a new version. Both readers return `Rejected`, so the cell is refused as `experiment.harness_rejected_input` and the experiment is blamed. Only empty or truncated output, exit 124–127 and a signal are `jobs.config_check_failed`.

**Location map.** `read_config_check` in both driver crates; `CheckVerdict` in `crates/candler-harness/src/driver.rs`.

**Hand-off questions.** Should `CheckVerdict` gain a third value, `Unreadable`, that maps to `jobs.config_check_failed`?

**Scope boundary.** The verdict shape is a trait change, so both drivers and the conformance suite move together.

## O6. The check grouping key splits more than the verdict needs

**What was noticed.**
- The key hashes the whole `.claude/settings.json` and `opencode.json`. A sweep over N models or effort levels therefore runs N checks that are identical in effect.
- Skills are keyed by treatment ref rather than by the staged skill digests, which `stage_container` discards. Two refs that hold one tree are checked twice.
- Every distinct cell is fully staged, including a `git archive` for skills, before groups are merged.

Each costs time, never correctness.

**Location map.** `crates/candler-core/src/jobs/orchestrate/config.rs` (`group_key`, `files_digest`, `grouped`); `servers.rs` (`stage_container`).

**Hand-off questions.** What does a 20-cell sweep spend on checks today?

## O7. The collision targets are derived twice

**What was noticed.** `refuse_instruction_collision` is fed the agent inputs in two ways: the CLI passes `declared.targets` (`experiment/inputs.rs`), where the key is the input kind, and the supervisor passes `spec.inputs` (`input_targets`), where the key is the path. Today both give the same set.

**Location map.** `crates/candler-core/src/runner/plan.rs` (`PlanCtx.input_targets`); `crates/candler-core/src/experiment/inputs.rs`; `crates/candler-core/src/jobs/orchestrate/mod.rs`.

## O8. No cancel signal reaches the check, and paths are compared without Unicode normalisation

**What was noticed.**
- `check_config` takes no cancel watch, as `check_servers` does not either. A job cancelled during a check waits for the check's timeout.
- The collision check case-folds and strips `./`, but does not apply NFC. The workspace has no normalisation crate.

**Location map.** `crates/candler-core/src/jobs/orchestrate/{config,servers}.rs`; `crates/candler-core/src/runner/plan.rs`.

## O9. OpenCode's census reads lines that OpenCode does not escape

**What was noticed.** OpenCode's census lists the files from the `Instructions from: <path>` lines of the first request's system text. OpenCode does not escape them, so an instruction text that holds such a line adds an entry. The text comes from the consumer, not from the agent. The `loaded_instructions` doc comment states this.

**Location map.** `crates/candler-harness-opencode/src/driver.rs` (`loaded`).

**Hand-off questions.** Is the census meant as a proof, or only as a diagnostic? The proof is `context_start`, read from the trace.

## O10. A second timing guard fails under load

**What was noticed.** `stopped_run_has_its_agent_recorded_before_the_hook_sweeps` failed once in a full `cargo nextest run --workspace` on 04b6456b ("the record did not appear while the run was live"). It passed alone, and in the next full run. This is the same family as round 04's O6 (`guard_container_stop`).

**Location map.** `crates/candler-core/tests/it/guard_container_record.rs:691`.

## Process lessons

- **A leaf's gate must run the whole workspace suite.** The `preflight_check` fix round ran `candler-core` and `contract_pages` only, and the `install_claude_code` fix round ran a filtered `candler-core`. Together they left 33 tests red on the milestone branch:
  - the test fakes of the CLI and the core did not answer the new job-start check;
  - a level 1 guard still expected Claude Code to refuse `instructions.workspace`.

  The coordinator's full run after the third merge found them, and `fix/m6.5-check-fakes` fixed them (merge 3f19823e). The seam fix (merge 481e5301) then missed one more: `guard_census_gateway` still expected the absolute `/work/AGENTS.md` that the fix made relative. The coordinator's full run after that merge found it. Level 3 and 4 briefs name `cargo nextest run --workspace` as a gate.
- **A build directory shared between worktrees can run another checkout's tests.** Cargo keys a workspace member by its path relative to the workspace, so a slot that several worktrees use reuses a test binary whose `env!("CARGO_MANIFEST_DIR")` names the worktree that built it. Once that worktree was deleted, `committed_schema_files_hold_no_pattern_that_depends_on_the_regex_engine` failed with `NotFound`. It passed after `candler-schemas` was rebuilt in place. Whoever deletes a worktree also rebuilds, or deletes, the slots it used.
- **A rule added from a reading, not a measurement, refused valid cells.** The `install_opencode` fix round was asked to refuse OpenCode's `level=ERROR` lines. The seam review then ran the real image and saw environment errors at ERROR in 7 of about 13 starts (`watcher backend not supported`, `Failed to fetch models.dev`). The probe had recorded both consumer failures at WARN. The seam fix (merge 481e5301) returned the rule to WARN only. A check reader's rules come from recorded output.
