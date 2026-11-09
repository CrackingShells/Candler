# HTN plug-in preloads its checker libraries

**Goal**: The plug-in imports the checkers' heavy third-party libraries once in the helper's parent, before any fork, and ships as `htn-creator-eval` 0.5.1 with identical outcomes.
**Pre-conditions**:
- [ ] [cmd] `git -C "$R" rev-parse -q --verify 'refs/tags/htn-creator-eval@0.5.0^{commit}'`
- [ ] [nl] The implementer works in its own `$R` worktree on `m2a/htn_preload`, cut from `feat/candler-htn-plugin`
**Success Gates**:
- ⬜ [cmd] HTNSTD passes, and OWNED passes with `^\.claude/skills/htn-creator-workspace/(candler-plugin\.toml$|candler-plugin/(src/htn_creator_candler/|tests/test_guard_preload\.py$|(pyproject\.toml|uv\.lock|README\.md)$)|infrastructure/CHANGELOG\.md$)`
- ⬜ [cmd] KP1 (`test_guard_kind_parity`, local, against `$GOLDEN`) and KP2 pass unchanged, and the report records KP1's wall-clock (291.72 s before)
- ⬜ [cmd] `grader.py` and the checkers stay byte-identical to 28be2c15
- ⬜ [cmd] With the Candler integration tip's release `candler`, PERF12 at `--concurrency 4` is no slower than without the preload, and the report records both numbers
**References**: [profile §3, culprit 2](../../../../../../__reports__/m2a-evaluation-parity/throughput/00-regrade_profile_v0.md); `kinds/_runner.py` (`_FRESH_PREFIXES`, `runpy.run_module`); [rulings §Regrade throughput](../../../../../../__reports__/m2a-evaluation-parity/l1/00-rulings_v0.md); [R03 §Adapt, don't wrap or port](../../../../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md)

PERF12 is defined in `regrade_throughput.md`.

## Step 1: Preload the checkers' third-party libraries
**Goal**: A forked child finds libcst, arango, jsonschema and the other heavy libraries the checkers import already in `sys.modules`.
**Implementation Logic**:
1. Each child re-imports libcst (about 116 ms) or arango plus jsonschema (about 220 ms). `kinds/_runner.py` runs the checker through `runpy.run_module`, and the parent never imported those libraries. `_FRESH_PREFIXES = ("gtpyhop", "htn_creator_scripts.")` names the modules that must be fresh for each item, and third-party libraries are outside it.
2. Derive the preload list from the pinned checkers' module-level imports, measured with `python -X importtime`. Keep only modules outside `_FRESH_PREFIXES`. Import them when the plug-in module is imported, which the helper does before its first fork.
3. A new guard, KP4 (R4), `tests/test_guard_preload.py`, makes two checks:
   - after the plug-in module is imported, no module under `_FRESH_PREFIXES` is in `sys.modules`;
   - every module on the preload list is.
   Oracle: the fresh-prefix rule as the htn_kinds report states it. Bite: add `gtpyhop` to the preload list.
4. KP1 and KP2 pass unchanged.
**Deliverables**: the preload list and its import in `src/htn_creator_candler/`, `tests/test_guard_preload.py` (KP4); the commit body carries PERF12 before and after
**Consistency Checks**: `cd "$RW/$PLUGIN" && uv run --frozen pytest -q` (expected: PASS)
**Commit**: `perf(htn-eval): preload the checkers' libraries before the first fork`

## Step 2: Release it as 0.5.1
**Goal**: The live identity names the code that graded.
**Implementation Logic**:
1. Bump the plug-in to 0.5.1 (`pyproject.toml`, the manifest, the lock). Add `## [0.5.1] - <date>` to `infrastructure/CHANGELOG.md`: "Changed: the checkers' libraries load once per request; outcomes are identical (KP1)". Under I4, fixes after the tag ship as 0.5.x.
2. The coordinator lands the branch with `--no-ff` on `feat/candler-htn-plugin`, merges it into `feat/skill-htn-creator`, and cuts the local annotated tag `htn-creator-eval@0.5.1` on that merge. Nothing is pushed.
**Deliverables**: the version bump and the CHANGELOG entry, plus the leaf report as the final message, with PERF12, KP1 and the KP4 bite
**Consistency Checks**: `cd "$RW/$PLUGIN" && uv lock --check` (expected: PASS)
**Commit**: `chore(htn-eval): bump htn-creator-eval to 0.5.1`
