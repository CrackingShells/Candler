# HTN kinds: adapted grader functions

**Goal**: In `$RW`, the plug-in package `htn_creator_candler` holds one pure kind per grader check type, each `(params, context, prior) -> outcome` with R03's signature. The kinds reuse `htn_creator_eval`'s pure helpers at `28be2c15` unmodified and reproduce the recorded A–C grading of all 240 runs (KP1), and every redesigned divergence from the grader is listed in `deltas.toml` (KP2).
**Pre-conditions**:
- [ ] [cmd] `git -C "$RW" rev-parse --abbrev-ref HEAD` prints a branch `m2a/htn_kinds` cut from `feat/candler-htn-plugin`
- [ ] [cmd] The `$RW` environment check of the README Gotchas passes, and the HTNSTD baseline is recorded in R03 §Environment
- [ ] [cmd] `git -C "$R" show 28be2c15:.claude/skills/htn-creator-workspace/infrastructure/evals/eval.json | shasum -a 256 | grep -q '^ca7f12afee55152e'`
**Success Gates**:
- ⬜ [cmd] HTNSTD and SCAFFOLD pass, SUBJ passes in `$RW` (base `feat/candler-htn-plugin`), and OWNED passes in `$RW` with `^\.claude/skills/htn-creator-workspace/candler-plugin/(src/htn_creator_candler/kinds/|tests/(test_guard_kind_parity|test_guard_deltas)\.py$|tests/fixtures/kinds/|deltas\.toml$|pyproject\.toml$|uv\.lock$)`
- ⬜ [cmd] `cd "$RW/$PLUGIN" && CANDLER_GOLDEN_CORPUS="$GOLDEN" uv run --frozen pytest -q -m golden tests/test_guard_kind_parity.py` passes (KP1), and fails when the variable is unset. The report records its wall-clock.
- ⬜ [cmd] `cd "$RW/$PLUGIN" && uv run --frozen pytest -q tests/test_guard_deltas.py` passes (KP2, KP3)
- ⬜ [cmd] Byte-identity: `cd "$RW/$PLUGIN" && uv run --frozen python -c 'import htn_creator_eval.grader as g;print(g.__file__)' | xargs shasum -a 256` equals `git -C "$R" show 28be2c15:.claude/skills/htn-creator-workspace/infrastructure/htn_creator_eval/grader.py | shasum -a 256`, and likewise for every checker script the kinds run
- ⬜ [cmd] `test -z "$(grep -rnE 'candler_evaluator' "$RW/$PLUGIN/src/htn_creator_candler/kinds")"` (kinds do not import the helper)
- ⬜ [nl] The adversarial verifier closes with no open BLOCKER or MAJOR before `htn_plugin` step 3
**References**: [R03 §Adapt don't wrap, §Quirk table, §Kind signature, §Needs](../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md); [R04 KP1–KP3](../../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md); `htn_creator_eval/grader.py` at `28be2c15` — `grade_expectation`, the `grade_*` functions, `classify_outcome`, `_finalize_metric`, `parse_ratio_markers`, `find_output`; `evals/eval.json` at `28be2c15`; M1 `legacy_mapping.md` — how 0.4.2 rows map to outcome records

## Step 1: Set up the plug-in project with a pinned instrument
**Goal**: A uv project in `$PLUGIN` installs the grader and the instrument from `git+file` at `28be2c15`, never editable and never from `$RW`.
**Implementation Logic**:
1. `pyproject.toml` of distribution `htn-creator-candler` (package `htn_creator_candler`, src layout), version `0.5.0`, Python as the workspace infrastructure's. Dependencies: `htn-creator-eval` and `htn-creator-scripts` from `git+file://$R@28be2c15#subdirectory=…`, plus pytest and ruff as dev dependencies. `candler-evaluator` is not a dependency yet; `htn_plugin` adds it.
2. `uv lock`. The report shows `uv lock --check` passes and `grep -nE 'editable|candler-htn-plugin|mcp-python-ingestion"' uv.lock` finds no editable or `$RW` source.
3. A `[tool.pytest.ini_options]` marker `golden` and `scaffold`.
**Deliverables**: `$PLUGIN/pyproject.toml`; `$PLUGIN/uv.lock`; `$PLUGIN/src/htn_creator_candler/__init__.py`; `$PLUGIN/src/htn_creator_candler/kinds/__init__.py`
**Consistency Checks**: `cd "$RW/$PLUGIN" && uv lock --check && uv run --frozen python -c 'import htn_creator_eval.grader, htn_creator_candler.kinds'` (expected: PASS)
**Commit**: `build(htn-eval): set up the candler plug-in with the pinned instrument`

## Step 2: Write the kinds as pure functions
**Goal**: Each grader check type has one kind that takes its spec as params and returns one outcome, keeping every quirk that changes an A–C row.
**Implementation Logic**:
1. `kinds/` holds one function per `grading_spec.type` the 37 A–C entries use, named after it: `grep_present`, `grep_absent`, `grep_all_present`, `run_script`, `run_script_module`, `metric_extract`, `metric_count`, `planner_solvability`, plus the closed `ratio` kind that replaces `metric_compute` (C6, its only user). No kind is written for a type the suite does not use, such as `grep_count_ge`. Each has R03's signature, reads files only from `context.artifacts` under `outputs/` with the grader's own `find_output` rule, and returns the outcome R03 maps (status, value, weight, cause, evidence).
2. Kept quirks, as explicit params with the grader's default: `on_missing: fail` (B8); an optional source missing → `blocked {artifact: <path>}`; a broken metric → value 0 and pass; C6 divide-by-zero → `0.0` and pass; the grader's regexes, rounding and int-vs-float; a 0/0 pass → `not_applicable {reason: empty_population}`.
3. Redesigned behaviours, each a row of `deltas.toml` (`expectation_kind`, `grader_behaviour`, `kind_behaviour`, `why`): a checker timeout or spawn failure → `not_measured {reason: instrument_error}`; a ratio `ValueError` → `not_measured`; ratio specs move into params.
4. Checkers run through `runpy.run_module` in the caller's process with stdout and stderr captured and `SystemExit` read as the exit code. The helper's fork isolates them; the kinds do not spawn.
5. Reuse `htn_creator_eval`'s pure helpers (`classify_outcome`, `_finalize_metric`, `parse_ratio_markers`, `extract_with_regexes`, `_cast`) by import; never copy or edit them.
**Deliverables**: `$PLUGIN/src/htn_creator_candler/kinds/{__init__,grep,scripts,metrics,ratio,planner,_files}.py` (the kind functions above, `KINDS` registry dict); `$PLUGIN/deltas.toml`
**Consistency Checks**: `cd "$RW/$PLUGIN" && uv run --frozen ruff check && uv run --frozen pytest -q -m scaffold` (expected: PASS)
**Commit**: `feat(htn-eval): adapt the grader checks as pure candler kinds`

## Step 3: Prove kind parity on the corpus
**Goal**: Driven by the corpus's own `outputs/`, the kinds give exactly the recorded A–C rows of all 240 runs.
**Implementation Logic**:
1. `tests/test_guard_kind_parity.py`, KP1 (R4), marked `golden`, fails when `CANDLER_GOLDEN_CORPUS` is unset: for each run and each of the 37 A–C expectations of `eval.json` at `28be2c15`, call the kind with the params the suite translation will give (a local table in the test, not the plug-in's translator) on that run's `outputs/`, and compare status, value (int vs float included), weight and cause with the run's `eval-K-grading.json` mapped by M1's legacy mapping, with the 0/0 pass mapped to `not_applicable {empty_population}`. It asserts 240 runs and 8,880 rows first, and that the B8, C1 and C6 quirk rows are present.
2. Oracle: the raw `eval-K-grading.json`. Bite: flip the `on_missing` default; record it and the wall-clock.
**Deliverables**: `$PLUGIN/tests/test_guard_kind_parity.py` (KP1); `$PLUGIN/tests/conftest.py`
**Consistency Checks**: `cd "$RW/$PLUGIN" && CANDLER_GOLDEN_CORPUS="$GOLDEN" uv run --frozen pytest -q -m golden tests/test_guard_kind_parity.py` (expected: PASS)
**Commit**: `test(htn-eval): prove kind parity on the 240 golden runs`

## Step 4: Pin the divergences from the grader
**Goal**: Where the corpus never reaches, the pinned grader and the kinds disagree exactly where `deltas.toml` says, and KP1's machinery is shown able to fail.
**Implementation Logic**:
1. `tests/test_guard_deltas.py`, KP2 (R4): extract `grader.py` from `git -C $R show 28be2c15:…` into tmp and run it in a separate process on synthetic fixtures (`tests/fixtures/kinds/`, hand-written from the grader's text, ≤ 20 KB). Run the kinds on the same fixtures; the set of differing (fixture, field) pairs equals `deltas.toml`'s rows, with none unlisted and none stale. The test's docstring says KP2 proves the glue only.
2. KP3 (R4): one mutated reference row, and one regex hit flipped in a copy of an `outputs/` file: each makes KP1's compare function fail. KP3 runs on the synthetic fixtures, not the corpus.
3. Delete every `mark.scaffold` test, run the one-shot mutation sweep over the kinds (survivors that matter become HP1 rows in `htn_plugin`), record the bites, and dispatch the adversarial verifier: the runpy glue (argv, cwd, `sys.path`, `sys.modules`), capture ordering, `SystemExit` forms, the file rule, int vs float, the 0/0 path.
**Deliverables**: `$PLUGIN/tests/test_guard_deltas.py` (KP2, KP3); `$PLUGIN/tests/fixtures/kinds/`
**Consistency Checks**: `cd "$RW/$PLUGIN" && uv run --frozen pytest -q tests/test_guard_deltas.py && test -z "$(git grep -n 'mark.scaffold' -- tests src)"` (expected: PASS)
**Commit**: `test(htn-eval): pin every divergence from the grader in deltas.toml`
