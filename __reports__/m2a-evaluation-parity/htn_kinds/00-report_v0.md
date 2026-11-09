# htn_kinds: leaf report (v0)

The implementer's first report follows as it came back, with its headings moved one level down. The rulings on its findings and on the adversarial verifier's are in [l1/00-rulings_v0.md](../l1/00-rulings_v0.md) §htn_kinds and §htn_kinds verifier. The leaf lives in the HTN repo, on `feat/candler-htn-plugin` in `$RW`.

## First report

Branch `m2a/htn_kinds` in `$RW` (cut from `feat/candler-htn-plugin` at 28be2c15). Nothing pushed. `$R` untouched.

### 1. Commits
| sha | subject |
|---|---|
| 2322ad13 | build(htn-eval): set up the candler plug-in with the pinned instrument |
| 48c7b2d5 | feat(htn-eval): adapt the grader checks as pure candler kinds |
| ac375152 | test(htn-eval): prove kind parity on the 240 golden runs |
| 1976fddd | test(htn-eval): pin every divergence from the grader in deltas.toml |

All end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. `deltas.toml` lands in the fourth commit, with the test that proves it (step 2's deliverable list names it, but a delta list without KP2 would be unproven). The Step 4 commit also carries `addopts = "-m 'not golden'"` in `pyproject.toml` (see finding 4).

### 2. Gates
| gate | result | last line |
|---|---|---|
| HTNSTD plug-in: `uv run --frozen pytest -q` | PASS | `6 passed, 1 deselected in 35.61s` |
| HTNSTD plug-in: `uv run --frozen ruff check` | PASS | `All checks passed!` |
| HTNSTD root: `mamba run -n forMpyIn pytest -q -p no:cacheprovider` | PASS | 83 distinct failing or erroring ids, all 83 in `htnstd_baseline.txt`; 0 new, 0 fixed (the run printed no summary line to the log) |
| SCAFFOLD | PASS | 7 tests collected (`-m "golden or not golden"`), 7 `Guards:` lines; `git grep -n 'mark.scaffold' -- tests src` is empty |
| SUBJ (base `feat/candler-htn-plugin`) | PASS | `cog check` says `No errored commits` (the HTN repo has no cog.toml); longest subject 70 characters |
| OWNED | PASS with one known exception | only `src/htn_creator_candler/__init__.py` (step 1's deliverable) falls outside the regex; finding 2 |
| KP1 `CANDLER_GOLDEN_CORPUS=$GOLDEN pytest -q -m golden tests/test_guard_kind_parity.py` | PASS | `1 passed in 314.39s` (`KP1 wall-clock: 314.3 s over 240 runs, 8880 rows`) |
| KP1 with the variable unset | FAIL as required | `FAILED ...test_kinds_reproduce_the_recorded_grading_of_every_golden_run`, `1 failed in 0.16s` |
| KP2/KP3 `pytest -q tests/test_guard_deltas.py` | PASS | `6 passed in 28.44s` |
| Byte-identity, grader | PASS | installed `htn_creator_eval/grader.py` sha256 `5970c6d5...c9ec4` equals `git show 28be2c15:...grader.py` |
| Byte-identity, checkers | PASS | all 16 checker modules the 37 checks run (`validate_domain`, `validate_problems`, `validate_mapping_config`, `validate_method_decompositions`, `check_*` x9, `analyze_*` x3) match 28be2c15 |
| Byte-identity, schema | PASS | `kinds/_data/action_tool_mapping_schema.json` sha256 `a28cd7e80a9bc2bb...` equals the pinned file |
| No helper import | PASS | `grep -rnE 'candler_evaluator' kinds/` empty |
| `uv lock --check` | PASS | `Resolved 65 packages` |

### 3. Guards
| test | Guards line | oracle | bite tried |
|---|---|---|---|
| `test_guard_kind_parity::test_kinds_reproduce_the_recorded_grading_of_every_golden_run` | R1 | raw `eval-K-grading.json` of the 240 runs, mapped by M1's legacy mapping | `on_missing` default flipped to `blocked` (copy of src on `PYTHONPATH`): FAILS, 258 s, B8 status/cause and A2 cause mismatch on every missing-target run |
| `test_guard_deltas::test_the_kinds_differ_from_the_pinned_grader_exactly_where_deltas_toml_says` | R2 | grader at 28be2c15, extracted by `git archive`, run in its own process on 13 synthetic cases | the mutation sweep below: 89 of 173 and 93 of 141 mutants killed with it |
| `test_a_mutated_reference_row_fails_the_compare_function` | R3 | the pinned grader's own entries for the base fixture (the unmutated pair is the positive control) | A1 status flipped and C1 count made a float: each named by `compare_run` |
| `test_a_flipped_regex_hit_in_an_outputs_copy_fails_the_compare_function` | R3 | the same entries on the unflipped base | `Class: Action` rewritten in a copy of `domain.py`: rows change |
| `test_the_glue_gives_what_a_real_child_process_gives` | R4 | a real `python -m` child, same cwd | `& 0xFF` mask, SystemExit branches, argv, chdir (sweep) |
| `test_the_glue_leaves_the_process_as_it_found_it` | R5 | snapshot before the call (cwd, argv, path, modules, streams, bytecode flag, fds, set-aside `gtpyhop`) | restores and purge (sweep) |
| `test_the_glue_captures_descriptor_level_writes_in_order` | R6 | the write order in the probe's own text | write order and child-process line (sweep) |

### 4. One-off records
- KP1 wall-clock: 380.8 s (first full pass, idle machine), 314.3 s (final HEAD, run beside the root pytest). Bite run: 258.3 s.
- Corpus: 12 cells, 240 runs, 8,880 rows, 0 mismatches. Quirk rows present: B8 (60 fails), C1 (pass, value 0), C6 (float 0.0).
- Synthetic cases (11.7 KB of fixtures): base, missing_artifacts, empty_outputs, broken_domain, file_rule, zero_all_present, zero_absent, no_scenarios (agreement); ratio_marker_missing, ratio_inconsistent, spawn_failure, dot_path, on_missing_blocked (differences).
- `deltas.toml` rows (5): `ratio_value_error` (8 case/step pairs; grader crash against `not_measured`), `spawn_failure_metric_value` (C1, C8), `spawn_failure_metric_threshold` (C4), `dot_path_artifact`, `on_missing_blocked`. The 0/0 population is not a row: M1's legacy mapping already reads it as `not_applicable {empty_population}`.
- Mutation sweep (one-shot script, 22 operator swaps per code line, run under KP2/KP3 only, KP1 not included): sweep 1 had 173 mutants (including docstring lines), 89 killed. After adding the glue tests and filtering docstrings, sweep 2 had 141 mutants, 93 killed. Two survivors were then killed with new fixtures (`file_rule` has a shallower sibling: `min`->`max` in `find_artifact`; `no_scenarios`: planner total 0 is equivalent through the 0/0 mapping).
- Survivors that matter, HP1 candidates for `htn_plugin` (KP1 kills some of the evidence-free ones on the corpus, which the sweep did not run):
  - `alias` param honoured (`_common.py` `params.get("alias") or minted`): untested; no outcome field depends on it.
  - `runpy.run_module(alter_sys=True)` -> False: no checker the suite runs reads `sys.modules["__main__"]`.
  - `write_through` on stderr, `closefd` of the stream wrappers, `rmtree(ignore_errors)`: no observable outcome.
  - `metrics.py` / `planner.py` evidence strings and the `exit_code == -1 or "staging failed"` pair (each half implies the other): equivalent or evidence-only.
  - `ratio.py` evidence text for missing dependencies (`ids[0]`).

### 5. Findings and deviations
1. **Spec defect: the Step 1 grep gate.** `grep -nE 'editable|candler-htn-plugin|mcp-python-ingestion"' uv.lock` matches the root entry `htn-creator-candler` (`source = { editable = "." }`), which must be installed for `htn_creator_candler.kinds` to import. The instrument packages are `git+file` at 28be2c15 as required. Propose: allow only the root editable and require the two instrument sources to be `git+file`.
2. **Spec defect: OWNED regex.** It omits `src/htn_creator_candler/__init__.py` and `tests/conftest.py`, both named in the leaf's Deliverables. I wrote no `conftest.py` (shared test helpers live in `test_guard_kind_parity.py` and KP2/KP3 import them).
3. **`.gitignore` ignores `uv.lock`.** Step 1 used `git add -f`.
4. **`pytest -q` and the golden guard conflict.** The HTNSTD gate runs `pytest -q`, and KP1 must fail without the corpus. Resolved by `addopts = "-m 'not golden'"`; the explicit `-m golden` of the KP1 gate overrides it. Needs ruling if the gate text should change instead.
5. **Pinned scripts wheel omits `schemas/action_tool_mapping_schema.json`.** B8, C-M1 and C-M2 returned `not_measured` until the kinds shipped a byte-identical copy (`kinds/_data/`) and staged it into each staging root. The grader at the pin only found it through a source or editable layout. `htn_plugin` should know the pinned wheel is not self-sufficient.
6. **`tomllib` in KP2** needs Python 3.11; `requires-python` is `>=3.10` (the venv is 3.12). Not changed (it would rewrite the lock).
7. **Capture order differs from a real child.** A child on a pipe block-buffers `print`, so `print` plus `os.write` output interleaves differently; the kinds capture in write order. No checker mixes them, so it is not a row of `deltas.toml`; R6's docstring says so.
8. **A grader NOT_MEASURED has no cause in the legacy mapping.** KP2's oracle reads it as `not_measured {instrument_error}` (documented in `grader_outcome`); the corpus has no such row.
9. **Shapes I had to choose (unspecified):** wire-shaped kind outputs; `<<domain_name>>` and `<<alias>>` tokens (optional `alias` param); ratio markers referenced by id; a 0/0 fail also maps to `not_applicable`; the kinds apply no timeout (the helper's fork does); `find_artifact` re-implemented over `context.artifacts` and checked equal to the grader's `find_output` on all 240 runs; `run_script` runs the real target in a private staging root as cwd.
10. **Dot-led path difference** (delta `dot_path_artifact`) follows from the ruled file rule; the grader's `rglob` finds such files.
11. The adversarial verifier is dispatched by the coordinator; the glue concerns it names (argv, cwd, `sys.path`, `sys.modules`, capture order, SystemExit forms, file rule, int vs float, 0/0) are covered by R4-R6, the `file_rule` case, KP1's strict JSON-text compare and the 0/0 agreement cases.

## Adversarial verifier: findings and their fate
KP1 passed unbitten (215 s), and failed with the corpus variable unset. The grader, all checkers and the schema copy are byte-identical to `28be2c15`. The glue matched a real `python -m` child on every `SystemExit` form tried.

| Finding | Fate |
|---|---|
| MAJOR-1: a 0/0 fail became `not_applicable`, and KP2's oracle reused KP1's mapping, so the path was unguarded (own bite 4b survived) | Fixed: only a 0/0 pass maps to `not_applicable`. Fixture `zero_fail`, with the grader's raw entry as oracle |
| MAJOR-2: non-UTF-8 checker stdout decoded leniently (pass, where the grader gives `not_measured`) | Fixed: strict decode. Fixture `binary_stdout` agrees with the grader on every field |
| MAJOR-3: a closed `sys.stdout` made the first restore step raise, and cwd, argv, path, fds and streams stayed unrestored | Fixed: every restore step runs on its own. Guard R8 |
| MINOR: `SystemExit` ints beyond a C long read as 0 | Fixed: 255 plus the `OverflowError` text, as a child gives. R4 covers five edge values |
| MINOR: `find_artifact` ties followed the order of `context.artifacts` | Fixed: ties sort by path. The grader follows filesystem order on an equal-depth tie, so the two may differ there; no fixture can pin that order |
| MINOR: the `.claude` clause was untested (own bite 4a survived) | Fixed: R9 passes artifacts with no dot filter |
| MINOR: `on_missing` accepted any value | Fixed: `fail` or `blocked` only (R10) |
| MINOR: no timeout and no row | Row `checker_timeout`, `covered_by = "helper"` |
| Spec note: Candler blocks C6 after an assert fail of C1 or C5, the grader computes it | Row `c6_after_failed_assert`, `covered_by = "engine"` |
| MINOR: in-process state leaks (`os.environ`, recursion limit, `sys.modules`, a host `domain` module, `os._exit`) | Accepted: the helper forks per item. KP2's docstring says so |
| MINOR: KP1 compares `find_artifact` with `find_output` for 3 names | Accepted |
| NITs: surrogate escapes, `fill` on every string | Accepted |

## Fix round
- 774cfef1 `fix(htn-eval): keep a 0/0 fail, decode strictly, restore every step`
- 483f968b `test(htn-eval): pin the verifier cases and the unreachable deltas`

Rows marked `covered_by` are left out of KP2's stale check by that field only. An unknown value fails, and so does a marked row that a fixture reaches. KP2 now has 10 tests. Its bites, each failing a named test: the 0/0 mapping reverted, the `.claude` clause removed, the tie key reduced to depth, the `on_missing` check disabled, and the restore flush unguarded.

## Re-check
The verifier closed all three MAJORs with its own probes and bites. KP1 passed unbitten again (199 s), so the tie-break changed no row. It found no new BLOCKER or MAJOR. Two notes are accepted:
- Nothing proves a `covered_by` row unreachable. `checker_timeout` could be reached by a fixture sleeping past the grader's 60 s.
- Non-UTF-8 output from a metric kind falls under the `spawn_failure_metric_value` row's class, since the grader takes its spawn-failure path, but the row's wording does not name it.

## Landing gates (coordinator, at 483f968b)
| Gate | Result |
|---|---|
| OWNED (widened regex) against `feat/candler-htn-plugin` | PASS |
| SUBJ | 6 subjects, longest 70 characters; 6 Sonnet trailers |
| Lock: `uv lock --check`, root project the only editable source, both instruments git at `28be2c15` | PASS |
| HTNSTD plug-in: `pytest -q`, `ruff check` | `10 passed, 1 deselected`; `All checks passed!` |
| KP2 and KP3 | `10 passed` |
| SCAFFOLD | no scaffold marker; 10 tests = 10 `Guards:` lines, and 1 = 1 (counted with `-m "golden or not golden"`) |
| Byte-identity | `grader.py` and every `htn_creator_scripts` module equal `28be2c15` |
| No helper import in `kinds/` | PASS |
| KP1 with `$GOLDEN` | `1 passed in 158.56s`; with the variable unset, `1 failed` |
| HTNSTD root: `mamba run -n forMpyIn pytest -q -p no:cacheprovider`, with `mcp_python_ingestion` resolved from the leaf worktree | 83 failing ids, exactly the 83 of `htnstd_baseline.txt`; 0 new, 0 fixed |
| Fixture budget | `tests/fixtures/kinds/` 15,536 bytes (≤ 20 KB), `deltas.toml` 4,653 bytes (≤ 5 KB) |

Landed as `11ca78d2 merge(htn-eval): land htn_kinds` on `feat/candler-htn-plugin`, `--no-ff`, unpushed.
