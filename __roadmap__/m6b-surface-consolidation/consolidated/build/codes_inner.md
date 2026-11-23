# Inner refusal codes

**Goal**: A value-invalid experiment or suite file prints its inner code as the refusal's `code` (`experiment.factor_without_levels`, `suite.id_repeated`, and the other 12 inner codes of the codes review) instead of `*.document_invalid`, keeping its `side` and its `path`; `document_invalid` stays for pure shape errors that serde cannot name (G8).
**Estimate**: about 25 Sonnet calls ($1.0), cap 38; at 38 calls, stop, commit what is green, list what is left `[V]`
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (build README, Gotchas)
- [ ] [nl] OWNED (regex): `^crates/candler-schemas/src/(execution|evaluation|refusal|refusal_impls)(\.rs|/.*\.rs)$|^crates/(candler-schemas|candler-core|candler)/tests/.*\.rs$|^docs/contract/[a-z-]+\.md$|^__reports__/m6b-surface-consolidation/build/codes_inner_v0\.md$`. Tests change only where they assert `document_invalid` on a value-invalid file (R16 §Guards that change), plus the one new test; pages change only by regeneration and the two Notes lines `docs/contract/experiment-file.md:108` and `docs/contract/suite-file.md:66`. `codes.toml` is not edited: no row is added, retired or reworded
**Success Gates**:
- ⬜ [cmd] Targeted tests: `cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/inner_codes_print_as_code/) | test(/^guard_refusal::/) | test(/^guard_experiment_file::/) | test(/schema_drift|drift/)' && cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_refusal_codes::/) | test(/^guard_spec_file::/) | test(/^guard_resolve::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^contract_pages::/) | test(/^guard_cli_refusal::/)'`
- ⬜ [cmd] Ownership check prints nothing; `cargo clippy -p candler-schemas -p candler-core -p candler --all-targets -- -D warnings`
- ⬜ [cmd] The schemas commit touches one crate: `h=$(git log --format=%H --grep='^fix(schemas)!' $BASE_LEAF..HEAD); test -n "$h" && test -z "$(git show --name-only --format= $h | grep -v '^crates/candler-schemas/')"`, and its body names the migration: `git log -1 --format=%b $h | grep -q 'document_invalid'`
- ⬜ [cmd] DIGEST passes; `git diff --quiet $BASE_LEAF -- docs/contract/codes.toml`
- ⬜ [nl] Every refusal keeps a path: the report lists each of the 14 inner variants as "prints its code with a pointer" or "falls back to `document_invalid` with the inner code in `causes[]`", with the reason; the `read_value` finding (does validating after the parse loosen any JSON reader) is answered with evidence; the verifier closed with no open finding
**References**: [R16 §Verdicts G8, §Codes review, §D3, §`schemas@` releases](../../../../__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md); R18 `leaves/codes_review_v0.md` §G8 (the 39 shadowed rows, the 14 inner codes, option 2 and its triad); [R13 codes.toml](../../../../docs/contract/codes.toml); [R19 CONTRIBUTING.md](../../../../CONTRIBUTING.md) `:20` (breaking changes) and the release lines

## Step 1: Settle the two costs and write the confirm test, seen red
**Goal**: The leaf knows, before code, which variants can carry a pointer after the parse and whether moving validation loosens a reader; the test fails on the tip.
**Implementation Logic**:
1. Through colgrep (index `crates/candler-schemas` of this worktree), read `execution_path` in `refusal_impls.rs`, where today's pointer comes from serde (`execution.rs`, the `Located` path), `try_from` and `read_value`, and the same for the suite in `evaluation.rs`. Verify R16's claim that 14 inner codes are shadowed on the file-read path; a different count goes to the coordinator.
2. Answer in the report: (a) for each of the 14 inner variants, can the post-parse validator supply a JSON pointer; (b) does any reader that is not the file reader (a JSON read of the type through `read_value`) rely on validation inside `try_from`, and how the fix keeps it validating.
3. `inner_codes_print_as_code` in `crates/candler-schemas/tests/it/guard_refusal.rs`, role confirm: a table of the 14 inner codes, each row a minimal invalid file; each expected code is read from `docs/contract/codes.toml` by key, never from the code under test; each row asserts `code`, `side` and that `path` is present (or, for a fallback variant decided in 2a, `document_invalid` with the inner code in `causes[]`).
**Deliverables**: `crates/candler-schemas/tests/it/guard_refusal.rs`: `inner_codes_print_as_code`; `__reports__/m6b-surface-consolidation/build/codes_inner_v0.md`: section `## Probe`
**Consistency Checks**: `cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/inner_codes_print_as_code/)'` (expected: FAIL)
**Commit**: `test(schemas): confirm that inner codes print as the refusal code`

## Step 2: Validate after the parse and print the inner code
**Goal**: The confirm test passes inside `candler-schemas`; nothing outside the crate changes in this commit.
**Implementation Logic**:
1. Parse into the plain shape, then run the existing single validator (`new()`), so `ExecutionError` and `EvaluationError` keep their typed variants up to the refusal; the variant maps to its `code` through the existing table. This moves the one validator after the parse; it adds no second validation layer.
2. Variants that carry no pointer after the parse follow the fallback of step 1: `document_invalid` with the inner code in `causes[]`, so no refusal loses its path.
3. JSON readers keep validating (step 1, 2b).
4. Commit as `fix(schemas)!:` with the migration in the body: a caller that matched `experiment.document_invalid` or `suite.document_invalid` for a value-invalid file now receives the inner code (list the 14, and the fallback ones); `side` and `path` are unchanged; shape errors still print `document_invalid`.
**Deliverables**: `crates/candler-schemas/src/execution.rs` (or `execution/`), `crates/candler-schemas/src/evaluation.rs` (or `evaluation/`), `crates/candler-schemas/src/refusal_impls.rs`, `crates/candler-schemas/src/refusal.rs` as needed
**Consistency Checks**: `cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/inner_codes_print_as_code/) | test(/^guard_refusal::/) | test(/^guard_experiment_file::/) | test(/drift/)'` (expected: PASS)
**Commit**: `fix(schemas)!: print the inner code for a value-invalid file`

## Step 3: Update the other crates' expectations and the two Notes
**Goal**: Every test that asserted the defect now asserts the inner code; the pages say what happens.
**Implementation Logic**:
1. Run the targeted tests of `candler-core` and `candler`; each failure that asserts `document_invalid` on a value-invalid file takes the inner code. Any other failure stops the implementer (build README, Behaviour changes).
2. Rewrite the Notes lines `experiment-file.md:108` and `suite-file.md:66` to say which code a value-invalid file prints, with a new `[checked: v@<version read>]` stamp; regenerate the generated sections with `CANDLER_BLESS_CONTRACT=1` if they moved.
3. Two commits: the test expectations (`test(schemas): …`), then the Notes (`docs(schemas): …`).
**Deliverables**: the changed expectations under `crates/candler-core/tests/` and `crates/candler/tests/`; `docs/contract/experiment-file.md`, `docs/contract/suite-file.md`: the Notes lines
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_refusal_codes::/) | test(/^guard_spec_file::/) | test(/^guard_resolve::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^contract_pages::/) | test(/^guard_cli_refusal::/)'` (expected: PASS)
**Commit**: `test(schemas): expect the inner code where a file is value-invalid`

## Step 4: Bites, the report and the verifier
**Goal**: The test is shown to bite, and the leaf is verified.
**Implementation Logic**:
1. Bites, each made, seen red and reverted: map one inner variant back to `Document` (red on its row); drop the pointer of one variant (red on the path assertion); hard-code one expected code in the test instead of reading `codes.toml`, then rename that row's key locally (the table must go red, showing the expectation comes from the registry).
2. Write the report: the probe answers, the fallback list, the changed expectations with their reasons, the Bites, D3's four answers, and whether R16's "keep or cull" for a confirm applies (R14), follow-ups (the 25 `format.*` and `canonical.*` rows stay behind `document_invalid`).
3. Dispatch a fresh Sonnet verifier (model passed explicitly) with this brief verbatim, adding only the branch name. **Verifier brief:** "You verify the leaf `__roadmap__/m6b-surface-consolidation/consolidated/build/codes_inner.md` on branch <branch>. Read the build README (Gotchas), R16 (`__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md`) row G8 and §Codes review, `__reports__/m6b-surface-consolidation/leaves/codes_review_v0.md` §G8, then the diff from the leaf's branch point and the code through colgrep. Assume something is wrong. Attack: a refusal that lost its `path` or its `side`; a second validation layer instead of the one validator moved after the parse; a JSON reader of the type that no longer validates; a `codes.toml` row added, retired or reworded; a `detail` string parsed to find a code; a release-triggering commit that touches a crate other than `candler-schemas`, or a `!` commit without its migration; a test expectation changed that did not assert the defect; an expected code read from the code under test rather than `codes.toml`; a hand edit to a generated page section; a comment carrying history or ids. Report each finding with the step, `path:line` and the evidence. Change nothing."
4. Fix the findings; have the same verifier re-check through SendMessage; record each finding and its disposition in the report.
**Deliverables**: `__reports__/m6b-surface-consolidation/build/codes_inner_v0.md`: sections `## Probe`, `## Guards changed`, `## Bites`, `## D3`, `## Verifier`, `## Follow-ups`
**Consistency Checks**: `for s in Probe 'Guards changed' Bites D3 Verifier Follow-ups; do grep -q "^## $s" __reports__/m6b-surface-consolidation/build/codes_inner_v0.md || exit 1; done` (expected: PASS)
**Commit**: `reports(api): record the inner refusal codes build`
