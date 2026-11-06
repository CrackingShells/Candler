# HTN plug-in: manifest, environment, suite

**Goal**: In `$RW`, the HTN kinds become the Candler plug-in `htn-creator-eval@0.5.0`. It has a manifest, a frozen uv environment that pins `candler-evaluator` from the integration branch, and a suite translated from `eval.json` at `28be2c15`. The translator refuses any other `eval.json` (HP2), and the suite's inverse fold equals the source apart from the declared moves (HP1).
**Pre-conditions**:
- [ ] [cmd] `git -C "$RW" log --first-parent --format=%s feat/candler-htn-plugin | grep -qx 'merge(htn-eval): land htn_kinds'`
- [ ] [cmd] `git -C /Users/hacker/Documents/src/CrackingShells/Candler rev-parse --verify -q "$(git rev-parse feat/m2a-evaluation-parity)^{commit}"`, i.e. the commit to pin is reachable from the Candler main checkout
- [ ] [nl] The `htn_kinds` adversarial verifier is closed before Step 3 (proxy: its report)
**Success Gates**:
- ⬜ [cmd] HTNSTD, SUBJ (in `$RW`) and SCAFFOLD pass, and OWNED passes in `$RW` with `^\.claude/skills/htn-creator-workspace/candler-plugin/(src/htn_creator_candler/(?!kinds/)|tests/(test_guard_suite\.py|fixtures/suite/)|plugin\.toml|suite/|pyproject\.toml|uv\.lock|README\.md)`
- ⬜ [cmd] `cd "$RW/$PLUGIN" && uv run --frozen pytest -q tests/test_guard_suite.py` passes (HP1, HP2)
- ⬜ [cmd] `cd "$RW/$PLUGIN" && uv lock --check && test -z "$(grep -nE 'editable|candler-htn-plugin|mcp-python-ingestion"' uv.lock)"`, and `grep -c 'git+file:///Users/hacker/Documents/src/CrackingShells/Candler' uv.lock` is at least 1
- ⬜ [cmd] `cd "$RW/$PLUGIN" && uv run --frozen python -c 'from candler_evaluator.identity import identity;print(identity("htn-creator-scripts").digest)' | grep -q '^git:28be2c15'`
- ⬜ [nl] The adversarial verifier holds no open BLOCKER or MAJOR: params the corpus never measures (from the htn_kinds mutation sweep), weights, `depends_on`, optional flags
**References**: [R03 §Identity, §Needs, §C6 dependencies, §Cross-repo pin, §Scope move](../../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md); `crates/candler-schemas/schema/{plugin-manifest,suite}.schema.json`; `evals/eval.json` at `28be2c15` (sha `ca7f12af…`); [R04 HP1, HP2](../../../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md)

## Step 1: Declare the manifest and pin the helper
**Goal**: The plug-in has a schema-valid manifest naming its kinds, and its lock takes `candler-evaluator` from a commit of the integration branch.
**Implementation Logic**:
1. The manifest, valid against `plugin-manifest.schema.json`: name `htn-creator-eval`, version `0.5.0`, the entry command (`python -m htn_creator_candler`), and `environment = { kind = "uv", project, lock }`. The manifest holds no kinds: `describe` returns one `KindSpec` per kind of `kinds.KINDS` with its params schema, and the needs are `artifacts` (`outputs/**`) and `metadata` (`plan.prompt_domain_name`), and nothing else (A–C scope). `project` and `lock` are `RelPath`s relative to the manifest's directory and `RelPath` refuses `.`, while `$PLUGIN` is itself the uv project (`htn_kinds` put `pyproject.toml` and `uv.lock` there). So the manifest sits one level up, at `.claude/skills/htn-creator-workspace/candler-plugin.toml`, with `project = "candler-plugin"` and `lock = "candler-plugin/uv.lock"`. The coordinator widens this leaf's OWNED and the `$PLUGIN` gates to that path before dispatch.
2. `src/htn_creator_candler/__main__.py` calls `candler_evaluator.serve` with the kinds and the alias minting R03 describes (the plug-in mints `{alias}`; nothing is templated).
3. `pyproject.toml` adds `candler-evaluator @ git+file:///Users/hacker/Documents/src/CrackingShells/Candler@<sha>#subdirectory=python/candler-evaluator`, with `<sha>` the integration tip. `uv lock`, and record the sha in the report. The coordinator re-pins after the final rebase.
**Deliverables**: `$PLUGIN/plugin.toml`; `$PLUGIN/src/htn_creator_candler/__main__.py`; `$PLUGIN/pyproject.toml`; `$PLUGIN/uv.lock`
**Consistency Checks**: `cd "$RW/$PLUGIN" && uv lock --check && uv run --frozen python -c 'import candler_evaluator, htn_creator_candler.__main__'` (expected: PASS)
**Commit**: `feat(htn-eval): declare the candler plug-in manifest`

## Step 2: Translate the suite from the pinned eval.json
**Goal**: A committed translator turns `eval.json` at `28be2c15` into the plug-in's Candler suite, refusing any other source.
**Implementation Logic**:
1. `src/htn_creator_candler/translate.py <eval.json> <out dir>`: refuse unless the input's sha256 is `ca7f12afee55152ed0e543a1d9b457c29fc5c02fd49e1ec6a417a9dcc1c97f9e`. Emit the 37 A–C entries (D-E1…6 left out and listed in the suite's README) with their kind, params (quirk params made explicit, ratio specs moved into params), weight, role, `applies_to`, optional flag and `depends_on` (C6 on C1 and C5).
2. `{domain_name}` becomes the `plan.prompt_domain_name` metadata need, and no `{…}` template survives.
3. Commit the generated `suite/` and the translator together.
**Deliverables**: `$PLUGIN/src/htn_creator_candler/translate.py` (`translate`, `EXPECTED_SHA256`); `$PLUGIN/suite/`
**Consistency Checks**: `cd "$RW/$PLUGIN" && git -C "$R" show 28be2c15:.claude/skills/htn-creator-workspace/infrastructure/evals/eval.json > "$TMPDIR/eval.json" && uv run --frozen python -m htn_creator_candler.translate "$TMPDIR/eval.json" "$TMPDIR/suite" && diff -r "$TMPDIR/suite" suite` (expected: PASS)
**Commit**: `feat(htn-eval): translate the pinned eval.json into a candler suite`

## Step 3: Guard the suite translation
**Goal**: HP1 and HP2 decide the translation against the pinned source.
**Implementation Logic**:
1. `tests/test_guard_suite.py`:
   - HP1 (R4): fold the suite back into `eval.json`'s shape (an inverse written in the test from `eval.json`'s text). It equals the A–C part of `eval.json` at `28be2c15` except for a hand-written list of declared moves (template → need, ratio spec → params, C6 dependencies, explicit quirk params). The suite has 37 entries and no `{…}`. Survivors from the `htn_kinds` mutation sweep become rows here. Oracle: `eval.json` at `ca7f12af`. Bite: swap two weights.
   - HP2 (R7): the translator exits non-zero on a copy of `eval.json` with one byte changed, and zero on the pristine copy (positive control). Oracle: the sha literal. Bite: skip the sha check.
2. Delete scaffold tests, record bites, dispatch the adversarial verifier.
**Deliverables**: `$PLUGIN/tests/test_guard_suite.py` (HP1, HP2); `$PLUGIN/tests/fixtures/suite/`
**Consistency Checks**: `cd "$RW/$PLUGIN" && uv run --frozen pytest -q tests/test_guard_suite.py && test -z "$(git grep -n 'mark.scaffold' -- tests src)"` (expected: PASS)
**Commit**: `test(htn-eval): guard the suite translation against the pinned source`
