# Comparability diff and stratum verdict

**Goal**: `candler_core::manifest::compare` diffs two `Comparable` values by field-path segments and gives a stratum verdict (comparable, refused with the offending paths, or malformed), so that arms are never compared unless they differ only in declared factors (I7).
**Pre-conditions**:
- [ ] [cmd] Phase 0 is landed: `for s in '^docs(design): ' '^build(repo): ' '^ci(repo): ' '^feat(schemas): ' '^feat(orchestrator): scaffold'; do git log --format=%s origin/main..feat/m2b-execution-parity | grep -q "$s" || exit 1; done && test -s __roadmap__/m2b-execution-parity/feature_map/feature_map.json`
- [ ] [nl] api_review closed with no open finding on `Comparable`, `Deviation` and the nuisance set
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_comparability)'` passes, holding B03 and B04. **B03 verdict table**: rows C01 to C23, read from the data file `tests/fixtures/comparability/verdicts.json` (the lead's rows copied, rows added and never edited), each stating its two arms, the declared set F, and the literal verdict (comparable, `refused(<paths>)` with the exact path list, or malformed); the oracle is the R01 sentence the row cites, and no row computes its expectation from `compare`. **B04 nuisance set**: the crate's `NUISANCE_PATHS` equals the nuisance list parsed from the manifest's hashed-paths file (`hashed_paths.toml`) by the test, as sets of strings, and no `comparable.*` path is in it
- ⬜ [cmd] STD, SUBJ and LINUX pass
- ⬜ [nl] The leaf report has a bite record, one mutation per B test (B03: the path match becomes `starts_with` on strings; B04: `comparable.inputs` is added to the nuisance set), each run only against `guard_comparability`, each restored with `git checkout --`
- ⬜ [cmd] `! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/manifest/compare.rs crates/candler-core/tests/guard_comparability.rs`
- ⬜ [nl] The required adversarial verifier reports no open finding. It states each test's oracle, names tautologies, and re-runs three bites from the leaf report plus one of its own (at most two e2e). It attacks nuisance leakage (a `comparable.*` path in the nuisance set), a factor hiding a drift (a declared path covering a sibling or a second diff), a lying `factors[]` in a manifest, path-prefix matches (`model.idx` against `model.id`), and list order and duplicates
**References**: [R01 §Run manifest, §Experiment file, §Error model](../../__design__/architecture.md) — I7, the hashed set, `factors[]` and `unrecorded`; [Architecture §11](../../__reports__/m2b-execution-parity/00-architecture_v0.md) — field paths; [Test definition B03, B04, C01 to C23, F5](../../__reports__/m2b-execution-parity/01-test_definition_v0.md) — the rows and the ruling on `census.init_tools`

## Step 1: Diff two comparable values by path segment
**Goal**: Two `Comparable` values give the exact list of paths where they differ, and a path never matches another by characters.
**Implementation Logic**:
1. `compare(stored: &Comparable, observed: &Comparable) -> Vec<Deviation>` flattens each side to `(path segments, leaf)` pairs and walks both: a path present on one side only, or with unequal leaves, yields `Deviation {path, stored: Option<_>, observed: Option<_>}` (the schema type; this leaf does not touch `candler-schemas`). `unrecorded` (a value with no `value`) against a value is a difference (C16). The result is sorted by path and has no duplicates.
2. Paths are `Vec<String>` segments split on `.`, and the covering test is `fn covers(declared: &[&str], path: &[&str]) -> bool`: segment-wise prefix, so `model.id` covers `model.id` and `model.id.x`, never `model.idx` (C07). No function in this module calls `str::starts_with` on a path.
3. The six `comparable` parts (`values`, `harness`, `treatment`, `snapshots`, `inputs`, `environments`) are all compared; lists are compared in their stored order because a reader refuses an unsorted or duplicated list (Step 2).
**Deliverables**: `crates/candler-core/src/manifest/compare.rs` (`compare`, `covers`, `flatten`); `crates/candler-core/tests/scaffold_comparability.rs` (class-A: identical arms give an empty diff; one differing leaf; a missing path; `unrecorded` against a value; `model.id` against `model.idx`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(scaffold_comparability)' && cargo clippy -p candler-core --tests -- -D warnings` (expected: PASS)
**Commit**: `feat(manifest): diff comparable values by path segment`

## Step 2: Give the stratum verdict
**Goal**: A set of arms and the declared factors give `Comparable`, `Refused(paths)` or `Malformed`, and the manifest's own `factors[]` is never trusted.
**Implementation Logic**:
1. `NUISANCE_PATHS: &[&str]`, the top-level manifest fields outside the hash: `schema`, `run`, `experiment`, `factors`, `sources`, `secrets`, `measured`, `deviations`, `started_at`, `ended_at`, `host`, `job`, `integrity`, `import`. It is the set `hashed_paths.toml` lists; no `comparable.*` path is in it.
2. `ArmView::from_manifest(&RunManifest) -> Result<ArmView, VerdictError>` reads `comparable`, `factors` and `measured[]` `census.init_tools` (an absent value is `unrecorded`). It refuses a `comparable` list that is unsorted or holds a duplicate with `VerdictError::Malformed` (C19).
3. `stratum_verdict(arms: &[ArmView], declared: &[FieldPath], nuisance: &[&str], init_tools: bool) -> Verdict` with `Verdict {Comparable, Refused(Vec<FieldPath>), Malformed(Vec<VerdictError>)}`: it diffs every arm against the first with `compare`, drops a deviation `covers`ed by a declared factor and one whose top-level segment is in `nuisance`, and names the rest (C04: a declared factor never covers a second diff; C06: a sibling is not covered). The declared set is the experiment file's factor paths; the arms' `factors[]` is read only to be ignored (C18).
4. `census.init_tools` differing across arms is a deviation at the path `census.init_tools` (C20, C21), exempted only when some declared factor starts with segment `tools` or `mcp` (C22, C23, F5).
5. A `Malformed` arm yields no verdict for the stratum, whatever the other arms hold.
**Deliverables**: `crates/candler-core/src/manifest/compare.rs` (`NUISANCE_PATHS`, `ArmView`, `stratum_verdict`, `Verdict`, `VerdictError`); `crates/candler-core/tests/scaffold_comparability.rs` (class-A: C02, C03, C18, C19 and C22 as quick checks)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(scaffold_comparability)'` (expected: PASS)
**Commit**: `feat(manifest): give the stratum verdict over declared factors`

## Step 3: Hold the verdict table and the nuisance set, then delete the scaffolding
**Goal**: The 23 rows and the hashed-paths equality are tests with oracles that do not run the code to find their answers, and the scaffolding is gone.
**Implementation Logic**:
1. `tests/fixtures/comparability/verdicts.json`: rows C01 to C23, copied verbatim from the test definition (two arms as literal `comparable` documents, `declared`, `nuisance` implied, `init_tools` values, expected verdict with exact paths). Add rows, never edit one. Rows to add: two declared factors with one drifting sibling; a factor path longer than the diff (`model.id.x` declared, `model.id` differing, refused); a three-arm stratum where only the third drifts; duplicate list entries in one arm only.
2. **B03** in `tests/guard_comparability.rs`: reads the table, builds each arm from its literal document, calls `stratum_verdict`, and compares to the literal expected verdict; the doc has `/// Guards:`, `/// Oracle:` (the R01 sentence per row) and `/// Bite:` (`starts_with`).
3. **B04**: parses `crates/candler-schemas/tests/vectors/manifest/hashed_paths.toml` (read only, located from `CARGO_MANIFEST_DIR/../candler-schemas`; the test file is not edited) for its nuisance list and asserts set equality with `NUISANCE_PATHS`, plus that no entry begins with segment `comparable`.
4. Delete `tests/scaffold_comparability.rs` and write the bite record: B03's `starts_with` mutation (restore with `git checkout --`) and B04's `comparable.inputs` addition, one at a time, each against `guard_comparability` only. Nothing here spawns or signals.
**Deliverables**: `crates/candler-core/tests/guard_comparability.rs` (the B03 and B04 tests); `crates/candler-core/tests/fixtures/comparability/verdicts.json`; the bite record in the leaf report
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_comparability)' && ! git grep --untracked -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/manifest crates/candler-core/tests` (expected: PASS)
**Commit**: `test(manifest): prove the stratum verdict and the nuisance set`
