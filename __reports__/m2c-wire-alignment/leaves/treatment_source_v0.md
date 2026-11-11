# Leaf report: treatment_source (findings)

Branch `m2c/treatment_source`, cut from `feat/m2c-wire-alignment` at ea4905e.

## Verdict
Both steps landed, one commit each. T1 and T2 pass and every bite turns its guard red. `request.treatment_source` is gone from the sources, and the oracle lost exactly one row. There are four findings, all ruled on in fix round 1 (F1, F2, F3 accepted; F4 reversed, see below).

## 1. Commits
| sha | subject |
|:----|:--------|
| (squashed) | feat(experiment): read the treatment's source from the experiment file |
| (squashed) | feat(experiment)!: replace the treatment source setting with the file's |
| (squashed) | reports(experiment): record the treatment_source leaf |

## 2. Gates
| gate | result | last relevant line |
|:-----|:-------|:-------------------|
| T1 `test(/^source_only_/)` | PASS | 2 tests run: 2 passed |
| T2 `binary(guard_cli_run) & test(/treatment_source/)` | PASS | 3 passed (the whole binary: 4 passed) |
| setting gone: `! git grep -nF 'request.treatment_source' -- ':(glob)crates/*/src/**'` | PASS | no hit |
| G5b `^-path` count in `field_paths.toml` | PASS | 1 |
| G5a no oracle or vector edited (outside `field_paths.toml`) | PASS | 0 diff lines |
| resolve_surface, run_plan, job_orchestrate, guard_cli_run | PASS | tuple is now (17, 18, 5), counted by hand: 19 setting rows less 1 |
| G5c digest guards | PASS | `guard_cell_digest` 9/9; core digest set 45/45 |
| GOLDEN | PASS | 2/2 and 1/1 |
| STD, `-p candler-core -p candler` | PASS | fmt, clippy `-D warnings`, doc `-D warnings`; nextest 149 passed (core), 27 passed (candler) |
| LINUX, `-p` form, `--tests` | PASS | Finished |
| SPAWN, after each commit | PASS | `guard_signal` 9/9 |
| SUBJ (`cog check ea4905e..HEAD`, length) | PASS | No errored commits |
| OWNED check | PASS with F2 | every changed path is OWNED except `image.rs` (F2) |
| DRIFT | not run | no change to `candler-schemas` |

I excluded `nuisance_set_is_the_hand_written_one` (`candler-core::guard_comparability`) from the `-p candler-core` STD run. It is red at ea4905e and belongs to `verdict_gating`.

## 3. Guards
| id | name | Guards | Oracle | Bite and the failure seen |
|:---|:-----|:-------|:-------|:--------------------------|
| T1 | `source_only_changes_the_source_entry` (`guard_treatment_source.rs`) | I7, I12: two files differing only in `[treatment] source`, naming two checkouts of one commit, give one cell digest, and their treatment `sources[]` entries differ only in `from` | `git rev-parse v1^{commit}` in each checkout | The tag set to the source in the derived treatment: `assertion left == right failed: the source moves no digest`. The first attempt, a `source` key in the derived JSON, did not turn it red, because the cell reads the typed root and drops the extra key, so I used the tag. |
| T1b | `source_only_is_recorded_when_there_is_one` | I12: no source, no `from`, so the bytes of the existing entries are unchanged | the entry written by hand | An empty `from` for a treatment with no source: assertion failed, left has `"from": ""`. It was green before the code, because it guards behaviour that must not change. |
| T2a | `treatment_source_is_relative_to_the_experiment_file` (`guard_cli_run.rs`) | a relative source resolves against the experiment file, with `candler run` started in another directory | hand-written: `skills` sits beside the file, `work/skills` does not exist | Source joined to the cwd: the launcher exits 2, `... does not name a commit in .../work/skills (git exited 128)`. |
| T2b | `treatment_source_is_required_by_a_tag` | a tag with no source is refused, naming `treatment.source`, and no job id is printed | hand-written | Refusal dropped (source defaults to `.`): `the refusal names the path`, with the message `.../work/. (git exited 128)`. |
| T2d | `treatment_source_is_required_by_a_name_alone` | a treatment with no tag, no commit and no source is refused before any job exists, naming `treatment.source` | hand-written: message holds the path, no job id, no job directory | The identity step accepting a missing source: `the launcher refuses` (the run started). |
| T2c | `treatment_source_moved_tag_is_refused_on_resume` | a moved tag is refused on the second run, and `--force` runs and records the deviation | `git rev-parse` before and after the move | The treatment fact check removed from `check_facts`: `the launcher refuses` (it did not). |

All three T2 guards go red in that order, and each file was restored from a copy afterwards.

## 4. Findings
| # | Kind | Finding | Decision |
|:--|:-----|:--------|:---------|
| F1 | Deviation, accepted | The leaf records `from` "as written", and the shared constraint repeats it, but it also offers `relative_to` as the way to resolve the source. Both cannot hold. `FileValues::relative_to` makes `request.treatment.source` absolute, so the identity step cannot see the written text. The schema doc says "as the experiment file wrote it". I took `relative_to`, because the job spec is serialised to the supervisor and its consumers (the job, the skill archive, the `treatment:` requirement sources) need the resolved path. `from` therefore holds the resolved path (ruling: provenance, outside the digest). The schema doc's wording lives in `candler-schemas`, which I do not own, so it is left for `design_record`. A file that writes an absolute source gives the same value. | None needed. |
| F2 | Out of OWNED, minimal hunk | `resolve/steps/image.rs` read `REQUEST_TREATMENT_SOURCE` for the `treatment:` requirement sources, so removing the setting does not build without it. I read `request.treatment` as a `TreatmentSpec` there, refuse a missing source naming `treatment.source`, and deleted the now unused `required_text`. `declared_inputs` also edits this file, so the coordinator merges the hunks. | Accepted. |
| F3 | Spec defect (gate wording) | T2 says a moved tag is refused "naming `treatment.commit`". The refusal path is `treatment`: `check_facts` in `runner/plan.rs` compares the whole root, and prints `treatment {stored: {...}, observed: {...}}`. Naming `treatment.commit` needs a change to `plan.rs`, which is not OWNED, and it moves the `deviations[].path` that manifests record. T2c asserts that the message names `treatment` and holds both commits (`git rev-parse`), and that the forced run's manifest records both. | Accepted: no `plan.rs` change. |
| F4 | Reversed in round 1 | A treatment with no tag, no commit and no source is refused again, by the identity step (no separate `require_source`), naming `treatment.source`, before any job exists. The step now requires a source of every treatment. T2d guards it. Kept from the first round because they still make sense: `request_settings` without its unused `config` parameter, `OrchestrateError::Treatment` (an unreadable `request.treatment` setting), and `CellRun.treatment_source: Option<PathBuf>` (the identity step makes a source certain on the run path, so the option only covers specs built elsewhere; it can go back to `PathBuf` if the coordinator prefers, at the cost of a wider `CellRun::new` hunk against `declared_inputs`). `require_source` is deleted. | Done. |

Other points: the `StepReport` gained `origins` (serde default, skipped when empty) and `derive_from`, and `Layered` gained `origins` and `origin()`, so the layers turn them into `ValueSource::derived_from`. `sources_of` needed no edit, because it already keeps the typed-root entries. The comment of `field_paths.toml` line 4 no longer names the removed setting. The real-git T1 and the fake-docker T2 leave no process or container behind, and my target dir is removed.

## Fix round 2 (verifier: no blocker, two should-fix items and one note)
Commits after the coordinator's rebase onto 4ca847c are squashed into the three above; the nuisance test is fixed there, so STD runs with no exclusion.

| item | change | guard and bite |
|:-----|:-------|:---------------|
| 1 retire the flag | `request_settings` refuses `--set request.treatment_source=...` by name, before any job exists, with a message that points to `[treatment] source` in the experiment file. The path is built with `concat!` in `run.rs`, so the literal stays out of `crates/*/src` and the "setting gone" grep holds. | `treatment_source_setting_is_refused_by_name`, a relative and an absolute value. Oracle: hand-written message parts, no job id, no job directory. Bite: drop the check; `skills: the launcher refuses` failed. |
| 2 `--set request.treatment` | A treatment given by `--set` has its `source` resolved against the current directory (through `FileValues::relative_to`), a file's against the file. `request_settings` takes the `cwd`. | `treatment_source_given_by_set_is_relative_to_the_cwd`. Oracle: `../skills` from `work`; every `from` in the manifests is absolute, has no `.` or `..` segment and canonicalises to the skills repository. Bite: leave it unresolved; `` `../skills` is absolute `` failed. |
| 3 N1 normalise `from` | `relative_to` normalises the treatment's source lexically (drops `.`, applies `..` to the path text, drops the trailing slash, follows no symlink), for a file's source and a `--set` one alike, so the identity step, `from` and `CellRun` all see one absolute path. | `source_only_one_checkout_is_one_entry`, in T1's file: four spellings of one checkout give the plain path's entry and cell digest. Oracle: the path as the test created it. Bite: skip the normalisation; the `.../one/` spelling recorded a different `from`. |

Out of scope, noted: `request_settings` still accepts any other unknown `request.*` key from `--set`, and a stray value in a cell is possible through such a key. Only the retired name is refused.
