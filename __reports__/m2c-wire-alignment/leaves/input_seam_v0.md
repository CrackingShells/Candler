# Leaf report: input_seam (findings)

Branch `m2c/input_seam`, base 8f3a103. The seam holds: no defect between `declared_inputs`' writer and `evaluator_inputs`' reader. One guard (S1), two bites seen, no `src/` change.

## 1. Commits
| sha | subject |
|:----|:--------|
| 3c24ac2 | test(experiment): guard the input seam from the run to the evaluator |
| (last) | reports(experiment): record the input_seam leaf |

## 2. Gates
| gate | result | last line |
|:-----|:-------|:----------|
| S1 `binary(guard_input_seam)` | PASS | `1 test run: 1 passed` |
| STD, `-p candler` (fmt, clippy `--all-targets -D warnings`, nextest, doc `-D warnings`) | PASS | nextest: `36 tests run: 36 passed, 5 skipped` |
| LINUX (`-p candler --tests`) | PASS | `Finished` |
| SPAWN | PASS | `guard_signal`: 9 passed |
| SUBJ (base 8f3a103) | PASS | `cog check` clean, subjects at most 72 characters |
| OWNED | PASS | the diff lists the guard, `support/seam.rs`, `fixtures/plugins/needs-inputs/*` and this report |
| DRIFT, G5a | not applicable | no schemas file or vector touched |

## 3. Guard
**S1 `input_seam_plugin_reads_both_trees_and_the_agent_never_held_the_canary`** (`guard_input_seam.rs`)
- Guards: R01 Evaluator protocol and I8. `candler run` declares `servers` (agent, at `srv`) and `truth` (evaluators); `candler evaluate` runs a plug-in whose `needs.inputs` is `["servers","truth"]`. The plug-in reads both trees with the sha256 of every file, each context `digest` is the list digest, no file is writable, and the canary occurs nowhere it must not.
- Trees: `servers` has an empty file and `nested/deep/b.txt`; `truth` has an empty file, `nested/dir/key.txt`, `answer.txt` holding the canary bytes, and `nested/<canary name>.txt`.
- Oracle: file hashes by `shasum`; the list digest is `shasum` of the hand-written RFC 8785 text of the sorted `[{path, sha256:"sha256:<hex>"}]` list (the entry hash carries the `sha256:` prefix, as the W3 vectors do); the canary is a literal in content and in a file name. No Candler code computes an expectation.
- Scans: bytes and name of the canary under `store/runs` (holds the workspace and `tree.json`), under the job dir, and in the fake docker's call log (the recorded argv). Control: the agent's own input is visible in the argv as `srv/nested/deep/b.txt`. File digests that only `truth` has are also absent from the argv (the empty file's digest is shared with `servers`, so it is skipped).
- Bites, each injected, seen red, restored (`git checkout`):
  - Evaluator-only input also pushed into `resolved.mounts` (`experiment/inputs.rs`): `` `canary-name-5b1e0d97c3a6` in the recorded docker argv `` (left: the docker log). The bytes needle stays green, since mounts name blob paths, not content: the name canary is what catches this route.
  - List blob stored pretty-printed under its own sha (not the input's digest): evaluate fails with `input `servers` has no list blob in the store`, so the plug-in's read is red.
- Red before green: the guard was first red on setup problems (F1, F2), then on my oracle (the `sha256:` prefix); the two bites above are the red for the right reason.

## 4. Findings
| # | kind | finding |
|:--|:-----|:--------|
| F1 | test-world defect, worked around | `support/cli_run.rs`'s skills repo keeps its skill at `demo/`, while the harness archives `.claude/skills/<name>`: every cell of that world fails with `the cell cannot be planned: the skill tree could not be archived`, so no run seals and `evaluate` selects nothing. Its own guards only wait for `job.ended`. Not edited (not OWNED); `support/seam.rs::lay_out_skill` commits `.claude/skills/demo/SKILL.md` and moves `v1`. |
| F2 | test-world gap, fixed in round 1 | The world's experiment lacks `permission_mode`, which the harness requires (`cell has no value for permission_mode`). Fixed in the world in round 1. |
| F3 | note | `cli_run.rs` is included with `#[expect(dead_code)]` on the `mod` in `guard_input_seam.rs`, since this binary uses only part of it. |
| F4 | note | The fake docker produces no agent output, yet the run seals and is evaluable, so the seam is exercised without a container. |
| F5 | names | New files: `tests/support/seam.rs`, `tests/fixtures/plugins/needs-inputs/{plugin.toml,suite.toml,kinds/needs_inputs.py}`; plug-in `seam@0.1.0`, kind `read_inputs`. It names no domain; the files hold no `htn`, `planner` or `tier`. The helper environment is synced into the world's state home, as `guard_cli` does (needs `uv`). |
| F6 | none | No seam defect: blob keys, list format and read-only materialisation agree; no `src/` change. No confirms left. |

## Fix round 1
The coordinator widened OWNED to `support/cli_run.rs` and the guards that drive it. Only `guard_cli_run.rs` does (`git grep -ln 'support/cli_run'`), besides S1.

**Commit order.** The world fix is the first commit and S1 the second, so the branch was rebuilt (`reset --mixed` to 8f3a103, then three commits) instead of fixups; `seam.rs` and `guard_input_seam.rs` lost `lay_out_skill` and the `permission_mode` edit in the S1 commit itself.

**World.** `cli_run.rs` lays the skill out at `.claude/skills/demo/SKILL.md` (and `move_tag` edits it there), and its experiment carries `permission_mode = "plan"` under `[fixed]`. A cell of that world now seals a run. New helpers: `World::sealed_runs()` (runs whose directory holds `run.done`) and `World::assert_sealed(n)` (message `no run sealed: <k> sealed, <n> expected`, then the fake's calls).

**CLI guards touched, all in `guard_cli_run.rs`:**
| guard | now also asserts |
|:------|:-----------------|
| `b35_run_returns_supervisor_outlives` | one run sealed after the job ends |
| `treatment_source_is_relative_to_the_experiment_file` | one run sealed (through `finish`) |
| `treatment_source_given_by_set_is_relative_to_the_cwd` | one run sealed (through `finish`); the manifests read for `from` are a sealed run's |
| `treatment_source_moved_tag_is_refused_on_resume` | one run sealed after the first launch; two after `--force`, so the manifest holding `deviations` with both commits is a finished run's |
The three refusal guards (`treatment_source_is_required_by_a_tag`, `..._by_a_name_alone`, `..._setting_is_refused_by_name`) start no job and are unchanged.

**No guard went red** once runs seal: all 7 pass, so the earlier claims held (the manifest was written at planning, before the cell failed).

**Bite.** Skill back at `demo/` in `cli_run.rs`: `treatment_source_moved_tag_is_refused_on_resume` and `b35_run_returns_supervisor_outlives` fail with `no run sealed: 0 sealed, 1 expected` (nextest stopped at the first failures, so the other two `finish` users were not shown). Restored.

**Gates after the round:** see the message that carries it.

## Fix round 2
The verifier found no I8 leak and no seam defect. One should-fix and two notes.

**F1 (should-fix): a reader that skips its list-digest check went unseen.** With `listed != digest` disabled in `evaluation/context/assemble.rs`, 24 tests stayed green. New guard in `guard_run_context.rs`, its own commit `test(evaluation): refuse a list blob that does not match its digest`:
- `needs_inputs_refused_when_the_list_blob_is_not_its_digests_list`. Guards: R01 Evaluator protocol; a list blob that is not the list its digest names is refused as non-canonical, naming the kind, never served. The blob under the digest key is overwritten with a valid list of an existing file with another sha256. The same entries pretty-printed would pass: the check recomputes the digest from the parsed entries, so only different entries can fail it.
- Oracle: hand-written; the two canonical list texts are written in the test, and the reason is the refusal's one text. The input is served before the overwrite, so the refusal is the overwrite's.
- Bite: `listed != digest` made `false && ...`; red with `a list that is not its digest's is refused: Assembled { run: RunKey { ... replicate: 6, attempt: 2 }, ...` (`expect_err` on an Ok). Restored.
- The `needs_inputs_refused` prefix carries it into EV2's filter: 2 tests match, both pass.

**F3 (note):** the canary-bytes scan of the recorded argv could never fire, since the argv holds blob paths and names. S1 now scans the argv for the canary name and the file digests only, with a comment saying why; the bytes and the name are still searched under the run and job directories. (Fixup of the S1 commit.)

**F4 (note):** `World` removes its directory on drop, so a red guard no longer leaks its `candler-guard-run-*` world. The explicit `remove` stays. (Fixup of the world commit.)

Commits were fixed up and autosquashed onto 8f3a103; the new test commit sits before the report commit. Gates: see the message that carries this round.
