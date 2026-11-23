# Docs chores

**Goal**: R01 gains the responsibility record ("Four audiences, one source per fact", the offline rule, its consequences and the box border) and R16's ten drafted §Future features rows, and loses the drift of seam 6; R01 and `docs/statistics.md` use the ruled words (Q7, Q8) and stop promising paired tests (X8); `README.md` becomes the store window (X5, D2); the helper's README becomes an author page with one tested example (X11, G16's residual).
**Estimate**: about 35 Sonnet calls ($1.4), cap 50; at 50 calls, stop, commit what is done, list what is left
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (build README, Gotchas)
- [ ] [cmd] `knowledge_pages` and `output_contract` are merged (they edit `docs/contract/` and R01 §CLI surface first): `git log --format=%s $BUILD_BASE..HEAD | grep -q '^feat(resolve): read the configuration paths from one table' && git log --format=%s $BUILD_BASE..HEAD | grep -q '^feat(cli)!: print refusals as json by default'`
- [x] [nl] The Q7 word is "comparability class" (key `comparability_class`), as in `analysis_record`'s pre-condition
- [ ] [nl] OWNED (regex): `^README\.md$|^docs/statistics\.md$|^python/candler-evaluator/README\.md$|^docs/contract/evaluator-helper\.md$|^__design__/architecture\.md$|^__reports__/m6b-surface-consolidation/build/docs_chores_v0\.md$`. In `evaluator-helper.md`, the Surface section only; in R01, not §CLI surface's `--format` and refusal lines (`output_contract`'s). No code, no test: a test of a stale sentence is a test per incident (R12, the fail list)
**Success Gates**:
- ⬜ [cmd] The store window is short: `n=$(wc -l < README.md); test $n -ge 60 && test $n -le 80`, and it holds no inventory: `! grep -qE 'candler-harness-claude-code|candler-harness-opencode' README.md`
- ⬜ [cmd] The drift is gone: `! grep -q 'M2a fills it in' python/candler-evaluator/README.md && ! grep -q 'not offered yet' docs/statistics.md && ! grep -q 'schemas for CLI outputs' __design__/architecture.md && ! grep -q 'M6a builds the paired tests' __design__/architecture.md`
- ⬜ [cmd] The record and the rows are in R01: `grep -q 'Four audiences, one source per fact' __design__/architecture.md && for r in 'Declared-versus-census check' 'Bind-mount probe on the store root' 'Docker lock and `candler gc`' 'Reported model and errata' 'Two counts, a regrade gap and double grading' 'Check a plug-in and a suite without a job' 'Glossary page' 'Token sums in reports' 'Treatment layer in a plug-in' 'Merge of the dependency-code twins'; do grep -qF "$r" __design__/architecture.md || exit 1; done`
- ⬜ [cmd] Ownership check prints nothing; `cargo nextest run -p candler --no-tests=fail -E 'test(/^contract_pages::/)'` (the hand Surface section keeps the page valid)
- ⬜ [nl] The quick start was run as written on this worktree and its output matches the README; the X11 example is the file a test imports (`engine_second.py:3`); no interactive text links to `docs/`; the report lists each R01 and `docs/` change with its source row in R16
**References**: [R16 §Verdicts X5, X8, X11, G16, Q2, Q7, Q8, §Seams 6, §D2, §Responsibility record (draft), §Drafted R01 §Future features rows](../../../../../../__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md); R18 `leaves/docs_and_skills_v0.md` §X5, §X11, §D2 (the store-window outline) and `leaves/analysis_outputs_v0.md` §X8; [R01 §Layer responsibilities, §Analysis statistics, §Future features](../../../../../../__design__/architecture.md)

## Step 1: The responsibility record, the Future rows and the R01 drift
**Goal**: R01 states who each text serves and what it may hold, lists the deferred items with their triggers, and loses three drifted lines.
**Implementation Logic**:
1. A new `### Documents and surfaces` right after `### Layer responsibilities`: R16's record (the audiences table, the offline rule, the consequences, the box border), worded as R01 is (present tense, no report ids such as Q4 or Q9: the reasons stay in R16).
2. §Future features: add R16's ten drafted rows in R01's row form (name, what, why, trigger), widening the existing row at `:1800` for "Two counts…" as R16 says; drop the ruling ids from their text. In the row at `:1804`, remove "schemas for CLI outputs" (`output_contract` and `analysis_record` built them).
3. Seam 6 (line numbers as of `a0df4d48`; `output_contract` edited the same table first): the `resolve` row says it takes an experiment (`:326`); the `analyze` row says it reads `--experiment` (`:328`); drop ":1757"'s "M6a builds the paired tests" clause (the Future row "Paired designs" exists). Leave `:333-335` (seam 1: true).
**Deliverables**: `__design__/architecture.md`: §Documents and surfaces, §Future features rows, the seam 6 lines
**Consistency Checks**: `grep -q 'Four audiences, one source per fact' __design__/architecture.md && ! grep -q 'schemas for CLI outputs' __design__/architecture.md && ! grep -q 'M6a builds the paired tests' __design__/architecture.md` (expected: PASS)
**Commit**: `docs(design): record the four audiences and the deferred surface rows`

## Step 2: The ruled words and X8
**Goal**: R01 and `docs/statistics.md` say "stratum" only for a block, name the interval's reading `interval_reading`, and stop promising paired tests.
**Implementation Logic**:
1. Q7: where R01 §Analysis statistics and `docs/statistics.md` (`:80`, `:166-167`) use "stratum" for a comparability class, use the ruled word; the block sense keeps "stratum".
2. Q8: where they name the field `reading`, name it `interval_reading`; §Readings' values ("effect", "inconclusive") are unchanged.
3. X8: `docs/statistics.md:165` says the runs are not paired, so a paired test does not apply (R01's "Paired designs" Future row says when that changes).
4. Two commits: R01 (`docs(design): …`), the page (`docs(stats): …`).
**Deliverables**: `__design__/architecture.md` §Analysis statistics; `docs/statistics.md`
**Consistency Checks**: `! grep -q 'not offered yet' docs/statistics.md` (expected: PASS)
**Commit**: `docs(stats): name the comparability class and the interval reading`

## Step 3: The README as the store window
**Goal**: A visitor learns, in 60 to 80 lines, whether Candler fits and how to try it.
**Implementation Logic**:
1. Sections: what Candler is and is not; six to eight main features; a quick start (a source build, then `candler resolve` on an experiment fixture of the repository, saying Docker is needed for runs); Learn more (links to `docs/`); status (one line, no milestone or crate list); contributing; license.
2. The crate inventory and the verb list leave (R16 §Responsibility record: no copy of a code fact). No version numbers, no plans.
3. Run the quick start as written on this worktree; fix the text until its output matches.
**Deliverables**: `README.md`
**Consistency Checks**: `n=$(wc -l < README.md); test $n -ge 60 && test $n -le 80` (expected: PASS)
**Commit**: `docs(readme): rewrite the readme as the project's store window`

## Step 4: The helper's author page
**Goal**: A plug-in author learns the Python API from one page whose example cannot rot.
**Implementation Logic**:
1. Rewrite `python/candler-evaluator/README.md` as an author page: what a kind is, its signature, `RunContext`, `KINDS`, `needs`, `params_schema`, each said once with the docstring as the source; one example, the file an engine fixture imports (`engine_second.py:3`), quoted as it is. "M2a fills it in" goes.
2. In `docs/contract/evaluator-helper.md` §Surface, one sentence pointing to the helper's docstrings and README for the author API. No other section changes.
3. Two commits: the README (`docs(evaluator-helper): …`), the Surface line (`docs(evaluator-helper): …`).
**Deliverables**: `python/candler-evaluator/README.md`; `docs/contract/evaluator-helper.md` §Surface
**Consistency Checks**: `! grep -q 'M2a fills it in' python/candler-evaluator/README.md && cargo nextest run -p candler --no-tests=fail -E 'test(/^contract_pages::/)'` (expected: PASS)
**Commit**: `docs(evaluator-helper): rewrite the helper readme as an author page`

## Step 5: The report
**Goal**: The leaf's changes are recorded with their sources.
**Implementation Logic**:
1. Write the report: each change with its R16 row; the quick start's run (command and the lines it printed); follow-ups seen in passing. No verifier (R16: no wire or guard behaviour changes); the L2 coordinator reads the diff at integration.
**Deliverables**: `__reports__/m6b-surface-consolidation/build/docs_chores_v0.md`: sections `## Changes`, `## Quick start run`, `## Follow-ups`
**Consistency Checks**: `for s in Changes 'Quick start run' Follow-ups; do grep -q "^## $s" __reports__/m6b-surface-consolidation/build/docs_chores_v0.md || exit 1; done` (expected: PASS)
**Commit**: `reports(api): record the docs chores build`
